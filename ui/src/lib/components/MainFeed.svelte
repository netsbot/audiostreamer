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
  import SearchResults from "./SearchResults.svelte";
  import HomeFeed from "./HomeFeed.svelte";
  import LibraryView from "./LibraryView.svelte";
  import { navigation } from "$lib/navigation.svelte";
  import { search } from "$lib/search.svelte";

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
    navigation.openAlbum(id);
  }

  function openPlaylist(id: string, type = "playlists") {
    navigation.openPlaylist(id, type);
  }
</script>

<main
  class="min-w-0 flex-1 overflow-y-auto no-scrollbar p-12 bg-zinc-950 pb-32"
>
  {#if navigation.activeView === "search"}
    <SearchResults 
      searchResults={search.results} 
      openAlbum={openAlbum} 
      openPlaylist={openPlaylist}
      clearSearch={() => search.clearSearch()} 
      getArtworkUrl={getArtworkUrl}
    />
  {/if}

  {#if navigation.activeView === "album"}
    <div in:fade={{ duration: 300 }}>
      <button 
        class="mb-8 flex items-center gap-2 text-zinc-400 hover:text-white transition-colors font-bold text-sm"
        onclick={() => navigation.activeView = (searchQuery ? 'search' : 'home')}
      >
        <ArrowLeft class="size-4" /> Back
      </button>
      <AlbumView albumId={navigation.selectedAlbumId} />
    </div>
  {/if}

  {#if navigation.activeView === "playlist"}
    <div in:fade={{ duration: 300 }}>
      <button
        class="mb-8 flex items-center gap-2 text-zinc-400 hover:text-white transition-colors font-bold text-sm"
        onclick={() => navigation.activeView = (searchQuery ? 'search' : 'home')}
      >
        <ArrowLeft class="size-4" /> Back
      </button>
      <PlaylistView playlistId={navigation.selectedPlaylistId} playlistType={navigation.selectedPlaylistType} />
    </div>
  {/if}

  {#if navigation.activeView === "home"}
    <HomeFeed {openAlbum} {openPlaylist} />
  {/if}

  {#if navigation.activeView === "library"}
    <LibraryView />
  {/if}
</main>
