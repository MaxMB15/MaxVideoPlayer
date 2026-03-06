import { useState, useCallback, useRef, useEffect } from "react";
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
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const load = useCallback(async (url: string) => {
    setError(null);
    try {
      await mpvLoad(url);
      setState((s) => ({ ...s, currentUrl: url, isPlaying: true, isPaused: false }));
    } catch (e) {
      const msg = String(e);
      setError(msg);
      throw e;
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

  return { state, error, load, play, pause, stop, seek, setVolume, refresh };
}
