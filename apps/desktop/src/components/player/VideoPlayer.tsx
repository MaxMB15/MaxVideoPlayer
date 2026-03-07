import { Controls } from "./Controls";
import { ChannelOverlay } from "./ChannelOverlay";
import { useState, useCallback, useEffect } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { useMpv } from "@/hooks/useMpv";
import type { Channel } from "@/lib/types";

export function PlayerView() {
  const mpv = useMpv();
  const location = useLocation();
  const navigate = useNavigate();
  const [showControls, setShowControls] = useState(true);
  const [showChannelOsd, setShowChannelOsd] = useState(false);
  const [activeChannelName, setActiveChannelName] = useState<string | null>(
    null
  );

  const navState = location.state as {
    url?: string;
    channelName?: string;
  } | null;

  useEffect(() => {
    if (navState?.url) {
      mpv.load(navState.url).catch(() => {});
      setActiveChannelName(navState.channelName ?? null);
    }
  }, [navState?.url]);

  const handleSelectChannel = useCallback(
    (channel: Channel) => {
      mpv.load(channel.url).catch(() => {});
      setActiveChannelName(channel.name);
    },
    [mpv.load]
  );

  useEffect(() => {
    if (!showControls) return;
    const timer = setTimeout(() => setShowControls(false), 4000);
    return () => clearTimeout(timer);
  }, [showControls]);

  const handleMouseMove = useCallback(() => {
    setShowControls(true);
  }, []);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      switch (e.key) {
        case " ":
          e.preventDefault();
          mpv.state.isPaused ? mpv.play() : mpv.pause();
          break;
        case "ArrowLeft":
          mpv.seek(Math.max(0, mpv.state.position - 10));
          break;
        case "ArrowRight":
          mpv.seek(mpv.state.position + 10);
          break;
        case "ArrowUp":
          mpv.setVolume(Math.min(150, mpv.state.volume + 5));
          break;
        case "ArrowDown":
          mpv.setVolume(Math.max(0, mpv.state.volume - 5));
          break;
        case "c":
          setShowChannelOsd((v) => !v);
          break;
        case "Escape":
          if (showChannelOsd) {
            setShowChannelOsd(false);
          } else {
            mpv.stop();
            navigate("/");
          }
          break;
      }
      setShowControls(true);
    },
    [mpv, showChannelOsd, navigate]
  );

  const handleStop = useCallback(() => {
    mpv.stop();
    navigate("/");
  }, [mpv, navigate]);

  return (
    <div
      className="player-container relative h-full w-full bg-transparent focus:outline-none"
      onMouseMove={handleMouseMove}
      onClick={() => setShowControls(true)}
      onKeyDown={handleKeyDown}
      tabIndex={0}
    >
      {/* Fallback banner: shown when embedded renderer failed and video is in a separate window */}
      {mpv.fallbackActive && (
        <div className="absolute top-4 left-1/2 -translate-x-1/2 z-50 flex items-center gap-3 rounded-lg border border-yellow-500/40 bg-yellow-950/80 px-4 py-2 text-yellow-200 text-sm shadow-lg backdrop-blur-sm max-w-xl">
          <span>⚠</span>
          <span>Video is playing in a separate window with native controls (embedded renderer unavailable).</span>
        </div>
      )}

      {/* Embedded MPV renders below; transparent so native video shows through */}
      <div className="absolute inset-0 flex flex-col items-center justify-center bg-transparent">
        {mpv.error && (
          <div className="text-center p-6 max-w-md">
            <p className="text-destructive text-sm mb-2">{mpv.error}</p>
            <p className="text-muted-foreground text-xs">
              Run ./scripts/build-libmpv.sh macos before dev/build.
            </p>
          </div>
        )}
        {!mpv.error &&
          !mpv.state.currentUrl &&
          !mpv.state.isPlaying &&
          !mpv.state.isPaused && (
            <p className="text-muted-foreground text-lg">
              Select a channel to start watching
            </p>
          )}
      </div>

      {activeChannelName && showControls && (
        <div className="absolute top-0 left-0 right-0 bg-gradient-to-b from-black/60 to-transparent p-4">
          <p className="text-white text-sm font-medium">{activeChannelName}</p>
        </div>
      )}

      {!mpv.fallbackActive && (
        <Controls
          state={{
            isPlaying: mpv.state.isPlaying,
            isPaused: mpv.state.isPaused,
            currentUrl: mpv.state.currentUrl,
            volume: mpv.state.volume,
            position: mpv.state.position,
            duration: mpv.state.duration,
          }}
          visible={showControls}
          onPlay={mpv.play}
          onPause={mpv.pause}
          onStop={handleStop}
          onSeek={mpv.seek}
          onVolumeChange={mpv.setVolume}
        />
      )}

      {showChannelOsd && (
        <ChannelOverlay
          onClose={() => setShowChannelOsd(false)}
          onSelectChannel={handleSelectChannel}
        />
      )}
    </div>
  );
}
