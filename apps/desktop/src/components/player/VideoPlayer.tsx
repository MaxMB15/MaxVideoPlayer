import { Controls } from "./Controls";
import { ChannelOverlay } from "./ChannelOverlay";
import { SubtitlePicker } from "./SubtitlePicker";
import { MovieInfoDrawer } from "@/components/channels/MovieInfoDrawer";
import { SeriesDetailModal } from "@/components/channels/SeriesDetailModal";
import { LiveInfoDrawer } from "@/components/channels/LiveInfoDrawer";
import { useState, useCallback, useEffect, useRef, useMemo } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { useMpv } from "@/hooks/useMpv";
import { useChannels } from "@/hooks/useChannels";
import { mpvSetBounds, recordPlayStart, recordPlayEnd, fetchOmdbData, fetchMdbListData } from "@/lib/tauri";
import type { Channel, OmdbData, MdbListData } from "@/lib/types";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useFullscreen } from "@/lib/fullscreen-context";

const showTitle = (name: string): string => name.replace(/\s+S\d{1,3}E\d{1,3}.*/i, "").trim();

interface EnrichedMeta {
	omdbData: OmdbData | null;
	mdbListData: MdbListData | null;
}

const sortEpisodes = (eps: Channel[]): Channel[] =>
	[...eps].sort((a, b) => {
		const sa = a.season ?? 0,
			sb = b.season ?? 0;
		if (sa !== sb) return sa - sb;
		return (a.episode ?? 0) - (b.episode ?? 0);
	});

