<script lang="ts">
  import ArrowRight from "lucide-svelte/icons/arrow-right";
  import Search from "lucide-svelte/icons/search";
  import Play from "lucide-svelte/icons/play";
  import ListMusic from "lucide-svelte/icons/list-music";
  import Video from "lucide-svelte/icons/video";
  import Radio from "lucide-svelte/icons/radio";
  import { invoke } from "@tauri-apps/api/core";
  import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
  import { fade, fly } from "svelte/transition";
  import ArrowLeft from "lucide-svelte/icons/chevron-left";
  import AlbumView from "./AlbumView.svelte";
  import PlaylistView from "./PlaylistView.svelte";
  import SearchHeader from "./SearchHeader.svelte";
  import SearchResults from "./SearchResults.svelte";
  import HomeFeed from "./HomeFeed.svelte";

  let searchQuery = $state("");
  let activeView = $state("home"); // 'home', 'search', 'album', 'playlist'
  let selectedAlbumId = $state("");
  let selectedPlaylistId = $state("");
  let selectedPlaylistType = $state("playlists");

  let searchResults = $state({
    top: [] as any[],
    songs: [] as any[],
    albums: [] as any[],
    artists: [] as any[],
    playlists: [] as any[],
    musicVideos: [] as any[],
    stations: [] as any[],
  });
  let isSearching = $state(false);
  let devToken = $state<string | null>(null);
  let userToken = $state<string | null>(null);


  async function handleSearch() {
    if (!searchQuery.trim()) {
      searchResults = {
        top: [],
        songs: [],
        albums: [],
        artists: [],
        playlists: [],
        musicVideos: [],
        stations: [],
      };
      return;
    }

    isSearching = true;
    try {
      if (!devToken) {
        devToken = await invoke("get_apple_music_token");
      }
      if (!userToken) {
        userToken = await invoke("get_apple_music_user_token");
      }

      const response = await tauriFetch(
        `https://api.music.apple.com/v1/catalog/us/search?term=${encodeURIComponent(searchQuery)}&types=songs,albums,artists,playlists,music-videos,stations&limit=25&with=topResults`,
        {
          method: "GET",
          headers: {
            Authorization: `Bearer ${devToken}`,
            "media-user-token": userToken || "",
            "User-Agent":
              "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36",
            Origin: "https://music.apple.com",
            Referer: "https://music.apple.com/",
            Accept: "*/*",
            "Accept-Language": "en-GB",
          },
        },
      );

      console.log("response", response);

      const data = await response.json();
      const results = data.results || {};
      console.log("Raw API Results:", results);

      searchResults = {
        top: results.topResults?.data || results.top?.data || [],
        songs: results.songs?.data || [],
        albums: results.albums?.data || [],
        artists: results.artists?.data || [],
        playlists: results.playlists?.data || [],
        musicVideos: results["music-videos"]?.data || [],
        stations: results.stations?.data || [],
      };
      activeView = "search";
    } catch (error) {
      console.error("Search failed:", error);
    } finally {
      isSearching = false;
    }
  }

  function clearSearch() {
    searchQuery = "";
    activeView = "home";
    searchResults = {
      top: [],
      songs: [],
      albums: [],
      artists: [],
      playlists: [],
      musicVideos: [],
      stations: [],
    };
  }

  function getArtworkUrl(artwork: any, size = 1200) {
    if (!artwork || !artwork.url) return "";
    return artwork.url
      .replace("{w}", size.toString())
      .replace("{h}", size.toString())
      .replace("{c}", "SC")
      .replace(/SH\./g, "SC.")
      .replace(/SH\?/g, "SC?");
  }

  function openAlbum(id: string) {
    selectedAlbumId = id;
    activeView = "album";
  }

  function openPlaylist(id: string, type = "playlists") {
    selectedPlaylistId = id;
    selectedPlaylistType = type;
    activeView = "playlist";
  }
</script>

<main
  class="ml-64 mr-[400px] flex-1 overflow-y-auto no-scrollbar p-12 bg-zinc-950 pb-32"
>
  <SearchHeader 
    bind:searchQuery={searchQuery} 
    isSearching={isSearching} 
    handleSearch={handleSearch} 
  />

  {#if activeView === "search"}
    <SearchResults 
      searchResults={searchResults} 
      openAlbum={openAlbum} 
      openPlaylist={openPlaylist}
      clearSearch={clearSearch} 
      getArtworkUrl={getArtworkUrl}
    />
  {/if}

  {#if activeView === "album"}
    <div in:fade={{ duration: 300 }}>
      <button 
        class="mb-8 flex items-center gap-2 text-zinc-400 hover:text-white transition-colors font-bold text-sm"
        onclick={() => activeView = (searchQuery ? 'search' : 'home')}
      >
        <ArrowLeft class="size-4" /> Back
      </button>
      <AlbumView albumId={selectedAlbumId} />
    </div>
  {/if}

  {#if activeView === "playlist"}
    <div in:fade={{ duration: 300 }}>
      <button
        class="mb-8 flex items-center gap-2 text-zinc-400 hover:text-white transition-colors font-bold text-sm"
        onclick={() => activeView = (searchQuery ? 'search' : 'home')}
      >
        <ArrowLeft class="size-4" /> Back
      </button>
      <PlaylistView playlistId={selectedPlaylistId} playlistType={selectedPlaylistType} />
    </div>
  {/if}

  {#if activeView === "home"}
    <HomeFeed {openAlbum} {openPlaylist} />
  {/if}
</main>
