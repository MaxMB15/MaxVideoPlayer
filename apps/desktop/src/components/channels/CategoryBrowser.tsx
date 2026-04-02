import { ChevronRight, FolderOpen, Tv2 } from "lucide-react";

interface SuperCategoryInfo {
	name: string;
	groupCount: number;
	channelCount: number;
}

interface CategoryBrowserProps {
	superCategories: SuperCategoryInfo[];
	topLevelGroups: { name: string; channelCount: number }[];
	onSelectCategory: (name: string) => void;
	onSelectGroup: (name: string) => void;
	onManage: () => void;
}

export const CategoryBrowser = ({
	superCategories,
	topLevelGroups,
	onSelectCategory,
	onSelectGroup,
	onManage,
}: CategoryBrowserProps) => (
	<div className="px-3 pt-2 pb-1">
		<div className="flex items-center justify-between mb-2">
			<span className="text-xs font-semibold text-muted-foreground uppercase tracking-wider">
				Categories
			</span>
			<button onClick={onManage} className="text-xs text-primary hover:underline">
				Manage
			</button>
		</div>
		<div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 gap-2">
			{superCategories.map((cat) => (
				<button
					key={cat.name}
					onClick={() => onSelectCategory(cat.name)}
					className="flex flex-col items-start gap-1.5 p-3 rounded-xl bg-secondary/80 border border-border/50 hover:bg-accent hover:border-primary/30 transition-all text-left group"
				>
					<div className="flex items-center justify-between w-full">
						<FolderOpen className="h-4 w-4 text-primary/70" />
						<ChevronRight className="h-3.5 w-3.5 text-muted-foreground/50 group-hover:text-primary transition-colors" />
					</div>
					<div className="text-sm font-medium leading-tight">{cat.name}</div>
					<div className="text-[10px] text-muted-foreground">
						{cat.groupCount} groups &middot; {cat.channelCount.toLocaleString()} ch
					</div>
				</button>
			))}
			{topLevelGroups.map((group) => (
				<button
					key={group.name}
					onClick={() => onSelectGroup(group.name)}
					className="flex flex-col items-start gap-1.5 p-3 rounded-xl bg-secondary/80 border border-border/50 hover:bg-accent hover:border-primary/30 transition-all text-left group"
				>
					<div className="flex items-center justify-between w-full">
						<Tv2 className="h-4 w-4 text-muted-foreground/70" />
						<ChevronRight className="h-3.5 w-3.5 text-muted-foreground/50 group-hover:text-primary transition-colors" />
					</div>
					<div className="text-sm font-medium leading-tight">{group.name}</div>
					<div className="text-[10px] text-muted-foreground">
						{group.channelCount.toLocaleString()} channels
					</div>
				</button>
			))}
		</div>
	</div>
);
