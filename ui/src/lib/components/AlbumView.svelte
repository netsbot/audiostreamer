<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
  import { fade, fly } from "svelte/transition";
  import Play from "lucide-svelte/icons/play";
  import Shuffle from "lucide-svelte/icons/shuffle";
  import Loader2 from "lucide-svelte/icons/loader-2";
  import TrackList from "./TrackList.svelte";

  let { albumId = "", albumType = "albums" } = $props();

  let albumData = $state<any>(null);
  let isLoading = $state(true);
  let isNotesExpanded = $state(false);

  function resolveTrack(track: any) {
    const catalogTrack = track.relationships?.catalog?.data?.[0];
    const attrs = track.attributes || catalogTrack?.attributes || {};
    const id = catalogTrack?.id || track.id;

    return {
      id,
      attrs,
    };
  }

  async function fetchAlbumDetails() {
    if (!albumId) return;

    isLoading = true;
    albumData = null;

    try {
      const devToken = (await invoke("get_apple_music_token")) as string;
      const userToken = (await invoke("get_apple_music_user_token")) as string;

      const url =
        albumType === "library-albums"
          ? `https://api.music.apple.com/v1/me/library/albums/${albumId}?include=tracks,artists`
          : `https://api.music.apple.com/v1/catalog/us/albums/${albumId}?include=tracks,artists&views=related-albums`;

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
      albumData = data?.data?.[0] || null;
    } catch (error) {
      console.error("Failed to fetch album details:", error);
      albumData = null;
    } finally {
      isLoading = false;
    }
  }

  function getArtworkUrl(artwork: any, size = 1400) {
    if (!artwork || !artwork.url) return "";
    return artwork.url
      .replace("{w}", size.toString())
      .replace("{h}", size.toString())
      .replace("{c}", "SC")
      .replace(/SH\./g, "SC.")
      .replace(/SH\?/g, "SC?");
  }

  $effect(() => {
    if (albumId) {
      void fetchAlbumDetails();
    }
  });

  import { playback, type QueueTrack } from "$lib/playback.svelte";

  function buildAlbumQueue(): QueueTrack[] {
    const tracks = albumData?.relationships?.tracks?.data || [];
    return tracks
      .map((track: any) => {
        const resolved = resolveTrack(track);
        if (!resolved.id) return null;
        return {
          id: resolved.id,
          metadata: {
            title: resolved.attrs.name || "Unknown",
            artist: resolved.attrs.artistName || "Unknown Artist",
            album: resolved.attrs.albumName || albumData?.attributes?.name || "",
            artwork_url: getArtworkUrl(resolved.attrs.artwork || albumData?.attributes?.artwork, 600),
            duration_ms: resolved.attrs.durationInMillis,
          },
        } as QueueTrack;
      })
      .filter((t: QueueTrack | null): t is QueueTrack => !!t);
  }

  async function playTrack(track: any, index?: number) {
    const resolved = resolveTrack(track);
    if (!resolved.id) return;

    const queue = buildAlbumQueue();
    const startIndex = typeof index === "number"
      ? index
      : queue.findIndex((entry) => entry.id === resolved.id);

    await playback.playSong(resolved.id, {
      title: resolved.attrs.name || "Unknown",
      artist: resolved.attrs.artistName || "Unknown Artist",
      album: resolved.attrs.albumName || albumData?.attributes?.name || "",
      artwork_url: getArtworkUrl(resolved.attrs.artwork || albumData?.attributes?.artwork, 600),
      duration_ms: resolved.attrs.durationInMillis,
    }, {
      queue,
      startIndex,
    });
  }

  function playAlbum() {
    const firstTrack = albumData?.relationships?.tracks?.data?.[0];
    if (firstTrack) {
      void playTrack(firstTrack);
    }
  }

  function getSquareEditorialVideo(attrs: any): string | null {
    const editorial = attrs?.editorialVideo;
    return (
      editorial?.motionSquareVideo1x1?.video ||
      editorial?.motionDetailSquare?.video ||
      null
    );
  }
</script>

