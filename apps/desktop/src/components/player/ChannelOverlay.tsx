import { useChannels } from "@/hooks/useChannels";
import { ScrollArea } from "@/components/ui/scroll-area";
import { X } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { Channel } from "@/lib/types";

interface ChannelOverlayProps {
	onClose: () => void;
	onSelectChannel: (channel: Channel) => void;
}

export const ChannelOverlay = ({ onClose, onSelectChannel }: ChannelOverlayProps) => {
	const { channels } = useChannels();

	const handleSelect = (ch: Channel) => {
		onSelectChannel(ch);
		onClose();
	};

	return (
		<div className="absolute right-0 top-0 bottom-0 w-80 bg-black/90 backdrop-blur-sm border-l border-white/10 flex flex-col">
			<div className="flex items-center justify-between p-3 border-b border-white/10">
				<h3 className="text-sm font-semibold text-white">Channels</h3>
				<Button
					variant="ghost"
					size="icon"
					onClick={onClose}
					className="h-7 w-7 text-white hover:bg-white/20"
				>
					<X className="h-4 w-4" />
				</Button>
			</div>
			<ScrollArea className="flex-1">
				<div className="p-2">
					{channels.map((ch) => (
						<button
							key={ch.id}
							onClick={() => handleSelect(ch)}
							className="flex items-center gap-2 w-full p-2 rounded-md text-left text-white hover:bg-white/10 transition-colors focus:outline-none focus:ring-1 focus:ring-primary"
							tabIndex={0}
						>
							<div className="h-8 w-8 rounded bg-white/10 flex items-center justify-center overflow-hidden shrink-0">
								{ch.logoUrl ? (
									<img
										src={ch.logoUrl}
										alt=""
										className="h-full w-full object-contain"
										loading="lazy"
									/>
								) : (
									<span className="text-[10px] text-white/50">TV</span>
								)}
							</div>
							<div className="min-w-0 flex-1">
								<p className="text-xs font-medium truncate">{ch.name}</p>
								<p className="text-[10px] text-white/50 truncate">
									{ch.groupTitle}
								</p>
							</div>
						</button>
					))}
				</div>
			</ScrollArea>
		</div>
	);
};
