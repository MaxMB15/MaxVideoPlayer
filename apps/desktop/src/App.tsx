import { Routes, Route } from "react-router-dom";
import { AppLayout } from "./components/AppLayout";
import { PlayerView } from "./components/player/VideoPlayer";
import { ChannelList } from "./components/channels/ChannelList";
import { PlaylistManager } from "./components/playlist/PlaylistManager";
import { Settings } from "./components/settings/Settings";
import { UpdateBanner } from "./components/UpdateBanner";
import { SplashScreen } from "./components/SplashScreen";
import { DonationPopup } from "./components/DonationPopup";
import { ChannelsContext, useChannels, useChannelsProvider } from "./hooks/useChannels";
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
				<AppRoutes updateState={updateState} />
			</FullscreenProvider>
		</ChannelsContext.Provider>
	);
}

// Inner component so useSplashScreen can access ChannelsContext via useChannels.
interface AppRoutesProps {
	updateState: ReturnType<typeof useUpdateChecker>;
}

const AppRoutes = ({ updateState }: AppRoutesProps) => {
	const { refreshProviders } = useChannels();
	const splash = useSplashScreen({
		// After splash finishes any playlist/EPG refreshes, sync ChannelsContext so
		// polling in useChannels reads fresh lastUpdated timestamps (avoids re-triggering
		// the same refresh 60s later due to stale state).
		onComplete: () => {
			refreshProviders().catch(() => {});
		},
	});

	const donation = useDonationPrompt({ enabled: splash.dismissed });

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
			{donation.shouldShow && !updateState.update && (
				<DonationPopup onDismiss={donation.dismiss} />
			)}
		</>
	);
}
