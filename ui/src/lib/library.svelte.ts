import { fetchAppleMusic, fetchAppleMusicJson } from "$lib/appleMusic";

export interface LibraryPlaylist {
  id: string;
  name: string;
  artworkUrl: string | null;
  trackCount: number | null;
  curatorName: string | null;
}

class LibraryState {
  private readonly favoritesCacheKey = "audiostreamer:favorites-cache:v1";
  private readonly favoritesCacheTtlMs = 5 * 60 * 1000;
  private favoritesLoading = false;
  private playlistsInFlight: Promise<void> | null = null;
  private favoritesInFlight: Promise<void> | null = null;

  playlists = $state<LibraryPlaylist[]>([]);
  isLoading = $state(false);
  error = $state<string | null>(null);
  loaded = $state(false);
  favoritesLastFetchedAt = $state(0);

  async fetchPlaylists(force = false) {
    if (this.playlistsInFlight) return this.playlistsInFlight;

    const run = async () => {
    if (this.loaded && !force) return;
    if (this.isLoading) return;

    this.isLoading = true;
    this.error = null;

    try {
      let allPlaylists: any[] = [];
      let nextHref: string | null = "/v1/me/library/playlists?limit=100&include=catalog";
      const apiBase = "https://api.music.apple.com";

      while (nextHref) {
        const url = nextHref.startsWith("http") ? nextHref : `${apiBase}${nextHref}`;
        const cacheKey = `library-playlists-${url}`;
        const data = await fetchAppleMusicJson(url, cacheKey, 1800, { method: "GET" });

        const batch = data.data || [];
        allPlaylists = [...allPlaylists, ...batch];
        
        // Follow cursor as in musickit.js pattern
        nextHref = data.next ?? null;
      }

      this.playlists = allPlaylists.map((item: any) => ({
        id: item.id,
        name: item.attributes?.name || "Untitled Playlist",
        artworkUrl: this.formatArtworkUrl(item.attributes?.artwork),
        trackCount: item.relationships?.tracks?.data?.length || 0,
        curatorName: item.attributes?.curatorName || null
      }));

      this.loaded = true;
    } catch (e) {
      console.error("Library fetch failed:", e);
      this.error = e instanceof Error ? e.message : String(e);
    } finally {
      this.isLoading = false;
    }
    };

    this.playlistsInFlight = run().finally(() => {
      this.playlistsInFlight = null;
    });
    return this.playlistsInFlight;
  }

  favoritedIds = $state<Set<string>>(new Set());

  private loadFavoritesCache(): { ids: string[]; fetchedAt: number } | null {
    if (typeof localStorage === "undefined") return null;
    try {
      const raw = localStorage.getItem(this.favoritesCacheKey);
      if (!raw) return null;
      const parsed = JSON.parse(raw) as { ids?: unknown; fetchedAt?: unknown };
      if (!Array.isArray(parsed.ids)) return null;
      if (typeof parsed.fetchedAt !== "number") return null;
      const ids = parsed.ids.filter((v): v is string => typeof v === "string");
      return { ids, fetchedAt: parsed.fetchedAt };
    } catch {
      return null;
    }
  }

  private saveFavoritesCache(ids: Set<string>, fetchedAt = Date.now()) {
    if (typeof localStorage === "undefined") return;
    try {
      localStorage.setItem(this.favoritesCacheKey, JSON.stringify({
        ids: [...ids],
        fetchedAt,
      }));
    } catch {
      // Ignore cache write failures.
    }
  }

  async toggleFavorite(id: string, type: 'songs' | 'albums' | 'playlists' = 'songs') {
    const isFav = this.favoritedIds.has(id);
    const method = isFav ? "DELETE" : "POST";

    // Optimistic UI
    if (!isFav) this.favoritedIds.add(id);
    else this.favoritedIds.delete(id);
    this.favoritedIds = new Set(this.favoritedIds);
    this.favoritesLastFetchedAt = Date.now();
    this.saveFavoritesCache(this.favoritedIds, this.favoritesLastFetchedAt);

    try {
      const baseUrl = "https://amp-api.music.apple.com/v1/me/favorites";
      const params = new URLSearchParams({
        "art[url]": "f",
        [`ids[${type}]`]: id,
        "l": "en-GB",
        "platform": "web"
      });

      const response = await fetchAppleMusic(`${baseUrl}?${params.toString()}`, {
        method,
      });

      if (!response.ok) {
        throw new Error(`API failed: ${response.status}`);
      }
    } catch (e) {
      console.error("Toggle favorite failed:", e);
      // Rollback
      if (!isFav) this.favoritedIds.delete(id);
      else this.favoritedIds.add(id);
      this.favoritedIds = new Set(this.favoritedIds);
      this.favoritesLastFetchedAt = Date.now();
      this.saveFavoritesCache(this.favoritedIds, this.favoritesLastFetchedAt);
    }
  }

