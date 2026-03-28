import { Pin, PinOff, ChevronRight } from "lucide-react";
import type { Category } from "@/lib/types";

interface GroupListProps {
	groups: string[];
	categories: Category[];
	onSelectGroup: (name: string) => void;
	isPinned: (name: string) => boolean;
	onTogglePin: (name: string) => void;
}

export const GroupList = ({
	groups,
	categories,
	onSelectGroup,
	isPinned,
	onTogglePin,
}: GroupListProps) => {
	const getChannelCount = (groupName: string) =>
		categories.find((c) => c.name === groupName)?.channelCount ?? 0;

	return (
		<div className="px-4 pt-2 flex flex-col gap-1">
			{groups.map((name) => (
				<div
					key={name}
					className="flex items-center justify-between p-2.5 rounded-lg bg-secondary hover:bg-accent transition-colors cursor-pointer"
					onClick={() => onSelectGroup(name)}
				>
					<div className="flex items-center gap-2">
						<span className="text-sm">{name}</span>
						<span className="text-xs text-muted-foreground">
							{getChannelCount(name)} ch
						</span>
					</div>
					<div className="flex items-center gap-2">
						<button
							onClick={(e) => {
								e.stopPropagation();
								onTogglePin(name);
							}}
							className={`p-1 rounded hover:bg-background transition-colors ${
								isPinned(name) ? "text-primary" : "text-muted-foreground"
							}`}
						>
							{isPinned(name) ? (
								<PinOff className="h-3.5 w-3.5" />
							) : (
								<Pin className="h-3.5 w-3.5" />
							)}
						</button>
						<ChevronRight className="h-4 w-4 text-muted-foreground" />
					</div>
				</div>
			))}
		</div>
	);
};
