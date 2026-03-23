import { Controls } from "./Controls";
import { ChannelOverlay } from "./ChannelOverlay";
import { SubtitlePicker } from "./SubtitlePicker";
import { SubtitleOverlay } from "./SubtitleOverlay";
import { MovieInfoDrawer } from "@/components/channels/MovieInfoDrawer";
import { SeriesDetailModal } from "@/components/channels/SeriesDetailModal";
import { LiveInfoDrawer } from "@/components/channels/LiveInfoDrawer";
import { useState, useCallback, useEffect, useRef, useMemo } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { useMpv } from "@/hooks/useMpv";
import { useChannels } from "@/hooks/useChannels";
import {
	mpvSetBounds,
	recordPlayStart,
	recordPlayEnd,
	fetchOmdbData,
	fetchMdbListData,
	searchSubtitles,
	downloadSubtitle,
	readSubtitleFile,
	mpvSubAdd,
	mpvSubRemove,
} from "@/lib/tauri";
import { parseSrt } from "@/lib/subtitle-parser";
import type { Channel, OmdbData, MdbListData, SubtitleCue, SubtitleEntry } from "@/lib/types";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useFullscreen } from "@/lib/fullscreen-context";

const showTitle = (name: string): string => name.replace(/\s+S\d{1,3}E\d{1,3}.*/i, "").trim();

