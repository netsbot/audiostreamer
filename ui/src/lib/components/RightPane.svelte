<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import X from 'lucide-svelte/icons/x';
  import Play from 'lucide-svelte/icons/play';
  import Heart from 'lucide-svelte/icons/heart';
  import { playback } from '$lib/playback.svelte';
  import { library } from '$lib/library.svelte';
  import { extractTtml, parseTtmlToLines, type LyricLine } from '$lib/lyrics/ttml';

  let lines = $state<LyricLine[]>([]);
  let isLoading = $state(false);
  let error = $state('');
  let loadedTrackId = $state<string | null>(null);
  let scroller = $state<HTMLElement | null>(null);
  let lastScrolledLineIndex = $state(-1);
  const lineRefs = new Map<number, HTMLElement>();

  function getLineDisplayEnd(lineIndex: number): number {
    const currentLine = lines[lineIndex];
    const nextLine = lines[lineIndex + 1];
    if (!currentLine) return 0;

    if (nextLine && nextLine.start > currentLine.start) {
      return Math.max(nextLine.start, currentLine.end);
    }

    return currentLine.end;
  }

  async function loadLyricsForTrack(adamId: string) {
    isLoading = true;
    error = '';
    lastScrolledLineIndex = -1;
    lineRefs.clear();
    scroller?.scrollTo({ top: 0, behavior: 'auto' });
    try {
      const raw = await invoke<string>('get_lyrics_payload', { adamId });
      const json = JSON.parse(raw);
      const ttml = extractTtml(json);
      if (!ttml) throw new Error('No TTML lyrics found');
      lines = parseTtmlToLines(ttml);
      loadedTrackId = adamId;
    } catch (e) {
      lines = [];
      loadedTrackId = null;
      error = e instanceof Error ? e.message : String(e);
    } finally {
      isLoading = false;
    }
  }

  $effect(() => {
    if (playback.currentTrackId && playback.currentTrackId !== loadedTrackId) {
      loadLyricsForTrack(playback.currentTrackId);
    }
  });

  function registerLine(node: HTMLElement, lineIndex: number) {
    lineRefs.set(lineIndex, node);
    return {
      destroy() { lineRefs.delete(lineIndex); }
    };
  }

  function scrollToLine(lineIndex: number) {
    if (!scroller) return;
    const el = lineRefs.get(lineIndex);
    if (!el) return;
    const scrollerRect = scroller.getBoundingClientRect();
    const elRect = el.getBoundingClientRect();
    const relativeTop = elRect.top - scrollerRect.top + scroller.scrollTop;
    const targetTop = Math.max(relativeTop - scroller.clientHeight * 0.35, 0);
    scroller.scrollTo({ top: targetTop, behavior: 'smooth' });
  }

  $effect(() => {
    const t = playback.smoothTime;
    let activeIndex = -1;
    for (let i = 0; i < lines.length; i++) {
      if (t >= lines[i].start && t < getLineDisplayEnd(i)) {
            activeIndex = i;
            break;
        }
    }
    
    if (activeIndex >= 0 && activeIndex !== lastScrolledLineIndex) {
      lastScrolledLineIndex = activeIndex;
      requestAnimationFrame(() => scrollToLine(activeIndex));
    }
  });
</script>

<aside 
  class="relative h-full w-88 shrink-0 bg-zinc-900/80 backdrop-blur-3xl border-l border-white/5 flex flex-col overflow-hidden"
