import { Download, RefreshCw } from "lucide-react";
import type { SplashScreenState, SplashStep, StepStatus } from "@/hooks/useSplashScreen";
import type { Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { useState } from "react";

interface SplashScreenProps {
	splash: SplashScreenState;
}

export function SplashScreen({ splash }: SplashScreenProps) {
	const { steps, allDone, progress, update, hasProviders, dismiss } = splash;

	return (
		<div className="fixed inset-0 z-50 flex bg-background">
			<LeftPanel
				steps={steps}
				allDone={allDone}
				progress={progress}
				update={update}
				hasProviders={hasProviders}
				onDismiss={dismiss}
			/>
			<RightPanel />
		</div>
	);
}

// ── Left panel ──────────────────────────────────────────────────────────────

interface LeftPanelProps {
	steps: SplashStep[];
	allDone: boolean;
	progress: number;
	update: Update | null;
	hasProviders: boolean;
	onDismiss: () => void;
}

function LeftPanel({ steps, allDone, progress, update, hasProviders, onDismiss }: LeftPanelProps) {
	const [installing, setInstalling] = useState(false);
	const [installProgress, setInstallProgress] = useState<number | null>(null);

	const handleInstall = async () => {
		if (!update) return;
		setInstalling(true);
		setInstallProgress(0);
		try {
			let downloaded = 0;
			let total: number | undefined;
			await update.downloadAndInstall((event) => {
				if (event.event === "Started") {
					total = event.data.contentLength ?? undefined;
				} else if (event.event === "Progress") {
					downloaded += event.data.chunkLength;
					if (total) setInstallProgress(Math.round((downloaded / total) * 100));
				}
			});
			await relaunch();
		} catch {
			setInstalling(false);
			setInstallProgress(null);
		}
	};

	return (
		<div className="flex-1 flex flex-col items-center justify-center gap-6 px-10 border-r border-border">
			{/* Logo */}
			<div className="flex items-center gap-3 self-start">
				<div className="w-10 h-10 rounded-xl bg-primary flex items-center justify-center text-primary-foreground shrink-0">
					<svg viewBox="0 0 24 24" fill="currentColor" className="w-5 h-5">
						<polygon points="5,3 19,12 5,21" />
					</svg>
				</div>
				<div>
					<p className="text-base font-bold leading-tight">MaxVideoPlayer</p>
					<p className="text-xs text-muted-foreground">Open Source IPTV Player</p>
				</div>
			</div>

			{/* Welcome text (no providers) or progress (has providers) */}
			{!hasProviders ? (
				<div className="self-start space-y-1">
					<p className="text-sm font-semibold">Welcome to MaxVideoPlayer</p>
					<p className="text-xs text-muted-foreground">
						Add a playlist in the Playlists tab to get started.
					</p>
				</div>
			) : (
				<>
					{/* Progress bar */}
					<div className="w-full">
						<div className="h-1 w-full rounded-full bg-secondary overflow-hidden">
							<div
								className="h-full bg-primary rounded-full transition-all duration-300"
								style={{ width: `${Math.round(progress * 100)}%` }}
							/>
						</div>
					</div>

					{/* Step list */}
					<div className="w-full space-y-2">
						{steps.map((step) => (
							<StepRow key={step.id} step={step} />
						))}
					</div>
				</>
			)}

			{/* No-providers: single update step */}
			{!hasProviders && steps.length > 0 && (
				<div className="w-full space-y-2">
					{steps.map((step) => (
						<StepRow key={step.id} step={step} />
					))}
				</div>
			)}

			{/* Update card */}
			{allDone && update && (
				<div className="w-full rounded-xl bg-primary/10 border border-primary/25 px-4 py-3 space-y-1">
					<p className="text-sm font-semibold text-primary">
						Update available — v{update.version}
					</p>
					<p className="text-xs text-muted-foreground">
						{update.body ?? "A new version is ready to install."}
					</p>
					{installing && installProgress !== null && (
						<div className="mt-1 h-1 w-full rounded-full bg-secondary overflow-hidden">
							<div
								className="h-full bg-primary transition-all duration-200"
								style={{ width: `${installProgress}%` }}
							/>
						</div>
					)}
				</div>
			)}

			{/* Action row */}
			<div className="flex items-center gap-3 self-start">
				{allDone && update ? (
					<>
						<button
							onClick={handleInstall}
							disabled={installing}
							className="flex items-center gap-1.5 text-sm font-semibold bg-primary text-primary-foreground px-4 py-2 rounded-lg hover:bg-primary/90 transition-colors disabled:opacity-60"
						>
							{installing ? (
								<RefreshCw className="h-4 w-4 animate-spin" />
							) : (
								<Download className="h-4 w-4" />
							)}
							{installing
								? installProgress !== null
									? `Downloading… ${installProgress}%`
									: "Installing…"
								: "Install Update"}
						</button>
						<button
							onClick={onDismiss}
							className="text-sm text-muted-foreground underline hover:text-foreground transition-colors"
						>
							Skip for now →
						</button>
					</>
				) : (
					<button
						onClick={onDismiss}
						disabled={!allDone}
						className="text-sm font-semibold bg-primary text-primary-foreground px-4 py-2 rounded-lg hover:bg-primary/90 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
					>
						Get Started →
					</button>
				)}
			</div>
		</div>
	);
}

// ── Step row ──────────────────────────────────────────────────────────────

function StepRow({ step }: { step: SplashStep }) {
	return (
		<div className="flex items-center gap-2.5 text-xs">
			<StepIcon status={step.status} />
			<span
				className={
					step.status === "done"
						? "text-foreground"
						: step.status === "active"
							? "text-primary font-medium"
							: "text-muted-foreground"
				}
			>
				{step.label}
			</span>
		</div>
	);
}

function StepIcon({ status }: { status: StepStatus }) {
	if (status === "done") {
		return (
			<span className="w-4 h-4 rounded-full bg-green-500 flex items-center justify-center shrink-0">
				<svg viewBox="0 0 12 12" fill="none" className="w-2.5 h-2.5">
					<path d="M2 6l3 3 5-5" stroke="white" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
				</svg>
			</span>
		);
	}
	if (status === "active") {
		return (
			<RefreshCw className="w-4 h-4 text-primary animate-spin shrink-0" />
		);
	}
	return (
		<span className="w-4 h-4 rounded-full border border-border shrink-0" />
	);
}

// ── Right panel ───────────────────────────────────────────────────────────

function RightPanel() {
	return (
		<div className="w-52 flex flex-col items-center justify-center gap-4 px-6 bg-card">
			<span className="text-4xl" role="img" aria-label="coffee">☕</span>
			<div className="text-center space-y-1">
				<p className="text-sm font-semibold">Support free &amp; open source software</p>
				<p className="text-xs text-muted-foreground">
					MaxVideoPlayer is free forever. If it saves you money, consider buying me a coffee.
				</p>
			</div>
			<a
				href="https://buymeacoffee.com/MaxMB15"
				target="_blank"
				rel="noreferrer"
				className="w-full text-center text-sm font-semibold bg-[#5F7FFF] text-white px-4 py-2 rounded-lg hover:opacity-90 transition-opacity"
			>
				Buy me a coffee
			</a>
			<p className="text-[10px] text-muted-foreground text-center">
				No account needed · takes 2 seconds
			</p>
		</div>
	);
}
