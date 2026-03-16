import { Routes, Route } from "react-router-dom";
import { AppLayout } from "./components/AppLayout";
import { PlayerView } from "./components/player/VideoPlayer";
import { ChannelList } from "./components/channels/ChannelList";
import { PlaylistManager } from "./components/playlist/PlaylistManager";
import { Settings } from "./components/settings/Settings";
import { UpdateBanner } from "./components/UpdateBanner";
import { ChannelsContext, useChannelsProvider } from "./hooks/useChannels";
import { useUpdateChecker } from "./hooks/useUpdateChecker";
import { FullscreenProvider } from "./lib/fullscreen-context";

export default function App() {
	const channelsValue = useChannelsProvider();
	const updateState = useUpdateChecker();

	return (
		<ChannelsContext.Provider value={channelsValue}>
			<FullscreenProvider>
				<Routes>
					<Route element={<AppLayout />}>
						<Route path="/" element={<ChannelList />} />
						<Route path="/player" element={<PlayerView />} />
						<Route path="/playlists" element={<PlaylistManager />} />
						<Route path="/settings" element={<Settings />} />
					</Route>
				</Routes>
				<UpdateBanner state={updateState} />
			</FullscreenProvider>
		</ChannelsContext.Provider>
	);
}
