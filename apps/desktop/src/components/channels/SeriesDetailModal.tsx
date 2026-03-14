import { useState, useMemo, useEffect } from "react";
import { X, Play, ChevronLeft, ChevronRight, MonitorPlay, Loader2 } from "lucide-react";
import type { Channel, OmdbData, MdbListData } from "@/lib/types";
import { fetchOmdbData } from "@/lib/tauri";
import { RatingsRow } from "@/components/ui/ratings-row";

interface SeriesDetailDrawerProps {
	showTitle: string;
	episodes: Channel[];
	onClose: () => void;
	onPlay: (channel: Channel) => void;
	// Optional pre-fetched data from PlayerView to avoid double-fetch
	prefetchedOmdbData?: OmdbData | null;
	prefetchedMdbListData?: MdbListData | null;
}

const episodeTitle = (name: string): string => {
	const stripped = name.replace(/^.*?\bS\d{1,3}E\d{1,3}\s*/i, "").trim();
	return stripped || name;
};

const dedupeEpisodes = (episodes: Channel[]): Channel[] => {
	const seen = new Map<string, { ch: Channel; extraSources: string[] }>();
	for (const ep of episodes) {
		const key = `${ep.season ?? 0}x${ep.episode ?? ep.name}`;
		if (!seen.has(key)) {
			seen.set(key, { ch: { ...ep }, extraSources: [...ep.sources] });
		} else {
			const entry = seen.get(key)!;
			entry.extraSources.push(ep.url, ...ep.sources);
			if (!entry.ch.logoUrl && ep.logoUrl) entry.ch.logoUrl = ep.logoUrl;
		}
	}
	return Array.from(seen.values()).map(({ ch, extraSources }) => ({
		...ch,
		sources: extraSources,
	}));
};

type Step = "seasons" | "episodes" | "sources";

