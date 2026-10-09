// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Max Boksem. See NOTICE for additional terms under GPLv3 section 7.

import { useEffect, useState } from "react";

/**
 * `value`, but only turning true once it has stayed true for `delayMs`.
 * Used for loading indicators so quick loads don't flash them on screen.
 */
export const useDelayedFlag = (value: boolean, delayMs: number): boolean => {
	const [delayed, setDelayed] = useState(false);
	useEffect(() => {
		if (!value) {
			setDelayed(false);
			return;
		}
		const t = setTimeout(() => setDelayed(true), delayMs);
		return () => clearTimeout(t);
	}, [value, delayMs]);
	return value && delayed;
};
