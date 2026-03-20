import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";
import { useSplashScreen } from "./useSplashScreen";

// Mock Tauri API calls
vi.mock("@/lib/tauri", () => ({
	refreshProvider: vi.fn(),
	refreshEpg: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-updater", () => ({
	check: vi.fn(),
}));

vi.mock("./useChannels", () => ({
	useChannels: vi.fn(),
	loadProviderSettings: vi.fn(),
	getEpgLastRefresh: vi.fn(),
	setEpgLastRefresh: vi.fn(),
}));

import * as tauri from "@/lib/tauri";
import { check } from "@tauri-apps/plugin-updater";
import * as channelsHook from "./useChannels";

const mockUseChannels = vi.mocked(channelsHook.useChannels);
const mockRefreshProvider = vi.mocked(tauri.refreshProvider);
const mockRefreshEpg = vi.mocked(tauri.refreshEpg);
const mockCheck = vi.mocked(check);
const mockLoadProviderSettings = vi.mocked(channelsHook.loadProviderSettings);
const mockGetEpgLastRefresh = vi.mocked(channelsHook.getEpgLastRefresh);

describe("useSplashScreen", () => {
	beforeEach(() => {
		sessionStorage.clear();
		// Default: no providers, already initialized
		mockUseChannels.mockReturnValue({
			providers: [],
			initialized: true,
		} as never);
		mockCheck.mockResolvedValue(null);
		mockLoadProviderSettings.mockReturnValue({
			autoRefresh: false,
			refreshIntervalHours: 24,
			epgAutoRefresh: false,
			epgRefreshIntervalHours: 24,
		});
		mockGetEpgLastRefresh.mockReturnValue(0);
	});

	afterEach(() => {
		vi.clearAllMocks();
	});

	it("starts with dismissed=false and 4 steps when session is fresh", async () => {
		const { result } = renderHook(() => useSplashScreen());
		// Steps are initialized immediately (all 4 visible from the first render)
		expect(result.current.steps).toHaveLength(4);
		expect(result.current.dismissed).toBe(false);
		// Wait for async completion
		await waitFor(() => expect(result.current.allDone).toBe(true));
		expect(result.current.steps.every((s) => s.status === "done")).toBe(true);
	});

	it("marks dismissed immediately if session key is already set", () => {
		sessionStorage.setItem("splash-shown", "true");
		const { result } = renderHook(() => useSplashScreen());
		expect(result.current.dismissed).toBe(true);
		expect(result.current.allDone).toBe(true);
	});

	it("completes all steps and sets allDone when there are no providers", async () => {
		const { result } = renderHook(() => useSplashScreen());

		await waitFor(() => expect(result.current.allDone).toBe(true));

		expect(result.current.steps.every((s) => s.status === "done")).toBe(true);
		expect(result.current.progress).toBe(1);
	});

	it("does not call refreshProvider when autoRefresh is false", async () => {
		mockUseChannels.mockReturnValue({
			providers: [{ id: "p1", lastUpdated: new Date(0).toISOString(), epgUrl: null }],
			initialized: true,
		} as never);
		mockLoadProviderSettings.mockReturnValue({
			autoRefresh: false,
			refreshIntervalHours: 24,
			epgAutoRefresh: false,
			epgRefreshIntervalHours: 24,
		});

		const { result } = renderHook(() => useSplashScreen());
		await waitFor(() => expect(result.current.allDone).toBe(true));

		expect(mockRefreshProvider).not.toHaveBeenCalled();
	});

	it("calls refreshProvider when autoRefresh=true and interval elapsed", async () => {
		mockUseChannels.mockReturnValue({
			providers: [{ id: "p1", lastUpdated: new Date(0).toISOString(), epgUrl: null }],
			initialized: true,
		} as never);
		mockLoadProviderSettings.mockReturnValue({
			autoRefresh: true,
			refreshIntervalHours: 24,
			epgAutoRefresh: false,
			epgRefreshIntervalHours: 24,
		});
		mockRefreshProvider.mockResolvedValue(undefined);

		const { result } = renderHook(() => useSplashScreen());
		await waitFor(() => expect(result.current.allDone).toBe(true));

		expect(mockRefreshProvider).toHaveBeenCalledWith("p1");
	});

	it("calls refreshEpg when epgAutoRefresh=true, epgUrl set, and interval elapsed", async () => {
		mockUseChannels.mockReturnValue({
			providers: [
				{
					id: "p1",
					lastUpdated: new Date().toISOString(),
					epgUrl: "http://epg.example.com/guide.xml",
				},
			],
			initialized: true,
		} as never);
		mockLoadProviderSettings.mockReturnValue({
			autoRefresh: false,
			refreshIntervalHours: 24,
			epgAutoRefresh: true,
			epgRefreshIntervalHours: 24,
		});
		mockGetEpgLastRefresh.mockReturnValue(0); // never refreshed
		mockRefreshEpg.mockResolvedValue(undefined);

		const { result } = renderHook(() => useSplashScreen());
		await waitFor(() => expect(result.current.allDone).toBe(true));

		expect(mockRefreshEpg).toHaveBeenCalledWith("p1");
	});

	it("does not call refreshEpg when epgAutoRefresh=true but interval not elapsed", async () => {
		mockUseChannels.mockReturnValue({
			providers: [
				{
					id: "p1",
					lastUpdated: new Date().toISOString(),
					epgUrl: "http://epg.example.com/guide.xml",
				},
			],
			initialized: true,
		} as never);
		mockLoadProviderSettings.mockReturnValue({
			autoRefresh: false,
			refreshIntervalHours: 24,
			epgAutoRefresh: true,
			epgRefreshIntervalHours: 24,
		});
		mockGetEpgLastRefresh.mockReturnValue(Date.now() - 1000); // refreshed 1 second ago

		const { result } = renderHook(() => useSplashScreen());
		await waitFor(() => expect(result.current.allDone).toBe(true));

		expect(mockRefreshEpg).not.toHaveBeenCalled();
	});

	it("shows update step as 'Update available' when update is found", async () => {
		mockCheck.mockResolvedValue({ version: "1.2.3" } as never);

		const { result } = renderHook(() => useSplashScreen());
		await waitFor(() => expect(result.current.allDone).toBe(true));

		expect(result.current.update).toEqual({ version: "1.2.3" });
		const updateStep = result.current.steps.find((s) => s.id === "updates");
		expect(updateStep?.label).toContain("1.2.3");
	});

	it("dismiss() sets dismissed=true and stores session key", async () => {
		const { result } = renderHook(() => useSplashScreen());
		await waitFor(() => expect(result.current.allDone).toBe(true));

		act(() => result.current.dismiss());

		expect(result.current.dismissed).toBe(true);
		expect(sessionStorage.getItem("splash-shown")).toBe("true");
	});

	it("calls onComplete callback when loading finishes", async () => {
		const onComplete = vi.fn();
		const { result } = renderHook(() => useSplashScreen({ onComplete }));
		await waitFor(() => expect(result.current.allDone).toBe(true));

		expect(onComplete).toHaveBeenCalledOnce();
	});
});
