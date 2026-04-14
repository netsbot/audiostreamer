import { invoke } from "@tauri-apps/api/core";
import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import { navigation } from "./navigation.svelte";

interface SearchResults {
  top: any[];
  songs: any[];
  albums: any[];
  artists: any[];
  playlists: any[];
  musicVideos: any[];
  stations: any[];
}

class SearchState {
  query = $state("");
  results = $state<SearchResults>({
    top: [],
    songs: [],
    albums: [],
    artists: [],
    playlists: [],
    musicVideos: [],
    stations: [],
  });
  isSearching = $state(false);
  
  private devToken: string | null = null;
  private userToken: string | null = null;

  async handleSearch() {
    if (!this.query.trim()) {
      this.clearSearch();
      return;
    }

    this.isSearching = true;
    try {
      if (!this.devToken) {
        this.devToken = await invoke("get_apple_music_token");
      }
      if (!this.userToken) {
        this.userToken = await invoke("get_apple_music_user_token");
      }

      const response = await tauriFetch(
        `https://api.music.apple.com/v1/catalog/us/search?term=${encodeURIComponent(this.query)}&types=songs,albums,artists,playlists,music-videos,stations&limit=25&with=topResults`,
        {
          method: "GET",
          headers: {
            Authorization: `Bearer ${this.devToken}`,
            "media-user-token": this.userToken || "",
            "User-Agent": "Mozilla/5.0",
            Origin: "https://music.apple.com",
            Referer: "https://music.apple.com/",
            Accept: "*/*",
          },
        },
      );

      const data = await response.json();
      const res = data.results || {};

      this.results = {
        top: res.topResults?.data || res.top?.data || [],
        songs: res.songs?.data || [],
        albums: res.albums?.data || [],
        artists: res.artists?.data || [],
        playlists: res.playlists?.data || [],
        musicVideos: res["music-videos"]?.data || [],
        stations: res.stations?.data || [],
      };
      
      navigation.activeView = "search";
    } catch (error) {
      console.error("Search failed:", error);
    } finally {
      this.isSearching = false;
    }
  }

  clearSearch() {
    this.query = "";
    this.results = {
      top: [],
      songs: [],
      albums: [],
      artists: [],
      playlists: [],
      musicVideos: [],
      stations: [],
    };
    if (navigation.activeView === 'search') {
      navigation.activeView = "home";
    }
  }
}

export const search = new SearchState();
