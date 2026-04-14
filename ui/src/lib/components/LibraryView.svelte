<script lang="ts">
  import { fade, fly } from "svelte/transition";
  import Play from "lucide-svelte/icons/play";
  import { library } from "$lib/library.svelte";
  import { navigation } from "$lib/navigation.svelte";
  import { playback } from "$lib/playback.svelte";

  const gridClass = $derived(playback.lyricsPaneOpen
    ? "grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 gap-6"
    : "grid grid-cols-2 lg:grid-cols-4 xl:grid-cols-5 2xl:grid-cols-6 gap-8");

  function openPlaylist(id: string) {
    navigation.openPlaylist(id, 'library-playlists');
  }
</script>

<div class="pb-24" in:fade={{ duration: 400 }}>
  <div class="mb-10 px-2" in:fly={{ y: 20, duration: 400 }}>
    <h1 class="text-4xl font-black tracking-tighter text-white">Playlists</h1>
    <p class="text-zinc-500 text-sm mt-1 font-medium">Your library</p>
  </div>

  <div class={gridClass}>
    {#each library.playlists as pl, i}
      <div 
        class="group cursor-pointer flex flex-col gap-3"
        in:fly={{ y: 20, duration: 400, delay: Math.min(i * 30, 400) }}
      >
        <div 
          class="relative aspect-square rounded-2xl overflow-hidden bg-zinc-900 border border-white/5 shadow-xl transition-all duration-500 group-hover:border-white/20 group-hover:scale-[1.02] group-hover:shadow-2xl"
          onclick={() => openPlaylist(pl.id)}
          onkeydown={(e) => e.key === 'Enter' && openPlaylist(pl.id)}
          role="button"
          tabindex="0"
        >
          {#if pl.artworkUrl}
            <img 
              src={pl.artworkUrl.replace('300x300', '600x600')} 
              alt={pl.name}
              class="w-full h-full object-cover"
              loading="lazy"
            />
          {:else}
            <div class="w-full h-full flex items-center justify-center bg-zinc-800">
              <Play class="size-12 text-zinc-600" />
            </div>
          {/if}

          <!-- Play Overlay -->
          <div class="absolute inset-0 bg-black/40 opacity-0 group-hover:opacity-100 transition-opacity duration-300 flex items-center justify-center">
            <button 
              class="w-14 h-14 bg-red-600 text-white rounded-full flex items-center justify-center shadow-2xl hover:bg-red-500 hover:scale-110 transition-all duration-200"
              onclick={(e) => { e.stopPropagation(); openPlaylist(pl.id); }}
            >
              <Play class="size-7 fill-current translate-x-0.5" />
            </button>
          </div>
        </div>

        <div class="px-1">
          <h3 class="font-bold text-white text-[15px] truncate group-hover:text-red-500 transition-colors">
            {pl.name}
          </h3>
          <p class="text-zinc-500 text-[13px] truncate mt-0.5 font-medium">
            {pl.curatorName || 'Playlist'} • {pl.trackCount} Songs
          </p>
        </div>
      </div>
    {/each}
  </div>

  {#if library.playlists.length === 0 && !library.isLoading}
    <div class="flex flex-col items-center justify-center h-[40vh] text-zinc-500">
      <p>No playlists found in your library.</p>
    </div>
  {/if}
</div>
