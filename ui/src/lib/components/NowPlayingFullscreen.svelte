<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import Minimize2 from 'lucide-svelte/icons/minimize-2';
  import Shuffle from 'lucide-svelte/icons/shuffle';
  import SkipBack from 'lucide-svelte/icons/skip-back';
  import Play from 'lucide-svelte/icons/play';
  import Pause from 'lucide-svelte/icons/pause';
  import SkipForward from 'lucide-svelte/icons/skip-forward';
  import Repeat from 'lucide-svelte/icons/repeat';
  import MicVocal from 'lucide-svelte/icons/mic-vocal';
  import Music from 'lucide-svelte/icons/music';
  import Heart from 'lucide-svelte/icons/heart';
  import { playback } from '$lib/playback.svelte';
  import { library } from '$lib/library.svelte';
  import { extractTtml, parseTtmlToLines, type LyricLine } from '$lib/lyrics/ttml';

  let lines = $state<LyricLine[]>([]);
  let isLoading = $state(false);
  let error = $state('');
  let loadedTrackId = $state<string | null>(null);
  let scroller = $state<HTMLElement | null>(null);
  let backdropWrapper = $state<HTMLElement | null>(null);
  let backdropCanvas = $state<HTMLCanvasElement | null>(null);
  let lastScrolledLineIndex = $state(-1);
  const lineRefs = new Map<number, HTMLElement>();
  let backdropImage: HTMLImageElement | null = null;
  let backdropResizeObserver: ResizeObserver | null = null;
  const isNowPlayingOnly = $derived(playback.fullscreenMode === 'now-playing');

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
      destroy() {
        lineRefs.delete(lineIndex);
      },
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

  function handleSeek(e: MouseEvent) {
    if (!playback.totalTime) return;
    const target = e.currentTarget as HTMLDivElement;
    const rect = target.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const percentage = Math.max(0, Math.min(1, x / rect.width));
    playback.seekTo(percentage * playback.totalTime);
  }

  function handleSeekKeydown(e: KeyboardEvent) {
    if (!playback.totalTime) return;
    if (e.key === 'ArrowLeft') {
      e.preventDefault();
      playback.seekTo(Math.max(0, playback.currentTime - 5));
    } else if (e.key === 'ArrowRight') {
      e.preventDefault();
      playback.seekTo(Math.min(playback.totalTime, playback.currentTime + 5));
    }
  }

  function formatTime(seconds: number): string {
    if (!seconds || Number.isNaN(seconds)) return '0:00';
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    return `${mins}:${secs.toString().padStart(2, '0')}`;
  }

  async function loadBackdropImage(url: string | undefined) {
    if (!url) {
      backdropImage = null;
      drawBackdrop();
      return;
    }

    const image = new Image();
    image.crossOrigin = 'anonymous';

    await new Promise<void>((resolve) => {
      image.onload = () => resolve();
      image.onerror = () => resolve();
      image.src = url;
    });

    backdropImage = image.naturalWidth > 0 ? image : null;
    drawBackdrop();
  }

  function drawBackdrop() {
    if (!backdropCanvas || !backdropWrapper) return;

    const dpr = Math.max(1, window.devicePixelRatio || 1);
    const width = Math.floor(backdropWrapper.clientWidth * dpr);
    const height = Math.floor(backdropWrapper.clientHeight * dpr);

    if (!width || !height) return;

    if (backdropCanvas.width !== width || backdropCanvas.height !== height) {
      backdropCanvas.width = width;
      backdropCanvas.height = height;
    }

    const ctx = backdropCanvas.getContext('2d');
    if (!ctx) return;

    ctx.clearRect(0, 0, width, height);

    if (backdropImage) {
      const scale = 1.2;
      const drawW = width * scale;
      const drawH = height * scale;
      const x = (width - drawW) / 2;
      const y = (height - drawH) / 2;

      ctx.filter = `blur(${Math.max(40, Math.round(width * 0.035))}px) saturate(1.2) contrast(1.05)`;
      ctx.globalAlpha = 0.8;
      ctx.drawImage(backdropImage, x, y, drawW, drawH);
      ctx.filter = 'none';
      ctx.globalAlpha = 1;
    } else {
      const fallback = ctx.createLinearGradient(0, 0, width, height);
      fallback.addColorStop(0, 'rgba(38, 15, 19, 0.95)');
      fallback.addColorStop(1, 'rgba(12, 12, 12, 0.98)');
      ctx.fillStyle = fallback;
      ctx.fillRect(0, 0, width, height);
    }

    const topLeftAura = ctx.createRadialGradient(width * 0.2, height * 0.3, 0, width * 0.2, height * 0.3, width * 0.65);
    topLeftAura.addColorStop(0, 'rgba(140, 14, 34, 0.44)');
    topLeftAura.addColorStop(1, 'rgba(120, 10, 25, 0)');
    ctx.fillStyle = topLeftAura;
    ctx.fillRect(0, 0, width, height);

    const bottomRightAura = ctx.createRadialGradient(width * 0.82, height * 0.7, 0, width * 0.82, height * 0.7, width * 0.7);
    bottomRightAura.addColorStop(0, 'rgba(56, 56, 56, 0.34)');
    bottomRightAura.addColorStop(1, 'rgba(34, 34, 34, 0)');
    ctx.fillStyle = bottomRightAura;
    ctx.fillRect(0, 0, width, height);

    const vignette = ctx.createLinearGradient(0, 0, width, height);
    vignette.addColorStop(0, 'rgba(0, 0, 0, 0.2)');
    vignette.addColorStop(1, 'rgba(0, 0, 0, 0.7)');
    ctx.fillStyle = vignette;
    ctx.fillRect(0, 0, width, height);
  }

  $effect(() => {
    void loadBackdropImage(playback.currentTrack?.artwork_url);
  });

  $effect(() => {
    if (!backdropCanvas || !backdropWrapper) return;
    drawBackdrop();

    backdropResizeObserver?.disconnect();
    backdropResizeObserver = new ResizeObserver(() => {
      drawBackdrop();
    });
    backdropResizeObserver.observe(backdropWrapper);

    return () => {
      backdropResizeObserver?.disconnect();
      backdropResizeObserver = null;
    };
  });
