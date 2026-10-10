// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Max Boksem. See NOTICE for additional terms under GPLv3 section 7.

import { createContext, useContext, useState } from "react";

interface FullscreenContextValue {
	isFullscreen: boolean;
	setFullscreen: (v: boolean) => void;
}

export const FullscreenContext = createContext<FullscreenContextValue>({
	isFullscreen: false,
	setFullscreen: () => {},
});

export const FullscreenProvider = ({ children }: { children: React.ReactNode }) => {
	const [isFullscreen, setFullscreen] = useState(false);
	return (
		<FullscreenContext.Provider value={{ isFullscreen, setFullscreen }}>
			{children}
		</FullscreenContext.Provider>
	);
};

export const useFullscreen = () => useContext(FullscreenContext);
