<script lang="ts">
  import Play from "lucide-svelte/icons/play";
  import ListMusic from "lucide-svelte/icons/list-music";
  import Video from "lucide-svelte/icons/video";
  import Radio from "lucide-svelte/icons/radio";
  import Search from "lucide-svelte/icons/search";
  import { fly } from "svelte/transition";
  import { playback, type QueueTrack } from "$lib/playback.svelte";
  import { library } from "$lib/library.svelte";
  import Heart from "lucide-svelte/icons/heart";
  import { snapShelf } from "$lib/actions/snapShelf";

  let { 
    searchResults, 
    openAlbum = (id: string) => {}, 
    openPlaylist = (id: string, type: string = "playlists") => {},
    clearSearch = () => {},
    getArtworkUrl = (artwork: any, size?: number) => artwork?.url 
  } = $props();

  function artworkSrc(artwork: any, size: number) {
    if (!artwork) return "";
    if (typeof artwork === "string") {
      return artwork
        .replace("{w}", size.toString())
        .replace("{h}", size.toString())
        .replace("{f}", "webp")
        .replace("{c}", "");
    }
    return getArtworkUrl(artwork, size);
  }

  function buildSongQueue(items: any[]): QueueTrack[] {
    return items.map((song) => ({
      id: song.id,
      metadata: {
        title: song.attributes.name,
        artist: song.attributes.artistName,
        album: song.attributes.albumName || "",
        artwork_url: getArtworkUrl(song.attributes.artwork, 600),
        duration_ms: song.attributes.durationInMillis,
      },
    }));
  }

  async function playSong(item: any, queueSource: any[] = []) {
    console.log("playing song:", item.id);
    const queue = buildSongQueue(queueSource.length > 0 ? queueSource : [item]);
    const startIndex = queue.findIndex((track) => track.id === item.id);
    await playback.playSong(item.id, {
        title: item.attributes.name,
        artist: item.attributes.artistName,
        album: item.attributes.albumName || "",
        artwork_url: getArtworkUrl(item.attributes.artwork, 600),
        duration_ms: item.attributes.durationInMillis
    }, {
        queue,
        startIndex,
    });
  }

  async function playStationItem(station: any) {
    await playback.playStation(station.id, {
      name: station.attributes?.name || "Station",
      subtitle: station.attributes?.editorialNotes?.short || station.attributes?.description?.short || "",
      artwork_url: artworkSrc(station.attributes?.artwork, 600),
    });
  }

  const shelfItemWidth = "shrink-0 w-44 w-[var(--snap-item-width)] last:mr-6 snap-start snap-always";
</script>

<div
  class="text-white"
  in:fly={{ y: 20, duration: 400 }}