>
  <div class="flex flex-col gap-6 p-10 pb-4 shrink-0">
    <div class="flex items-center justify-between">
      <div class="flex items-center gap-3">
        {#if playback.currentTrackId}
          <button 
            onclick={() => library.toggleFavorite(playback.currentTrackId!)}
            class="text-zinc-500 hover:text-white transition-colors"
          >
            <Heart 
              class="size-4 {library.isFavorite(playback.currentTrackId!) ? 'text-red-500 fill-red-500' : ''}" 
            />
          </button>
        {/if}
      </div>
    </div>
  </div>

  <div bind:this={scroller} class="flex-1 overflow-y-auto no-scrollbar p-10 pt-4">
    {#if playback.rightPaneMode === 'lyrics'}
      <div class="space-y-7 pb-28">
        {#if isLoading}
          <p class="text-zinc-500 text-sm">Loading lyrics...</p>
        {:else if lines.length === 0}
          <p class="text-zinc-500 text-sm">No lyrics loaded{error ? ` (${error})` : ''}</p>
        {:else}
          {#each lines as line, lineIndex}
            {@const lineDisplayEnd = getLineDisplayEnd(lineIndex)}
            {@const lineActive = playback.smoothTime >= line.start && playback.smoothTime < lineDisplayEnd}
            <div class="display-synced-line mb-10 transition-all duration-700" style="filter: blur({lineActive ? '0px' : '2px'});">
              <button 
                class="line w-full text-left border-none bg-transparent p-0 cursor-pointer transition-all duration-500 whitespace-normal {lineActive ? 'opacity-100' : 'opacity-20 hover:opacity-40'}"
                onclick={() => playback.seekTo(line.start)}
              >
                <div 
                  use:registerLine={lineIndex}
                  class="lyrics-container flex flex-col items-start gap-2"
                >
                  <!-- Primary Vocals -->
                  <div class="primary-vocals block text-[2rem] font-semibold tracking-tight leading-[1.2] whitespace-normal">
                    {#each line.words as word}
                      {#if word.syllables.some(s => !s.isBackground)}
                        <span class="group inline-flex whitespace-nowrap overflow-visible align-baseline" class:mr-[0.22em]={word.hasTrailingSpace}>
                          {#each word.syllables.filter(s => !s.isBackground) as syllable}
                            {@const duration = Math.round((syllable.end - syllable.start) * 1000)}
                            {@const delay = Math.round((syllable.start - line.start) * 1000)}
                            {@const safeDuration = Math.max(0.001, syllable.end - syllable.start)}
                            {@const progress = line.fullLineHighlight
                              ? (lineActive ? 100 : 0)
                              : Math.max(0, Math.min(100, ((playback.smoothTime - syllable.start) / safeDuration) * 100))}
                            {@const isActive = line.fullLineHighlight ? lineActive : progress > 0 && progress < 100}
                            
                            <div class="main relative inline-grid place-items-center overflow-visible">
                              <span 
                                class="syllable relative inline-grid place-items-center text-white/20 whitespace-pre transition-transform duration-500 px-3 py-1.5 -mx-3 -my-1.5 overflow-visible"
                                class:translate-y-[-3px]={isActive}
                                class:is-glowing={isActive}
                                data-content={syllable.text}
                                data-duration={duration}
                                data-delay={delay}
                                style="--gradient-progress: {progress}%; --mask: {progress >= 100 ? 'none' : `linear-gradient(to right, #000 0%, #000 var(--gradient-progress), transparent calc(var(--gradient-progress) + 40%))`}; --overlay-opacity: {progress > 0 ? 1 : 0};"
                              >
                                <span class="grid-area-[1/1]">{syllable.text}</span>
                              </span>
                            </div>
                          {/each}
                        </span>
                      {/if}
                    {/each}
                  </div>

                  <!-- Background Vocals -->
                  {#if line.words.some(w => w.syllables.some(s => s.isBackground))}
                    <div class="background-vocals flex flex-wrap items-center opacity-60 text-[1.4rem] font-medium text-zinc-400 mt-1 tracking-tight">
                      {#each line.words as word}
                        {#if word.syllables.some(s => s.isBackground)}
                           <span class="inline-flex" class:mr-[0.15em]={word.hasTrailingSpace}>
                            {#each word.syllables.filter(s => s.isBackground) as syllable}
                              {@const duration = Math.round((syllable.end - syllable.start) * 1000)}
                              {@const safeDuration = Math.max(0.001, syllable.end - syllable.start)}
                              {@const progress = line.fullLineHighlight
                                ? (lineActive ? 100 : 0)
                                : Math.max(0, Math.min(100, ((playback.smoothTime - syllable.start) / safeDuration) * 100))}
                              {@const isActive = line.fullLineHighlight ? lineActive : progress > 0 && progress < 100}

                              <span 
                                class="syllable relative inline-grid place-items-center text-zinc-600 whitespace-pre transition-all duration-500 overflow-visible"
                                class:is-glowing={isActive}
                                class:text-white={isActive}
                                data-content={syllable.text}
                                style="--gradient-progress: {progress}%; --mask: {progress >= 100 ? 'none' : `linear-gradient(to right, #000 0%, #000 var(--gradient-progress), transparent calc(var(--gradient-progress) + 40%))`}; --overlay-opacity: {progress > 0 ? 1 : 0};"
                              >
                                <span class="grid-area-[1/1]">{syllable.text}</span>
                              </span>
                            {/each}

                           </span>
                        {/if}
                      {/each}
                    </div>
                  {/if}
                </div>
              </button>
            </div>
          {/each}
        {/if}
      </div>
    {:else}
      <div class="space-y-4 pb-28">
        <h3 class="text-zinc-500 text-[10px] font-bold uppercase tracking-widest mb-4">Upcoming</h3>
        {#if playback.activeQueue.length === 0}
          <p class="text-zinc-500 text-sm">Queue is empty</p>
        {:else}
          {#each playback.activeQueue as track, i}
            {@const isCurrent = i === playback.activeQueueIndex}
            <div 
              class="group relative flex items-center gap-4 p-2 -mx-2 rounded-xl transition-all {isCurrent ? 'bg-white/10' : 'hover:bg-white/5'}"
            >
              <div class="relative size-12 shrink-0 overflow-hidden rounded-lg bg-zinc-800 shadow-md">
                {#if track.metadata.artwork_url}
                  <img src={track.metadata.artwork_url} alt={track.metadata.title} class="size-full object-cover {isCurrent ? 'opacity-40' : ''}" />
                {/if}
                <button 
                  class="absolute inset-0 flex items-center justify-center bg-black/40 opacity-0 group-hover:opacity-100 transition-opacity"
                  onclick={() => playback.jumpToQueueIndex(i)}
                >
                  <Play class="size-5 text-white fill-current" />
                </button>
              </div>
              
              <div class="flex-1 min-w-0">
                <p class="text-sm font-bold truncate {isCurrent ? 'text-red-500' : 'text-white'}">
                  {track.metadata.title}
                </p>
                <p class="text-xs text-zinc-400 truncate">
                  {track.metadata.artist}
                </p>
              </div>

              <button 
                class="size-8 flex items-center justify-center text-zinc-500 hover:text-white opacity-0 group-hover:opacity-100 transition-opacity"
                onclick={() => playback.removeFromQueue(i)}
                title="Remove from queue"
              >
                <X class="size-4" />
              </button>
            </div>
          {/each}
        {/if}
      </div>
    {/if}
  </div>
</aside>

<style>
  .background-vocals .syllable::after {
    content: attr(data-content);
    grid-area: 1 / 1;
    color: white;
    width: 100%;
    white-space: pre;
    opacity: var(--overlay-opacity);
    -webkit-mask-image: var(--mask);
    mask-image: var(--mask);
    text-shadow: none;
  }

  .syllable::after {
    content: attr(data-content);
    grid-area: 1 / 1;
    color: white;
    width: 100%;
    white-space: pre;
    opacity: var(--overlay-opacity);
    -webkit-mask-image: var(--mask);
    mask-image: var(--mask);
  }

  .syllable.is-glowing::after {
    text-shadow: none;
  }

  .syllable.is-glowing {
    filter: drop-shadow(0 0 8px rgba(255, 255, 255, 0.55)) drop-shadow(0 0 20px rgba(255, 255, 255, 0.3));
  }

  .grid-area-\[1\/1\] {
    grid-area: 1 / 1;
  }
</style>
