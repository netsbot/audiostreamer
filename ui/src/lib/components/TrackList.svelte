<script lang="ts">
  import Play from "lucide-svelte/icons/play";
  import Clock from "lucide-svelte/icons/clock";

  type ResolvedTrack = {
    id?: string;
    attrs: any;
  };

  let {
    tracks = [] as any[],
    resolveTrack = (track: any): ResolvedTrack => ({ id: track?.id, attrs: track?.attributes || {} }),
    onPlay = (_track: any, _index: number) => {},
    getArtworkUrl = (_artwork: any, _size?: number) => "",
    fallbackArtwork = null as any,
  } = $props();

  function formatDuration(ms: number) {
    if (!ms) return "--:--";
    const totalSeconds = Math.floor(ms / 1000);
    const minutes = Math.floor(totalSeconds / 60);
    const seconds = totalSeconds % 60;
    return `${minutes}:${seconds.toString().padStart(2, "0")}`;
  }
</script>

<div class="w-full">
  <div class="grid grid-cols-[1fr_8rem] items-center px-4 py-3 border-b border-white/5 text-[10px] font-black text-zinc-500 uppercase tracking-[0.15em] mb-2">
    <span>Track</span>
    <span class="text-right flex justify-end mr-4"><Clock class="size-3" /></span>
  </div>

  <div class="flex flex-col">
    {#each tracks as track, i}
      {@const resolved = resolveTrack(track)}
      {@const rowArtwork = resolved.attrs.artwork || fallbackArtwork}
      <button
        type="button"
        class="grid grid-cols-[1fr_8rem] items-center px-4 py-3 rounded-xl hover:bg-white/[0.04] transition-all group border border-transparent hover:border-white/5 text-left"
        onclick={() => onPlay(track, i)}
      >
        <div class="flex items-center min-w-0">
          <div class="text-zinc-500 font-bold text-sm relative w-8 flex justify-start items-center flex-shrink-0">
            <span class="group-hover:opacity-0 transition-opacity text-xs">{i + 1}</span>
            <Play class="size-3 absolute opacity-0 group-hover:opacity-100 transition-opacity text-white fill-current" />
          </div>

          <div class="w-10 h-10 rounded-md overflow-hidden mr-3 flex-shrink-0 border border-white/5 bg-zinc-900">
            {#if rowArtwork}
              <img
                src={getArtworkUrl(rowArtwork, 160)}
                alt={resolved.attrs.name || "Track artwork"}
                class="w-full h-full object-cover"
                loading="lazy"
                decoding="async"
              />
            {:else}
              <div class="w-full h-full bg-zinc-800"></div>
            {/if}
          </div>

          <div class="flex flex-col min-w-0">
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
      </button>
    {/each}
  </div>
</div>
