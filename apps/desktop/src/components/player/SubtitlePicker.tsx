import { useState, useEffect } from "react";
import { Check, Loader2, X } from "lucide-react";
import { searchSubtitles, downloadSubtitle, mpvSubAdd, mpvSubRemove } from "@/lib/tauri";
import type { SubtitleEntry, SubtitleSearchResult } from "@/lib/types";
import { cn } from "@/lib/utils";

interface SubtitlePickerProps {
	imdbId: string;
	season?: number;
	episode?: number;
	onClose: () => void;
}

export const SubtitlePicker = ({ imdbId, season, episode, onClose }: SubtitlePickerProps) => {
	const [loading, setLoading] = useState(true);
	const [result, setResult] = useState<SubtitleSearchResult | null>(null);
	const [selectedFileId, setSelectedFileId] = useState<number | null>(null);
	const [downloadingId, setDownloadingId] = useState<number | null>(null);
	const [error, setError] = useState<string | null>(null);

	// Search on mount — lazy, runs once
	useEffect(() => {
		searchSubtitles(imdbId, season, episode)
			.then(setResult)
			.catch((e) => setError(String(e)))
			.finally(() => setLoading(false));
	}, [imdbId, season, episode]);

	const handleSelect = async (entry: SubtitleEntry) => {
		if (downloadingId !== null) return;

		setDownloadingId(entry.fileId);
		setError(null);
		try {
			// Remove previous subtitle (index -1 removes all)
			await mpvSubRemove(-1);
			const localPath = await downloadSubtitle(entry.fileId);
			await mpvSubAdd(localPath);
			setSelectedFileId(entry.fileId);
		} catch (e) {
			setError("Failed to load subtitle. Try another.");
		} finally {
			setDownloadingId(null);
		}
	};

	const handleNone = async () => {
		await mpvSubRemove(-1).catch(() => {});
		setSelectedFileId(null);
	};

	// Group entries by language
	const grouped = (result?.languages ?? []).map((lang) => ({
		lang,
		entries: result!.entries.filter((e) => e.languageCode === lang),
	}));

	return (
		<div className="absolute bottom-20 right-4 z-50 w-72 max-h-[50vh] overflow-y-auto rounded-xl bg-black/90 backdrop-blur-sm border border-white/10 shadow-2xl text-white text-sm">
			{/* Header */}
			<div className="flex items-center justify-between px-3 py-2.5 border-b border-white/10">
				<span className="font-semibold text-xs tracking-wide uppercase text-white/70">
					Subtitles
				</span>
				<button onClick={onClose} className="text-white/50 hover:text-white">
					<X className="h-4 w-4" />
				</button>
			</div>

			{loading && (
				<div className="flex items-center gap-2 p-3 text-white/60">
					<Loader2 className="h-4 w-4 animate-spin" />
					<span>Searching…</span>
				</div>
			)}

			{!loading && !result?.entries.length && (
				<div className="p-3 text-white/50 text-xs">No subtitles found</div>
			)}

			{!loading && result && result.entries.length > 0 && (
				<div className="p-1">
					{/* None option */}
					<button
						onClick={handleNone}
						className={cn(
							"w-full text-left px-3 py-1.5 rounded-lg text-xs hover:bg-white/10",
							selectedFileId === null && "text-white font-medium"
						)}
					>
						{selectedFileId === null && <Check className="inline h-3 w-3 mr-1.5" />}
						None
					</button>

					{/* Grouped by language */}
					{grouped.map(({ lang, entries }) => (
						<div key={lang}>
							<div className="px-3 py-1 text-[10px] uppercase tracking-wide text-white/40 font-medium mt-1">
								{lang} ({entries.length})
							</div>
							{entries.map((entry) => (
								<button
									key={entry.fileId}
									onClick={() => handleSelect(entry)}
									disabled={downloadingId !== null}
									className="w-full text-left px-3 py-1.5 rounded-lg text-xs hover:bg-white/10 disabled:opacity-50 flex items-center gap-1.5"
								>
									{downloadingId === entry.fileId ? (
										<Loader2 className="h-3 w-3 animate-spin shrink-0" />
									) : selectedFileId === entry.fileId ? (
										<Check className="h-3 w-3 shrink-0 text-blue-400" />
									) : (
										<span className="w-3 shrink-0" />
									)}
									<span className="truncate">
										{entry.releaseName ?? `Subtitle ${entry.fileId}`}
									</span>
									<span className="text-white/30 shrink-0">.{entry.format}</span>
								</button>
							))}
						</div>
					))}
				</div>
			)}

			{error && (
				<div className="px-3 py-2 text-xs text-red-400 border-t border-white/10">
					{error}
				</div>
			)}
		</div>
	);
};
