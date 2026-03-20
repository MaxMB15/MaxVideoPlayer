import { useState, useCallback, useEffect } from "react";

const STORAGE_KEY = "donation-last-shown";
const THIRTY_DAYS_MS = 30 * 24 * 60 * 60 * 1000;

interface UseDonationPromptOptions {
	enabled: boolean;
}

interface DonationPromptState {
	shouldShow: boolean;
	dismiss: () => void;
}

function shouldShowNow(): boolean {
	const stored = localStorage.getItem(STORAGE_KEY);
	if (!stored) return true;
	const lastShown = parseInt(stored, 10);
	return isNaN(lastShown) ? true : Date.now() - lastShown >= THIRTY_DAYS_MS;
}

export function useDonationPrompt({ enabled }: UseDonationPromptOptions): DonationPromptState {
	const [shouldShow, setShouldShow] = useState(false);

	// Re-evaluate whenever enabled flips to true (i.e. splash just dismissed)
	useEffect(() => {
		if (enabled) setShouldShow(shouldShowNow());
	}, [enabled]);

	const dismiss = useCallback(() => {
		localStorage.setItem(STORAGE_KEY, String(Date.now()));
		setShouldShow(false);
	}, []);

	return { shouldShow, dismiss };
}
