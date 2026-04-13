<script lang="ts">
  import Sidebar from '$lib/components/Sidebar.svelte';
  import MainFeed from '$lib/components/MainFeed.svelte';
  import LyricsPane from '$lib/components/LyricsPane.svelte';
  import PlaybackBar from '$lib/components/PlaybackBar.svelte';
  import { initPlaybackBridge } from '$lib/playbackStore';
  import { onMount } from 'svelte';

  onMount(() => {
    initPlaybackBridge();
  });
</script>

<div class="flex flex-col h-screen bg-zinc-950 text-zinc-100 overflow-hidden font-sans">
  <main class="flex flex-1 overflow-hidden">
    <Sidebar />
    <MainFeed />
    <LyricsPane />
  </main>
  <footer class="shrink-0">
    <PlaybackBar />
  </footer>
</div>

<style>
  :global(body) {
    overflow: hidden;
  }
</style>
