<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import Maximize2 from 'lucide-svelte/icons/maximize-2';
  import { playback } from '$lib/playback.svelte';

  type Word = {
    syllables: Syllable[];
    hasTrailingSpace: boolean;
  };

  type LyricLine = {
    text: string;
    start: number;
    end: number;
    words: Word[];
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

  function parseTtmlToLines(ttml: string): LyricLine[] {
    const parser = new DOMParser();
    const doc = parser.parseFromString(ttml, 'application/xml');
    const pNodes = Array.from(doc.getElementsByTagName('p'));
    const result: LyricLine[] = [];

    for (const pNode of pNodes) {
      const lineStart = parseTtmlTime(pNode.getAttribute('begin'));
      const lineEnd = parseTtmlTime(pNode.getAttribute('end'));
      const allSyllables: Syllable[] = [];
      let pendingText = '';

      for (const child of Array.from(pNode.childNodes)) {
        if (child.nodeType === Node.TEXT_NODE) {
          pendingText += child.textContent ?? '';
          continue;
        }
        if (child.nodeType !== Node.ELEMENT_NODE) continue;
        const el = child as Element;
        if (el.tagName !== 'span' || isBackgroundSpan(el)) continue;

        const raw = el.textContent ?? '';
        if (!raw) continue;

        const text = `${pendingText}${raw}`;
        pendingText = '';
        allSyllables.push({
          text,
          start: parseTtmlTime(el.getAttribute('begin')) || lineStart,
          end: parseTtmlTime(el.getAttribute('end')) || lineEnd,
        });
      }

      if (pendingText.trim()) {
        const lastEnd = allSyllables.length > 0 ? allSyllables[allSyllables.length - 1].end : lineEnd;
        allSyllables.push({ text: pendingText, start: lastEnd, end: lineEnd });
      }

      if (allSyllables.length === 0) {
        const text = (pNode.textContent ?? '').trim();
        if (!text) continue;
        result.push({
          text, start: lineStart, end: lineEnd,
          words: [{ syllables: [{ text, start: lineStart, end: lineEnd }], hasTrailingSpace: false }]
        });
        continue;
      }

      // Group syllables into words
      const words: Word[] = [];
      let currentWordSyllables: Syllable[] = [];
      
      for (const s of allSyllables) {
        currentWordSyllables.push(s);
        if (s.text.endsWith(' ')) {
          words.push({ syllables: currentWordSyllables, hasTrailingSpace: true });
          currentWordSyllables = [];
        }
      }
      if (currentWordSyllables.length > 0) {
        words.push({ syllables: currentWordSyllables, hasTrailingSpace: false });
      }

      const lineText = allSyllables.map((s) => s.text).join('').trim();
      result.push({ text: lineText, start: lineStart, end: lineEnd, words });
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
        if (t >= lines[i].start && t < lines[i].end) {
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
  bind:this={scroller} 
  class="fixed right-0 top-0 h-[calc(100vh-80px)] w-[480px] bg-zinc-900/80 backdrop-blur-3xl border-l border-white/5 p-12 overflow-y-auto no-scrollbar"
  style="
    --lyrics-linear-gradient: linear-gradient(180deg, transparent, #000 40px, #000 calc(100% - 40px), transparent);
    -webkit-mask-image: var(--lyrics-linear-gradient);
    mask-image: var(--lyrics-linear-gradient);
  "
>
  <div class="flex items-center justify-between mb-12">
    <h2 class="text-xs font-black uppercase tracking-[0.2em] text-zinc-500">Lyrics</h2>
    <button class="text-zinc-500 hover:text-white">
      <Maximize2 class="size-4" />
    </button>
  </div>

  <div class="space-y-8 pb-32">
    {#if isLoading}
      <p class="text-zinc-500 text-sm">Loading lyrics...</p>
    {:else if lines.length === 0}
      <p class="text-zinc-500 text-sm">No lyrics loaded{error ? ` (${error})` : ''}</p>
    {:else}
      {#each lines as line, lineIndex}
        {@const lineActive = playback.smoothTime >= line.start && playback.smoothTime < line.end}
        <div class="display-synced-line mb-10 transition-all duration-700" style="filter: blur({lineActive ? '0px' : '2px'});">
          <button 
            class="line w-full text-left border-none bg-transparent p-0 cursor-pointer transition-all duration-500 whitespace-normal {lineActive ? 'opacity-100' : 'opacity-20 hover:opacity-40'}"
            onclick={() => playback.seekTo(line.start)}
          >
            <div 
              use:registerLine={lineIndex}
              class="primary-vocals block text-[2.75rem] font-black tracking-tight leading-[1.15]"
            >
              {#each line.words as word}
                <span class="group inline-block whitespace-normal wrap-break-word overflow-visible">
                  {#each word.syllables as syllable}
                    {@const duration = Math.round((syllable.end - syllable.start) * 1000)}
                    {@const delay = Math.round((syllable.start - line.start) * 1000)}
                    {@const progress = Math.max(0, Math.min(100, ((playback.smoothTime - syllable.start) / (syllable.end - syllable.start)) * 100))}
                    {@const isActive = progress > 0 && progress < 100}
                    
                    <div class="main relative inline-grid place-items-center overflow-visible">
                      <span 
                        class="syllable relative inline-grid place-items-center text-white/20 whitespace-pre transition-transform duration-500 px-4 py-2 mx-[-1rem] my-[-0.5rem] overflow-visible"
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
                {#if word.hasTrailingSpace}
                  <span class="inline"> </span>
                {/if}
              {/each}
            </div>
          </button>
        </div>
      {/each}
    {/if}
  </div>

  <div class="absolute bottom-0 left-0 w-full h-32 bg-linear-to-t from-zinc-900 via-zinc-900/80 to-transparent pointer-events-none"></div>
</aside>

<style>
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
    text-shadow: 0 0 15px rgba(var(--gradient-color, 255), var(--gradient-color, 255), var(--gradient-color, 255), 0.6);
  }

  .grid-area-\[1\/1\] {
    grid-area: 1 / 1;
  }
</style>
