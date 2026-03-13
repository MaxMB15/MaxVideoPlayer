import { useState, useEffect } from "react";
import type { Platform, LayoutMode } from "@/lib/types";

export const usePlatform = () => {
	const [platform, setPlatform] = useState<Platform>("macos");
	const [layoutMode, setLayoutMode] = useState<LayoutMode>("desktop");

	useEffect(() => {
		const detect = async () => {
			try {
				const { platform: osPlatform } = await import("@tauri-apps/plugin-os");
				const p = osPlatform();

				let detected: Platform = "macos";
				if (p === "macos") detected = "macos";
				else if (p === "ios") detected = "ios";
				else if (p === "android") detected = "android";
				else if (p === "windows") detected = "windows";
				else if (p === "linux") detected = "linux";

				setPlatform(detected);

				if (detected === "ios") {
					setLayoutMode("mobile");
				} else if (detected === "android") {
					// Fire Stick / Android TV uses "tv" mode, phones use "mobile".
					// Heuristic: treat large android viewports as TV.
					const isTV = window.innerWidth >= 960 && window.innerHeight >= 540;
					setLayoutMode(isTV ? "tv" : "mobile");
				} else {
					setLayoutMode("desktop");
				}
			} catch {
				setPlatform("macos");
				setLayoutMode("desktop");
			}
		};
		detect();
	}, []);

	return { platform, layoutMode };
};
