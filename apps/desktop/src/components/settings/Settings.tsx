import { useState, useEffect, useRef } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Slider } from "@/components/ui/slider";
import { usePlatform } from "@/hooks/usePlatform";
import { Settings as SettingsIcon, Monitor, Smartphone, Tv, Eye, EyeOff, CheckCircle, XCircle } from "lucide-react";
import { getOmdbApiKey, setOmdbApiKey, fetchOmdbData, clearWatchHistory } from "@/lib/tauri";

type OmdbStatus = "idle" | "valid" | "invalid";
type SaveStatus = "idle" | "saved";
type HistoryStatus = "idle" | "cleared";

export function Settings() {
  const { platform, layoutMode } = usePlatform();
  const [hwAccel, setHwAccel] = useState(true);
  const [defaultVolume, setDefaultVolume] = useState(100);

  // OMDB state
  const [omdbKey, setOmdbKey] = useState("");
  const [omdbKeyVisible, setOmdbKeyVisible] = useState(false);
  const [omdbStatus, setOmdbStatus] = useState<OmdbStatus>("idle");
  const [saveStatus, setSaveStatus] = useState<SaveStatus>("idle");
  const [saveError, setSaveError] = useState<string | null>(null);
  const [omdbTesting, setOmdbTesting] = useState(false);
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // History state
  const [historyStatus, setHistoryStatus] = useState<HistoryStatus>("idle");
  const [historyError, setHistoryError] = useState<string | null>(null);
  const historyTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    getOmdbApiKey().then((key) => {
      if (key) setOmdbKey(key);
    });
    return () => {
      if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
      if (historyTimerRef.current) clearTimeout(historyTimerRef.current);
    };
  }, []);

  const platformIcon = {
    desktop: Monitor,
    mobile: Smartphone,
    tv: Tv,
  }[layoutMode];
  const PlatformIcon = platformIcon;

  async function handleSaveOmdbKey() {
    try {
      await setOmdbApiKey(omdbKey);
      setSaveStatus("saved");
      setSaveError(null);
      setOmdbStatus("idle");
      if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
      saveTimerRef.current = setTimeout(() => setSaveStatus("idle"), 2000);
    } catch {
      setSaveError("Failed to save. Please try again.");
    }
  }

  async function handleTestOmdbKey() {
    setOmdbTesting(true);
    setOmdbStatus("idle");
    try {
      const result = await fetchOmdbData("test", "The Matrix", "movie");
      setOmdbStatus(result ? "valid" : "invalid");
    } catch {
      setOmdbStatus("invalid");
    } finally {
      setOmdbTesting(false);
    }
  }

  async function handleClearHistory() {
    if (!window.confirm("Are you sure you want to clear all watch history? This cannot be undone.")) return;
    try {
      await clearWatchHistory();
      setHistoryStatus("cleared");
      setHistoryError(null);
      if (historyTimerRef.current) clearTimeout(historyTimerRef.current);
      historyTimerRef.current = setTimeout(() => setHistoryStatus("idle"), 2000);
    } catch {
      setHistoryError("Failed to clear history. Please try again.");
    }
  }

  return (
    <div className="h-full overflow-y-auto">
      <div className="flex flex-col gap-6 p-4 max-w-2xl mx-auto">
        <div className="flex items-center gap-3">
          <SettingsIcon className="h-6 w-6 text-primary" />
          <h1 className="text-2xl font-bold">Settings</h1>
        </div>

        <Card>
        <CardHeader>
          <CardTitle className="text-base">Platform</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="flex items-center gap-3">
            <PlatformIcon className="h-5 w-5 text-muted-foreground" />
            <div>
              <p className="text-sm font-medium capitalize">{platform}</p>
              <p className="text-xs text-muted-foreground">
                Layout: {layoutMode}
              </p>
            </div>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">Playback</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-sm font-medium">Hardware Acceleration</p>
              <p className="text-xs text-muted-foreground">
                Use GPU decoding when available
              </p>
            </div>
            <Button
              variant={hwAccel ? "default" : "secondary"}
              size="sm"
              onClick={() => setHwAccel(!hwAccel)}
            >
              {hwAccel ? "On" : "Off"}
            </Button>
          </div>

          <div>
            <div className="flex items-center justify-between mb-2">
              <p className="text-sm font-medium">Default Volume</p>
              <span className="text-sm text-muted-foreground">
                {defaultVolume}%
              </span>
            </div>
            <Slider
              value={defaultVolume}
              min={0}
              max={150}
              step={5}
              onValueChange={setDefaultVolume}
            />
          </div>
        </CardContent>
      </Card>

      {/* Integrations section */}
      <Card>
        <CardHeader>
          <CardTitle className="text-base">Integrations</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div>
            <p className="text-sm font-medium mb-1">OMDB API</p>
            <div className="flex items-center gap-2">
              <div className="relative flex-1">
                <Input
                  type={omdbKeyVisible ? "text" : "password"}
                  placeholder="Enter API key…"
                  value={omdbKey}
                  onChange={(e) => {
                    setOmdbKey(e.target.value);
                    setOmdbStatus("idle");
                    setSaveStatus("idle");
                  }}
                  className="pr-10"
                />
                <button
                  type="button"
                  className="absolute inset-y-0 right-2 flex items-center text-muted-foreground hover:text-foreground"
                  onClick={() => setOmdbKeyVisible((v) => !v)}
                  aria-label={omdbKeyVisible ? "Hide key" : "Show key"}
                >
                  {omdbKeyVisible ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                </button>
              </div>
              <Button
                size="sm"
                variant="secondary"
                onClick={handleSaveOmdbKey}
                disabled={!omdbKey.trim()}
              >
                {saveStatus === "saved" ? (
                  <span className="flex items-center gap-1 text-green-500">
                    <CheckCircle className="h-4 w-4" /> Saved
                  </span>
                ) : "Save"}
              </Button>
              <Button
                size="sm"
                variant="outline"
                onClick={handleTestOmdbKey}
                disabled={!omdbKey.trim() || omdbTesting}
              >
                {omdbTesting ? "Testing…" : "Test"}
              </Button>
            </div>

            {/* Save error */}
            {saveError && (
              <p className="mt-1 text-xs text-destructive">{saveError}</p>
            )}

            {/* Status line */}
            <div className="mt-2 text-xs">
              {omdbStatus === "valid" && (
                <span className="flex items-center gap-1 text-green-500">
                  <CheckCircle className="h-3 w-3" /> Valid key · 1000 calls/day limit
                </span>
              )}
              {omdbStatus === "invalid" && (
                <span className="flex items-center gap-1 text-destructive">
                  <XCircle className="h-3 w-3" /> Invalid key
                </span>
              )}
              {omdbStatus === "idle" && !omdbKey && (
                <span className="text-muted-foreground">
                  No API key configured.{" "}
                  <a
                    href="https://www.omdbapi.com/apikey.aspx"
                    target="_blank"
                    rel="noreferrer"
                    className="underline hover:text-foreground"
                  >
                    Get a free key at omdbapi.com
                  </a>
                </span>
              )}
            </div>
          </div>
        </CardContent>
      </Card>

      {/* History section */}
      <Card>
        <CardHeader>
          <CardTitle className="text-base">Watch History</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="space-y-2">
            <div className="flex items-center gap-4">
              <Button
                variant="destructive"
                size="sm"
                onClick={handleClearHistory}
              >
                Clear All History…
              </Button>
              {historyStatus === "cleared" && (
                <span className="flex items-center gap-1 text-xs text-green-500">
                  <CheckCircle className="h-3 w-3" /> History cleared
                </span>
              )}
            </div>
            {historyError && (
              <p className="text-xs text-destructive">{historyError}</p>
            )}
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">About</CardTitle>
        </CardHeader>
        <CardContent>
          <p className="text-sm text-muted-foreground">
            MaxVideoPlayer v0.1.0
          </p>
          <p className="text-xs text-muted-foreground mt-1">
            Built with Tauri v2, React, and libmpv
          </p>
        </CardContent>
      </Card>
      </div>
    </div>
  );
}
