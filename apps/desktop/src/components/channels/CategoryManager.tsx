import { useState, useEffect } from "react";
import { RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { getGroupHierarchy, categorizeProvider, getGeminiApiKey } from "@/lib/tauri";
import type { GroupHierarchyEntry } from "@/lib/types";
import type { Channel } from "@/lib/types";

interface CategoryManagerProps {
	providerId: string;
	contentType: string;
	channels: Channel[];
	onClose: () => void;
	onHierarchyChanged: () => void;
}

export const CategoryManager = ({
	providerId,
	contentType,
	channels,
	onClose,
	onHierarchyChanged,
}: CategoryManagerProps) => {
	const [entries, setEntries] = useState<GroupHierarchyEntry[]>([]);
	const [categorizing, setCategorizing] = useState(false);
	const [error, setError] = useState<string | null>(null);

	useEffect(() => {
		getGroupHierarchy(providerId, contentType)
			.then(setEntries)
			.catch(() => {});
	}, [providerId, contentType]);

	const superCats = [
		...new Set(entries.filter((e) => e.superCategory).map((e) => e.superCategory!)),
	];
	const uncategorized = entries.filter((e) => !e.superCategory);

	const handleRecategorize = async () => {
		setCategorizing(true);
		setError(null);
		try {
			const apiKey = await getGeminiApiKey();
			if (!apiKey) {
				setError("No Gemini API key configured. Add one in Settings → AI.");
				return;
			}
			const groupNames = [
				...new Set(
					channels.filter((c) => c.contentType === contentType).map((c) => c.groupTitle)
				),
			];
			const groupsWithSamples: [string, string[]][] = groupNames.map((g) => [
				g,
				channels
					.filter((c) => c.groupTitle === g)
					.slice(0, 5)
					.map((c) => c.name),
			]);
			const result = await categorizeProvider(
				providerId,
				contentType,
				apiKey,
				groupsWithSamples
			);
			setEntries(result);
			onHierarchyChanged();
		} catch (e) {
			setError(String(e));
		} finally {
			setCategorizing(false);
		}
	};

	// TODO: Wire up drag-and-drop to move groups between categories
	// Uses updateGroupHierarchyEntry(providerId, contentType, groupName, newSuperCategory, sortOrder)

	return (
		<div className="flex flex-col h-full">
			<div className="flex items-center justify-between p-4 border-b border-border">
				<div>
					<h2 className="text-base font-semibold">Manage Categories</h2>
					<p className="text-xs text-muted-foreground mt-0.5">
						{contentType} channels &middot; {entries.length} groups
					</p>
				</div>
				<div className="flex gap-2">
					<Button
						size="sm"
						variant="outline"
						onClick={handleRecategorize}
						disabled={categorizing}
					>
						<RefreshCw
							className={`h-3.5 w-3.5 mr-1.5 ${categorizing ? "animate-spin" : ""}`}
						/>
						{categorizing ? "Categorizing..." : "Re-categorize with AI"}
					</Button>
					<Button size="sm" variant="ghost" onClick={onClose}>
						Close
					</Button>
				</div>
			</div>

			{error && (
				<div className="mx-4 mt-3 p-2.5 rounded-lg bg-destructive/10 text-destructive text-xs">
					{error}
				</div>
			)}

			<div className="flex-1 overflow-y-auto p-4 space-y-4">
				{superCats.map((catName) => {
					const groups = entries.filter((e) => e.superCategory === catName);
					return (
						<div key={catName}>
							<div className="flex items-center justify-between mb-2">
								<span className="text-sm font-medium">{catName}</span>
								<span className="text-xs text-muted-foreground">
									{groups.length} groups
								</span>
							</div>
							<div className="bg-secondary rounded-lg p-2 flex flex-wrap gap-1.5">
								{groups.map((g) => (
									<span
										key={g.groupName}
										className="bg-background px-2.5 py-1 rounded text-xs flex items-center gap-1"
									>
										{g.groupName}
										{g.isUserOverride && (
											<span className="text-[9px] bg-primary/20 text-primary px-1 rounded">
												user
											</span>
										)}
									</span>
								))}
							</div>
						</div>
					);
				})}

				{uncategorized.length > 0 && (
					<div>
						<div className="flex items-center justify-between mb-2">
							<span className="text-sm font-medium text-muted-foreground">
								Uncategorized
							</span>
							<span className="text-xs text-muted-foreground">
								{uncategorized.length} groups
							</span>
						</div>
						<div className="bg-secondary rounded-lg p-2 flex flex-wrap gap-1.5 border border-dashed border-border">
							{uncategorized.map((g) => (
								<span
									key={g.groupName}
									className="bg-background px-2.5 py-1 rounded text-xs"
								>
									{g.groupName}
								</span>
							))}
						</div>
					</div>
				)}
			</div>

			<div className="p-3 border-t border-border">
				<p className="text-[10px] text-muted-foreground">
					<span className="bg-primary/20 text-primary px-1 rounded text-[9px]">user</span>{" "}
					= manually placed (preserved on re-categorize)
				</p>
			</div>
		</div>
	);
};