/** Extract season/episode numbers from a channel name like "Show S01E05". */
const parseSeasonEpisode = (name: string): { season?: number; episode?: number } => {
	const m = name.match(/S(\d{1,3})E(\d{1,3})/i);
	if (m) return { season: parseInt(m[1], 10), episode: parseInt(m[2], 10) };
	return {};
};

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
	const [selectedSubtitleId, setSelectedSubtitleId] = useState<number | null>(null);
	const [subtitleCues, setSubtitleCues] = useState<SubtitleCue[]>([]);
	const [selectedSubtitleEntry, setSelectedSubtitleEntry] = useState<SubtitleEntry | null>(null);
	const [subtitleFontSize, setSubtitleFontSize] = useState(18);
	const [subtitleFontFamily, setSubtitleFontFamily] = useState("system-ui, sans-serif");
	const [subtitlePos, setSubtitlePos] = useState({ x: 50, y: 88 });
	const [subtitleDelay, setSubtitleDelay] = useState(0);
	const [subtitleEditMode, setSubtitleEditMode] = useState(false);
	const [autoplay, setAutoplay] = useState(true);

	// Remembers which language + rank-within-language the user last picked so the
	// same subtitle can be auto-selected when navigating to the next episode.
	const [subtitlePreference, setSubtitlePreference] = useState<{
		languageCode: string;
		rankInLanguage: number;
	} | null>(null);
	// Refs used by the auto-load effect to avoid stale closures.
	const subtitlePreferenceRef = useRef(subtitlePreference);
	subtitlePreferenceRef.current = subtitlePreference;
	// Incremented each time an episode navigation should trigger subtitle auto-load.
	// Using state (not a ref) so the effect dependency is tracked by React.
	const [autoLoadTrigger, setAutoLoadTrigger] = useState(0);

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

	// On Linux with CSD (client-side decorations), the native video surface is
	// positioned relative to the full window surface (including the GTK header bar).
	// CSS getBoundingClientRect() returns coordinates relative to the WebView viewport
	// (below the header bar). Compute the header bar height once and add it to y.
	// On macOS the offset is handled natively in set_frame, so this is a no-op (0).
	const decoOffsetRef = useRef(0);
	useEffect(() => {
		getCurrentWindow()
			.innerSize()
			.then((size) => {
				// innerSize() returns physical pixels (includes CSD header bar on Linux).
				// window.innerHeight is CSS pixels (viewport only, excludes header bar).
				// The difference (in CSS pixels) is the header bar height.
				const cssHeight = size.height / window.devicePixelRatio;
				const offset = cssHeight - window.innerHeight;
				// Only apply if positive and reasonable (< 100px) to avoid wrong values
				// on platforms where innerSize already matches viewport.
				decoOffsetRef.current = offset > 0 && offset < 100 ? offset : 0;
				console.log(
					`[VideoPlayer] decoOffset: tauriInnerSize=${size.width}x${size.height} dpr=${window.devicePixelRatio} cssHeight=${cssHeight} windowInnerHeight=${window.innerHeight} offset=${offset} applied=${decoOffsetRef.current}`,
				);
			})
			.catch(() => {});
	}, []);

	useEffect(() => {
		const el = containerRef.current;
		if (!el) return;
		const report = () => {
			const r = el.getBoundingClientRect();
			console.log(
				`[VideoPlayer] bounds: rect=(${r.x.toFixed(1)}, ${r.y.toFixed(1)}, ${r.width.toFixed(1)}, ${r.height.toFixed(1)}) decoOffset=${decoOffsetRef.current} → mpvSetBounds(${r.x.toFixed(1)}, ${(r.y + decoOffsetRef.current).toFixed(1)}, ${r.width.toFixed(1)}, ${r.height.toFixed(1)})`,
			);
			mpvSetBounds(r.x, r.y + decoOffsetRef.current, r.width, r.height).catch(() => {});
		};
		report();
		const ro = new ResizeObserver(report);
		ro.observe(el);
		return () => ro.disconnect();
	}, [mpv.state.currentUrl]);

	// Arrow keys reposition subtitle overlay when settings pane is open
	useEffect(() => {
		if (!subtitleEditMode) return;
		const handleKey = (e: KeyboardEvent) => {
			if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(e.key)) return;
			e.preventDefault();
			setSubtitlePos((p) => ({
				x:
					e.key === "ArrowLeft"
						? Math.max(5, p.x - 2)
						: e.key === "ArrowRight"
							? Math.min(95, p.x + 2)
							: p.x,
				y:
					e.key === "ArrowUp"
						? Math.max(3, p.y - 2)
						: e.key === "ArrowDown"
							? Math.min(97, p.y + 2)
							: p.y,
			}));
		};
		window.addEventListener("keydown", handleKey);
		return () => window.removeEventListener("keydown", handleKey);
	}, [subtitleEditMode]);

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
		} else {
			// Navigating back to player without a new channel (e.g. via sidebar menu)
			const saved = sessionStorage.getItem("mvp_lastChannel");
			if (saved) {
				try {
					const ch: Channel = JSON.parse(saved);
					setActiveChannel(ch);
					setActiveChannelName(ch.name);
				} catch {}
			}
			const savedEpisodes = sessionStorage.getItem("mvp_lastSeriesEpisodes");
			if (savedEpisodes) {
				try {
					setSeriesEpisodes(JSON.parse(savedEpisodes));
				} catch {}
			}
		}
	}, [navState?.url]);

	// Persist last active channel and series episode list so they can be restored when navigating back
	useEffect(() => {
		if (activeChannel) {
			sessionStorage.setItem("mvp_lastChannel", JSON.stringify(activeChannel));
		}
	}, [activeChannel]);

	useEffect(() => {
		if (seriesEpisodes.length > 0) {
			sessionStorage.setItem("mvp_lastSeriesEpisodes", JSON.stringify(seriesEpisodes));
		}
	}, [seriesEpisodes]);

	// Pre-fetch enriched metadata when activeChannel changes to a movie or series
	useEffect(() => {
		setShowSubtitlePicker(false);
		setSelectedSubtitleId(null);
		setSubtitleCues([]);
		setSelectedSubtitleEntry(null);
		setSubtitleEditMode(false);
		setSubtitleDelay(0);
		setSubtitlePos({ x: 50, y: 88 });

		if (!activeChannel || activeChannel.contentType === "live") {
			setEnrichedMeta(null);
			return;
		}

		let cancelled = false;
		setEnrichedMeta(null);

		const mediaType = activeChannel.contentType === "series" ? "series" : "movie";
		const titleForOmdb = activeChannel.seriesTitle ?? showTitle(activeChannel.name);

		fetchOmdbData(activeChannel.id, titleForOmdb, mediaType as "movie" | "series")
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

	// Season/episode for subtitle search: prefer structured channel data, fall back to parsing the name
	const { season: parsedSeason, episode: parsedEpisode } = activeChannel
		? parseSeasonEpisode(activeChannel.name)
		: {};
	const subtitleSeason = activeChannel?.season ?? parsedSeason;
	const subtitleEpisode = activeChannel?.episode ?? parsedEpisode;

	// Refs so the auto-load effect can read fresh values without stale closures.
	const subtitleSeasonRef = useRef(subtitleSeason);
	subtitleSeasonRef.current = subtitleSeason;
	const subtitleEpisodeRef = useRef(subtitleEpisode);
	subtitleEpisodeRef.current = subtitleEpisode;

	// Auto-load: fires when playEpisode increments autoLoadTrigger, then waits for
	// activeImdbId to resolve. Using a trigger counter avoids the cancellation bug
	// where setEnrichedMeta(null) → activeImdbId → null mid-flight would kill the search.
	useEffect(() => {
		if (autoLoadTrigger === 0) return; // no episode navigation yet
		if (!activeImdbId) return; // OMDB still in-flight; re-run when it resolves
		const pref = subtitlePreferenceRef.current;
		if (!pref) return;

		let cancelled = false;

		searchSubtitles(activeImdbId, subtitleSeasonRef.current, subtitleEpisodeRef.current)
			.then(async (result) => {
				if (cancelled) return;
				const langEntries = (result?.entries ?? []).filter(
					(e) => e.languageCode === pref.languageCode
				);
				if (langEntries.length === 0) return; // preferred language not available — keep none
				const entry = langEntries[Math.min(pref.rankInLanguage, langEntries.length - 1)];
				await mpvSubRemove(-1).catch(() => {});
				const localPath = await downloadSubtitle(entry.fileId);
				mpvSubAdd(localPath).catch(() => {});
				const content = await readSubtitleFile(localPath);
				const cues = parseSrt(content);
				if (cancelled) return;
				setSelectedSubtitleId(entry.fileId);
				setSubtitleCues(cues);
				setSelectedSubtitleEntry(entry);
			})
			.catch(() => {}); // silent — subtitle is best-effort

		return () => {
			cancelled = true;
		};
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [autoLoadTrigger, activeImdbId]);

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
			setSelectedSubtitleId(null);
			setSubtitleCues([]);
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
			setSelectedSubtitleId(null);
			setSubtitleCues([]);
			setSelectedSubtitleEntry(null);
			setSubtitleEditMode(false);
			setSubtitleDelay(0);
			// Increment trigger so the auto-load effect fires for this episode.
			setAutoLoadTrigger((t) => t + 1);
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

	const autoplayRef = useRef(autoplay);
	autoplayRef.current = autoplay;

	// Track the last position+duration seen while actively playing.
	// MPV resets position/duration to 0 when a video ends or a new one loads,
	// so we can't rely on mpvStateRef at the moment isPlaying goes false.
	const lastPlayingStateRef = useRef({ position: 0, duration: 0 });
	useEffect(() => {
		if (mpv.state.isPlaying && mpv.state.duration > 0) {
			lastPlayingStateRef.current = {
				position: mpv.state.position,
				duration: mpv.state.duration,
			};
		}
	}, [mpv.state.isPlaying, mpv.state.position, mpv.state.duration]);

	const prevIsPlayingRef = useRef(false);
	useEffect(() => {
		const wasPlaying = prevIsPlayingRef.current;
		prevIsPlayingRef.current = mpv.state.isPlaying;

		// Only react to the transition: was playing → now not playing.
		if (!wasPlaying || mpv.state.isPlaying) return;

		// Distinguish EOF from a user pause: check if position was near the end.
		// With keep-open=yes, EOF lands in isPaused=true (last frame frozen) — we can't
		// use isPaused=false as the EOF signal anymore, so we use position proximity instead.
		const { position, duration } = lastPlayingStateRef.current;
		if (duration <= 0 || position < duration - 5) return; // mid-video pause — do nothing

		// EOF reached. With autoplay on and a next episode available, advance.
		if (autoplayRef.current && activeChannelRef.current?.contentType === "series") {
			const next = nextEpisodeRef.current;
			if (next) playEpisode(next);
		}
		// Otherwise (autoplay off, or movie, or last episode): keep-open=yes already holds
		// MPV on the last frame with isPaused=true — controls remain visible, no action needed.
	}, [mpv.state.isPlaying, playEpisode]);

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
			// While the subtitle settings pane is open, arrow keys belong to subtitle
			// position/delay handlers — don't let them also seek or change volume.
			const isArrow = ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(e.key);
			if (subtitleEditMode && isArrow) return;

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
		[
			mpv,
			isFullscreen,
			showInfoDrawer,
			showChannelOsd,
			navigate,
			setFullscreen,
			subtitleEditMode,
		]
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
							Check that libmpv is installed. See README for setup instructions.
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
					autoplay={autoplay}
					onAutoplayChange={setAutoplay}
					hasSubtitles={selectedSubtitleId !== null}
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
					season={subtitleSeason}
					episode={subtitleEpisode}
					onClose={() => {
						setShowSubtitlePicker(false);
						setSubtitleEditMode(false);
					}}
					onSubtitleSelected={(id, cues, entry, rankInLanguage) => {
						setSelectedSubtitleId(id);
						setSubtitleCues(cues ?? []);
						setSelectedSubtitleEntry(entry ?? null);
						if (entry && rankInLanguage !== undefined) {
							setSubtitlePreference({
								languageCode: entry.languageCode,
								rankInLanguage,
							});
						} else {
							// User clicked None — clear preference so next episode gets no subtitle.
							setSubtitlePreference(null);
						}
					}}
					currentSelectedId={selectedSubtitleId}
					currentSelectedEntry={selectedSubtitleEntry}
					subtitleFontSize={subtitleFontSize}
					subtitleFontFamily={subtitleFontFamily}
					subtitleDelay={subtitleDelay}
					onFontSizeChange={setSubtitleFontSize}
					onFontFamilyChange={setSubtitleFontFamily}
					onDelayChange={(d) => {
						setSubtitleDelay(d);
						import("@/lib/tauri").then(({ mpvSetSubDelay }) =>
							mpvSetSubDelay(d).catch(() => {})
						);
					}}
					onSettingsModeChange={(active) => setSubtitleEditMode(active)}
				/>
			)}

			{(subtitleCues.length > 0 || subtitleEditMode) && (
				<SubtitleOverlay
					cues={subtitleCues}
					position={mpv.state.position}
					fontSize={subtitleFontSize}
					fontFamily={subtitleFontFamily}
					posX={subtitlePos.x}
					posY={subtitlePos.y}
					delay={subtitleDelay}
					editMode={subtitleEditMode}
					onPositionChange={(x, y) => setSubtitlePos({ x, y })}
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