</script>

<svelte:window onkeydown={(event) => event.key === 'Escape' && (playback.isLyricsFullscreen = false)} />

<div
  data-testid="modal-content-wrapper"
  id="modal-content-wrapper"
  class="content-wrapper fixed inset-0 z-60 overflow-hidden text-zinc-100"
  role="dialog"
  tabindex="0"
  aria-modal="false"
  aria-label="View Full Lyrics"
>
  <article class="lyrics__container">
    <div style="display: contents; --lyrics-border-radius: 0;">
      <div class="dt-lyrics__platter" data-testid="now-playing-backdrop" bind:this={backdropWrapper}>
        <canvas class="backdrop-canvas" bind:this={backdropCanvas}></canvas>
      </div>
    </div>

  <div class="relative z-10 flex h-full min-h-0 flex-col">
    <header class="absolute left-0 top-0 z-20 flex w-full items-center justify-between px-8 py-6">
      <div class="flex items-center gap-6">
        <button
          class="text-neutral-400 hover:text-white"
          onclick={() => playback.isLyricsFullscreen = false}
          aria-label="Close full screen"
        >
          <Minimize2 class="size-6" />
        </button>
        <div class="flex flex-col">
          <span class="text-[10px] font-bold uppercase tracking-[0.2em] text-neutral-500">Playing from library</span>
          <span class="text-sm font-semibold text-neutral-200">{playback.currentTrack?.album || 'Now playing'}</span>
        </div>
      </div>

    </header>

    <main class="flex h-screen w-full flex-col items-center justify-center gap-8 px-8 pb-8 pt-20 md:px-20">
      {#if isNowPlayingOnly}
      <section class="flex h-full w-full items-center justify-center">
        <div class="flex w-full max-w-6xl flex-col items-center gap-12 text-center lg:gap-14">
          <div class="flex w-full flex-col items-center gap-8">
            <div class="relative">
              {#if playback.currentTrack?.artwork_url}
                <img
                  src={playback.currentTrack.artwork_url}
                  alt={playback.currentTrack.title}
                  class="h-100 w-100 rounded-2xl object-cover shadow-[0_40px_100px_-20px_rgba(0,0,0,0.85)] lg:h-150 lg:w-150"
                />
              {:else}
                <div class="flex h-100 w-100 items-center justify-center rounded-2xl bg-zinc-900 shadow-[0_40px_100px_-20px_rgba(0,0,0,0.85)] lg:h-150 lg:w-150">
                  <Music class="size-20 text-zinc-700" />
                </div>
              {/if}
              <div class="pointer-events-none absolute inset-0 rounded-2xl ring-1 ring-white/10"></div>
            </div>

            <div class="w-100 text-left lg:w-150">
              <div class="flex items-start justify-between gap-4">
                <div class="min-w-0 flex-1">
                  <h1 class="mb-3 truncate text-sm font-semibold tracking-tight text-white md:text-base lg:text-lg">
                    {playback.currentTrack?.title || ''}
                  </h1>
                  <p class="truncate text-sm font-medium tracking-tight text-neutral-300 md:text-base lg:text-lg">
                    {playback.currentTrack?.artist || ''}
                  </p>
                </div>
                {#if playback.currentTrackId}
                  <button class="shrink-0 text-neutral-300 hover:text-red-500" onclick={() => library.toggleFavorite(playback.currentTrackId!)}>
                    <Heart class="size-5 {library.isFavorite(playback.currentTrackId!) ? 'fill-red-500 text-red-500' : ''}" />
                  </button>
                {/if}
              </div>
            </div>
          </div>

          <div class="w-100 rounded-2xl bg-transparent px-0 py-6 lg:w-150">
            <div class="space-y-7">
              <div class="flex w-full items-center gap-4">
                <span class="w-12 text-right font-mono text-sm text-neutral-400">{formatTime(playback.currentTime)}</span>
                <div
                  class="group relative h-1.5 flex-1 cursor-pointer rounded-full bg-white/15"
                  role="slider"
                  tabindex="0"
                  aria-label="Seek"
                  aria-valuemin="0"
                  aria-valuemax={playback.totalTime}
                  aria-valuenow={playback.currentTime}
                  onclick={handleSeek}
                  onkeydown={handleSeekKeydown}
                >
                  <div
                    class="absolute left-0 top-0 h-full rounded-full bg-red-600"
                    style="width: {playback.totalTime > 0 ? Math.min((playback.currentTime / playback.totalTime) * 100, 100) : 0}%"
                  ></div>
                  <div
                    class="absolute top-1/2 h-4 w-4 -translate-y-1/2 rounded-full bg-white shadow-lg"
                    style="left: calc({playback.totalTime > 0 ? Math.min((playback.currentTime / playback.totalTime) * 100, 100) : 0}% - 8px)"
                  ></div>
                </div>
                <span class="w-12 font-mono text-sm text-neutral-400">{formatTime(playback.totalTime)}</span>
              </div>

              <div class="flex items-center justify-center gap-10">
                <button class="text-neutral-300 hover:text-white" onclick={() => playback.toggleShuffle()}>
                  <Shuffle class="size-6" />
                </button>
                <button class="text-white hover:text-red-400" onclick={() => playback.playPrevious()}>
                  <SkipBack class="size-10 fill-current" />
                </button>
                <button class="flex h-18 w-18 items-center justify-center rounded-full bg-white text-black shadow-[0_0_40px_rgba(255,255,255,0.25)]" onclick={() => playback.togglePlayback()}>
                  {#if playback.isPlaying}
                    <Pause class="size-9 fill-current" />
                  {:else}
                    <Play class="size-9 fill-current" />
                  {/if}
                </button>
                <button class="text-white hover:text-red-400" onclick={() => playback.playNext()}>
                  <SkipForward class="size-10 fill-current" />
                </button>
                <button class="text-neutral-300 hover:text-white" onclick={() => playback.toggleRepeat()}>
                  <Repeat class="size-6" />
                </button>
              </div>
            </div>
          </div>

        </div>
      </section>
      {:else}
      <div class="flex w-full flex-col items-center gap-12 md:flex-row lg:gap-24">
      <section class="flex w-full flex-col items-center md:w-1/2 md:items-end">
        <div class="relative group">
          {#if playback.currentTrack?.artwork_url}
            <img
              src={playback.currentTrack.artwork_url}
              alt={playback.currentTrack.title}
              class="h-92 w-92 rounded-2xl object-cover shadow-[0_32px_64px_-12px_rgba(0,0,0,0.8)] lg:h-135 lg:w-135"
            />
          {:else}
            <div class="flex h-92 w-92 items-center justify-center rounded-2xl bg-zinc-900 shadow-[0_32px_64px_-12px_rgba(0,0,0,0.8)] lg:h-135 lg:w-135">
              <Music class="size-16 text-zinc-700" />
            </div>
          {/if}
          <div class="pointer-events-none absolute inset-0 rounded-2xl ring-1 ring-white/10"></div>
        </div>

        <div class="mt-8 w-92 text-left md:mt-12 lg:w-135">
          <div class="flex items-start justify-between gap-4">
            <div class="min-w-0 flex-1">
              <h1 class="mb-2 truncate text-sm font-semibold tracking-tight text-white md:text-base lg:text-lg">
                {playback.currentTrack?.title || ''}
              </h1>
              <p class="truncate text-sm font-medium tracking-tight text-neutral-400 md:text-base lg:text-lg">
                {playback.currentTrack?.artist || ''}
              </p>
            </div>
            {#if playback.currentTrackId}
              <button class="shrink-0 text-neutral-400 transition-colors hover:text-red-500" onclick={() => library.toggleFavorite(playback.currentTrackId!)}>
                <Heart class="size-5 {library.isFavorite(playback.currentTrackId!) ? 'fill-red-500 text-red-500' : ''}" />
              </button>
            {/if}
          </div>
        </div>

        <div class="mt-8 w-92 space-y-6 lg:mt-10 lg:w-135">
          <div class="flex w-full items-center gap-4">
            <span class="w-10 text-right font-mono text-[11px] text-neutral-500">{formatTime(playback.currentTime)}</span>
            <div
              class="group relative h-0.75 flex-1 cursor-pointer rounded-full bg-white/10"
              role="slider"
              tabindex="0"
              aria-label="Seek"
              aria-valuemin="0"
              aria-valuemax={playback.totalTime}
              aria-valuenow={playback.currentTime}
              onclick={handleSeek}
              onkeydown={handleSeekKeydown}
            >
              <div
                class="absolute left-0 top-0 h-full rounded-full bg-red-600"
                style="width: {playback.totalTime > 0 ? Math.min((playback.currentTime / playback.totalTime) * 100, 100) : 0}%"
              ></div>
              <div
                class="absolute top-1/2 h-3 w-3 -translate-y-1/2 rounded-full bg-white opacity-100 shadow-lg"
                style="left: calc({playback.totalTime > 0 ? Math.min((playback.currentTime / playback.totalTime) * 100, 100) : 0}% - 6px)"
              ></div>
            </div>
            <span class="w-10 font-mono text-[11px] text-neutral-500">{formatTime(playback.totalTime)}</span>
          </div>

          <div class="flex items-center justify-between">
            <button class="text-neutral-400 hover:text-white" onclick={() => playback.toggleShuffle()}>
              <Shuffle class="size-5" />
            </button>

            <div class="flex items-center gap-6">
              <button class="text-white hover:text-red-400" onclick={() => playback.playPrevious()}>
                <SkipBack class="size-8 fill-current" />
              </button>
              <button class="flex h-14 w-14 items-center justify-center rounded-full bg-white text-black shadow-[0_0_30px_rgba(255,255,255,0.2)]" onclick={() => playback.togglePlayback()}>
                {#if playback.isPlaying}
                  <Pause class="size-7 fill-current" />
                {:else}
                  <Play class="size-7 fill-current" />
                {/if}
              </button>
              <button class="text-white hover:text-red-400" onclick={() => playback.playNext()}>
                <SkipForward class="size-8 fill-current" />
              </button>
            </div>

            <button class="text-neutral-400 hover:text-white" onclick={() => playback.toggleRepeat()}>
              <Repeat class="size-5" />
            </button>
          </div>
        </div>
      </section>

      <section class="lyrics-shell hidden h-125 w-1/2 flex-col justify-center overflow-hidden lyrics-gradient md:flex">
        <div bind:this={scroller} class="h-full overflow-y-auto no-scrollbar py-20 pl-5 pr-12">
          {#if isLoading}
            <p class="text-sm text-white/50">Loading lyrics...</p>
          {:else if lines.length === 0}
            <p class="text-sm text-white/50">No lyrics loaded{error ? ` (${error})` : ''}</p>
          {:else}
            <div class="space-y-8 pb-24">
              {#each lines as line, lineIndex}
                {@const lineDisplayEnd = getLineDisplayEnd(lineIndex)}
                {@const lineActive = playback.smoothTime >= line.start && playback.smoothTime < lineDisplayEnd}
                <div style="filter: blur({lineActive ? '0px' : '1.2px'});">
                  <button
                    class="w-full border-none bg-transparent p-0 text-left whitespace-normal lyrics-line {lineActive ? 'opacity-100' : 'opacity-40 hover:opacity-65'}"
                    onclick={() => playback.seekTo(line.start)}
                  >
                    <div use:registerLine={lineIndex} class="flex flex-col items-start gap-2">
                      <div class="block text-3xl font-bold leading-[1.2] tracking-tight whitespace-normal lg:text-4xl {lineActive ? 'text-white' : 'text-neutral-500/70'}">
                        {#each line.words as word}
                          {#if word.syllables.some((s) => !s.isBackground)}
                            <span class="group inline-flex whitespace-nowrap overflow-visible align-baseline" class:mr-[0.22em]={word.hasTrailingSpace}>
                              {#each word.syllables.filter((s) => !s.isBackground) as syllable}
                                {@const duration = Math.round((syllable.end - syllable.start) * 1000)}
                                {@const safeDuration = Math.max(0.001, syllable.end - syllable.start)}
                                {@const progress = line.fullLineHighlight
                                  ? (lineActive ? 100 : 0)
                                  : Math.max(0, Math.min(100, ((playback.smoothTime - syllable.start) / safeDuration) * 100))}
                                {@const isActive = line.fullLineHighlight ? lineActive : progress > 0 && progress < 100}

                                <div class="relative inline-grid place-items-center overflow-visible">
                                  <span
                                    class="relative inline-grid place-items-center whitespace-pre px-3 py-1.5 -mx-3 -my-1.5 overflow-visible {lineActive ? 'text-white/20' : 'text-neutral-600/40'}"
                                    class:translate-y-[-3px]={isActive}
                                    class:is-glowing={isActive}
                                    data-content={syllable.text}
                                    data-duration={duration}
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

                      {#if line.words.some((w) => w.syllables.some((s) => s.isBackground))}
                        <div class="flex flex-wrap items-center gap-0 text-[1.25rem] font-medium text-zinc-400/70 lg:text-[1.45rem]">
                          {#each line.words as word}
                            {#if word.syllables.some((s) => s.isBackground)}
                              <span class="inline-flex" class:mr-[0.15em]={word.hasTrailingSpace}>
                                {#each word.syllables.filter((s) => s.isBackground) as syllable}
                                  {@const duration = Math.round((syllable.end - syllable.start) * 1000)}
                                  {@const safeDuration = Math.max(0.001, syllable.end - syllable.start)}
                                  {@const progress = line.fullLineHighlight
                                    ? (lineActive ? 100 : 0)
                                    : Math.max(0, Math.min(100, ((playback.smoothTime - syllable.start) / safeDuration) * 100))}
                                  {@const isActive = line.fullLineHighlight ? lineActive : progress > 0 && progress < 100}

                                  <span
                                    class="relative inline-grid place-items-center whitespace-pre text-zinc-600 overflow-visible"
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
            </div>
          {/if}
        </div>
      </section>
      </div>

      {/if}

      <button
        class="fixed bottom-8 right-8 z-30 transition-opacity {isNowPlayingOnly ? 'text-neutral-400 opacity-55 hover:opacity-80' : 'text-neutral-100 opacity-100'}"
        aria-label={isNowPlayingOnly ? 'Show lyrics fullscreen' : 'Show now playing fullscreen'}
        title={isNowPlayingOnly ? 'Lyrics' : 'Now Playing'}
        onclick={() => {
          if (isNowPlayingOnly) {
            playback.fullscreenMode = 'lyrics';
            playback.isLyricsFullscreen = true;
            playback.lyricsPaneOpen = true;
            playback.rightPaneMode = 'lyrics';
          } else {
            playback.fullscreenMode = 'now-playing';
            playback.isLyricsFullscreen = true;
          }
        }}
      >
        <MicVocal class="size-6" />
      </button>
    </main>
  </div>
  </article>
</div>

<style>
  .content-wrapper {
    background: #0c0c0c;
  }

  .lyrics__container {
    position: relative;
    width: 100%;
    height: 100%;
  }

  .dt-lyrics__platter {
    position: absolute;
    inset: 0;
    overflow: hidden;
    pointer-events: none;
    z-index: 0;
  }

  .backdrop-canvas {
    width: 100%;
    height: 100%;
    display: block;
  }

  .lyrics-gradient {
    -webkit-mask-image: linear-gradient(to bottom, transparent, black 20%, black 80%, transparent);
    mask-image: linear-gradient(to bottom, transparent, black 20%, black 80%, transparent);
  }

  .lyrics-shell {
    position: relative;
  }

  .lyrics-line {
    margin-left: 16px;
  }

  .grid-area-\[1\/1\] {
    grid-area: 1 / 1;
  }

  .is-glowing {
    filter: drop-shadow(0 0 8px rgba(255, 255, 255, 0.55)) drop-shadow(0 0 20px rgba(255, 255, 255, 0.3));
  }

  .is-glowing::after {
    text-shadow: none;
  }

  span[data-content]::after {
    content: attr(data-content);
    grid-area: 1 / 1;
    color: white;
    width: 100%;
    white-space: pre;
    opacity: var(--overlay-opacity);
    -webkit-mask-image: var(--mask);
    mask-image: var(--mask);
  }
</style>