import { ChevronRight } from "lucide-react";

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
	<div className="px-4 pt-2">
		<div className="flex items-center justify-between mb-2.5">
			<span className="text-sm font-semibold text-muted-foreground">All Categories</span>
			<button onClick={onManage} className="text-xs text-primary hover:underline">
				Manage
			</button>
		</div>
		<div className="flex flex-col gap-1.5">
			{superCategories.map((cat) => (
				<button
					key={cat.name}
					onClick={() => onSelectCategory(cat.name)}
					className="flex items-center justify-between p-3 rounded-lg bg-secondary hover:bg-accent transition-colors text-left"
				>
					<div>
						<div className="text-sm font-medium">{cat.name}</div>
						<div className="text-xs text-muted-foreground">
							{cat.groupCount} groups &middot; {cat.channelCount} channels
						</div>
					</div>
					<ChevronRight className="h-4 w-4 text-muted-foreground" />
				</button>
			))}
			{topLevelGroups.map((group) => (
				<button
					key={group.name}
					onClick={() => onSelectGroup(group.name)}
					className="flex items-center justify-between p-3 rounded-lg bg-secondary hover:bg-accent transition-colors text-left"
				>
					<div>
						<div className="text-sm font-medium">{group.name}</div>
						<div className="text-xs text-muted-foreground">
							{group.channelCount} channels
						</div>
					</div>
					<ChevronRight className="h-4 w-4 text-muted-foreground" />
				</button>
			))}
		</div>
	</div>
);