<div class="container mx-auto px-4 pb-24">
  {#if isLoading}
    <div class="flex flex-col items-center justify-center h-[60vh] gap-4" in:fade>
      <Loader2 class="size-10 text-red-500 animate-spin" />
      <p class="text-zinc-500 font-medium animate-pulse">Gathering tracks...</p>
    </div>
  {:else if albumData}
    <div in:fade={{ duration: 500 }}>
      <!-- Hero Section -->
      <div class="flex flex-col md:flex-row gap-10 mb-12 items-end">
        <div class="w-64 h-64 md:w-80 md:h-80 flex-shrink-0 shadow-2xl rounded-2xl overflow-hidden group relative bg-zinc-900 border border-white/5">
          {#if getSquareEditorialVideo(albumData.attributes)}
            <video
              src={getSquareEditorialVideo(albumData.attributes)}
              class="w-full h-full object-cover"
              autoplay
              muted
              loop
              playsinline
            ></video>
          {:else}
            <img
              src={getArtworkUrl(albumData.attributes.artwork)}
              alt={albumData.attributes.name}
              class="w-full h-full object-cover"
            />
          {/if}
          <div class="absolute inset-0 bg-black/20 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center">
             <div class="bg-white/10 backdrop-blur-md p-6 rounded-full blur-none">
                <Play class="size-10 text-white fill-current translate-x-1" />
             </div>
          </div>
        </div>

        <div class="flex-grow flex flex-col items-start gap-4">
          <div class="flex flex-col gap-1">
            <span class="text-[10px] font-black text-red-500 uppercase tracking-[0.2em]">Album</span>
            <h1 class="text-4xl md:text-6xl font-black text-white tracking-tighter leading-tight line-clamp-2">
              {albumData.attributes.name}
            </h1>
          </div>

          <div class="flex flex-col gap-2">
            <div class="flex items-center gap-2">
              <span class="text-xl font-bold text-red-500 hover:underline cursor-pointer">
                {albumData.attributes.artistName}
              </span>
              <span class="text-zinc-600">•</span>
              <span class="text-zinc-400 font-bold">{new Date(albumData.attributes.releaseDate).getFullYear()}</span>
            </div>
            
            <div class="text-[11px] font-bold text-zinc-500 uppercase tracking-widest flex items-center gap-3">
              <span>{albumData.attributes.isSingle ? 'Single' : 'Album'}</span>
              <span class="w-1 h-1 rounded-full bg-zinc-700"></span>
              <span>{albumData.attributes.genreNames?.[0] || 'Music'}</span>
              <span class="w-1 h-1 rounded-full bg-zinc-700"></span>
              <span>{albumData.attributes.trackCount} Songs</span>
              <span class="w-1 h-1 rounded-full bg-zinc-700"></span>
              <span>Apple Music</span>
            </div>
          </div>

          <div class="flex items-center gap-4 mt-4">
            <button 
              class="flex items-center gap-2 px-8 py-3 bg-red-600 hover:bg-red-500 text-white rounded-full font-black text-sm transition-all hover:scale-105 active:scale-95 shadow-xl shadow-red-600/20"
              onclick={playAlbum}
            >
              <Play class="size-4 fill-current" /> Play
            </button>
            <button 
              class="flex items-center gap-2 px-8 py-3 bg-white/5 hover:bg-white/10 text-white border border-white/10 rounded-full font-black text-sm transition-all hover:scale-105 active:scale-95"
            >
              <Shuffle class="size-4" /> Shuffle
            </button>
          </div>
        </div>
      </div>

      <!-- Editorial Notes -->
      {#if albumData.attributes.editorialNotes?.standard}
        <div class="mb-12 max-w-3xl">
          <h3 class="text-xs font-black text-white/40 uppercase tracking-[0.2em] mb-4">Editorial Notes</h3>
          <div 
            class="text-zinc-400 text-sm leading-relaxed space-y-4 editorial-content {isNotesExpanded ? '' : 'line-clamp-3'}"
          >
            {@html albumData.attributes.editorialNotes.standard}
          </div>
          <button 
            class="mt-4 text-xs font-bold text-red-500 hover:text-red-400 transition-colors uppercase tracking-widest"
            onclick={() => isNotesExpanded = !isNotesExpanded}
          >
            {isNotesExpanded ? 'Read Less' : 'Read More'}
          </button>
        </div>
      {/if}

      <TrackList
        tracks={albumData.relationships.tracks.data}
        resolveTrack={resolveTrack}
        onPlay={playTrack}
        getArtworkUrl={getArtworkUrl}
        fallbackArtwork={albumData.attributes.artwork}
      />

      <!-- Related Albums Section -->
      {#if albumData.views?.['related-albums']?.data?.length > 0}
        <section class="mt-20">
          <h3 class="text-xl font-bold mb-6 text-white/90">Related Albums</h3>
          <div class="flex gap-6 overflow-x-auto no-scrollbar pb-4 snap-x snap-mandatory">
            {#each albumData.views['related-albums'].data as related}
              <button 
                class="shrink-0 w-44 lg:w-[calc((100%-6rem)/5)] text-left group transition-all snap-start"
                onclick={() => {
                  albumId = related.id;
                  window.scrollTo({ top: 0, behavior: 'smooth' });
                }}
              >
                <div class="aspect-square rounded-xl overflow-hidden mb-3 relative border border-white/5 bg-zinc-900 shadow-xl group-hover:border-white/20 transition-all">
                  <img 
                    src={getArtworkUrl(related.attributes.artwork, 400)} 
                    alt={related.attributes.name}
                    class="w-full h-full object-cover"
                  />
                </div>
                <h5 class="font-bold text-white text-[13px] truncate group-hover:text-red-500 transition-colors">
                  {related.attributes.name}
                </h5>
                <p class="text-zinc-500 text-[11px] truncate">
                  {related.attributes.artistName}
                </p>
              </button>
            {/each}
          </div>
        </section>
      {/if}

      <!-- Footer Info -->
      <div class="mt-12 pt-8 border-t border-white/5 text-xs text-zinc-500 font-medium pb-20">
        <p class="mb-2 text-zinc-400 font-bold uppercase tracking-widest text-[10px]">Released by {albumData.attributes.recordLabel}</p>
        <p>{new Date(albumData.attributes.releaseDate).toLocaleDateString(undefined, { year: 'numeric', month: 'long', day: 'numeric' })}</p>
        <p class="mt-1 opacity-50 uppercase tracking-widest text-[10px] font-black mt-2">© {albumData.attributes.copyright || 'Apple Music'}</p>
      </div>
    </div>
  {/if}
</div>
