export function parseDateMs(value: string | null | undefined): number {
	if (!value) return 0;
	const ms = Date.parse(value);
	return isNaN(ms) ? 0 : ms;
}
