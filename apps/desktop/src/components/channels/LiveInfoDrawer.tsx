import { useState, useEffect } from "react";
import { X, Tv2, CalendarClock } from "lucide-react";
import type { Channel } from "@/lib/types";

interface LiveInfoDrawerProps {
  channel: Channel;
  onClose: () => void;
}

export function LiveInfoDrawer({ channel, onClose }: LiveInfoDrawerProps) {
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    const id = requestAnimationFrame(() => setVisible(true));
    return () => cancelAnimationFrame(id);
  }, []);

  const handleClose = () => {
    setVisible(false);
    setTimeout(onClose, 300);
  };

  return (
    <div className="fixed inset-0 z-50">
      <div
        className={`absolute inset-0 bg-black/60 backdrop-blur-sm transition-opacity duration-300 ${
          visible ? "opacity-100" : "opacity-0"
        }`}
        onClick={handleClose}
      />
      <div
        className={`absolute bottom-0 left-0 right-0 bg-card rounded-t-2xl shadow-2xl flex flex-col transition-transform duration-300 ease-out max-h-[85vh] overflow-hidden ${
          visible ? "translate-y-0" : "translate-y-full"
        }`}
      >
        {/* Handle */}
        <div className="flex justify-center pt-3 pb-1 shrink-0">
          <div className="w-9 h-1 rounded-full bg-border" />
        </div>

        {/* Close button */}
        <div className="flex justify-end px-5 pt-1 shrink-0">
          <button
            onClick={handleClose}
            aria-label="Close"
            className="text-muted-foreground hover:text-foreground transition-colors"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {/* Channel info */}
        <div className="flex gap-4 px-5 pb-5 shrink-0">
          {/* Logo */}
          <div className="w-20 h-20 rounded-xl bg-secondary overflow-hidden shrink-0 flex items-center justify-center">
            {channel.logoUrl ? (
              <img src={channel.logoUrl} alt="" className="h-full w-full object-contain p-1" loading="lazy" />
            ) : (
              <Tv2 className="h-8 w-8 text-muted-foreground/30" />
            )}
          </div>

          {/* Details */}
          <div className="flex flex-col justify-center gap-1.5 flex-1 min-w-0">
            <div className="flex items-center gap-2">
              <span className="flex items-center gap-1 text-[10px] font-medium text-red-400">
                <span className="h-1.5 w-1.5 rounded-full bg-red-400 animate-pulse" />
                LIVE
              </span>
            </div>
            <p className="text-base font-semibold leading-tight line-clamp-2">{channel.name}</p>
            {channel.groupTitle && (
              <p className="text-xs text-muted-foreground">{channel.groupTitle}</p>
            )}
          </div>
        </div>

        <div className="border-t border-border mx-5 shrink-0" />

        {/* EPG Schedule — scaffolding */}
        <div className="flex-1 overflow-y-auto px-5 py-4">
          <div className="flex items-center gap-2 mb-3">
            <CalendarClock className="h-4 w-4 text-muted-foreground" />
            <p className="text-sm font-medium">Schedule</p>
          </div>
          <div className="flex flex-col items-center justify-center py-6 gap-2 text-center">
            <CalendarClock className="h-8 w-8 text-muted-foreground/30" />
            <p className="text-sm text-muted-foreground">EPG schedule coming soon</p>
            <p className="text-xs text-muted-foreground/60">
              Programme guide will appear here once EPG data is configured.
            </p>
          </div>
        </div>

        <div className="shrink-0 pb-2" />
      </div>
    </div>
  );
}
