import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";

// Mock Tauri event listener
const mockListenCallbacks = new Map<string, (event: unknown) => void>();
vi.mock("@tauri-apps/api/event", () => ({
	listen: vi.fn((eventName: string, callback: (event: unknown) => void) => {
		mockListenCallbacks.set(eventName, callback);
		const unlisten = vi.fn();
		return Promise.resolve(unlisten);
	}),
}));

vi.mock("@/lib/tauri", () => ({
	mpvLoad: vi.fn().mockResolvedValue(undefined),
	mpvPlay: vi.fn().mockResolvedValue(undefined),
	mpvPause: vi.fn().mockResolvedValue(undefined),
	mpvStop: vi.fn().mockResolvedValue(undefined),
	mpvSeek: vi.fn().mockResolvedValue(undefined),
	mpvSetVolume: vi.fn().mockResolvedValue(undefined),
	mpvGetState: vi.fn().mockResolvedValue({
		isPlaying: false,
		isPaused: false,
		currentUrl: null,
		volume: 100,
		position: 0,
		duration: 0,
	}),
}));

import {
	mpvLoad,
	mpvPlay,
	mpvPause,
	mpvStop,
	mpvSeek,
	mpvSetVolume,
	mpvGetState,
} from "@/lib/tauri";
import { useMpv } from "./useMpv";

const mockMpvLoad = vi.mocked(mpvLoad);
const mockMpvPlay = vi.mocked(mpvPlay);
const mockMpvPause = vi.mocked(mpvPause);
const mockMpvStop = vi.mocked(mpvStop);
const mockMpvSeek = vi.mocked(mpvSeek);
const mockMpvSetVolume = vi.mocked(mpvSetVolume);
const mockMpvGetState = vi.mocked(mpvGetState);

