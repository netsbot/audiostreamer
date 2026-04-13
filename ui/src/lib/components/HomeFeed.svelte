<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
  import { fade, fly } from "svelte/transition";
  import Play from "lucide-svelte/icons/play";
  import MoreHorizontal from "lucide-svelte/icons/more-horizontal";
  import Loader2 from "lucide-svelte/icons/loader-2";
  import { playSong } from "$lib/playbackStore";

  let { openAlbum = (id: string) => {} } = $props();

  let recommendations = $state<any[]>([]);
  let resourceMap = $state<Record<string, any>>({});
  let isLoading = $state(true);
  let error = $state<string | null>(null);

  function getArtworkUrl(artwork: any, size = 1000, format = "webp") {
    if (!artwork || !artwork.url) return "";
    return artwork.url
      .replace("{w}", size.toString())
      .replace("{h}", size.toString())
      .replace("{f}", format)
      .replace("{c}", "SC")
      .replace(/SH\./g, "SC.") // Global replacement of SH crop if followed by a dot
      .replace(/SH\?/g, "SC?"); // Same if followed by query params
  }

  function getArtworkSrcset(artwork: any, format = "webp") {
    if (!artwork || !artwork.url) return "";
    const sizes = [400, 800, 1200, 1600, 2000];
    return sizes
      .map(s => `${getArtworkUrl(artwork, s, format)} ${s}w`)
      .join(", ");
  }

  function resolveResource(item: any): any {
    // If the item already has attributes, use it directly
    if (item.attributes) return item;
    // Otherwise look it up in the resource map
    const typeMap = resourceMap[item.type];
    if (typeMap && typeMap[item.id]) {
      return { ...item, attributes: typeMap[item.id].attributes, relationships: typeMap[item.id].relationships };
    }
    return item;
  }

  function getShelfItems(rec: any): any[] {
    // 'contents' relationship holds the actual items; 'primary-content' is typically empty
    const contents = rec.relationships?.contents?.data
      || rec.relationships?.['primary-content']?.data
      || [];
    return contents.map(resolveResource).filter((item: any) => item.attributes);
  }

  function getShelfTitle(rec: any): string {
    return rec.attributes?.title?.stringForDisplay || rec.attributes?.title || rec.attributes?.name || "For You";
  }

  function getShelfSubtitle(rec: any): string | null {
    return rec.attributes?.subtitle?.stringForDisplay || rec.attributes?.subtitle || null;
  }

  function getItemArtworkObject(item: any): any {
    const attrs = item.attributes;
    if (!attrs) return null;
    return attrs.artwork;
  }

  function getItemTitle(item: any): string {
    return item.attributes?.name || item.attributes?.title || "";
  }

  function getItemSubtitle(item: any): string {
    const attrs = item.attributes;
    if (!attrs) return "";
    return attrs.artistName
      || (Array.isArray(attrs.artistNames) ? attrs.artistNames.join(", ") : attrs.artistNames)
      || attrs.curatorName
      || attrs.description?.short
      || "";
  }

  function isRoundArtwork(item: any): boolean {
    return item.type === "artists" || item.type === "social-profiles";
  }

  async function handleItemClick(item: any) {
    const type = item.type;
    if (type === "albums" || type === "library-albums") {
      openAlbum(item.id);
    } else if (type === "songs") {
      await playSong(item.id, {
        title: item.attributes.name,
        artist: item.attributes.artistName || "",
        album: item.attributes.albumName || "",
        artwork_url: getArtworkUrl(item.attributes.artwork, 600),
        duration_ms: item.attributes.durationInMillis,
      });
    } else if (type === "playlists" || type === "library-playlists") {
      // Could navigate to playlist view in the future — for now open as album-like
      openAlbum(item.id);
    }
  }

  async function fetchRecommendations() {
    isLoading = true;
    error = null;
    try {
      const devToken = (await invoke("get_apple_music_token")) as string;
      const userToken = (await invoke("get_apple_music_user_token")) as string;

      const params = new URLSearchParams({
        "art[url]": "f",
        "displayFilter[kind]": "MusicCircleCoverShelf,MusicConcertsEmptyShelf,MusicCoverGrid,MusicCoverShelf,MusicNotesHeroShelf,MusicSocialCardShelf,MusicSuperHeroShelf",
        "extend": "editorialArtwork,editorialVideo,plainEditorialCard,plainEditorialNotes",
        "extend[playlists]": "artistNames",
        "extend[stations]": "airTime,supportsAirTimeUpdates",
        "fields[artists]": "name,artwork,url",
        "format[resources]": "map",
        "include[albums]": "artists",
        "include[library-playlists]": "catalog",
        "include[personal-recommendation]": "primary-content",
        "include[stations]": "radio-show",
        "l": "en-GB",
        "meta[stations]": "inflectionPoints",
        "name": "listen-now",
        "omit[resource]": "autos",
        "platform": "web",
        "timezone": "+08:00",
        "types": "activities,albums,apple-curators,artists,concerts,curators,editorial-items,library-albums,library-playlists,music-movies,music-videos,playlists,social-profiles,social-upsells,songs,stations,tv-episodes,tv-shows,uploaded-audios,uploaded-videos",
        "with": "friendsMix,library,social",
      });

      const url = `https://amp-api.music.apple.com/v1/me/recommendations?${params.toString()}`;

      const response = await tauriFetch(url, {
        method: "GET",
        headers: {
          Authorization: `Bearer ${devToken}`,
          "media-user-token": userToken,
          "User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36",
          "sec-ch-ua-platform": '"Linux"',
          Referer: "https://beta.music.apple.com/",
          Origin: "https://beta.music.apple.com",
        },
      });

      if (!response.ok) {
        throw new Error(`API returned ${response.status}`);
      }

      const data = await response.json();
      console.log("Recommendations raw response keys:", Object.keys(data));

      // Store the resource map for resolving references
      if (data.resources) {
        resourceMap = data.resources;
        console.log("Resource map types:", Object.keys(data.resources));
        for (const [type, map] of Object.entries(data.resources)) {
          console.log(`  ${type}: ${Object.keys(map as any).length} items`);
        }
      }

      // With format[resources]=map, data[] contains bare stubs {type, id}.
      // The actual recommendation objects live in resources['personal-recommendation'][id].
      // We need to resolve them first.
      const rawRecs = data.data || [];
      recommendations = rawRecs.map((stub: any) => {
        const resolved = resolveResource(stub);
        return resolved;
      });

      console.log(`Found ${recommendations.length} recommendation shelves`);
    } catch (e: any) {
      console.error("Failed to fetch recommendations:", e);
      error = e.message || "Failed to load recommendations";
    } finally {
      isLoading = false;
    }
  }

  onMount(() => {
    fetchRecommendations();
  });
