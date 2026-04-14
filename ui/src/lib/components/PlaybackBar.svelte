<script lang="ts">
  import Shuffle from 'lucide-svelte/icons/shuffle';
  import Music from 'lucide-svelte/icons/music';
  import SkipBack from 'lucide-svelte/icons/skip-back';
  import Play from 'lucide-svelte/icons/play';
  import SkipForward from 'lucide-svelte/icons/skip-forward';
  import Repeat from 'lucide-svelte/icons/repeat';
  import Volume2 from 'lucide-svelte/icons/volume-2';
  import Mic2 from 'lucide-svelte/icons/mic-2';
  import MicVocal from 'lucide-svelte/icons/mic-vocal';
  import User from 'lucide-svelte/icons/user';
  import Pause from 'lucide-svelte/icons/pause';
  import Repeat1 from 'lucide-svelte/icons/repeat-1';
  import ListMusic from 'lucide-svelte/icons/list-music';
  
  import { playback } from '$lib/playback.svelte';

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
    if (!seconds || isNaN(seconds)) return '0:00';
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    return `${mins}:${secs.toString().padStart(2, '0')}`;
  }

  const defaultTrack = {
    title: 'Not Playing',
    artist: 'Select a song to start',
    album: '',
    image: ''
  };
</script>

<footer class="fixed bottom-0 w-full z-50 border-t border-white/5 bg-zinc-950/80 backdrop-blur-3xl shadow-2xl h-20 flex items-center px-8 justify-between">
  <!-- Track Info -->
  <div class="flex items-center gap-4 w-1/4">
    <div class="relative w-12 h-12 overflow-hidden rounded-lg shadow-lg bg-zinc-900 flex items-center justify-center">
      {#if playback.currentTrack?.artwork_url}
        <img src={playback.currentTrack.artwork_url} alt={playback.currentTrack.title} class="w-full h-full object-cover" />
      {:else}
        <Music class="size-6 text-zinc-700" />
      {/if}
    </div>
    <div class="flex flex-col overflow-hidden">
      <span class="text-sm font-bold truncate text-white">{playback.currentTrack?.title || defaultTrack.title}</span>
      <span class="text-xs text-zinc-400 truncate">{playback.currentTrack?.artist || defaultTrack.artist} {#if playback.currentTrack?.album} — {playback.currentTrack.album}{/if}</span>
    </div>
  </div>

  <!-- Controls -->
  <div class="flex flex-col items-center gap-1 w-1/2 max-w-2xl">
    <div class="flex items-center gap-6">
      <button 
        class="transition-colors {playback.isShuffled ? 'text-red-500' : 'text-zinc-400 hover:text-white'}"
        onclick={() => playback.toggleShuffle()}
        title="Shuffle"
      >
        <Shuffle class="size-4" />
      </button>

      <button 
        class="text-zinc-200 hover:text-white transition-colors"
        onclick={() => playback.playPrevious()}
        title="Previous Track"
      >
        <SkipBack class="size-5 fill-current" />
      </button>

      <button 
        class="w-10 h-10 flex items-center justify-center bg-white text-zinc-950 rounded-full hover:scale-105 transition-transform"
        onclick={() => playback.togglePlayback()}
      >
        {#if playback.isPlaying}
          <Pause class="size-5 fill-current" />
        {:else}
          <Play class="size-5 fill-current" />
        {/if}
      </button>

      <button 
        class="text-zinc-200 hover:text-white transition-colors"
        onclick={() => playback.playNext()}
        title="Next Track"
      >
        <SkipForward class="size-5 fill-current" />
      </button>

      <button 
        class="transition-colors {playback.repeatMode > 0 ? 'text-red-500' : 'text-zinc-400 hover:text-white'}"
        onclick={() => playback.toggleRepeat()}
        title={['Repeat Off', 'Repeat All', 'Repeat One'][playback.repeatMode]}
      >
        {#if playback.repeatMode === 2}
          <Repeat1 class="size-4" />
        {:else}
          <Repeat class="size-4" />
        {/if}
      </button>
    </div>
    <div class="flex items-center gap-3 w-full">
      <span class="text-[10px] text-zinc-500 font-medium font-mono w-10">{formatTime(playback.currentTime)}</span>
      <div
        class="flex-1 px-1 group relative cursor-pointer py-4 -my-4"
        role="slider"
        tabindex="0"
        aria-label="Seek"
        aria-valuemin="0"
        aria-valuemax={playback.totalTime}
        aria-valuenow={playback.currentTime}
        onclick={handleSeek}
        onkeydown={handleSeekKeydown}
      >
        <div class="h-1 w-full bg-zinc-800 rounded-full overflow-hidden">
            <div 
                class="h-full bg-red-600 transition-all duration-300 ease-out" 
                style="width: {playback.totalTime > 0 ? Math.min((playback.currentTime / playback.totalTime) * 100, 100) : 0}%"
            ></div>
        </div>
      </div>
      <span class="text-[10px] text-zinc-500 font-medium font-mono w-10 text-right">{formatTime(playback.totalTime)}</span>
    </div>
  </div>

  <!-- Volume & Tools -->
  <div class="flex items-center justify-end gap-4 w-1/4">
    <div class="flex items-center gap-2 mr-2">
      <Volume2 class="text-zinc-400 size-4" />
      <div class="w-20">
        <div class="h-1 w-full bg-zinc-800 rounded-full overflow-hidden">
          <div class="h-full bg-zinc-400" style="width: 75%"></div>
        </div>
      </div>
    </div>
    
    <div class="flex items-center bg-zinc-900/50 rounded-lg p-1 border border-white/5">
      <button
        class="p-1.5 rounded-md transition-all outline-none focus:outline-none {playback.lyricsPaneOpen && playback.rightPaneMode === 'lyrics' ? 'text-red-500 bg-white/5 shadow-sm' : 'text-zinc-500 hover:text-white'}"
        onclick={() => {
          if (playback.lyricsPaneOpen && playback.rightPaneMode === 'lyrics') {
            playback.lyricsPaneOpen = false;
          } else {
            playback.lyricsPaneOpen = true;
            playback.rightPaneMode = 'lyrics';
          }
        }}
        aria-label="Toggle lyrics"
        title="Lyrics"
      >
        <MicVocal class="size-4" />
      </button>

      <button
        class="p-1.5 rounded-md transition-all outline-none focus:outline-none {playback.lyricsPaneOpen && playback.rightPaneMode === 'queue' ? 'text-red-500 bg-white/5 shadow-sm' : 'text-zinc-500 hover:text-white'}"
        onclick={() => {
          if (playback.lyricsPaneOpen && playback.rightPaneMode === 'queue') {
            playback.lyricsPaneOpen = false;
          } else {
            playback.lyricsPaneOpen = true;
            playback.rightPaneMode = 'queue';
          }
        }}
        aria-label="Toggle queue"
        title="Playing Next"
      >
        <ListMusic class="size-4" />
      </button>
    </div>

    <button class="text-zinc-500 hover:text-white transition-colors ml-2"><User class="size-4" /></button>
  </div>
</footer>
