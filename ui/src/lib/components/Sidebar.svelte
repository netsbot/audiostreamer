<script lang="ts">
  import Home from 'lucide-svelte/icons/home';
  import Compass from 'lucide-svelte/icons/compass';
  import Radio from 'lucide-svelte/icons/radio';
  import LibraryIcon from 'lucide-svelte/icons/library';
  import PersonStanding from 'lucide-svelte/icons/person-standing';
  import ListMusic from 'lucide-svelte/icons/list-music';
  import Heart from 'lucide-svelte/icons/heart';
  import Loader2 from 'lucide-svelte/icons/loader-2';
  import { onMount } from 'svelte';
  import { library } from '$lib/library.svelte';
  import { navigation } from '$lib/navigation.svelte';
  import { search } from '$lib/search.svelte';
  import Search from 'lucide-svelte/icons/search';

  const navItems = [
    { icon: Home, label: 'Home', view: 'home' },
    { icon: Compass, label: 'Browse', view: 'browse' },
    { icon: Radio, label: 'Radio', view: 'radio' },
    { icon: LibraryIcon, label: 'Library', view: 'library' },
  ];

  onMount(() => {
    library.fetchPlaylists();
  });

  const favorites = $derived(library.playlists.find(p => 
    p.name.toLowerCase() === 'favorite songs' || 
    p.name.toLowerCase() === 'favorites' ||
    p.name.toLowerCase() === 'favourite songs'
  ));

  const otherPlaylists = $derived(library.playlists.filter(p => p.id !== favorites?.id));
</script>

<aside class="relative h-full w-56 shrink-0 border-r border-white/5 bg-zinc-900/50 flex flex-col py-8 px-5 gap-y-4">
  <!-- Search Bar -->
  <div class="relative mb-2 px-1">
    <Search
      class="absolute left-4 top-1/2 -translate-y-1/2 size-3.5 text-zinc-500"
    />
    <input
      type="text"
      bind:value={search.query}
      onkeydown={(e) => e.key === "Enter" && search.handleSearch()}
      placeholder="Search..."
      class="w-full bg-zinc-950/50 border border-white/5 rounded-lg py-2 pl-9 pr-3 text-xs focus:outline-none focus:ring-1 focus:ring-red-500/50 transition-all placeholder:text-zinc-600"
    />
    {#if search.isSearching}
      <div class="absolute right-3 top-1/2 -translate-y-1/2">
        <div class="size-2.5 border-2 border-red-500/50 border-t-red-500 rounded-full animate-spin"></div>
      </div>
    {/if}
  </div>

  <nav class="flex flex-col gap-y-1">
    {#each navItems as item}
      <button
        onclick={() => { 
          navigation.activeView = item.view as any;
          navigation.selectedPlaylistId = ''; // Clear selected playlist when clicking main nav
        }}
        class="flex items-center gap-3 py-2 transition-all w-full text-left {navigation.activeView === item.view && !navigation.selectedPlaylistId ? 'text-white font-bold translate-x-1' : 'text-zinc-400 hover:text-red-400'}"
      >
        <svelte:component this={item.icon} class="size-4 {navigation.activeView === item.view && !navigation.selectedPlaylistId ? 'text-red-500' : ''}" />
        <span class="text-sm">{item.label}</span>
      </button>
    {/each}
  </nav>

  <div class="mt-8 flex-1 flex flex-col overflow-hidden">
    <h3 class="px-3 text-[10px] uppercase tracking-widest text-zinc-500 font-bold mb-4">Playlists</h3>
    
    <div class="flex-1 overflow-y-auto no-scrollbar">
      <nav class="flex flex-col gap-y-0.5">
        <!-- Prominent Links -->
        <button 
          onclick={() => {
            navigation.activeView = 'library';
            navigation.selectedPlaylistId = '';
          }}
          class="flex items-center gap-3 px-3 py-2 transition-all w-full text-left {navigation.activeView === 'library' && !navigation.selectedPlaylistId ? 'text-white font-bold' : 'text-zinc-400 hover:text-red-400'}"
        >
          <ListMusic class="size-4 {navigation.activeView === 'library' && !navigation.selectedPlaylistId ? 'text-red-500' : ''}" />
          <span class="text-sm">All Playlists</span>
        </button>

        {#if favorites}
          <button 
            onclick={() => navigation.openPlaylist(favorites.id, 'library-playlists')}
            class="flex items-center gap-3 px-3 py-2 transition-all w-full text-left {navigation.selectedPlaylistId === favorites.id ? 'text-white font-bold' : 'text-zinc-400 hover:text-red-400'}"
          >
            <Heart class="size-4 {navigation.selectedPlaylistId === favorites.id ? 'text-red-500' : 'fill-red-500/10'}" />
            <span class="text-sm">Favorite Songs</span>
          </button>
        {/if}

        {#if library.isLoading && library.playlists.length === 0}
          <div class="px-3 py-2 flex items-center gap-2 text-zinc-500 text-xs">
            <Loader2 class="size-3 animate-spin" />
            <span>Loading...</span>
          </div>
        {:else}
          {#each otherPlaylists as pl}
            <button 
              onclick={() => navigation.openPlaylist(pl.id, 'library-playlists')} 
              class="px-3 py-2 text-sm text-left hover:text-white transition-colors truncate w-full {navigation.selectedPlaylistId === pl.id ? 'text-red-500 font-semibold' : 'text-zinc-400'}"
            >
              {pl.name}
            </button>
          {/each}
        {/if}
      </nav>
    </div>
  </div>
</aside>
