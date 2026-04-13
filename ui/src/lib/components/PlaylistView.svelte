<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
  import { fade } from "svelte/transition";
  import Play from "lucide-svelte/icons/play";
  import Shuffle from "lucide-svelte/icons/shuffle";
  import Clock from "lucide-svelte/icons/clock";
  import Loader2 from "lucide-svelte/icons/loader-2";
  import { playSong } from "$lib/playbackStore";

  let { playlistId = "", playlistType = "playlists" } = $props();

  let playlistData = $state<any>(null);
  let isLoading = $state(true);

  function getArtworkUrl(artwork: any, size = 1200) {
    if (!artwork || !artwork.url) return "";
    return artwork.url
      .replace("{w}", size.toString())
      .replace("{h}", size.toString())
      .replace("{f}", "webp")
      .replace("{c}", "");
  }

  function formatDuration(ms: number) {
    if (!ms) return "--:--";
    const totalSeconds = Math.floor(ms / 1000);
    const minutes = Math.floor(totalSeconds / 60);
    const seconds = totalSeconds % 60;
    return `${minutes}:${seconds.toString().padStart(2, "0")}`;
  }

  function resolveTrack(track: any) {
    const catalogTrack = track.relationships?.catalog?.data?.[0];
    const attrs = track.attributes || catalogTrack?.attributes || {};
    const id = catalogTrack?.id || track.id;

    return {
      id,
      attrs,
    };
  }

  async function fetchPlaylistDetails() {
    if (!playlistId) return;

    isLoading = true;
    playlistData = null;

    try {
      const devToken = (await invoke("get_apple_music_token")) as string;
      const userToken = (await invoke("get_apple_music_user_token")) as string;

      const url =
        playlistType === "library-playlists"
          ? `https://api.music.apple.com/v1/me/library/playlists/${playlistId}?include=tracks,catalog&limit[tracks]=100`
          : `https://api.music.apple.com/v1/catalog/us/playlists/${playlistId}?include=tracks,curator&limit[tracks]=100`;

      const response = await tauriFetch(url, {
        method: "GET",
        headers: {
          Authorization: `Bearer ${devToken}`,
          "media-user-token": userToken,
          Origin: "https://music.apple.com",
          Referer: "https://music.apple.com/",
        },
      });

      const data = await response.json();
      playlistData = data?.data?.[0] || null;
    } catch (error) {
      console.error("Failed to fetch playlist details:", error);
      playlistData = null;
    } finally {
      isLoading = false;
    }
  }

  async function playTrack(track: any) {
    const resolved = resolveTrack(track);
    if (!resolved.id) return;

    await playSong(resolved.id, {
      title: resolved.attrs.name || "Unknown",
      artist: resolved.attrs.artistName || "Unknown Artist",
      album: resolved.attrs.albumName || playlistData?.attributes?.name || "",
      artwork_url: getArtworkUrl(resolved.attrs.artwork || playlistData?.attributes?.artwork, 600),
      duration_ms: resolved.attrs.durationInMillis,
    });
  }

  function playPlaylist() {
    const firstTrack = playlistData?.relationships?.tracks?.data?.[0];
    if (firstTrack) {
      void playTrack(firstTrack);
    }
  }

  $effect(() => {
    if (playlistId) {
      void fetchPlaylistDetails();
    }
  });
</script>

<div class="container mx-auto px-4 pb-24">
  {#if isLoading}
    <div class="flex flex-col items-center justify-center h-[60vh] gap-4" in:fade>
      <Loader2 class="size-10 text-red-500 animate-spin" />
      <p class="text-zinc-500 font-medium animate-pulse">Loading playlist...</p>
    </div>
  {:else if playlistData}
    <div in:fade={{ duration: 400 }}>
      <div class="flex flex-col md:flex-row gap-10 mb-12 items-end">
        <div class="w-64 h-64 md:w-80 md:h-80 flex-shrink-0 shadow-2xl rounded-2xl overflow-hidden bg-zinc-900 border border-white/5">
          {#if playlistData.attributes?.artwork}
            <img
              src={getArtworkUrl(playlistData.attributes.artwork, 1400)}
              alt={playlistData.attributes?.name}
              class="w-full h-full object-cover"
            />
          {:else}
            <div class="w-full h-full flex items-center justify-center bg-zinc-800">
              <Play class="size-12 text-zinc-500" />
            </div>
          {/if}
        </div>

        <div class="flex-grow flex flex-col items-start gap-4">
          <span class="text-[10px] font-black text-red-500 uppercase tracking-[0.2em]">Playlist</span>
          <h1 class="text-4xl md:text-6xl font-black text-white tracking-tighter leading-tight line-clamp-2">
            {playlistData.attributes?.name}
          </h1>

          <div class="text-[11px] font-bold text-zinc-500 uppercase tracking-widest flex items-center gap-3">
            <span>{playlistData.attributes?.curatorName || "Apple Music"}</span>
            <span class="w-1 h-1 rounded-full bg-zinc-700"></span>
            <span>{playlistData.relationships?.tracks?.data?.length || 0} Songs</span>
          </div>

          <div class="flex items-center gap-4 mt-4">
            <button
              class="flex items-center gap-2 px-8 py-3 bg-red-600 hover:bg-red-500 text-white rounded-full font-black text-sm transition-colors"
              onclick={playPlaylist}
            >
              <Play class="size-4 fill-current" /> Play
            </button>
            <button
              class="flex items-center gap-2 px-8 py-3 bg-white/5 hover:bg-white/10 text-white border border-white/10 rounded-full font-black text-sm transition-colors"
            >
              <Shuffle class="size-4" /> Shuffle
            </button>
          </div>
        </div>
      </div>

      <div class="w-full">
        <div class="grid grid-cols-[1fr_8rem] items-center px-4 py-3 border-b border-white/5 text-[10px] font-black text-zinc-500 uppercase tracking-[0.15em] mb-2">
          <span>Track</span>
          <span class="text-right flex justify-end mr-4"><Clock class="size-3" /></span>
        </div>

        <div class="flex flex-col">
          {#each playlistData.relationships?.tracks?.data || [] as track, i}
            {@const resolved = resolveTrack(track)}
            <div
              class="grid grid-cols-[1fr_8rem] items-center px-4 py-3 rounded-xl hover:bg-white/[0.04] transition-all group cursor-pointer border border-transparent hover:border-white/5"
              onclick={() => playTrack(track)}
            >
              <div class="flex items-center min-w-0">
                <div class="text-zinc-500 font-bold text-sm relative w-8 flex justify-start items-center flex-shrink-0">
                  <span class="group-hover:opacity-0 transition-opacity text-xs">{i + 1}</span>
                  <Play class="size-3 absolute opacity-0 group-hover:opacity-100 transition-opacity text-white fill-current" />
                </div>

                <div class="flex flex-col min-w-0 ml-2">
                  <span class="text-[14px] font-bold text-white/90 truncate group-hover:text-red-500 transition-colors">
                    {resolved.attrs.name || "Unknown"}
                  </span>
                  <span class="text-[11px] text-zinc-400 font-medium truncate mt-0.5">
                    {resolved.attrs.artistName || "Unknown Artist"}
                  </span>
                </div>
              </div>

              <div class="text-right text-xs font-medium text-zinc-500 group-hover:text-zinc-300 transition-colors pr-4">
                {formatDuration(resolved.attrs.durationInMillis)}
              </div>
            </div>
          {/each}
        </div>
      </div>
    </div>
  {:else}
    <div class="text-zinc-500">Playlist not found.</div>
  {/if}
</div>
