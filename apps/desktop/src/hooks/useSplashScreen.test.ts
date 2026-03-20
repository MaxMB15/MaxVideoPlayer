import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";
import { useSplashScreen } from "./useSplashScreen";

// Mock Tauri API calls
vi.mock("@/lib/tauri", () => ({
	getProviders: vi.fn(),
	getAllChannels: vi.fn(),
	refreshProvider: vi.fn(),
	refreshEpg: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-updater", () => ({
	check: vi.fn(),
}));

vi.mock("./useChannels", () => ({
	loadProviderSettings: vi.fn(),
	getEpgLastRefresh: vi.fn(),
	setEpgLastRefresh: vi.fn(),
}));

import * as tauri from "@/lib/tauri";
import { check } from "@tauri-apps/plugin-updater";
import * as channelsHook from "./useChannels";

const mockGetProviders = vi.mocked(tauri.getProviders);
const mockGetAllChannels = vi.mocked(tauri.getAllChannels);
const mockRefreshProvider = vi.mocked(tauri.refreshProvider);
const mockRefreshEpg = vi.mocked(tauri.refreshEpg);
const mockCheck = vi.mocked(check);
const mockLoadProviderSettings = vi.mocked(channelsHook.loadProviderSettings);
const mockGetEpgLastRefresh = vi.mocked(channelsHook.getEpgLastRefresh);

describe("useSplashScreen", () => {
	beforeEach(() => {
		sessionStorage.clear();
		mockGetProviders.mockResolvedValue([]);
		mockGetAllChannels.mockResolvedValue([]);
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

	it("starts with dismissed=false and no steps when session is fresh", () => {
		const { result } = renderHook(() => useSplashScreen());
		expect(result.current.dismissed).toBe(false);
		expect(result.current.allDone).toBe(false);
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
		mockGetProviders.mockResolvedValue([
			{ id: "p1", lastUpdated: new Date(0).toISOString(), epgUrl: null } as never,
		]);
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
		mockGetProviders.mockResolvedValue([
			{ id: "p1", lastUpdated: new Date(0).toISOString(), epgUrl: null } as never,
		]);
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
		mockGetProviders.mockResolvedValue([
			{ id: "p1", lastUpdated: new Date().toISOString(), epgUrl: "http://epg.example.com/guide.xml" } as never,
		]);
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
		mockGetProviders.mockResolvedValue([
			{ id: "p1", lastUpdated: new Date().toISOString(), epgUrl: "http://epg.example.com/guide.xml" } as never,
		]);
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
