import {
  createContext,
  useContext,
  useState,
  useCallback,
  useEffect,
} from "react";
import type { Channel, Category, Provider } from "@/lib/types";
import {
  loadM3uPlaylist,
  loadM3uFile as loadM3uFileApi,
  loadXtreamProvider,
  getProviders,
  removeProvider as removeProviderApi,
  getAllChannels,
} from "@/lib/tauri";

interface ChannelsContextValue {
  channels: Channel[];
  categories: Category[];
  providers: Provider[];
  loading: boolean;
  error: string | null;
  loadM3u: (name: string, url: string) => Promise<void>;
  loadM3uFile: (name: string, path: string) => Promise<void>;
  loadXtream: (
    name: string,
    url: string,
    username: string,
    password: string
  ) => Promise<void>;
  refreshProviders: () => Promise<void>;
  refreshChannels: () => Promise<void>;
  removeProvider: (id: string) => Promise<void>;
}

export const ChannelsContext = createContext<ChannelsContextValue | null>(null);

export function useChannelsProvider(): ChannelsContextValue {
  const [channels, setChannels] = useState<Channel[]>([]);
  const [categories, setCategories] = useState<Category[]>([]);
  const [providers, setProviders] = useState<Provider[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  function deriveCategories(chs: Channel[]) {
    const map = new Map<string, number>();
    for (const ch of chs) {
      map.set(ch.groupTitle, (map.get(ch.groupTitle) ?? 0) + 1);
    }
    const cats: Category[] = Array.from(map.entries()).map(
      ([name, count], i) => ({
        id: `cat-${i}`,
        name,
        channelCount: count,
      })
    );
    setCategories(cats);
  }

  const refreshChannels = useCallback(async () => {
    try {
      const chs = await getAllChannels();
      setChannels(chs);
      deriveCategories(chs);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const refreshProviders = useCallback(async () => {
    try {
      const p = await getProviders();
      setProviders(p);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const loadM3u = useCallback(
    async (name: string, url: string) => {
      setLoading(true);
      setError(null);
      try {
        await loadM3uPlaylist(name, url);
        await refreshProviders();
        await refreshChannels();
      } catch (e) {
        setError(String(e));
        throw e;
      } finally {
        setLoading(false);
      }
    },
    [refreshProviders, refreshChannels]
  );

  const loadM3uFile = useCallback(
    async (name: string, path: string) => {
      setLoading(true);
      setError(null);
      try {
        await loadM3uFileApi(name, path);
        await refreshProviders();
        await refreshChannels();
      } catch (e) {
        setError(String(e));
        throw e;
      } finally {
        setLoading(false);
      }
    },
    [refreshProviders, refreshChannels]
  );

  const loadXtream = useCallback(
    async (name: string, url: string, username: string, password: string) => {
      setLoading(true);
      setError(null);
      try {
        await loadXtreamProvider(name, url, username, password);
        await refreshProviders();
        await refreshChannels();
      } catch (e) {
        setError(String(e));
        throw e;
      } finally {
        setLoading(false);
      }
    },
    [refreshProviders, refreshChannels]
  );

  const removeProvider = useCallback(
    async (id: string) => {
      try {
        await removeProviderApi(id);
        await refreshProviders();
        await refreshChannels();
      } catch (e) {
        setError(String(e));
      }
    },
    [refreshProviders, refreshChannels]
  );

  useEffect(() => {
    refreshProviders();
    refreshChannels();
  }, [refreshProviders, refreshChannels]);

  return {
    channels,
    categories,
    providers,
    loading,
    error,
    loadM3u,
    loadM3uFile,
    loadXtream,
    refreshProviders,
    refreshChannels,
    removeProvider,
  };
}

export function useChannels(): ChannelsContextValue {
  const ctx = useContext(ChannelsContext);
  if (!ctx) {
    throw new Error("useChannels must be used within a ChannelsProvider");
  }
  return ctx;
}
