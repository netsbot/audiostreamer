<script lang="ts">
  import Home from "lucide-svelte/icons/home";
  import ListMusic from "lucide-svelte/icons/list-music";
  import Heart from "lucide-svelte/icons/heart";
  import Loader2 from "lucide-svelte/icons/loader-2";
  import { onMount } from "svelte";
  import { library } from "$lib/library.svelte";
  import { navigation } from "$lib/navigation.svelte";
  import { search } from "$lib/search.svelte";
  import Search from "lucide-svelte/icons/search";

  const navItems = [{ icon: Home, label: "Home", view: "home" }];

  onMount(() => {
    library.fetchPlaylists();
    library.fetchFavorites();
  });

  const favorites = $derived(
    library.playlists.find(
      (p) =>
        p.name.toLowerCase() === "favorite songs" ||
        p.name.toLowerCase() === "favorites" ||
        p.name.toLowerCase() === "favourite songs",
    ),
  );

  const otherPlaylists = $derived(
    library.playlists.filter((p) => p.id !== favorites?.id),
  );
</script>

<aside
  class="relative h-full w-56 shrink-0 border-r border-white/5 bg-zinc-900/50 flex flex-col py-8 px-5 gap-y-4"
>
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
  </div>

  <nav class="flex flex-col gap-y-1">
    {#each navItems as item}
      <div
        role="button"
        tabindex="0"
        onclick={() => {
          navigation.activeView = item.view as any;
          navigation.selectedPlaylistId = ""; // Clear selected playlist when clicking main nav
        }}
        onkeydown={(e) =>
          (e.key === "Enter" || e.key === " ") &&
          (navigation.activeView = item.view as any)}
        class="flex items-center gap-3 px-3 py-2 transition-colors w-full text-left {navigation.activeView ===
          item.view && !navigation.selectedPlaylistId
          ? 'text-white font-bold'
          : 'text-zinc-400 hover:text-red-400'} cursor-pointer"
      >
        <item.icon
          class="size-4 {navigation.activeView === item.view &&
          !navigation.selectedPlaylistId
            ? 'text-red-500'
            : ''}"
        />
        <span class="text-sm">{item.label}</span>
      </div>
    {/each}
  </nav>

  <div class="mt-8 flex-1 flex flex-col overflow-hidden">
    <h3
      class="px-3 text-[10px] uppercase tracking-widest text-zinc-500 font-bold mb-4"
    >
      Playlists
    </h3>

    <div class="flex-1 overflow-y-auto no-scrollbar">
      <nav class="flex flex-col gap-y-0.5">
        {#if favorites}
          <button
            onclick={() =>
              navigation.openPlaylist(favorites.id, "library-playlists")}
            class="flex items-center gap-3 px-3 py-2 transition-colors w-full text-left {navigation.selectedPlaylistId ===
            favorites.id
              ? 'text-white font-bold'
              : 'text-zinc-400 hover:text-red-400'}"
          >
            <Heart
              class="size-4 {navigation.selectedPlaylistId === favorites.id
                ? 'text-red-500'
                : 'fill-red-500/10'}"
            />
            <span class="text-sm">Favorite Songs</span>
          </button>
        {/if}

        {#each otherPlaylists as pl}
          <button
            onclick={() => navigation.openPlaylist(pl.id, "library-playlists")}
            class="px-3 py-2 text-sm text-left hover:text-white transition-colors truncate w-full {navigation.selectedPlaylistId ===
            pl.id
              ? 'text-red-500 font-semibold'
              : 'text-zinc-400'}"
          >
            {pl.name}
          </button>
        {/each}
      </nav>
    </div>
  </div>
</aside>
