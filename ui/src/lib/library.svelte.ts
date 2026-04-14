import { invoke } from "@tauri-apps/api/core";
import { fetch as tauriFetch } from "@tauri-apps/plugin-http";

export interface LibraryPlaylist {
  id: string;
  name: string;
  artworkUrl: string | null;
  trackCount: number | null;
  curatorName: string | null;
}

class LibraryState {
  playlists = $state<LibraryPlaylist[]>([]);
  isLoading = $state(false);
  error = $state<string | null>(null);
  loaded = $state(false);

  async fetchPlaylists(force = false) {
    if (this.loaded && !force) return;
    if (this.isLoading) return;

    this.isLoading = true;
    this.error = null;

    try {
      const devToken = (await invoke("get_apple_music_token")) as string;
      const userToken = (await invoke("get_apple_music_user_token")) as string;

      let allPlaylists: any[] = [];
      let nextHref: string | null = "/v1/me/library/playlists?limit=100&include=catalog";
      const apiBase = "https://api.music.apple.com";

      while (nextHref) {
        const url = nextHref.startsWith("http") ? nextHref : `${apiBase}${nextHref}`;
        
        const response = await tauriFetch(url, {
          method: "GET",
          headers: {
            Authorization: `Bearer ${devToken}`,
            "media-user-token": userToken,
            Origin: "https://music.apple.com",
            Referer: "https://music.apple.com/",
          },
        });

        if (!response.ok) {
          throw new Error(`Failed to fetch playlists: ${response.statusText}`);
        }

        const data = await response.json();
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
  }

  favoritedIds = $state<Set<string>>(new Set());

  async toggleFavorite(id: string, type: 'songs' | 'albums' | 'playlists' = 'songs') {
    const isFav = this.favoritedIds.has(id);
    const method = isFav ? "DELETE" : "POST";

    // Optimistic UI
    if (!isFav) this.favoritedIds.add(id);
    else this.favoritedIds.delete(id);
    this.favoritedIds = new Set(this.favoritedIds);

    try {
      const devToken = await invoke("get_apple_music_token") as string;
      const userToken = await invoke("get_apple_music_user_token") as string;
      
      const baseUrl = "https://amp-api.music.apple.com/v1/me/favorites";
      const params = new URLSearchParams({
        "art[url]": "f",
        [`ids[${type}]`]: id,
        "l": "en-GB",
        "platform": "web"
      });

      const response = await tauriFetch(`${baseUrl}?${params.toString()}`, {
        method,
        headers: {
          Authorization: `Bearer ${devToken}`,
          "media-user-token": userToken,
          Origin: "https://music.apple.com",
          Referer: "https://music.apple.com/",
        }
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
    }
  }

  isFavorite(id: string): boolean {
    return this.favoritedIds.has(id);
  }

  async fetchFavorites() {
    try {
      const devToken = await invoke("get_apple_music_token") as string;
      const userToken = await invoke("get_apple_music_user_token") as string;
      
      // Fetch favorites from the library songs — Apple Music usually syncs 'Favorite' 
      // status into the library song attributes in recent API versions.
      // If NOT, we fallback to our local state or investigative amp-api GET.
      let nextHref: string | null = "/v1/me/library/songs?limit=100&extend=isFavorite";
      const apiBase = "https://api.music.apple.com";
      const newFavs = new Set<string>();

      while (nextHref) {
        const url = nextHref.startsWith("http") ? nextHref : `${apiBase}${nextHref}`;
        const response = await tauriFetch(url, {
          method: "GET",
          headers: {
            Authorization: `Bearer ${devToken}`,
            "media-user-token": userToken,
            Origin: "https://music.apple.com",
            Referer: "https://music.apple.com/",
          },
        });

        if (!response.ok) break;

        const data = await response.json();
        const songs = data.data || [];
        for (const s of songs) {
          // Check for 'isFavorite' attribute (standard for new system)
          // or fallback to checkingcatalog ID if it's a library resource
          if (s.attributes?.isFavorite) {
            newFavs.add(s.id); // Library ID
            // Also add catalog ID if related
            const catalogId = s.relationships?.catalog?.data?.[0]?.id;
            if (catalogId) newFavs.add(catalogId);
          }
        }
        nextHref = data.next ?? null;
      }
      this.favoritedIds = newFavs;
    } catch (e) {
      console.error("Fetch favorites failed:", e);
    }
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
