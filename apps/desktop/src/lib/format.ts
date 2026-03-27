/**
 * Format seconds into a human-readable time string.
 * - Under 1 hour: "M:SS" (e.g. "3:07")
 * - 1 hour or more: "H:MM:SS" (e.g. "1:03:07")
 */
export const formatTime = (seconds: number): string => {
	const h = Math.floor(seconds / 3600);
	const m = Math.floor((seconds % 3600) / 60);
	const s = Math.floor(seconds % 60);
	if (h > 0) {
		return `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
	}
	return `${m}:${String(s).padStart(2, "0")}`;
};
