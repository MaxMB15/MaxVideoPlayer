import { X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { PLAYER_SHORTCUTS } from "@/lib/hotkeys";

interface ShortcutsOverlayProps {
	onClose: () => void;
}

export const ShortcutsOverlay = ({ onClose }: ShortcutsOverlayProps) => (
	<div
		className="absolute inset-0 z-40 flex items-center justify-center bg-black/60 backdrop-blur-sm"
		onClick={(e) => {
			if (e.target === e.currentTarget) onClose();
		}}
	>
		<div
			role="dialog"
			aria-label="Keyboard shortcuts"
			className="w-full max-w-md mx-4 rounded-xl border border-white/10 bg-neutral-900/95 p-5 text-white shadow-2xl"
		>
			<div className="flex items-center justify-between mb-3">
				<h2 className="text-base font-semibold">Keyboard shortcuts</h2>
				<Button
					variant="ghost"
					size="icon"
					onClick={onClose}
					className="h-7 w-7 text-white hover:bg-white/20"
					aria-label="Close"
				>
					<X className="h-4 w-4" />
				</Button>
			</div>
			<ul className="space-y-1.5">
				{PLAYER_SHORTCUTS.map(({ keys, label }) => (
					<li
						key={label + keys.join()}
						className="flex items-center justify-between gap-4 text-sm"
					>
						<span className="text-white/70">{label}</span>
						<span className="flex gap-1 shrink-0">
							{keys.map((k) => (
								<kbd
									key={k}
									className="min-w-[1.75rem] rounded border border-white/20 bg-white/10 px-1.5 py-0.5 text-center text-xs font-mono text-white"
								>
									{k}
								</kbd>
							))}
						</span>
					</li>
				))}
			</ul>
		</div>
	</div>
);
