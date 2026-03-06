import { cn } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import type { Category } from "@/lib/types";

interface CategoryFilterProps {
  categories: Category[];
  selected: string | null;
  onSelect: (name: string | null) => void;
}

export function CategoryFilter({
  categories,
  selected,
  onSelect,
}: CategoryFilterProps) {
  return (
    <ScrollArea className="w-full">
      <div className="flex gap-2 pb-2">
        <button
          onClick={() => onSelect(null)}
          className={cn(
            "shrink-0 px-3 py-1.5 rounded-full text-xs font-medium transition-colors",
            selected === null
              ? "bg-primary text-primary-foreground"
              : "bg-secondary text-secondary-foreground hover:bg-secondary/80"
          )}
        >
          All
        </button>
        {categories.map((cat) => (
          <button
            key={cat.id}
            onClick={() => onSelect(cat.name === selected ? null : cat.name)}
            className={cn(
              "shrink-0 flex items-center gap-1.5 px-3 py-1.5 rounded-full text-xs font-medium transition-colors",
              selected === cat.name
                ? "bg-primary text-primary-foreground"
                : "bg-secondary text-secondary-foreground hover:bg-secondary/80"
            )}
          >
            {cat.name}
            <Badge variant="secondary" className="h-4 px-1 text-[10px]">
              {cat.channelCount}
            </Badge>
          </button>
        ))}
      </div>
    </ScrollArea>
  );
}