  isFavorite(id: string): boolean {
    return this.favoritedIds.has(id);
  }

  private isFavoriteSongsPlaylist(name: string | undefined): boolean {
    if (!name) return false;
    const normalized = name.trim().toLowerCase();
    return normalized === "favorite songs"
      || normalized === "favourite songs"
      || normalized === "favorites"
      || normalized === "favourites";
  }

  async fetchFavorites(force = false) {
    if (this.favoritesInFlight) return this.favoritesInFlight;

    const run = async () => {
    if (this.favoritesLoading) return;

    const now = Date.now();
    if (!force) {
      if (this.favoritesLastFetchedAt > 0 && (now - this.favoritesLastFetchedAt) < this.favoritesCacheTtlMs) {
        return;
      }

      const cached = this.loadFavoritesCache();
      if (cached) {
        this.favoritedIds = new Set(cached.ids);
        this.favoritesLastFetchedAt = cached.fetchedAt;
        if ((now - cached.fetchedAt) < this.favoritesCacheTtlMs) {
          return;
        }
      }
    }

    this.favoritesLoading = true;
    try {
      const newFavs = new Set<string>();

      // Reuse playlist dataset already needed by sidebar. Avoid duplicate startup requests.
      await this.fetchPlaylists();
      const favoritePlaylist = this.playlists.find((p) => this.isFavoriteSongsPlaylist(p.name));
      const favoritePlaylistId = favoritePlaylist?.id ?? null;

      if (!favoritePlaylistId) {
        this.favoritedIds = newFavs;
        this.favoritesLastFetchedAt = Date.now();
        this.saveFavoritesCache(newFavs, this.favoritesLastFetchedAt);
        return;
      }

      // Load songs from Favorite Songs playlist and fill set.
      const apiBase = "https://api.music.apple.com";
      let nextHref: string | null = `/v1/me/library/playlists/${favoritePlaylistId}/tracks?include=catalog&limit=100`;

      while (nextHref) {
        const url = nextHref.startsWith("http") ? nextHref : `${apiBase}${nextHref}`;
        const cacheKey = `favorite-tracks-${url}`;
        const data = await fetchAppleMusicJson(url, cacheKey, 300, { method: "GET" });

        const songs = data.data || [];
        const resourceLibrarySongs = data.resources?.["library-songs"] || {};
        for (const s of songs) {
          // Keep both library ids and catalog ids for UI checks from mixed sources.
          if (s.id) newFavs.add(s.id);
          const resourceSong = s.id ? resourceLibrarySongs[s.id] : undefined;
          const catalogId = s.relationships?.catalog?.data?.[0]?.id
            ?? s.attributes?.playParams?.catalogId
            ?? resourceSong?.relationships?.catalog?.data?.[0]?.id
            ?? resourceSong?.attributes?.playParams?.catalogId;
          if (catalogId) newFavs.add(catalogId);
        }
        nextHref = data.next ?? null;
      }
      this.favoritedIds = newFavs;
      this.favoritesLastFetchedAt = Date.now();
      this.saveFavoritesCache(newFavs, this.favoritesLastFetchedAt);
    } catch (e) {
      console.error("Fetch favorites failed:", e);
    } finally {
      this.favoritesLoading = false;
    }
    };

    this.favoritesInFlight = run().finally(() => {
      this.favoritesInFlight = null;
    });
    return this.favoritesInFlight;
  }

  private formatArtworkUrl(artwork: any, size = 300): string | null {
    if (!artwork || !artwork.url) return null;
    return artwork.url
      .replace("{w}", size.toString())
      .replace("{h}", size.toString())
      .replace("{f}", "webp")
      .replace("{c}", "");
  }
}

export const library = new LibraryState();
