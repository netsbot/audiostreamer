import { fetchAppleMusic } from "$lib/appleMusic";
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

  async handleSearch() {
    if (!this.query.trim()) {
      this.clearSearch();
      return;
    }

    this.isSearching = true;
    try {
      const response = await fetchAppleMusic(
        `https://api.music.apple.com/v1/catalog/vn/search?term=${encodeURIComponent(this.query)}&types=songs,albums,artists,playlists,music-videos,stations&limit=25&with=topResults`,
        {
          method: "GET",
          headers: {
            "User-Agent": "Mozilla/5.0",
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
