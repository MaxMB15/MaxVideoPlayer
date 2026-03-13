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
