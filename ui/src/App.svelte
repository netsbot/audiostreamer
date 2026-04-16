<script lang="ts">
  import Sidebar from '$lib/components/Sidebar.svelte';
  import MainFeed from '$lib/components/MainFeed.svelte';
  import RightPane from '$lib/components/RightPane.svelte';
  import NowPlayingFullscreen from '$lib/components/NowPlayingFullscreen.svelte';
  import PlaybackBar from '$lib/components/PlaybackBar.svelte';
  import { playback } from '$lib/playback.svelte';
  import { onMount } from 'svelte';

  let innerWidth = $state(0);
  let hasInitializedPaneState = $state(false);

  const MOBILE_BREAKPOINT = 1280;

  const isMobileLayout = $derived(innerWidth > 0 && innerWidth < MOBILE_BREAKPOINT);

  function handleWindowKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape' && playback.isLyricsFullscreen) {
      playback.isLyricsFullscreen = false;
    }
  }

  $effect(() => {
    if (innerWidth <= 0 || hasInitializedPaneState) return;
    playback.lyricsPaneOpen = innerWidth >= MOBILE_BREAKPOINT;
    hasInitializedPaneState = true;
  });

  onMount(() => {
    playback.initBridge();
  });
</script>

<svelte:window bind:innerWidth={innerWidth} onkeydown={handleWindowKeydown} />

<div class="flex flex-col h-screen bg-zinc-950 text-zinc-100 overflow-hidden font-sans">
  <main class="flex flex-1 overflow-hidden">
    <Sidebar />
    <MainFeed />
    {#if playback.isLyricsFullscreen}
      <NowPlayingFullscreen />
    {:else if playback.lyricsPaneOpen && !isMobileLayout}
      <RightPane />
    {/if}
  </main>

  {#if playback.lyricsPaneOpen && isMobileLayout && !playback.isLyricsFullscreen}
    <div class="fixed inset-0 z-40 bg-black/40" onclick={() => playback.lyricsPaneOpen = false} aria-hidden="true"></div>
    <div class="fixed top-0 right-0 bottom-20 z-50 w-full max-w-88">
      <RightPane />
    </div>
  {/if}

  <footer class="shrink-0">
    <PlaybackBar />
  </footer>
</div>

<style>
  :global(body) {
    overflow: hidden;
  }
</style>
