<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import Maximize2 from 'lucide-svelte/icons/maximize-2';
  import ListMusic from 'lucide-svelte/icons/list-music';
  import MicVocal from 'lucide-svelte/icons/mic-vocal';
  import X from 'lucide-svelte/icons/x';
  import Play from 'lucide-svelte/icons/play';
  import Heart from 'lucide-svelte/icons/heart';
  import { playback } from '$lib/playback.svelte';
  import { library } from '$lib/library.svelte';

  type Syllable = {
    text: string;
    start: number;
    end: number;
    isBackground: boolean;
  };

  type Word = {
    syllables: Syllable[];
    hasTrailingSpace: boolean;
  };

  type LyricLine = {
    text: string;
    start: number;
    end: number;
    words: Word[];
    fullLineHighlight: boolean;
  };

  let lines = $state<LyricLine[]>([]);
  let isLoading = $state(false);
  let error = $state('');
  let loadedTrackId = $state<string | null>(null);
  let scroller = $state<HTMLElement | null>(null);
  let lastScrolledLineIndex = $state(-1);
  const lineRefs = new Map<number, HTMLElement>();

  function parseTtmlTime(input: string | null): number {
    if (!input) return 0;
    const parts = input.split(':');
    if (parts.length === 1) return Number.parseFloat(parts[0]) || 0;
    if (parts.length === 2) {
      const minutes = Number.parseFloat(parts[0]) || 0;
      const seconds = Number.parseFloat(parts[1]) || 0;
      return minutes * 60 + seconds;
    }
    if (parts.length === 3) {
      const hours = Number.parseFloat(parts[0]) || 0;
      const minutes = Number.parseFloat(parts[1]) || 0;
      const seconds = Number.parseFloat(parts[2]) || 0;
      return hours * 3600 + minutes * 60 + seconds;
    }
    return 0;
  }

  function isBackgroundSpan(node: Element): boolean {
    return node.getAttribute('ttm:role') === 'x-bg';
  }

  function tokenizeByWhitespace(text: string): string[] {
    return text.match(/\s+|[^\s]+/g) ?? [];
  }

  function getLineDisplayEnd(lineIndex: number): number {
    const currentLine = lines[lineIndex];
    const nextLine = lines[lineIndex + 1];
    if (!currentLine) return 0;

    if (nextLine && nextLine.start > currentLine.start) {
      return Math.max(nextLine.start, currentLine.end);
    }

    return currentLine.end;
  }

  function parseTtmlToLines(ttml: string): LyricLine[] {
    const parser = new DOMParser();
    const doc = parser.parseFromString(ttml, 'application/xml');
    const pNodes = Array.from(doc.getElementsByTagName('p'));
    const result: LyricLine[] = [];

    for (let pIndex = 0; pIndex < pNodes.length; pIndex++) {
      const pNode = pNodes[pIndex];
      const lineStart = parseTtmlTime(pNode.getAttribute('begin'));
      const nextLineStart = pIndex + 1 < pNodes.length
        ? parseTtmlTime(pNodes[pIndex + 1].getAttribute('begin'))
        : 0;
      let lineEnd = parseTtmlTime(pNode.getAttribute('end'));

      // Some TTML variants omit line end times. Fall back to next line start
      // (or a reasonable default) so non-syllable lyrics stay visible.
      if (lineEnd <= lineStart) {
        if (nextLineStart > lineStart) {
          lineEnd = nextLineStart;
        } else {
          lineEnd = lineStart + 4;
        }
      }
      const allSyllables: Syllable[] = [];
      let pendingText = '';

      for (const child of Array.from(pNode.childNodes)) {
        if (child.nodeType === Node.TEXT_NODE) {
          pendingText += child.textContent ?? '';
          continue;
        }
        if (child.nodeType !== Node.ELEMENT_NODE) continue;
        const el = child as Element;
        if (el.tagName !== 'span') continue;
        const isBg = isBackgroundSpan(el);

        const raw = el.textContent ?? '';
        if (!raw) continue;

        const text = `${pendingText}${raw}`;
        pendingText = '';
        allSyllables.push({
          text,
          start: parseTtmlTime(el.getAttribute('begin')) || lineStart,
          end: parseTtmlTime(el.getAttribute('end')) || lineEnd,
          isBackground: isBg,
        });
      }

      if (pendingText.trim()) {
        const lastEnd = allSyllables.length > 0 ? allSyllables[allSyllables.length - 1].end : lineStart;
        const safeStart = Math.min(lastEnd, lineEnd - 0.001);
        allSyllables.push({ text: pendingText, start: safeStart, end: lineEnd, isBackground: false });
      }

      if (allSyllables.length === 0) {
        const text = (pNode.textContent ?? '').trim();
        if (!text) continue;
        result.push({
          text, start: lineStart, end: lineEnd,
          words: [{ syllables: [{ text, start: lineStart, end: lineEnd, isBackground: false }], hasTrailingSpace: false }],
          fullLineHighlight: true,
        });
        continue;
      }

      // Group syllables into words. This also handles non-syllable lyrics where
      // a single span (or line fallback) contains full text with spaces.
      const words: Word[] = [];
      let currentWordSyllables: Syllable[] = [];
      let fullLineHighlight = allSyllables.length <= 1;
      
      for (const rawSyllable of allSyllables) {
        const tokens = tokenizeByWhitespace(rawSyllable.text);
        const textTokens = tokens.filter((token) => !/^\s+$/.test(token));
        if (textTokens.length > 1) fullLineHighlight = true;
        const tokenCount = Math.max(textTokens.length, 1);
        const safeStart = rawSyllable.start;
        const safeEnd = rawSyllable.end > rawSyllable.start ? rawSyllable.end : rawSyllable.start + 0.25;
        const slot = (safeEnd - safeStart) / tokenCount;
        let textTokenIndex = 0;

        for (const token of tokens) {
          if (/^\s+$/.test(token)) {
            if (currentWordSyllables.length > 0) {
              words.push({ syllables: currentWordSyllables, hasTrailingSpace: true });
              currentWordSyllables = [];
            }
            continue;
          }

          currentWordSyllables.push({
            text: token,
            start: safeStart + slot * textTokenIndex,
            end: safeStart + slot * (textTokenIndex + 1),
            isBackground: rawSyllable.isBackground,
          });
          textTokenIndex += 1;
        }
      }

      if (currentWordSyllables.length > 0) {
        words.push({ syllables: currentWordSyllables, hasTrailingSpace: false });
      }

      const lineText = allSyllables.map((s) => s.text).join('').trim();
      result.push({ text: lineText, start: lineStart, end: lineEnd, words, fullLineHighlight });
    }
    return result;
  }

  function extractTtml(payload: any): string | null {
    const attrs = payload?.data?.[0]?.attributes;
    if (!attrs) return null;
    if (typeof attrs.ttml === 'string') return attrs.ttml;
    const localizations = attrs.ttmlLocalizations;
    if (!localizations) return null;
    if (typeof localizations === 'string') return localizations;
    if (Array.isArray(localizations)) {
      for (const loc of localizations) {
        if (typeof loc === 'string') return loc;
        if (typeof loc?.ttml === 'string') return loc.ttml;
        if (typeof loc?.value === 'string') return loc.value;
      }
    }
    if (typeof localizations === 'object') {
      for (const key of Object.keys(localizations)) {
        const val = localizations[key];
        if (typeof val === 'string') return val;
        if (typeof val?.ttml === 'string') return val.ttml;
      }
    }
    return null;
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
  <!-- Header / Tabs -->
  <div class="flex flex-col gap-6 p-10 pb-4 shrink-0">
    <div class="flex items-center justify-between">
      <div class="flex items-center gap-4">
        <button 
          class="text-xs font-black uppercase tracking-[0.2em] transition-colors {playback.rightPaneMode === 'lyrics' ? 'text-white' : 'text-zinc-500 hover:text-zinc-300'}"
          onclick={() => playback.rightPaneMode = 'lyrics'}
        >
          Lyrics
        </button>
        <div class="w-1 h-1 rounded-full bg-zinc-800"></div>
        <button 
          class="text-xs font-black uppercase tracking-[0.2em] transition-colors {playback.rightPaneMode === 'queue' ? 'text-white' : 'text-zinc-500 hover:text-zinc-300'}"
          onclick={() => playback.rightPaneMode = 'queue'}
        >
          Playing Next
        </button>
      </div>
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
        <button class="text-zinc-500 hover:text-white">
          <Maximize2 class="size-4" />
        </button>
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