</script>

{#snippet Card(item: any)}
  {@const resolved = resolveResource(item)}
  {@const artwork = getItemArtworkObject(resolved)}
  {#if resolved.attributes}
    <div class="flex-shrink-0 w-44 group cursor-pointer text-left">
      <div 
        class="product-lockup relative {isRoundArtwork(resolved) ? 'rounded-full' : 'rounded-xl'} overflow-hidden aspect-square mb-3 border border-white/5 shadow-2xl transition-all duration-500 group-hover:border-white/20"
        style="background-color: #{artwork?.bgColor || '18181b'}; --artwork-bg-color: #{artwork?.bgColor || '18181b'}; --aspect-ratio: 1;"
        onclick={() => handleItemClick(resolved)}
      >
        <div class="artwork-component w-full h-full">
           <picture>
             <source 
               type="image/webp" 
               srcset={getArtworkSrcset(artwork, 'webp')}
               sizes="(max-width: 640px) 176px, (max-width: 1024px) 220px, 400px"
             />
             <source 
               type="image/jpeg" 
               srcset={getArtworkSrcset(artwork, 'jpg')}
               sizes="(max-width: 640px) 176px, (max-width: 1024px) 220px, 400px"
             />
             <img
               src={getArtworkUrl(artwork, 1000)}
               alt={getItemTitle(resolved)}
               class="w-full h-full object-cover"
               loading="lazy"
               decoding="async"
             />
           </picture>
        </div>
        
        <!-- Controls Overlay -->
        <div class="absolute inset-0 bg-black/30 opacity-0 group-hover:opacity-100 transition-all duration-300 flex items-center justify-center">
           <div class="flex items-center gap-2 translate-y-4 group-hover:translate-y-0 transition-transform duration-300">
              <button 
                class="w-10 h-10 flex items-center justify-center bg-white text-zinc-950 rounded-full shadow-xl hover:scale-110 active:scale-95 transition-all"
                onclick={(e) => { e.stopPropagation(); handleItemClick(resolved); }}
                title="Play"
              >
                <Play class="size-5 fill-current translate-x-0.5" />
              </button>
              <button 
                class="w-10 h-10 flex items-center justify-center bg-black/40 backdrop-blur-md text-white rounded-full border border-white/10 hover:bg-black/60 transition-all"
                onclick={(e) => { e.stopPropagation(); }}
                title="More"
              >
                <MoreHorizontal class="size-5" />
              </button>
           </div>
        </div>
      </div>
      
      <div class="product-lockup__content">
        <h4 class="font-bold text-white text-[13px] truncate group-hover:text-red-500 transition-colors">
          {getItemTitle(resolved)}
        </h4>
        <p class="text-zinc-500 text-[11px] truncate mt-0.5 font-medium">
          {getItemSubtitle(resolved)}
        </p>
      </div>
    </div>
  {/if}
{/snippet}

<div in:fade={{ duration: 400 }}>
  {#if isLoading}
    <div class="flex flex-col items-center justify-center h-[60vh] gap-4" in:fade>
      <Loader2 class="size-10 text-red-500 animate-spin" />
      <p class="text-zinc-500 font-medium animate-pulse">Loading your recommendations...</p>
    </div>
  {:else if error}
    <div class="flex flex-col items-center justify-center h-[60vh] gap-4 text-center" in:fade>
      <p class="text-zinc-400 text-lg font-semibold">Could not load recommendations</p>
      <p class="text-zinc-600 text-sm max-w-md">{error}</p>
      <button
        class="mt-4 px-6 py-2 bg-red-600 hover:bg-red-500 text-white rounded-full font-bold text-sm transition-all hover:scale-105 active:scale-95"
        onclick={fetchRecommendations}
      >
        Retry
      </button>
    </div>
  {:else}
    <!-- Greeting -->
    <div class="mb-10" in:fly={{ y: 20, duration: 400 }}>
      <h1 class="text-4xl font-black tracking-tighter text-white">Listen Now</h1>
      <p class="text-zinc-500 text-sm mt-1 font-medium">Curated for you</p>
    </div>

    <!-- Render each recommendation shelf -->
    {#each recommendations as rec, shelfIndex}
      {@const items = getShelfItems(rec)}
      {@const title = getShelfTitle(rec)}
      {@const subtitle = getShelfSubtitle(rec)}
      {@const kind = rec.attributes?.display?.kind || ""}
      {#if items.length > 0}
        <section
          class="mb-14"
          in:fly={{ y: 20, duration: 400, delay: Math.min(shelfIndex * 60, 400) }}
        >
          <!-- Shelf Header -->
          <div class="flex items-end justify-between mb-5">
            <div>
              <h2 class="text-xl font-bold tracking-tight text-white">{title}</h2>
              {#if subtitle}
                <p class="text-zinc-500 text-xs mt-0.5">{subtitle}</p>
              {/if}
            </div>
            {#if items.length > 8}
              <button class="text-red-500 font-bold text-[10px] uppercase tracking-widest hover:text-red-400 transition-colors">
                See All
              </button>
            {/if}
          </div>

          <!-- Circle Cover Shelf (artists etc.) -->
          {#if kind === "MusicCircleCoverShelf"}
            <div class="flex gap-7 overflow-x-auto no-scrollbar pb-4">
              {#each items as item}
                {@render Card(item)}
              {/each}
            </div>

          <!-- Cover Grid — 2-column grid -->
          {:else if kind === "MusicCoverGrid"}
            <div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-6">
              {#each items.slice(0, 8) as item}
                {@render Card(item)}
              {/each}
            </div>

          <!-- Default: Horizontal Cover Shelf (most common) -->
          {:else}
            <div class="flex gap-5 overflow-x-auto no-scrollbar pb-4">
              {#each items as item}
                {@render Card(item)}
              {/each}
            </div>
          {/if}
        </section>
      {/if}
    {/each}

    <!-- Spacer for playback bar -->
    <div class="h-20"></div>
  {/if}
</div>
