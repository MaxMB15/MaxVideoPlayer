import { useState } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Slider } from "@/components/ui/slider";
import { usePlatform } from "@/hooks/usePlatform";
import { Settings as SettingsIcon, Monitor, Smartphone, Tv } from "lucide-react";

export function Settings() {
  const { platform, layoutMode } = usePlatform();
  const [hwAccel, setHwAccel] = useState(true);
  const [defaultVolume, setDefaultVolume] = useState(100);

  const platformIcon = {
    desktop: Monitor,
    mobile: Smartphone,
    tv: Tv,
  }[layoutMode];
  const PlatformIcon = platformIcon;

  return (
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
  );
}
