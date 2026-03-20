import { useEffect } from "react";
import { Routes, Route } from "react-router-dom";
import { AppLayout } from "./components/AppLayout";
import { PlayerView } from "./components/player/VideoPlayer";
import { ChannelList } from "./components/channels/ChannelList";
import { PlaylistManager } from "./components/playlist/PlaylistManager";
import { Settings } from "./components/settings/Settings";
import { UpdateBanner } from "./components/UpdateBanner";
import { SplashScreen } from "./components/SplashScreen";
import { DonationPopup } from "./components/DonationPopup";
import { ChannelsContext, useChannelsProvider } from "./hooks/useChannels";
import { useUpdateChecker } from "./hooks/useUpdateChecker";
import { useSplashScreen } from "./hooks/useSplashScreen";
import { useDonationPrompt } from "./hooks/useDonationPrompt";
import { FullscreenProvider } from "./lib/fullscreen-context";

export default function App() {
	const channelsValue = useChannelsProvider();
	const updateState = useUpdateChecker();

	return (
		<ChannelsContext.Provider value={channelsValue}>
			<FullscreenProvider>
				<AppRoutes
					channelsRefresh={channelsValue.refreshProviders}
					channelsRefreshAll={channelsValue.refreshChannels}
					updateState={updateState}
				/>
			</FullscreenProvider>
		</ChannelsContext.Provider>
	);
}

// Inner component so useSplashScreen can access ChannelsContext via useChannels if needed.
interface AppRoutesProps {
	channelsRefresh: () => Promise<void>;
	channelsRefreshAll: () => Promise<void>;
	updateState: ReturnType<typeof useUpdateChecker>;
}

function AppRoutes({ channelsRefresh, channelsRefreshAll, updateState }: AppRoutesProps) {
	const splash = useSplashScreen({
		onComplete: () => {
			// Re-sync channels state after splash loading (playlist/EPG refresh)
			channelsRefresh().catch(() => {});
			channelsRefreshAll().catch(() => {});
		},
	});

	const donation = useDonationPrompt({ enabled: splash.dismissed });

	// Inject BMC floating widget script once splash is dismissed
	useEffect(() => {
		if (!splash.dismissed) return;
		if (document.querySelector('script[data-name="BMC-Widget"]')) return;
		const script = document.createElement("script");
		script.setAttribute("data-name", "BMC-Widget");
		script.setAttribute("data-cfasync", "false");
		script.src = "https://cdnjs.buymeacoffee.com/1.0.0/widget.prod.min.js";
		script.setAttribute("data-id", "MaxMB15");
		script.setAttribute("data-description", "Support me on Buy me a coffee!");
		script.setAttribute("data-message", "");
		script.setAttribute("data-color", "#5F7FFF");
		script.setAttribute("data-position", "Right");
		script.setAttribute("data-x_margin", "18");
		script.setAttribute("data-y_margin", "18");
		document.body.appendChild(script);
	}, [splash.dismissed]);

	return (
		<>
			{!splash.dismissed && <SplashScreen splash={splash} />}

			<Routes>
				<Route element={<AppLayout />}>
					<Route path="/" element={<ChannelList />} />
					<Route path="/player" element={<PlayerView />} />
					<Route path="/playlists" element={<PlaylistManager />} />
					<Route path="/settings" element={<Settings />} />
				</Route>
			</Routes>

			<UpdateBanner state={updateState} hidden={!splash.dismissed} />
			{donation.shouldShow && (
				<DonationPopup onDismiss={donation.dismiss} />
			)}
		</>
	);
}