export const PlayerView = () => {
	const mpv = useMpv();
	const { channels } = useChannels();
	const location = useLocation();
	const navigate = useNavigate();
	const [showControls, setShowControls] = useState(true);
	const containerRef = useRef<HTMLDivElement>(null);
	const [showChannelOsd, setShowChannelOsd] = useState(false);
	const [showInfoDrawer, setShowInfoDrawer] = useState(false);
	const [activeChannelName, setActiveChannelName] = useState<string | null>(null);
	const [activeChannel, setActiveChannel] = useState<Channel | null>(null);
	// Episode list for series navigation — set when navigating from SeriesDetailModal
	const [seriesEpisodes, setSeriesEpisodes] = useState<Channel[]>([]);
	const { isFullscreen, setFullscreen } = useFullscreen();
	const [enrichedMeta, setEnrichedMeta] = useState<EnrichedMeta | null>(null);
	const [showSubtitlePicker, setShowSubtitlePicker] = useState(false);

	const navState = location.state as {
		url?: string;
		channelName?: string;
		channel?: Channel;
		seriesEpisodes?: Channel[];
	} | null;

	useEffect(() => {
		document.documentElement.style.backgroundColor = mpv.firstFrameReady
			? "transparent"
			: "black";
		document.body.style.backgroundColor = mpv.firstFrameReady ? "transparent" : "black";
		return () => {
			document.documentElement.style.backgroundColor = "";
			document.body.style.backgroundColor = "";
		};
	}, [mpv.firstFrameReady]);

	useEffect(() => {
		const el = containerRef.current;
		if (!el) return;
		const report = () => {
			const r = el.getBoundingClientRect();
			mpvSetBounds(r.x, r.y, r.width, r.height).catch(() => {});
		};
		report();
		const ro = new ResizeObserver(report);
		ro.observe(el);
		return () => ro.disconnect();
	}, [mpv.state.currentUrl]);

	useEffect(() => {
		if (navState?.url) {
			mpv.load(navState.url).catch(() => {});
			setActiveChannelName(navState.channelName ?? null);
			setActiveChannel(navState.channel ?? null);
			if (navState.seriesEpisodes?.length) {
				setSeriesEpisodes(navState.seriesEpisodes);
			}
			// Record play start
			if (navState.channel) {
				const ch = navState.channel;
				playStartTimeRef.current = Date.now();
				recordPlayStart(ch.id, ch.name, ch.logoUrl ?? null, ch.contentType).catch(() => {});
			}
		}
	}, [navState?.url]);

	// Pre-fetch enriched metadata when activeChannel changes to a movie or series
	useEffect(() => {
		setShowSubtitlePicker(false);

		if (!activeChannel || activeChannel.contentType === "live") {
			setEnrichedMeta(null);
			return;
		}

		let cancelled = false;
		setEnrichedMeta(null);

		const mediaType = activeChannel.contentType === "series" ? "series" : "movie";

		fetchOmdbData(activeChannel.id, activeChannel.name, mediaType as "movie" | "series")
			.then((omdbData) => {
				if (cancelled) return;
				setEnrichedMeta({ omdbData, mdbListData: null });

				if (omdbData?.imdbId) {
					const mdbMediaType = activeChannel.contentType === "series" ? "show" : "movie";
					fetchMdbListData(omdbData.imdbId, mdbMediaType)
						.then((mdbListData) => {
							if (cancelled) return;
							setEnrichedMeta({ omdbData, mdbListData });
						})
						.catch(() => {});
				}
			})
			.catch(() => {
				if (cancelled) return;
				setEnrichedMeta({ omdbData: null, mdbListData: null });
			});

		return () => {
			cancelled = true;
		};
	}, [activeChannel?.id]);

	const activeImdbId = enrichedMeta?.omdbData?.imdbId ?? null;
	const canShowSubtitles =
		activeImdbId !== null &&
		(activeChannel?.contentType === "movie" || activeChannel?.contentType === "series");

	const handleSelectChannel = useCallback(
		(channel: Channel) => {
			// Record end of current channel
			if (activeChannelRef.current && playStartTimeRef.current !== null) {
				const elapsed = Math.floor((Date.now() - playStartTimeRef.current) / 1000);
				recordPlayEnd(activeChannelRef.current.id, elapsed).catch(() => {});
			}
			playStartTimeRef.current = Date.now();
			recordPlayStart(
				channel.id,
				channel.name,
				channel.logoUrl ?? null,
				channel.contentType
			).catch(() => {});

			mpv.load(channel.url).catch(() => {});
			setActiveChannelName(channel.name);
			setActiveChannel(channel);
			setSeriesEpisodes([]);
		},
		[mpv.load]
	);

	// --- Series episode navigation ---

	// Derive episodes from passed list (Xtream) or local cache (M3U)
	const localSeriesEpisodes = useMemo(() => {
		if (!activeChannel || activeChannel.contentType !== "series") return [];
		const title = activeChannel.seriesTitle ?? showTitle(activeChannel.name);
		return channels.filter(
			(ch) => ch.contentType === "series" && (ch.seriesTitle ?? showTitle(ch.name)) === title
		);
	}, [activeChannel, channels]);

	const sortedEpisodes = useMemo(() => {
		const source = seriesEpisodes.length > 0 ? seriesEpisodes : localSeriesEpisodes;
		return sortEpisodes(source);
	}, [seriesEpisodes, localSeriesEpisodes]);

	const currentEpIdx = useMemo(
		() => (activeChannel ? sortedEpisodes.findIndex((ep) => ep.url === activeChannel.url) : -1),
		[activeChannel, sortedEpisodes]
	);

	const prevEpisode = currentEpIdx > 0 ? sortedEpisodes[currentEpIdx - 1] : null;
	const nextEpisode =
		currentEpIdx >= 0 && currentEpIdx < sortedEpisodes.length - 1
			? sortedEpisodes[currentEpIdx + 1]
			: null;

	const playEpisode = useCallback(
		(ep: Channel) => {
			// Record end of current episode
			if (activeChannelRef.current && playStartTimeRef.current !== null) {
				const elapsed = Math.floor((Date.now() - playStartTimeRef.current) / 1000);
				recordPlayEnd(activeChannelRef.current.id, elapsed).catch(() => {});
			}
			playStartTimeRef.current = Date.now();
			recordPlayStart(ep.id, ep.name, ep.logoUrl ?? null, ep.contentType).catch(() => {});

			mpv.load(ep.url).catch(() => {});
			setActiveChannelName(ep.name);
			setActiveChannel(ep);
			setShowInfoDrawer(false);
		},
		[mpv.load]
	);

	// --- Autoplay next episode ---
	// Use refs to avoid stale closures while keeping the effect dependency minimal
	const nextEpisodeRef = useRef(nextEpisode);
	nextEpisodeRef.current = nextEpisode;
	const mpvStateRef = useRef(mpv.state);
	mpvStateRef.current = mpv.state;
	const activeChannelRef = useRef(activeChannel);
	activeChannelRef.current = activeChannel;
	const playStartTimeRef = useRef<number | null>(null);

	// On unmount, record end of play
	useEffect(() => {
		return () => {
			if (activeChannelRef.current && playStartTimeRef.current !== null) {
				const elapsed = Math.floor((Date.now() - playStartTimeRef.current) / 1000);
				recordPlayEnd(activeChannelRef.current.id, elapsed).catch(() => {});
				playStartTimeRef.current = null;
			}
		};
	}, []);

	const prevIsPlayingRef = useRef(false);
	useEffect(() => {
		const wasPlaying = prevIsPlayingRef.current;
		prevIsPlayingRef.current = mpv.state.isPlaying;

		if (!wasPlaying || mpv.state.isPlaying || mpv.state.isPaused) return;
		if (activeChannelRef.current?.contentType !== "series") return;
		const { duration, position } = mpvStateRef.current;
		if (duration <= 0 || position < duration - 5) return;
		const next = nextEpisodeRef.current;
		if (next) playEpisode(next);
	}, [mpv.state.isPlaying, mpv.state.isPaused, playEpisode]);

	// --- Controls visibility ---
	useEffect(() => {
		if (!showControls) return;
		const timer = setTimeout(() => setShowControls(false), 4000);
		return () => clearTimeout(timer);
	}, [showControls]);

	const handleMouseMove = useCallback(() => setShowControls(true), []);

	// --- Fullscreen ---
	const toggleFullscreen = useCallback(() => {
		const next = !isFullscreen;
		setFullscreen(next);
		getCurrentWindow()
			.setFullscreen(next)
			.catch((e) => {
				console.error("[PlayerView] setFullscreen failed:", e);
			});
	}, [isFullscreen, setFullscreen]);

	// --- Keyboard ---
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
					if (isFullscreen) {
						setFullscreen(false);
						getCurrentWindow()
							.setFullscreen(false)
							.catch((e) => {
								console.error("[PlayerView] setFullscreen(false) failed:", e);
							});
					} else if (showInfoDrawer) {
						setShowInfoDrawer(false);
					} else if (showChannelOsd) {
						setShowChannelOsd(false);
					} else {
						// Note: playStartTimeRef is NOT nulled here intentionally.
						// The unmount cleanup records the elapsed time when the route changes.
						mpv.stop();
						navigate("/");
					}
					break;
			}
			setShowControls(true);
		},
		[mpv, isFullscreen, showInfoDrawer, showChannelOsd, navigate, setFullscreen]
	);

	const handleStop = useCallback(() => {
		if (activeChannelRef.current && playStartTimeRef.current !== null) {
			const elapsed = Math.floor((Date.now() - playStartTimeRef.current) / 1000);
			recordPlayEnd(activeChannelRef.current.id, elapsed).catch(() => {});
			playStartTimeRef.current = null;
		}
		mpv.stop();
		navigate("/");
	}, [mpv, navigate]);

	// --- Info drawer episode source ---
	const episodesForDrawer = sortedEpisodes.length > 0 ? sortedEpisodes : localSeriesEpisodes;
	const showTitleForDrawer = activeChannel?.seriesTitle ?? showTitle(activeChannel?.name ?? "");

	return (
		<div
			ref={containerRef}
			className={
				isFullscreen
					? "player-container fixed inset-0 z-[9999] bg-transparent focus:outline-none"
					: "player-container relative h-full w-full bg-transparent focus:outline-none"
			}
			onMouseMove={handleMouseMove}
			onClick={() => setShowControls(true)}
			onKeyDown={handleKeyDown}
			tabIndex={0}
		>
			{mpv.fallbackActive && (
				<div className="absolute top-4 left-1/2 -translate-x-1/2 z-50 flex items-center gap-3 rounded-lg border border-yellow-500/40 bg-yellow-950/80 px-4 py-2 text-yellow-200 text-sm shadow-lg backdrop-blur-sm max-w-xl">
					<span>⚠</span>
					<span>
						Video is playing in a separate window with native controls (embedded
						renderer unavailable).
					</span>
				</div>
			)}

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

			{/* Autoplay banner — shown for 3s before next episode starts */}
			{/* (simple version: no countdown, instant autoplay) */}

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
					isFullscreen={isFullscreen}
					onPlay={mpv.play}
					onPause={mpv.pause}
					onStop={handleStop}
					onSeek={mpv.seek}
					onVolumeChange={mpv.setVolume}
					onFullscreen={toggleFullscreen}
					onInfo={activeChannel ? () => setShowInfoDrawer(true) : undefined}
					onPrevEpisode={prevEpisode ? () => playEpisode(prevEpisode) : undefined}
					onNextEpisode={nextEpisode ? () => playEpisode(nextEpisode) : undefined}
					onSubtitles={
						canShowSubtitles ? () => setShowSubtitlePicker((v) => !v) : undefined
					}
				/>
			)}

			{showChannelOsd && (
				<ChannelOverlay
					onClose={() => setShowChannelOsd(false)}
					onSelectChannel={handleSelectChannel}
				/>
			)}

			{showSubtitlePicker && activeImdbId && (
				<SubtitlePicker
					imdbId={activeImdbId}
					season={activeChannel?.season ?? undefined}
					episode={activeChannel?.episode ?? undefined}
					onClose={() => setShowSubtitlePicker(false)}
				/>
			)}

			{/* Info drawers */}
			{showInfoDrawer && activeChannel && activeChannel.contentType === "series" && (
				<SeriesDetailModal
					showTitle={showTitleForDrawer}
					episodes={episodesForDrawer}
					onClose={() => setShowInfoDrawer(false)}
					onPlay={(ch) => playEpisode(ch)}
					prefetchedOmdbData={enrichedMeta?.omdbData}
					prefetchedMdbListData={enrichedMeta?.mdbListData}
				/>
			)}

			{showInfoDrawer && activeChannel && activeChannel.contentType === "movie" && (
				<MovieInfoDrawer
					movie={activeChannel}
					onClose={() => setShowInfoDrawer(false)}
					onPlay={(ch) => {
						// Record end of current playback
						if (activeChannel && playStartTimeRef.current !== null) {
							const elapsed = Math.floor(
								(Date.now() - playStartTimeRef.current) / 1000
							);
							recordPlayEnd(activeChannel.id, elapsed).catch(() => {});
						}
						// Start tracking new playback
						playStartTimeRef.current = Date.now();
						recordPlayStart(ch.id, ch.name, ch.logoUrl ?? null, ch.contentType).catch(
							() => {}
						);

						setShowInfoDrawer(false);
						mpv.load(ch.url).catch(() => {});
						setActiveChannelName(ch.name);
						setActiveChannel(ch);
					}}
					prefetchedOmdbData={enrichedMeta?.omdbData}
					prefetchedMdbListData={enrichedMeta?.mdbListData}
				/>
			)}

			{showInfoDrawer && activeChannel && activeChannel.contentType === "live" && (
				<LiveInfoDrawer channel={activeChannel} onClose={() => setShowInfoDrawer(false)} />
			)}
		</div>
	);
};
