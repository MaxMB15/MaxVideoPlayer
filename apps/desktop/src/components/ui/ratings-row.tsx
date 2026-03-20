import { Star } from "lucide-react";
import type { OmdbData, MdbListData } from "@/lib/types";

interface RatingsRowProps {
	omdbData?: OmdbData | null;
	mdbListData?: MdbListData | null;
}

const formatVotes = (n: number): string => {
	if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
	if (n >= 1_000) return `${(n / 1_000).toFixed(0)}K`;
	return n.toString();
};

export const RatingsRow = ({ omdbData, mdbListData }: RatingsRowProps) => {
	// Priority: use MDBList data when available, fall back to OMDB
	const rawOmdbRating = omdbData?.imdbRating;
	const omdbRatingClean = rawOmdbRating && rawOmdbRating !== "N/A" ? rawOmdbRating : null;
	const imdbRating =
		mdbListData?.imdbRating != null ? mdbListData.imdbRating.toFixed(1) : omdbRatingClean;
	const imdbVotes = mdbListData?.imdbVotes ?? null;
	const rtCritic = mdbListData?.tomatometer ?? null;
	// When MDBList has no tomatometer, fall back to OMDB's rottenTomatoes string
	const rtCriticFallback =
		rtCritic === null && omdbData?.rottenTomatoes && omdbData.rottenTomatoes !== "N/A"
			? omdbData.rottenTomatoes
			: null;
	const rtAudience = mdbListData?.tomatoAudienceScore ?? null;
	const rtAudienceCount = mdbListData?.tomatoAudienceCount ?? null;
	const rtCriticCount = mdbListData?.tomatometerCount ?? null;

	const hasAny = imdbRating || rtCritic !== null || rtCriticFallback || rtAudience !== null;
	if (!hasAny) return null;

	return (
		<div className="flex items-center gap-1.5 flex-wrap">
			{imdbRating && (
				<span className="flex items-center gap-1 text-[11px] font-semibold bg-yellow-500/15 text-yellow-600 dark:text-yellow-400 px-2 py-0.5 rounded-full">
					<Star className="h-2.5 w-2.5" />
					{imdbRating}
					{imdbVotes && (
						<span className="font-normal opacity-70">· {formatVotes(imdbVotes)}</span>
					)}
				</span>
			)}
			{rtCritic !== null && (
				<span className="text-[11px] font-semibold bg-red-500/15 text-red-500 px-2 py-0.5 rounded-full">
					🍅 {rtCritic}%
					{rtCriticCount && (
						<span className="font-normal opacity-70">
							{" "}
							· {formatVotes(rtCriticCount)}
						</span>
					)}
				</span>
			)}
			{rtCriticFallback && (
				<span className="text-[11px] font-semibold bg-red-500/15 text-red-500 px-2 py-0.5 rounded-full">
					🍅 {rtCriticFallback}
				</span>
			)}
			{rtAudience !== null && (
				<span className="text-[11px] font-semibold bg-orange-500/15 text-orange-500 px-2 py-0.5 rounded-full">
					🍿 {rtAudience}%
					{rtAudienceCount && (
						<span className="font-normal opacity-70">
							{" "}
							· {formatVotes(rtAudienceCount)}
						</span>
					)}
				</span>
			)}
		</div>
	);
};
