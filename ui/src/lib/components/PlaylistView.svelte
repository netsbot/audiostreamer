<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
  import { fade } from "svelte/transition";
  import Play from "lucide-svelte/icons/play";
  import Shuffle from "lucide-svelte/icons/shuffle";
  import Loader2 from "lucide-svelte/icons/loader-2";
  import { playback, type QueueTrack } from "$lib/playback.svelte";
    function buildPlaylistQueue(): QueueTrack[] {
      const tracks = playlistData?.relationships?.tracks?.data || [];
      return tracks
        .map((track: any) => {
          const resolved = resolveTrack(track);
          if (!resolved.id) {
            return null;
          }
          return {
            id: resolved.id,
            metadata: {
              title: resolved.attrs.name || "Unknown",
              artist: resolved.attrs.artistName || "Unknown Artist",
              album: resolved.attrs.albumName || playlistData?.attributes?.name || "",
              artwork_url: getArtworkUrl(resolved.attrs.artwork || playlistData?.attributes?.artwork, 600),
              duration_ms: resolved.attrs.durationInMillis,
            },
          } as QueueTrack;
        })
        .filter((entry: QueueTrack | null): entry is QueueTrack => Boolean(entry));
    }

  import TrackList from "./TrackList.svelte";

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

  async function playTrack(track: any, index?: number) {
    const resolved = resolveTrack(track);
    if (!resolved.id) return;

    const queue = buildPlaylistQueue();
    const startIndex = typeof index === "number"
      ? index
      : queue.findIndex((entry) => entry.id === resolved.id);

    await playback.playSong(resolved.id, {
      title: resolved.attrs.name || "Unknown",
      artist: resolved.attrs.artistName || "Unknown Artist",
      album: resolved.attrs.albumName || playlistData?.attributes?.name || "",
      artwork_url: getArtworkUrl(resolved.attrs.artwork || playlistData?.attributes?.artwork, 600),
      duration_ms: resolved.attrs.durationInMillis,
    }, {
      queue,
      startIndex,
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

  function getSquareEditorialVideo(attrs: any): string | null {
    const editorial = attrs?.editorialVideo;
    return editorial?.motionSquareVideo1x1?.video
      || editorial?.motionDetailSquare?.video
      || null;
  }
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
            {#if getSquareEditorialVideo(playlistData.attributes)}
              <video
                src={getSquareEditorialVideo(playlistData.attributes)}
                class="w-full h-full object-cover"
                autoplay
                muted
                loop
                playsinline
              ></video>
            {:else}
              <img
                src={getArtworkUrl(playlistData.attributes.artwork, 1400)}
                alt={playlistData.attributes?.name}
                class="w-full h-full object-cover"
              />
            {/if}
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

      <TrackList
        tracks={playlistData.relationships?.tracks?.data || []}
        resolveTrack={resolveTrack}
        onPlay={playTrack}
        getArtworkUrl={getArtworkUrl}
        fallbackArtwork={playlistData.attributes?.artwork}
      />
    </div>
  {:else}
    <div class="text-zinc-500">Playlist not found.</div>
  {/if}
</div>
