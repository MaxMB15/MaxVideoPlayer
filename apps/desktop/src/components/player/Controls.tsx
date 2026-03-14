import {
	Play,
	Pause,
	Square,
	Volume2,
	VolumeX,
	Maximize,
	Minimize2,
	Info,
	SkipBack,
	SkipForward,
	Subtitles,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Slider } from "@/components/ui/slider";
import { cn } from "@/lib/utils";
import type { PlayerState } from "@/lib/types";
import { useState, useEffect, useRef } from "react";

interface ControlsProps {
	state: PlayerState;
	visible: boolean;
	isFullscreen?: boolean;
	onPlay: () => void;
	onPause: () => void;
	onStop: () => void;
	onSeek: (position: number) => void;
	onVolumeChange: (volume: number) => void;
	onFullscreen?: () => void;
	onInfo?: () => void;
	onPrevEpisode?: () => void;
	onNextEpisode?: () => void;
	onSubtitles?: () => void;
	hasSubtitles?: boolean;
}

const formatTime = (seconds: number): string => {
	const h = Math.floor(seconds / 3600);
	const m = Math.floor((seconds % 3600) / 60);
	const s = Math.floor(seconds % 60);
	if (h > 0) {
		return `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
	}
	return `${m}:${String(s).padStart(2, "0")}`;
};

export const Controls = ({
	state,
	visible,
	isFullscreen,
	onPlay,
	onPause,
	onStop,
	onSeek,
	onVolumeChange,
	onFullscreen,
	onInfo,
	onPrevEpisode,
	onNextEpisode,
	onSubtitles,
	hasSubtitles,
}: ControlsProps) => {
	const [localPos, setLocalPos] = useState(state.position);
	const isSeeking = useRef(false);

	useEffect(() => {
		if (!isSeeking.current) {
			setLocalPos(state.position);
		}
	}, [state.position]);

	const handleSeekChange = (v: number) => {
		isSeeking.current = true;
		setLocalPos(v);
	};

	const handleSeekCommit = () => {
		isSeeking.current = false;
		onSeek(localPos);
	};

	return (
		<div
			className={cn(
				"absolute bottom-0 left-0 right-0 bg-gradient-to-t from-black/80 to-transparent p-4 pt-12 transition-opacity duration-300",
				visible ? "opacity-100" : "opacity-0 pointer-events-none"
			)}
		>
			{state.duration > 0 && (
				<div className="mb-3">
					<div onPointerUp={handleSeekCommit}>
						<Slider
							value={localPos}
							min={0}
							max={state.duration}
							step={1}
							onValueChange={handleSeekChange}
						/>
					</div>
					<div className="flex justify-between text-xs text-white/60 mt-1">
						<span>{formatTime(localPos)}</span>
						<span>{formatTime(state.duration)}</span>
					</div>
				</div>
			)}

			<div className="flex items-center gap-2">
				{/* Info button — leftmost */}
				{onInfo && (
					<Button
						variant="ghost"
						size="icon"
						className="text-white hover:bg-white/20"
						onClick={onInfo}
						aria-label="Channel info"
					>
						<Info className="h-5 w-5" />
					</Button>
				)}

				{onInfo && <div className="w-px h-4 bg-white/20 mx-0.5 shrink-0" />}

				{onSubtitles && (
					<>
						<div className="w-px h-4 bg-white/20 mx-0.5 shrink-0" />
						<Button
							variant="ghost"
							size="icon"
							className={cn("text-white hover:bg-white/20", hasSubtitles && "text-blue-400")}
							onClick={onSubtitles}
							aria-label="Subtitles"
						>
							<Subtitles className="h-5 w-5" />
						</Button>
					</>
				)}

				{/* Playback controls */}
				{state.isPaused || !state.isPlaying ? (
					<Button
						variant="ghost"
						size="icon"
						onClick={onPlay}
						className="text-white hover:bg-white/20"
					>
						<Play className="h-5 w-5" />
					</Button>
				) : (
					<Button
						variant="ghost"
						size="icon"
						onClick={onPause}
						className="text-white hover:bg-white/20"
					>
						<Pause className="h-5 w-5" />
					</Button>
				)}

				<Button
					variant="ghost"
					size="icon"
					onClick={onStop}
					className="text-white hover:bg-white/20"
				>
					<Square className="h-4 w-4" />
				</Button>

				{/* Volume */}
				<div className="flex items-center gap-2 ml-2">
					<Button
						variant="ghost"
						size="icon"
						onClick={() => onVolumeChange(state.volume > 0 ? 0 : 100)}
						className="text-white hover:bg-white/20"
					>
						{state.volume === 0 ? (
							<VolumeX className="h-5 w-5" />
						) : (
							<Volume2 className="h-5 w-5" />
						)}
					</Button>
					<div className="w-24">
						<Slider
							value={state.volume}
							min={0}
							max={150}
							step={1}
							onValueChange={onVolumeChange}
						/>
					</div>
				</div>

				<div className="flex-1" />

				{/* Episode navigation — right side */}
				{onPrevEpisode && (
					<Button
						variant="ghost"
						size="icon"
						className="text-white hover:bg-white/20"
						onClick={onPrevEpisode}
						aria-label="Previous episode"
					>
						<SkipBack className="h-5 w-5" />
					</Button>
				)}

				{onNextEpisode && (
					<Button
						variant="ghost"
						size="icon"
						className="text-white hover:bg-white/20"
						onClick={onNextEpisode}
						aria-label="Next episode"
					>
						<SkipForward className="h-5 w-5" />
					</Button>
				)}

				{/* Fullscreen */}
				<Button
					variant="ghost"
					size="icon"
					className="text-white hover:bg-white/20"
					onClick={onFullscreen}
				>
					{isFullscreen ? (
						<Minimize2 className="h-5 w-5" />
					) : (
						<Maximize className="h-5 w-5" />
					)}
				</Button>
			</div>
		</div>
	);
};
