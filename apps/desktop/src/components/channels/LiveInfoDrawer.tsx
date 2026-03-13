import { useState, useEffect, useRef, useCallback } from "react";
import { X, Tv2, Loader2 } from "lucide-react";
import type { Channel, EpgProgram } from "@/lib/types";
import { getEpgProgrammes } from "@/lib/tauri";

interface LiveInfoDrawerProps {
	channel: Channel;
	onClose: () => void;
}

const formatTime = (unix: number): string =>
	new Date(unix * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });

const formatDuration = (startTime: number, endTime: number): string => {
	const mins = Math.floor((endTime - startTime) / 60);
	if (mins < 60) return `${mins} min`;
	const h = Math.floor(mins / 60);
	const m = mins % 60;
	return m === 0 ? `${h}h` : `${h}h ${m}m`;
};

export const LiveInfoDrawer = ({ channel, onClose }: LiveInfoDrawerProps) => {
	const [visible, setVisible] = useState(false);
	const [programmes, setProgrammes] = useState<EpgProgram[]>([]);
	const [loading, setLoading] = useState(true);
	const [now, setNow] = useState(() => Math.floor(Date.now() / 1000));
	const currentRowRef = useRef<HTMLLIElement | null>(null);

	useEffect(() => {
		const id = requestAnimationFrame(() => setVisible(true));
		return () => cancelAnimationFrame(id);
	}, []);

	// Fetch today's schedule
	useEffect(() => {
		let cancelled = false;
		const todayStart = Math.floor(new Date().setHours(0, 0, 0, 0) / 1000);
		const todayEnd = todayStart + 86400;
		const channelId = channel.tvgId ?? channel.id;

		setLoading(true);
		getEpgProgrammes(channelId, todayStart, todayEnd)
			.then((data) => {
				if (!cancelled) {
					setProgrammes(data);
					setLoading(false);
				}
			})
			.catch(() => {
				if (!cancelled) {
					setProgrammes([]);
					setLoading(false);
				}
			});

		return () => {
			cancelled = true;
		};
	}, [channel.tvgId, channel.id]);

	// Update `now` every 30 seconds
	useEffect(() => {
		const id = setInterval(() => {
			setNow(Math.floor(Date.now() / 1000));
		}, 30_000);
		return () => clearInterval(id);
	}, []);

	// Auto-scroll to current programme after programmes load
	useEffect(() => {
		if (!loading && currentRowRef.current) {
			currentRowRef.current.scrollIntoView({ behavior: "smooth", block: "center" });
		}
	}, [loading]);

	const handleClose = useCallback(() => {
		setVisible(false);
		setTimeout(onClose, 300);
	}, [onClose]);

	const currentProgramme = programmes.find((p) => p.startTime <= now && p.endTime > now);
	const progress = currentProgramme
		? (now - currentProgramme.startTime) /
			(currentProgramme.endTime - currentProgramme.startTime)
		: 0;

	return (
		<div className="fixed inset-0 z-50">
			{/* Backdrop */}
			<div
				className={`absolute inset-0 bg-black/60 backdrop-blur-sm transition-opacity duration-300 ${
					visible ? "opacity-100" : "opacity-0"
				}`}
				onClick={handleClose}
			/>

			{/* Drawer */}
			<div
				className={`absolute bottom-0 left-0 right-0 bg-card rounded-t-2xl shadow-2xl flex flex-col transition-transform duration-300 ease-out max-h-[85vh] overflow-hidden ${
					visible ? "translate-y-0" : "translate-y-full"
				}`}
			>
				{/* Handle */}
				<div className="flex justify-center pt-3 pb-1 shrink-0">
					<div className="w-9 h-1 rounded-full bg-border" />
				</div>

				{/* Header row: logo + name + close */}
				<div className="flex items-center gap-3 px-5 pt-2 pb-4 shrink-0">
					{/* Logo */}
					<div className="w-10 h-10 rounded-lg bg-secondary overflow-hidden shrink-0 flex items-center justify-center">
						{channel.logoUrl ? (
							<img
								src={channel.logoUrl}
								alt=""
								className="h-full w-full object-contain p-0.5"
								loading="lazy"
							/>
						) : (
							<Tv2 className="h-5 w-5 text-muted-foreground/30" />
						)}
					</div>

					{/* Channel name + live badge */}
					<div className="flex flex-col justify-center gap-0.5 flex-1 min-w-0">
						<div className="flex items-center gap-1.5">
							<span className="flex items-center gap-1 text-[10px] font-medium text-red-400">
								<span className="h-1.5 w-1.5 rounded-full bg-red-400 animate-pulse" />
								LIVE
							</span>
						</div>
						<p className="text-sm font-semibold leading-tight truncate">
							{channel.name}
						</p>
					</div>

					{/* Close button */}
					<button
						onClick={handleClose}
						aria-label="Close"
						className="text-muted-foreground hover:text-foreground transition-colors shrink-0"
					>
						<X className="h-4 w-4" />
					</button>
				</div>

				<div className="border-t border-border mx-5 shrink-0" />

				{/* NOW PLAYING section */}
				{!loading && currentProgramme && (
					<div className="px-5 py-4 shrink-0">
						<p className="text-[10px] font-semibold tracking-widest text-muted-foreground uppercase mb-2">
							Now Playing
						</p>

						{/* Progress bar */}
						<div className="flex items-center gap-2 mb-2">
							<div className="flex-1 h-1.5 bg-secondary rounded-full overflow-hidden">
								<div
									className="h-full bg-primary rounded-full transition-all duration-1000"
									style={{
										width: `${Math.min(100, progress * 100).toFixed(1)}%`,
									}}
								/>
							</div>
							<span className="text-[11px] text-muted-foreground whitespace-nowrap shrink-0">
								{formatTime(currentProgramme.startTime)} →{" "}
								{formatTime(currentProgramme.endTime)}
								&ensp;(
								{formatDuration(
									currentProgramme.startTime,
									currentProgramme.endTime
								)}
								)
							</span>
						</div>

						{/* Title + description */}
						<p className="text-sm font-semibold leading-tight mb-1">
							{currentProgramme.title}
						</p>
						{currentProgramme.description && (
							<p className="text-xs text-muted-foreground leading-relaxed line-clamp-2">
								{currentProgramme.description}
							</p>
						)}
					</div>
				)}

				{!loading && currentProgramme && (
					<div className="border-t border-border mx-5 shrink-0" />
				)}

				{/* TODAY'S SCHEDULE section */}
				<div className="flex flex-col flex-1 min-h-0">
					<div className="px-5 pt-4 pb-2 shrink-0">
						<p className="text-[10px] font-semibold tracking-widest text-muted-foreground uppercase">
							Today's Schedule
						</p>
					</div>

					{loading ? (
						<div className="flex items-center justify-center flex-1 py-8">
							<Loader2 className="h-5 w-5 text-muted-foreground animate-spin" />
						</div>
					) : programmes.length === 0 ? (
						<div className="flex flex-col items-center justify-center flex-1 py-8 px-5 gap-2 text-center">
							<p className="text-sm text-muted-foreground">
								Schedule unavailable for this channel.
							</p>
							<p className="text-xs text-muted-foreground/60">
								No EPG data matched. Configure an EPG URL in provider settings to
								enable programme guides.
							</p>
						</div>
					) : (
						<ul className="flex-1 overflow-y-auto px-5 pb-4">
							{programmes.map((prog, idx) => {
								const isCurrent = prog.startTime <= now && prog.endTime > now;
								const isPast = prog.endTime <= now;

								return (
									<li
										key={`${prog.channelId}-${prog.startTime}-${idx}`}
										ref={isCurrent ? currentRowRef : null}
										className={`flex items-center gap-3 py-2.5 px-3 rounded-lg mb-0.5 transition-colors ${
											isCurrent ? "bg-primary/10" : "hover:bg-secondary/40"
										}`}
									>
										{/* Left accent bar */}
										<div
											className={`w-0.5 h-8 rounded-full shrink-0 ${
												isCurrent
													? "bg-primary"
													: isPast
														? "bg-border"
														: "bg-border/50"
											}`}
										/>

										{/* Time */}
										<span
											className={`text-xs tabular-nums shrink-0 w-12 ${
												isPast && !isCurrent
													? "text-muted-foreground/60"
													: "text-muted-foreground"
											}`}
										>
											{formatTime(prog.startTime)}
										</span>

										{/* Title */}
										<span
											className={`flex-1 text-sm leading-tight ${
												isCurrent
													? "font-semibold text-foreground"
													: isPast
														? "text-muted-foreground/70"
														: "text-foreground"
											}`}
										>
											{prog.title}
										</span>

										{/* Badge */}
										{isCurrent ? (
											<span className="text-[10px] font-semibold text-primary shrink-0">
												▶ now
											</span>
										) : isPast ? (
											<span className="text-[10px] text-muted-foreground/50 shrink-0">
												✓ past
											</span>
										) : null}
									</li>
								);
							})}
						</ul>
					)}
				</div>

				<div className="shrink-0 pb-2" />
			</div>
		</div>
	);
};