describe("useMpv", () => {
	beforeEach(() => {
		vi.clearAllMocks();
		mockListenCallbacks.clear();
		mockMpvGetState.mockResolvedValue({
			isPlaying: false,
			isPaused: false,
			currentUrl: null,
			volume: 100,
			position: 0,
			duration: 0,
		} as never);
	});

	afterEach(() => {
		vi.restoreAllMocks();
	});

	// ── Initial state ─────────────────────────────────────────────────

	it("returns default state on mount", async () => {
		const { result } = renderHook(() => useMpv());
		await waitFor(() => expect(mockMpvGetState).toHaveBeenCalled());

		expect(result.current.state).toEqual({
			isPlaying: false,
			isPaused: false,
			currentUrl: null,
			volume: 100,
			position: 0,
			duration: 0,
		});
		expect(result.current.error).toBeNull();
		expect(result.current.fallbackActive).toBe(false);
		expect(result.current.firstFrameReady).toBe(false);
	});

	it("restores firstFrameReady when already playing on mount", async () => {
		mockMpvGetState.mockResolvedValue({
			isPlaying: true,
			isPaused: false,
			currentUrl: "http://stream.url",
			volume: 80,
			position: 30,
			duration: 120,
		} as never);

		const { result } = renderHook(() => useMpv());
		await waitFor(() => expect(result.current.firstFrameReady).toBe(true));
	});

	// ── load ──────────────────────────────────────────────────────────

	it("calls mpvLoad and updates currentUrl", async () => {
		const { result } = renderHook(() => useMpv());
		await waitFor(() => expect(mockMpvGetState).toHaveBeenCalled());

		await act(async () => {
			await result.current.load("http://stream.url");
		});

		expect(mockMpvLoad).toHaveBeenCalledWith("http://stream.url");
		expect(result.current.state.currentUrl).toBe("http://stream.url");
	});

	it("prevents double-load while loading", async () => {
		let resolveLoad: () => void;
		mockMpvLoad.mockImplementation(
			() => new Promise<void>((r) => { resolveLoad = r; })
		);

		const { result } = renderHook(() => useMpv());
		await waitFor(() => expect(mockMpvGetState).toHaveBeenCalled());

		// Start first load
		let loadPromise: Promise<void>;
		act(() => {
			loadPromise = result.current.load("http://first.url");
		});

		// Try second load while first is in progress
		await act(async () => {
			await result.current.load("http://second.url");
		});

		// Only one call should have been made
		expect(mockMpvLoad).toHaveBeenCalledTimes(1);
		expect(mockMpvLoad).toHaveBeenCalledWith("http://first.url");

		// Resolve first load
		await act(async () => {
			resolveLoad!();
			await loadPromise!;
		});
	});

	it("sets error on load failure", async () => {
		mockMpvLoad.mockRejectedValue(new Error("codec not found"));

		const { result } = renderHook(() => useMpv());
		await waitFor(() => expect(mockMpvGetState).toHaveBeenCalled());

		let caught: Error | null = null;
		await act(async () => {
			try {
				await result.current.load("http://bad.url");
			} catch (e) {
				caught = e as Error;
			}
		});

		expect(caught?.message).toBe("codec not found");
		expect(result.current.error).toContain("codec not found");
	});

	it("resets state immediately when load is called", async () => {
		// Start with a playing state from poll
		mockMpvGetState.mockResolvedValue({
			isPlaying: true,
			isPaused: false,
			currentUrl: "http://old.url",
			volume: 80,
			position: 45,
			duration: 120,
		} as never);

		const { result } = renderHook(() => useMpv());
		await waitFor(() => expect(result.current.state.isPlaying).toBe(true));

		// Now load resets position/duration immediately
		mockMpvLoad.mockResolvedValue(undefined);
		await act(async () => {
			await result.current.load("http://new.url");
		});

		expect(result.current.state.position).toBe(0);
		expect(result.current.state.duration).toBe(0);
		expect(result.current.firstFrameReady).toBe(false);
	});

	// ── play / pause / stop ───────────────────────────────────────────

	it("play calls mpvPlay and sets isPlaying optimistically", async () => {
		const { result } = renderHook(() => useMpv());
		await waitFor(() => expect(mockMpvGetState).toHaveBeenCalled());

		await act(async () => {
			await result.current.play();
		});

		expect(mockMpvPlay).toHaveBeenCalledTimes(1);
		expect(result.current.state.isPlaying).toBe(true);
		expect(result.current.state.isPaused).toBe(false);
	});

	it("pause calls mpvPause and sets isPaused", async () => {
		const { result } = renderHook(() => useMpv());
		await waitFor(() => expect(mockMpvGetState).toHaveBeenCalled());

		await act(async () => {
			await result.current.pause();
		});

		expect(mockMpvPause).toHaveBeenCalledTimes(1);
		expect(result.current.state.isPaused).toBe(true);
	});

	it("stop resets all state to defaults", async () => {
		mockMpvGetState.mockResolvedValue({
			isPlaying: true,
			isPaused: false,
			currentUrl: "http://stream.url",
			volume: 80,
			position: 45,
			duration: 120,
		} as never);

		const { result } = renderHook(() => useMpv());
		await waitFor(() => expect(result.current.state.isPlaying).toBe(true));

		await act(async () => {
			await result.current.stop();
		});

		expect(mockMpvStop).toHaveBeenCalledTimes(1);
		expect(result.current.state).toEqual({
			isPlaying: false,
			isPaused: false,
			currentUrl: null,
			volume: 100,
			position: 0,
			duration: 0,
		});
	});

	// ── seek / volume ─────────────────────────────────────────────────

	it("seek calls mpvSeek with position and updates state", async () => {
		const { result } = renderHook(() => useMpv());
		await waitFor(() => expect(mockMpvGetState).toHaveBeenCalled());

		await act(async () => {
			await result.current.seek(42.5);
		});

		expect(mockMpvSeek).toHaveBeenCalledWith(42.5);
		expect(result.current.state.position).toBe(42.5);
	});

	it("setVolume calls mpvSetVolume and updates state", async () => {
		const { result } = renderHook(() => useMpv());
		await waitFor(() => expect(mockMpvGetState).toHaveBeenCalled());

		await act(async () => {
			await result.current.setVolume(75);
		});

		expect(mockMpvSetVolume).toHaveBeenCalledWith(75);
		expect(result.current.state.volume).toBe(75);
	});

	// ── Tauri events ──────────────────────────────────────────────────

	it("sets fallbackActive when render-fallback event fires", async () => {
		const { result } = renderHook(() => useMpv());
		await waitFor(() => expect(mockMpvGetState).toHaveBeenCalled());

		const callback = mockListenCallbacks.get("mpv://render-fallback");
		expect(callback).toBeDefined();

		act(() => {
			callback!({ payload: { reason: "OpenGL failed" } });
		});

		expect(result.current.fallbackActive).toBe(true);
	});

	it("sets firstFrameReady when first-frame event fires", async () => {
		const { result } = renderHook(() => useMpv());
		await waitFor(() => expect(mockMpvGetState).toHaveBeenCalled());

		const callback = mockListenCallbacks.get("mpv://first-frame");
		expect(callback).toBeDefined();

		act(() => {
			callback!({});
		});

		expect(result.current.firstFrameReady).toBe(true);
	});

	// ── Error resilience ──────────────────────────────────────────────

	it("play does not throw when mpvPlay fails", async () => {
		mockMpvPlay.mockRejectedValue(new Error("play failed"));

		const { result } = renderHook(() => useMpv());
		await waitFor(() => expect(mockMpvGetState).toHaveBeenCalled());

		// Should not throw
		await act(async () => {
			await result.current.play();
		});
	});

	it("pause does not throw when mpvPause fails", async () => {
		mockMpvPause.mockRejectedValue(new Error("pause failed"));

		const { result } = renderHook(() => useMpv());
		await waitFor(() => expect(mockMpvGetState).toHaveBeenCalled());

		await act(async () => {
			await result.current.pause();
		});
	});

	it("poll failure does not crash the hook", async () => {
		// First call succeeds (initial), then next poll fails
		mockMpvGetState
			.mockResolvedValueOnce({
				isPlaying: false,
				isPaused: false,
				currentUrl: null,
				volume: 100,
				position: 0,
				duration: 0,
			} as never)
			.mockRejectedValueOnce(new Error("poll failed"));

		const { result } = renderHook(() => useMpv());
		await waitFor(() => expect(mockMpvGetState).toHaveBeenCalled());

		// Hook should still be usable
		expect(result.current.state).toBeDefined();
	});
});
