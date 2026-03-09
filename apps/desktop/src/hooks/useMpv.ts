import { useState, useCallback, useRef, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import type { PlayerState } from "@/lib/types";
import {
  mpvLoad,
  mpvPlay,
  mpvPause,
  mpvStop,
  mpvSeek,
  mpvSetVolume,
  mpvGetState,
} from "@/lib/tauri";

const DEFAULT_STATE: PlayerState = {
  isPlaying: false,
  isPaused: false,
  currentUrl: null,
  volume: 100,
  position: 0,
  duration: 0,
};

export function useMpv() {
  const [state, setState] = useState<PlayerState>(DEFAULT_STATE);
  const [error, setError] = useState<string | null>(null);
  const [fallbackActive, setFallbackActive] = useState(false);
  const [firstFrameReady, setFirstFrameReady] = useState(false);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const loadingRef = useRef(false);

  // Listen for fallback event emitted when embedded renderer fails.
  useEffect(() => {
    const unlistenPromise = listen<{ reason: string }>("mpv://render-fallback", (event) => {
      console.warn("[useMpv] render fallback:", event.payload.reason);
      setFallbackActive(true);
    });
    return () => {
      unlistenPromise.then((fn) => fn());
    };
  }, []);

  // Listen for first-frame event so the frontend knows when the video is actually visible.
  useEffect(() => {
    const unlistenPromise = listen("mpv://first-frame", () => {
      setFirstFrameReady(true);
    });
    return () => {
      unlistenPromise.then((fn) => fn());
    };
  }, []);

  // On mount, check if mpv is already playing (e.g. user navigated away and back).
  // If so, restore firstFrameReady immediately so the background turns transparent.
  // Skip if a load is already in progress (loadingRef set before this IPC resolves)
  // to avoid a transparent flash that load() would immediately cancel.
  useEffect(() => {
    mpvGetState().then((s) => {
      if (!loadingRef.current && (s.isPlaying || s.isPaused)) {
        setFirstFrameReady(true);
      }
    }).catch(() => {});
  }, []);

  const load = useCallback(async (url: string) => {
    if (loadingRef.current) return;
    loadingRef.current = true;
    setError(null);
    setFallbackActive(false);
    setFirstFrameReady(false);
    // Reset playing state and position immediately so the bar doesn't show stale values.
    setState((s) => ({ ...s, isPlaying: false, isPaused: false, position: 0, duration: 0 }));
    try {
      await mpvLoad(url);
      // Don't set isPlaying optimistically — let the next poll confirm it from Rust
      // so transparency only kicks in once MPV is actually rendering frames.
      setState((s) => ({ ...s, currentUrl: url }));
    } catch (e) {
      const msg = String(e);
      setError(msg);
      throw e;
    } finally {
      loadingRef.current = false;
    }
  }, []);

  const play = useCallback(async () => {
    console.log("[useMpv] play called");
    try {
      await mpvPlay();
      setState((s) => ({ ...s, isPlaying: true, isPaused: false }));
    } catch (e) {
      console.error("[useMpv] mpvPlay failed:", e);
    }
  }, []);

  const pause = useCallback(async () => {
    console.log("[useMpv] pause called");
    try {
      await mpvPause();
      setState((s) => ({ ...s, isPaused: true }));
    } catch (e) {
      console.error("[useMpv] mpvPause failed:", e);
    }
  }, []);

  const stop = useCallback(async () => {
    console.log("[useMpv] stop called");
    try {
      await mpvStop();
      setState(DEFAULT_STATE);
    } catch (e) {
      console.error("[useMpv] mpvStop failed:", e);
    }
  }, []);

  const seek = useCallback(async (position: number) => {
    console.log("[useMpv] seek position=", position);
    try {
      await mpvSeek(position);
      setState((s) => ({ ...s, position }));
    } catch (e) {
      console.error("[useMpv] mpvSeek failed:", e);
    }
  }, []);

  const setVolume = useCallback(async (volume: number) => {
    console.log("[useMpv] setVolume volume=", volume);
    try {
      await mpvSetVolume(volume);
      setState((s) => ({ ...s, volume }));
    } catch (e) {
      console.error("[useMpv] mpvSetVolume failed:", e);
    }
  }, []);

  const refresh = useCallback(async () => {
    try {
      const s = await mpvGetState();
      console.debug("[useMpv] poll state:", JSON.stringify(s));
      setState({
        isPlaying: s.isPlaying,
        isPaused: s.isPaused,
        currentUrl: s.currentUrl,
        volume: s.volume,
        position: s.position,
        duration: s.duration,
      });
    } catch (e) {
      console.warn("[useMpv] poll failed:", e);
    }
  }, []);

  useEffect(() => {
    console.log("[useMpv] starting poll interval");
    refresh();
    pollRef.current = setInterval(refresh, 1000);
    return () => {
      console.log("[useMpv] clearing poll interval");
      if (pollRef.current) clearInterval(pollRef.current);
    };
  }, [refresh]);

  return { state, error, fallbackActive, firstFrameReady, load, play, pause, stop, seek, setVolume, refresh };
}
