/**
 * Returns the delay step size (seconds) based on how long the button has been held.
 * elapsed is in milliseconds.
 */
export const getDelayStep = (elapsed: number): number => {
	if (elapsed >= 3000) return 5.0;
	if (elapsed >= 1500) return 1.0;
	if (elapsed >= 600) return 0.5;
	return 0.1;
};

/**
 * Returns the repeat interval (ms) for the hold-to-accelerate timer.
 */
export const getDelayInterval = (elapsed: number): number => {
	if (elapsed >= 3000) return 50;
	if (elapsed >= 1500) return 80;
	if (elapsed >= 600) return 120;
	return 200;
};
