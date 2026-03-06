import { Trash2, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { AddProvider } from "./AddProvider";
import { useChannels } from "@/hooks/useChannels";

export function PlaylistManager() {
  const {
    providers,
    loadM3u,
    loadM3uFile,
    loadXtream,
    removeProvider,
    refreshChannels,
  } = useChannels();

  const handleAddM3u = async (name: string, url: string) => {
    await loadM3u(name, url);
  };

  const handleAddM3uFile = async (name: string, path: string) => {
    await loadM3uFile(name, path);
  };

  const handleAddXtream = async (
    name: string,
    url: string,
    username: string,
    password: string
  ) => {
    await loadXtream(name, url, username, password);
  };

  return (
    <div className="flex flex-col gap-6 p-4 max-w-2xl mx-auto">
      <h1 className="text-2xl font-bold">Playlists</h1>

      <AddProvider
        onAddM3u={handleAddM3u}
        onAddM3uFile={handleAddM3uFile}
        onAddXtream={handleAddXtream}
      />

      {providers.length > 0 && (
        <div className="flex flex-col gap-3">
          <h2 className="text-lg font-semibold">Your Providers</h2>
          {providers.map((p) => (
            <Card key={p.id}>
              <CardHeader className="py-4">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <CardTitle className="text-base">{p.name}</CardTitle>
                    <Badge variant="secondary" className="text-[10px]">
                      {p.type}
                    </Badge>
                  </div>
                  <div className="flex items-center gap-1">
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-8 w-8"
                      onClick={() => refreshChannels()}
                    >
                      <RefreshCw className="h-3.5 w-3.5" />
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-8 w-8 text-destructive"
                      onClick={() => removeProvider(p.id)}
                    >
                      <Trash2 className="h-3.5 w-3.5" />
                    </Button>
                  </div>
                </div>
              </CardHeader>
              <CardContent className="pt-0 pb-4">
                <p className="text-xs text-muted-foreground truncate">
                  {p.url}
                </p>
                <p className="text-xs text-muted-foreground mt-1">
                  {p.channelCount} channels
                  {p.lastUpdated && ` · Updated ${p.lastUpdated}`}
                </p>
              </CardContent>
            </Card>
          ))}
        </div>
      )}
    </div>
  );
}
