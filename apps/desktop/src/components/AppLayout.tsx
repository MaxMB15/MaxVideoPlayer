import { Outlet, NavLink, useLocation } from "react-router-dom";
import { usePlatform } from "@/hooks/usePlatform";
import { cn } from "@/lib/utils";
import { useEffect } from "react";
import { mpvSetVisible } from "@/lib/tauri";
import { useFullscreen } from "@/lib/fullscreen-context";
import { Tv, List, FolderOpen, Settings as SettingsIcon } from "lucide-react";

const navItems = [
	{ to: "/", label: "Channels", icon: Tv },
	{ to: "/player", label: "Player", icon: List },
	{ to: "/playlists", label: "Playlists", icon: FolderOpen },
	{ to: "/settings", label: "Settings", icon: SettingsIcon },
];

export const AppLayout = () => {
	const { layoutMode } = usePlatform();

	if (layoutMode === "tv") {
		return <TvLayout />;
	}

	if (layoutMode === "mobile") {
		return <MobileLayout />;
	}

	return <DesktopLayout />;
};

const DesktopLayout = () => {
	const { pathname } = useLocation();
	const isPlayer = pathname === "/player";
	const { isFullscreen } = useFullscreen();

	// Hide the native NSOpenGLView when not on the player route so it doesn't
	// bleed through transparent areas on other pages.
	useEffect(() => {
		mpvSetVisible(isPlayer).catch(() => {});
	}, [isPlayer]);

	return (
		<div className="flex h-screen overflow-hidden">
			<aside
				className={cn(
					"w-16 flex flex-col items-center py-3 gap-0.5 border-r border-border bg-card shrink-0",
					isFullscreen && "hidden"
				)}
			>
				{navItems.map(({ to, label, icon: Icon }) => (
					<NavLink
						key={to}
						to={to}
						className={({ isActive }) =>
							cn(
								"relative flex flex-col items-center justify-center w-full py-3 gap-1 text-muted-foreground transition-colors",
								isActive ? "text-primary" : "hover:text-foreground"
							)
						}
						title={label}
					>
						{({ isActive }) => (
							<>
								{isActive && (
									<span className="absolute left-0 top-2 bottom-2 w-0.5 rounded-r bg-primary" />
								)}
								<Icon className="h-5 w-5" />
								<span className="text-[9px] font-medium leading-none">{label}</span>
							</>
						)}
					</NavLink>
				))}
			</aside>
			<main className="flex-1 overflow-hidden">
				<Outlet />
			</main>
		</div>
	);
};

const MobileLayout = () => {
	return (
		<div className="flex flex-col h-screen">
			<main className="flex-1 overflow-auto">
				<Outlet />
			</main>
			<nav className="flex items-center justify-around border-t border-border bg-card/80 backdrop-blur-sm pb-safe">
				{navItems.map(({ to, label, icon: Icon }) => (
					<NavLink
						key={to}
						to={to}
						className={({ isActive }) =>
							cn(
								"flex flex-col items-center py-2 px-3 text-muted-foreground transition-colors",
								isActive ? "text-primary" : ""
							)
						}
					>
						<Icon className="h-5 w-5" />
						<span className="text-[10px] mt-0.5">{label}</span>
					</NavLink>
				))}
			</nav>
		</div>
	);
};

const TvLayout = () => {
	return (
		<div className="flex h-screen overflow-hidden">
			<aside className="w-20 flex flex-col items-center py-6 gap-2 border-r border-border bg-card/50">
				{navItems.map(({ to, label, icon: Icon }) => (
					<NavLink
						key={to}
						to={to}
						className={({ isActive }) =>
							cn(
								"flex flex-col items-center justify-center w-16 h-16 rounded-xl text-muted-foreground transition-colors focus:outline-none focus:ring-2 focus:ring-primary",
								isActive
									? "bg-primary/10 text-primary"
									: "hover:bg-accent hover:text-accent-foreground"
							)
						}
						tabIndex={0}
					>
						<Icon className="h-6 w-6" />
						<span className="text-xs mt-1">{label}</span>
					</NavLink>
				))}
			</aside>
			<main className="flex-1 overflow-hidden">
				<Outlet />
			</main>
		</div>
	);
};