export const SeriesDetailModal = ({
	showTitle,
	episodes,
	onClose,
	onPlay,
	prefetchedOmdbData,
	prefetchedMdbListData,
}: SeriesDetailDrawerProps) => {
	const [visible, setVisible] = useState(false);
	const [step, setStep] = useState<Step>("seasons");
	const [selectedSeason, setSelectedSeason] = useState<number | null>(null);
	const [sourceEp, setSourceEp] = useState<Channel | null>(null);
	const [omdbData, setOmdbData] = useState<OmdbData | null>(prefetchedOmdbData ?? null);
	const [omdbLoading, setOmdbLoading] = useState(!prefetchedOmdbData);
	const [mdbListData, setMdbListData] = useState<MdbListData | null>(
		prefetchedMdbListData ?? null,
	);

	const firstEp = episodes[0];
	const showLogoUrl = firstEp?.logoUrl;

	useEffect(() => {
		const id = requestAnimationFrame(() => setVisible(true));
		return () => cancelAnimationFrame(id);
	}, []);

	useEffect(() => {
		if (prefetchedOmdbData !== undefined) return; // already have data
		if (!firstEp) {
			setOmdbLoading(false);
			return;
		}
		setOmdbLoading(true);
		setOmdbData(null);
		setMdbListData(null);
		fetchOmdbData(firstEp.id, showTitle, "series")
			.then(setOmdbData)
			.catch(() => {})
			.finally(() => setOmdbLoading(false));
	}, [firstEp?.id, showTitle]);

	const handleClose = () => {
		setVisible(false);
		setTimeout(onClose, 300);
	};

	const deduped = useMemo(() => dedupeEpisodes(episodes), [episodes]);

	const seasons = useMemo(() => {
		const map = new Map<number, Channel[]>();
		for (const ep of deduped) {
			const s = ep.season ?? 0;
			if (!map.has(s)) map.set(s, []);
			map.get(s)!.push(ep);
		}
		for (const [, eps] of map) {
			eps.sort((a, b) => (a.episode ?? 0) - (b.episode ?? 0));
		}
		return Array.from(map.entries()).sort((a, b) => a[0] - b[0]);
	}, [deduped]);

	const currentEpisodes = useMemo(
		() =>
			selectedSeason !== null ? (seasons.find(([s]) => s === selectedSeason)?.[1] ?? []) : [],
		[seasons, selectedSeason],
	);

	const handleSeasonClick = (season: number) => {
		setSelectedSeason(season);
		setStep("episodes");
	};

	const handleEpisodeClick = (ep: Channel) => {
		if (ep.sources.length > 0) {
			setSourceEp(ep);
			setStep("sources");
		} else {
			onPlay(ep);
			handleClose();
		}
	};

	const handleBack = () => {
		if (step === "sources") {
			setSourceEp(null);
			setStep("episodes");
		} else if (step === "episodes") {
			setStep("seasons");
		}
	};

	const handleSourcePick = (channel: Channel) => {
		onPlay(channel);
		handleClose();
	};

	const backLabel =
		step === "episodes"
			? showTitle
			: step === "sources"
				? selectedSeason === 0
					? "Unknown Season"
					: `Season ${selectedSeason}`
				: "";

	// Derived display values
	const posterSrc =
		omdbData?.posterUrl && omdbData.posterUrl !== "N/A"
			? omdbData.posterUrl
			: (showLogoUrl ?? null);

	const year = omdbData?.year && omdbData.year !== "N/A" ? omdbData.year : null;
	const rated = omdbData?.rated && omdbData.rated !== "N/A" ? omdbData.rated : null;
	const genre = omdbData?.genre && omdbData.genre !== "N/A" ? omdbData.genre : null;
	const runtime = omdbData?.runtime && omdbData.runtime !== "N/A" ? omdbData.runtime : null;
	const director = omdbData?.director && omdbData.director !== "N/A" ? omdbData.director : null;
	const actors = omdbData?.actors && omdbData.actors !== "N/A" ? omdbData.actors : null;
	const plot =
		mdbListData?.description ??
		(omdbData?.plot && omdbData.plot !== "N/A" ? omdbData.plot : null);

	const truncatedActors = actors && actors.length > 60 ? actors.slice(0, 57) + "..." : actors;
	const genreRuntime = [genre, runtime ? `${runtime}/ep` : null].filter(Boolean).join(" · ");

	return (
		<div className="fixed inset-0 z-50">
			{/* Backdrop */}
			<div
				className={`absolute inset-0 bg-black/60 backdrop-blur-sm transition-opacity duration-300 ${
					visible ? "opacity-100" : "opacity-0"
				}`}
				onClick={handleClose}
			/>

			{/* Drawer */}
			<div
				className={`absolute bottom-0 left-0 right-0 bg-card rounded-t-2xl shadow-2xl flex flex-col transition-transform duration-300 ease-out max-h-[85vh] ${
					visible ? "translate-y-0" : "translate-y-full"
				}`}
			>
				{/* Handle */}
				<div className="flex justify-center pt-3 pb-1 shrink-0">
					<div className="w-9 h-1 rounded-full bg-border" />
				</div>

				{/* Header row */}
				<div className="flex items-center justify-between px-5 py-2 shrink-0">
					{step !== "seasons" ? (
						<button
							onClick={handleBack}
							className="flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground transition-colors"
						>
							<ChevronLeft className="h-4 w-4" />
							{backLabel}
						</button>
					) : (
						<div className="flex-1" />
					)}
					<button
						onClick={handleClose}
						aria-label="Close"
						className="shrink-0 text-muted-foreground hover:text-foreground transition-colors ml-3"
					>
						<X className="h-4 w-4" />
					</button>
				</div>

				{/* Info card — seasons step only */}
				{step === "seasons" && (
					<>
						<div className="flex gap-4 px-5 pb-4 shrink-0">
							{/* Poster */}
							<div className="w-20 h-28 rounded-xl bg-secondary overflow-hidden shrink-0 flex items-center justify-center">
								{posterSrc ? (
									<img
										src={posterSrc}
										alt=""
										className="h-full w-full object-cover"
										loading="lazy"
									/>
								) : (
									<MonitorPlay className="h-8 w-8 text-muted-foreground/30" />
								)}
							</div>

							{/* Show info — ~65% of remaining space */}
							<div className="flex flex-col justify-center gap-1.5 flex-[2] min-w-0">
								{omdbLoading ? (
									<div className="flex items-center gap-2 text-muted-foreground">
										<Loader2 className="h-3.5 w-3.5 animate-spin" />
										<span className="text-xs">Loading info…</span>
									</div>
								) : (
									<>
										{/* Title + year + rated */}
										<div className="flex items-baseline gap-1.5 flex-wrap">
											<p className="text-base font-semibold leading-tight line-clamp-2">
												{showTitle}
											</p>
											{year && (
												<span className="text-xs text-muted-foreground shrink-0">
													({year})
												</span>
											)}
											{rated && (
												<span className="text-[11px] font-semibold bg-secondary text-muted-foreground px-1.5 py-0.5 rounded shrink-0">
													{rated}
												</span>
											)}
										</div>

										{/* Ratings row */}
										<RatingsRow omdbData={omdbData} mdbListData={mdbListData} />

										{/* Genre + runtime */}
										{genreRuntime && (
											<p className="text-xs text-muted-foreground">
												{genreRuntime}
											</p>
										)}

										{/* Director / Created by */}
										{director && (
											<p className="text-xs text-muted-foreground truncate">
												Created by: {director}
											</p>
										)}

										{/* Cast */}
										{truncatedActors && (
											<p className="text-xs text-muted-foreground">
												Cast: {truncatedActors}
											</p>
										)}

										{/* Fallback: no OMDB data */}
										{!omdbData && (
											<p className="text-xs text-muted-foreground">
												Series · —
											</p>
										)}
									</>
								)}
							</div>

							{/* Season/episode stats — ~35% of remaining space */}
							<div className="flex flex-col justify-center gap-2.5 flex-[1] min-w-0 shrink-0">
								<div className="flex flex-col gap-1.5">
									<div className="flex flex-col items-center justify-center rounded-xl bg-secondary px-3 py-2.5 text-center">
										<span className="text-base font-bold tabular-nums leading-none">
											{seasons.length}
										</span>
										<span className="text-[11px] text-muted-foreground mt-0.5">
											{seasons.length === 1 ? "Season" : "Seasons"}
										</span>
									</div>
									<div className="flex flex-col items-center justify-center rounded-xl bg-secondary px-3 py-2.5 text-center">
										<span className="text-base font-bold tabular-nums leading-none">
											{deduped.length}
										</span>
										<span className="text-[11px] text-muted-foreground mt-0.5">
											{deduped.length === 1 ? "Episode" : "Episodes"}
										</span>
									</div>
								</div>
							</div>
						</div>

						{/* Plot — shown below the main row when available */}
						{plot && !omdbLoading && (
							<div className="px-5 pb-3 shrink-0">
								<p className="text-xs text-muted-foreground leading-relaxed line-clamp-3">
									{plot}
								</p>
							</div>
						)}

						<div className="border-t border-border mx-5 shrink-0" />
					</>
				)}

				{/* Step: Seasons */}
				{step === "seasons" && (
					<div className="overflow-y-auto flex-1 px-3 py-2">
						{seasons.map(([s, eps]) => (
							<button
								key={s}
								onClick={() => handleSeasonClick(s)}
								className="w-full flex items-center gap-3 px-3 py-3 rounded-xl hover:bg-accent transition-colors text-left"
							>
								<div className="w-8 h-8 rounded-lg bg-secondary flex items-center justify-center shrink-0">
									<span className="text-[10px] font-bold text-muted-foreground">
										{s === 0 ? "?" : String(s)}
									</span>
								</div>
								<div className="flex-1 min-w-0">
									<p className="text-sm font-medium">
										{s === 0 ? "Unknown Season" : `Season ${s}`}
									</p>
									<p className="text-xs text-muted-foreground mt-0.5">
										{eps.length} episodes
									</p>
								</div>
								<ChevronRight className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
							</button>
						))}
						{seasons.length === 0 && (
							<p className="text-sm text-muted-foreground text-center py-8">
								No episodes found
							</p>
						)}
					</div>
				)}

				{/* Step: Episodes */}
				{step === "episodes" && (
					<div className="overflow-y-auto flex-1 px-3 py-2">
						{currentEpisodes.map((ep) => (
							<button
								key={ep.id}
								onClick={() => handleEpisodeClick(ep)}
								className="w-full flex items-center gap-3 px-3 py-2.5 rounded-xl hover:bg-accent transition-colors text-left"
							>
								<div className="w-8 h-8 rounded-lg bg-secondary flex items-center justify-center shrink-0">
									<span className="text-[10px] font-bold text-muted-foreground">
										{ep.episode != null
											? String(ep.episode).padStart(2, "0")
											: "?"}
									</span>
								</div>
								<div className="flex-1 min-w-0">
									<p className="text-sm truncate">{episodeTitle(ep.name)}</p>
								</div>
								{ep.sources.length > 0 && (
									<span className="text-[10px] bg-secondary/80 px-1.5 py-0.5 rounded-full shrink-0 text-muted-foreground font-medium">
										{ep.sources.length + 1} src
									</span>
								)}
								<ChevronRight className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
							</button>
						))}
						{currentEpisodes.length === 0 && (
							<p className="text-sm text-muted-foreground text-center py-8">
								No episodes
							</p>
						)}
					</div>
				)}

				{/* Step: Sources */}
				{step === "sources" && sourceEp && (
					<div className="flex flex-col flex-1 overflow-hidden">
						<div className="px-5 pb-3 shrink-0">
							<p className="text-sm font-semibold truncate">
								{episodeTitle(sourceEp.name)}
							</p>
							<p className="text-xs text-muted-foreground mt-0.5">
								Choose a source to play
							</p>
						</div>
						<div className="border-t border-border mx-4 shrink-0" />
						<div className="overflow-y-auto flex-1 py-2 px-3">
							<button
								className="w-full flex items-center gap-3 px-3 py-3 rounded-xl hover:bg-accent transition-colors text-left"
								onClick={() => handleSourcePick(sourceEp)}
							>
								<div className="w-8 h-8 rounded-lg bg-primary/10 flex items-center justify-center shrink-0">
									<Play className="h-3.5 w-3.5 text-primary ml-0.5" />
								</div>
								<div className="flex-1">
									<p className="text-sm font-medium">Source 1</p>
									<p className="text-xs text-muted-foreground">Default</p>
								</div>
								<ChevronRight className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
							</button>
							{sourceEp.sources.map((src, idx) => (
								<button
									key={idx}
									className="w-full flex items-center gap-3 px-3 py-3 rounded-xl hover:bg-accent transition-colors text-left"
									onClick={() => handleSourcePick({ ...sourceEp, url: src })}
								>
									<div className="w-8 h-8 rounded-lg bg-secondary flex items-center justify-center shrink-0">
										<Play className="h-3.5 w-3.5 text-muted-foreground ml-0.5" />
									</div>
									<div className="flex-1">
										<p className="text-sm font-medium">Source {idx + 2}</p>
									</div>
									<ChevronRight className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
								</button>
							))}
						</div>
					</div>
				)}

				<div className="shrink-0 pb-2" />
			</div>
		</div>
	);
};
