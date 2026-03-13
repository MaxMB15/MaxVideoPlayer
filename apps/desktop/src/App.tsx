import { Routes, Route } from "react-router-dom";
import { AppLayout } from "./components/AppLayout";
import { PlayerView } from "./components/player/VideoPlayer";
import { ChannelList } from "./components/channels/ChannelList";
import { PlaylistManager } from "./components/playlist/PlaylistManager";
import { ProgramGuide } from "./components/epg/ProgramGuide";
import { Settings } from "./components/settings/Settings";
import { ChannelsContext, useChannelsProvider } from "./hooks/useChannels";
import { FullscreenProvider } from "./lib/fullscreen-context";

export default function App() {
	const channelsValue = useChannelsProvider();

	return (
		<ChannelsContext.Provider value={channelsValue}>
			<FullscreenProvider>
				<Routes>
					<Route element={<AppLayout />}>
						<Route path="/" element={<ChannelList />} />
						<Route path="/player" element={<PlayerView />} />
						<Route path="/guide" element={<ProgramGuide />} />
						<Route path="/playlists" element={<PlaylistManager />} />
						<Route path="/settings" element={<Settings />} />
					</Route>
				</Routes>
			</FullscreenProvider>
		</ChannelsContext.Provider>
	);
}