>
  <div class="flex items-center justify-between mb-8">
    <h2 class="text-3xl font-black tracking-tighter">Search Results</h2>
    <button
      class="text-[10px] font-bold text-zinc-500 uppercase tracking-widest hover:text-red-500 transition-colors"
      onclick={() => clearSearch()}>Clear Results</button
    >
  </div>

  <!-- Official Top Result -->
  {#if searchResults.top.length > 0}
    <section class="mb-12">
      <h3 class="text-xl font-bold mb-6 text-white/90">Top Results</h3>
      <div class="flex gap-6 overflow-x-auto no-scrollbar pb-4 snap-x snap-mandatory" use:snapShelf>
        {#each searchResults.top.slice(0, 6) as item}
          <button
            type="button"
            class="{shelfItemWidth} group cursor-pointer transition-all duration-300 text-left snap-start"
            onclick={() => {
              if (item.type === 'songs') {
                playSong(item, searchResults.songs || []);
              } else if (item.type === 'playlists' || item.type === 'library-playlists') {
                openPlaylist(item.id, item.type);
              } else {
                openAlbum(item.id);
              }
            }}
          >
            <div
              class="{item.type === 'artists'
                ? 'rounded-full'
                : 'rounded-xl'} overflow-hidden aspect-square mb-3 shadow-2xl relative border border-white/5 bg-zinc-900 transition-all duration-300 group-hover:border-white/20"
            >
              <img
                src={artworkSrc(item.attributes.artwork, 400)}
                class="w-full h-full object-cover"
                alt={item.attributes.name}
              />
              <div
                class="absolute inset-0 bg-black/35 flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity duration-300 pointer-events-none"
              >
                <div class="w-12 h-12 rounded-full bg-white/12 backdrop-blur-md border border-white/25 shadow-xl flex items-center justify-center opacity-0 group-hover:opacity-100 scale-95 group-hover:scale-100 transition-[opacity,transform] duration-200 will-change-[opacity,transform,backdrop-filter]">
                  <Play class="size-6 text-white fill-current translate-x-0.5" />
                </div>
              </div>
            </div>
            <div>
              <h4 class="font-bold text-white text-[13px] truncate">
                {item.attributes.name}
              </h4>
              <p class="text-zinc-500 text-[11px] truncate uppercase tracking-widest mt-0.5">
                {item.attributes.artistName || (item.type === "artists" ? "Artist" : "")}
              </p>
            </div>
          </button>
        {/each}
      </div>
    </section>
  {/if}

  <!-- Artists Section -->
  {#if searchResults.artists.length > 0}
    <section class="mb-12">
      <div class="flex justify-between items-end mb-6">
        <h3 class="text-xl font-bold text-white/90">Artists</h3>
        <button class="text-red-500 font-bold text-[10px] uppercase tracking-wider hover:text-red-400">See All</button>
      </div>
      <div class="flex gap-8 overflow-x-auto no-scrollbar pb-4 snap-x snap-mandatory" use:snapShelf>
        {#each searchResults.artists as artist}
          <div
            class="{shelfItemWidth} flex flex-col items-center group cursor-pointer text-center snap-start"
          >
            <div
              class="w-full aspect-square rounded-full overflow-hidden mb-3 border border-white/5 shadow-2xl bg-zinc-900 transition-all duration-300 group-hover:border-white/20"
            >
              {#if artist.attributes.artwork}
                <img
                  class="w-full h-full object-cover"
                  src={getArtworkUrl(artist.attributes.artwork, 400)}
                  alt={artist.attributes.name}
                />
              {:else}
                <div class="w-full h-full flex items-center justify-center bg-zinc-800">
                  <Search class="size-8 text-zinc-600" />
                </div>
              {/if}
            </div>
            <span class="font-bold text-white text-xs truncate w-full group-hover:text-red-500 transition-colors">
              {artist.attributes.name}
            </span>
          </div>
        {/each}
      </div>
    </section>
  {/if}

  <!-- Albums Section -->
  {#if searchResults.albums.length > 0}
    <section class="mb-12">
      <div class="flex justify-between items-end mb-6">
        <h3 class="text-xl font-bold text-white/90">Albums</h3>
        <button class="text-red-500 font-bold text-[10px] uppercase tracking-wider hover:text-red-400">See All</button>
      </div>
      <div class="flex gap-6 overflow-x-auto no-scrollbar pb-4 snap-x snap-mandatory" use:snapShelf>
        {#each searchResults.albums as album}
          <button
            type="button"
            class="{shelfItemWidth} group cursor-pointer text-left snap-start"
            onclick={() => openAlbum(album.id)}
          >
            <div
              class="aspect-square rounded-xl overflow-hidden mb-3 relative border border-white/5 shadow-2xl bg-zinc-900 group-hover:border-white/20 transition-all"
            >
              <img
                class="w-full h-full object-cover"
                src={getArtworkUrl(album.attributes.artwork, 400)}
                alt={album.attributes.name}
              />
              <div
                class="absolute inset-0 bg-black/25 opacity-0 group-hover:opacity-100 flex items-center justify-center transition-opacity duration-300 pointer-events-none"
              >
                <div class="w-11 h-11 rounded-full bg-white/12 backdrop-blur-md border border-white/25 shadow-xl flex items-center justify-center opacity-0 group-hover:opacity-100 scale-95 group-hover:scale-100 transition-[opacity,transform] duration-200 will-change-[opacity,transform,backdrop-filter]">
                  <Play class="size-5 text-white fill-current translate-x-0.5" />
                </div>
              </div>
            </div>
            <h5 class="font-bold text-white text-[13px] truncate group-hover:text-red-500 transition-colors">
              {album.attributes.name}
            </h5>
            <p class="text-zinc-500 text-[11px] truncate">
              {album.attributes.artistName}
            </p>
          </button>
        {/each}
      </div>
    </section>
  {/if}

  <!-- Songs Section -->
  {#if searchResults.songs.length > 0}
    <section class="mb-12">
      <h3 class="text-xl font-bold mb-6 text-white/90">Songs</h3>
      <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
        {#each searchResults.songs.slice(0, 6) as song}
          <div
            role="button"
            tabindex="0"
            class="bg-white/[0.03] backdrop-blur-3xl p-2 rounded-lg flex items-center gap-3 group hover:bg-white/[0.08] cursor-pointer border border-white/5 transition-all duration-200 text-left"
            onclick={() => playSong(song, searchResults.songs || [])}
            onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && playSong(song, searchResults.songs || [])}
          >
            <div class="w-12 h-12 rounded-md overflow-hidden flex-shrink-0 shadow-lg relative border border-white/5">
              <img
                class="w-full h-full object-cover"
                src={getArtworkUrl(song.attributes.artwork, 200)}
                alt={song.attributes.name}
              />
              <div class="absolute inset-0 bg-black/40 flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity">
                <Play class="size-4 text-white fill-current translate-x-0.5" />
              </div>
            </div>
            <div class="flex-grow min-w-0">
              <h4 class="font-bold text-[13px] truncate group-hover:text-red-500 transition-colors">
                {song.attributes.name}
              </h4>
              <p class="text-[10px] text-zinc-500 truncate">
                {song.attributes.artistName}
              </p>
            </div>
            <button
              class="opacity-0 group-hover:opacity-100 p-2 text-zinc-500 hover:text-red-500 transition-all"
              onclick={(e) => {
                e.stopPropagation();
                library.toggleFavorite(song.id);
              }}
            >
              <Heart 
                class="size-3.5 {library.isFavorite(song.id) ? 'text-red-500 fill-red-500 opacity-100' : ''}" 
              />
            </button>
          </div>
        {/each}
      </div>
    </section>
  {/if}

  <!-- Playlists, Videos, Stations... (Keeping the rest for completeness) -->
  {#if searchResults.playlists.length > 0}
    <section class="mb-12">
      <div class="flex justify-between items-end mb-6">
        <h3 class="text-xl font-bold text-white/90 flex items-center gap-2">
          <ListMusic class="size-5 text-red-500" /> Playlists
        </h3>
        <button class="text-zinc-500 font-bold text-[10px] uppercase tracking-wider hover:text-white transition-colors">See All</button>
      </div>
      <div class="flex gap-6 overflow-x-auto no-scrollbar pb-4 snap-x snap-mandatory" use:snapShelf>
        {#each searchResults.playlists as playlist}
          <button
            type="button"
            class="{shelfItemWidth} group cursor-pointer text-left snap-start"
            onclick={() => openPlaylist(playlist.id, playlist.type || 'playlists')}
          >
            <div class="aspect-square rounded-xl overflow-hidden mb-3 relative border border-white/5 shadow-2xl bg-zinc-900 group-hover:border-white/20 transition-all">
              <img
                class="w-full h-full object-cover"
                src={getArtworkUrl(playlist.attributes.artwork, 400)}
                alt={playlist.attributes.name}
              />
            </div>
            <h5 class="font-bold text-white text-[13px] truncate group-hover:text-red-500 transition-colors">
              {playlist.attributes.name}
            </h5>
            <p class="text-zinc-500 text-[11px] truncate">Apple Music</p>
          </button>
        {/each}
      </div>
    </section>
  {/if}

  {#if searchResults.musicVideos.length > 0}
    <section class="mb-12">
      <div class="flex justify-between items-end mb-6">
        <h3 class="text-xl font-bold text-white/90 flex items-center gap-2">
          <Video class="size-5 text-red-500" /> Music Videos
        </h3>
        <button class="text-zinc-500 font-bold text-[10px] uppercase tracking-wider hover:text-white transition-colors">See All</button>
      </div>
      <div class="flex gap-6 overflow-x-auto no-scrollbar pb-4 snap-x snap-mandatory" use:snapShelf>
        {#each searchResults.musicVideos as mv}
          <div class="{shelfItemWidth} group cursor-pointer snap-start">
            <div class="aspect-video rounded-xl overflow-hidden mb-3 relative border border-white/5 shadow-2xl bg-zinc-900 group-hover:border-white/20 transition-all">
              <img
                class="w-full h-full object-cover"
                src={artworkSrc(mv.attributes.artwork, 600)}
                alt={mv.attributes.name}
              />
              <div class="absolute inset-0 bg-black/40 opacity-0 group-hover:opacity-100 flex items-center justify-center transition-opacity">
                <Play class="size-8 text-white fill-current translate-x-1" />
              </div>
            </div>
            <div class="px-1">
              <h5 class="font-bold text-[13px] truncate group-hover:text-red-500 transition-colors">
                {mv.attributes.name}
              </h5>
              <p class="text-zinc-500 text-[11px] truncate">{mv.attributes.artistName}</p>
            </div>
          </div>
        {/each}
      </div>
    </section>
  {/if}

  {#if searchResults.stations.length > 0}
    <section class="mb-16">
      <div class="flex justify-between items-end mb-6">
        <h3 class="text-xl font-bold text-white/90 flex items-center gap-2">
          <Radio class="size-5 text-red-500" /> Radio Stations
        </h3>
        <button class="text-zinc-500 font-bold text-[10px] uppercase tracking-wider hover:text-white transition-colors">See All</button>
      </div>
      <div class="flex gap-8 overflow-x-auto no-scrollbar pb-6 snap-x snap-mandatory" use:snapShelf>
        {#each searchResults.stations as station}
          <button
            type="button"
            class="{shelfItemWidth} flex flex-col group cursor-pointer text-center snap-start"
            onclick={() => playStationItem(station)}
          >
            <div class="w-full aspect-square rounded-xl overflow-hidden mb-4 border border-white/5 shadow-2xl relative bg-zinc-900 group-hover:border-white/20 transition-all">
              {#if station.attributes.artwork}
                <img
                  class="w-full h-full object-cover"
                  src={artworkSrc(station.attributes.artwork, 400)}
                  alt={station.attributes.name}
                />
              {:else}
                <div class="w-full h-full flex items-center justify-center bg-zinc-800">
                  <Radio class="size-10 text-zinc-600" />
                </div>
              {/if}
              <div class="absolute inset-0 bg-red-600/10 opacity-0 group-hover:opacity-100 transition-opacity"></div>
            </div>
            <div class="px-2 text-white">
              <span class="font-bold text-[13px] line-clamp-2 w-full group-hover:text-red-500 transition-colors">
                {station.attributes.name}
              </span>
            </div>
          </button>
        {/each}
      </div>
    </section>
  {/if}
</div>
