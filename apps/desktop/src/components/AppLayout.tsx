import { Outlet, NavLink } from "react-router-dom";
import { usePlatform } from "@/hooks/usePlatform";
import { cn } from "@/lib/utils";
import {
  Tv,
  List,
  CalendarDays,
  FolderOpen,
  Settings as SettingsIcon,
} from "lucide-react";

const navItems = [
  { to: "/", label: "Channels", icon: Tv },
  { to: "/player", label: "Player", icon: List },
  { to: "/guide", label: "Guide", icon: CalendarDays },
  { to: "/playlists", label: "Playlists", icon: FolderOpen },
  { to: "/settings", label: "Settings", icon: SettingsIcon },
];

export function AppLayout() {
  const { layoutMode } = usePlatform();

  if (layoutMode === "tv") {
    return <TvLayout />;
  }

  if (layoutMode === "mobile") {
    return <MobileLayout />;
  }

  return <DesktopLayout />;
}

function DesktopLayout() {
  return (
    <div className="flex h-screen overflow-hidden">
      <aside className="w-16 flex flex-col items-center py-4 gap-1 border-r border-border bg-card/50">
        {navItems.map(({ to, label, icon: Icon }) => (
          <NavLink
            key={to}
            to={to}
            className={({ isActive }) =>
              cn(
                "flex flex-col items-center justify-center w-12 h-12 rounded-lg text-muted-foreground transition-colors",
                isActive
                  ? "bg-primary/10 text-primary"
                  : "hover:bg-accent hover:text-accent-foreground"
              )
            }
            title={label}
          >
            <Icon className="h-5 w-5" />
            <span className="text-[10px] mt-0.5">{label}</span>
          </NavLink>
        ))}
      </aside>
      <main className="flex-1 overflow-hidden">
        <Outlet />
      </main>
    </div>
  );
}

function MobileLayout() {
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
}

function TvLayout() {
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
}
