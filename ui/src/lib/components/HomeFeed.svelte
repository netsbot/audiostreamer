<script lang="ts">
  import { onMount } from "svelte";
  import Hls, { type Level } from "hls.js";
  import { invoke } from "@tauri-apps/api/core";
  import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
  import { fade, fly } from "svelte/transition";
  import Play from "lucide-svelte/icons/play";
  import MoreHorizontal from "lucide-svelte/icons/more-horizontal";
  import Loader2 from "lucide-svelte/icons/loader-2";
  import { playback } from "$lib/playback.svelte";

  let {
    openAlbum = (id: string) => {},
    openPlaylist = (id: string, type: string = "playlists") => {},
  } = $props();

  let recommendations = $state<any[]>([]);
  let resourceMap = $state<Record<string, any>>({});
  let isLoading = $state(true);
  let error = $state<string | null>(null);

  const heroWidthClass = $derived(playback.lyricsPaneOpen
    ? "lg:w-[calc((100%-4.5rem)/4)]"
    : "lg:w-[calc((100%-6rem)/5)]");

  const standardShelfWidth = $derived(playback.lyricsPaneOpen
    ? "shrink-0 w-44 lg:w-[calc((100%-6rem)/5)]"
    : "shrink-0 w-44 lg:w-[calc((100%-7.5rem)/6)]");

  function activateOnKey(event: KeyboardEvent, action: () => void) {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      action();
    }
  }

  function isAvcLevel(level: Level): boolean {
    const codecs = (level.codecs || "").toLowerCase();
    return codecs.includes("avc1") || codecs.includes("avc3");
  }

  function findScreenSizedAvcLevelIndex(levels: Level[], playerHeightCssPx: number): number {
    const avcLevels = levels
      .map((level, index) => ({
        index,
        height: level.height ?? 0,
        bitrate: level.bitrate ?? 0,
        avc: isAvcLevel(level),
      }))
      .filter((level) => level.avc)
      .sort((a, b) => (a.height - b.height) || (a.bitrate - b.bitrate));

    if (avcLevels.length === 0) return -1;

    const deviceScale = window.devicePixelRatio || 1;
    const targetHeight = Math.round(playerHeightCssPx * deviceScale * 1.15);

    // Keep a practical ceiling to avoid unstable playback on Linux/webkit.
    const capped = avcLevels.filter((level) => level.height <= 1080 && level.bitrate <= 5_000_000);
    const pool = capped.length > 0 ? capped : avcLevels;

    // Pick the first level that meets target height, otherwise the highest available.
    const atOrAboveTarget = pool.find((level) => level.height >= targetHeight);
    if (atOrAboveTarget) return atOrAboveTarget.index;

    return pool[pool.length - 1].index;
  }

  function bindHlsVideo(node: HTMLVideoElement, sourceUrl: string | null) {
    let hls: Hls | null = null;
    let resizeObserver: ResizeObserver | null = null;

    const onEnded = () => {
      // Fallback loop behavior for MSE/HLS edge-cases where `loop` isn't honored.
      node.currentTime = 0;
      void node.play().catch(() => {});
    };

    node.loop = true;
    node.addEventListener("ended", onEnded);

    const applyScreenSizedLevel = () => {
      if (!hls) return;
      const elementHeight = node.clientHeight || 720;
      const avcLevelIndex = findScreenSizedAvcLevelIndex(hls.levels, elementHeight);
      if (avcLevelIndex >= 0) {
        hls.currentLevel = avcLevelIndex;
        hls.nextLevel = avcLevelIndex;
        hls.loadLevel = avcLevelIndex;
      }
    };

    const detach = () => {
      if (resizeObserver) {
        resizeObserver.disconnect();
        resizeObserver = null;
      }
      if (hls) {
        hls.destroy();
        hls = null;
      }
      node.removeAttribute("src");
      node.load();
    };

    const attach = (url: string | null) => {
      detach();
      if (!url) return;

      const isHls = url.toLowerCase().includes(".m3u8");

      if (isHls && Hls.isSupported()) {
        hls = new Hls({
          enableWorker: true,
          lowLatencyMode: false,
          capLevelToPlayerSize: true,
        });

        hls.on(Hls.Events.MEDIA_ATTACHED, () => {
          hls?.loadSource(url);
        });

        hls.on(Hls.Events.MANIFEST_PARSED, () => {
          if (!hls) return;

          // Pick AVC quality based on current rendered size.
          applyScreenSizedLevel();

          void node.play().catch(() => {});
        });

        hls.on(Hls.Events.ERROR, (_, data) => {
          if (data.fatal && hls) {
            hls.destroy();
            hls = null;
          }
        });

        hls.attachMedia(node);

        resizeObserver = new ResizeObserver(() => {
          applyScreenSizedLevel();
        });
        resizeObserver.observe(node);
        return;
      }

      node.src = url;
      node.load();
    };

    attach(sourceUrl);

    return {
      update(nextUrl: string | null) {
        if (nextUrl !== sourceUrl) {
          sourceUrl = nextUrl;
          attach(sourceUrl);
        }
      },
      destroy() {
        node.removeEventListener("ended", onEnded);
        detach();
      },
    };
  }

  function getArtworkUrl(artwork: any, width = 1000, height = 1000, format = "webp") {
    if (!artwork || !artwork.url) return "";
    
    // Pure template replacement - no regex token forcing
    return artwork.url
      .replace("{w}", width.toString())
      .replace("{h}", height.toString())
      .replace("{f}", format)
      .replace("{c}", ""); // Remove the placeholder if it exists
  }

  function getArtworkSrcset(artwork: any, format = "webp", isPortrait = false) {
    if (!artwork || !artwork.url) return "";
    const widths = [450, 600, 900, 1200];
    return widths
      .map(w => {
        const h = isPortrait ? Math.round(w * 4 / 3) : w;
        return `${getArtworkUrl(artwork, w, h, format)} ${w}w`;
      })
      .join(", ");
  }

  function resolveResource(item: any): any {
    if (!item) return null;
    // If the item already has attributes, use it directly
    if (item.attributes) return item;
    // Otherwise look it up in the resource map
    const typeMap = resourceMap[item.type];
    if (typeMap && typeMap[item.id]) {
      return { ...item, attributes: typeMap[item.id].attributes, relationships: typeMap[item.id].relationships };
    }
    return item;
  }

  function resolveRelationshipContent(item: any, name: string): any {
    const relData = item.relationships?.[name]?.data;
    if (Array.isArray(relData) && relData.length > 0) {
      return resolveResource(relData[0]);
    }
    return null;
  }

  function resolveEditorialData(item: any): any {
    if (!item) return null;
    
    // Path A: item.attributes.plainEditorialCard[meta.editorialCard]
    // This is the "correct path" for vcard, radio-show, and other specifically mapped cards
    const edId = item.meta?.editorialCard;
    const plain = item.attributes?.plainEditorialCard;
    if (plain && edId && plain[edId]) {
      return plain[edId];
    }

    // Fallback: Path B: First entry in plainEditorialCard (extended resource itself)
    if (plain) {
      const firstEntry = Object.values(plain)[0];
      if (firstEntry) return firstEntry;
    }

    // Fallback: Path C: Linked ID to resourceMap['editorial-item']
    const map = resourceMap['editorial-items'] || resourceMap['editorial-item'];
    if (edId && map?.[edId]) {
      return map[edId];
    }

    return null;
  }

  function getShelfItems(rec: any): any[] {
    // 'contents' relationship holds the actual items; 'primary-content' is typically empty
    const contents = rec.relationships?.contents?.data
      || rec.relationships?.['primary-content']?.data
      || [];
    const items = contents.map(resolveResource).filter((item: any) => item.attributes);

    // Items resolved via resourceMap
    return items;
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

  function getHeroArtwork(item: any): any {
    if (!item) return null;

    const edData = resolveEditorialData(item);
    const attrs = item.attributes;
    const edAttrs = edData?.attributes || edData;

    // Strict priority for Portrait (3:4) assets for Hero layout
    const heroArt = 
        // 1. SuperHeroTall (Gold standard)
        edAttrs?.editorialArtwork?.superHeroTall || 
        attrs?.editorialArtwork?.superHeroTall ||
        
        // 2. Portrait fallback
        edAttrs?.editorialArtwork?.superHeroPortrait || 
        attrs?.editorialArtwork?.superHeroPortrait ||
        
        // 3. Wide Banners (SubscriptionHero) last 
        edAttrs?.editorialArtwork?.subscriptionHero || 
        attrs?.editorialArtwork?.subscriptionHero ||
        edAttrs?.editorialArtwork?.bannerUber ||
        attrs?.editorialArtwork?.bannerUber ||

        // 4. Default artwork
        edAttrs?.artwork || 
        attrs?.artwork;

    if (heroArt && (heroArt.url || heroArt.editorialArtwork)) return heroArt;
    return null;
  }

  function getVideoUrl(item: any): string | null {
    if (!item) return null;
    
    const edData = resolveEditorialData(item);
    const edAttrs = edData?.attributes || edData;
    const attrs = item.attributes;

    const video = 
        // 1. Tall Video Priority
        edAttrs?.editorialVideo?.motionTallVideo3x4?.video || 
        attrs?.editorialVideo?.motionTallVideo3x4?.video ||
        edAttrs?.editorialVideo?.motionDetailTall?.video ||
        attrs?.editorialVideo?.motionDetailTall?.video ||
        
        // 2. Square Video Fallback
        edAttrs?.editorialVideo?.motionSquareVideo1x1?.video || 
        attrs?.editorialVideo?.motionSquareVideo1x1?.video ||
        edAttrs?.editorialVideo?.motionDetailSquare?.video ||
        attrs?.editorialVideo?.motionDetailSquare?.video;
    
    if (video) return video;

    const child = resolveRelationshipContent(item, 'editorial-item') 
               || resolveRelationshipContent(item, 'contents')
               || resolveRelationshipContent(item, 'radio-show');
    
    if (child && child !== item) return getVideoUrl(child);

    return null;
  }

  function getEyebrow(item: any): string {
    if (!item) return "";
    
    const edData = resolveEditorialData(item);
    const edAttrs = edData?.attributes || edData;
    const metaReason = item.meta?.reason?.stringForDisplay;
    if (metaReason) return metaReason;

    const edEyebrow = edAttrs?.socialParagraph 
                   || edAttrs?.editorialNotes?.short;
    if (edEyebrow) return edEyebrow;

    const attrs = item.attributes;
    const eyebrow = attrs?.socialParagraph 
        || (attrs?.editorialNotes?.short ? attrs.editorialNotes.short : "");
    if (eyebrow) return eyebrow;

    const child = resolveRelationshipContent(item, 'editorial-item') 
               || resolveRelationshipContent(item, 'contents')
               || resolveRelationshipContent(item, 'radio-show');
    
    if (child && child !== item) return getEyebrow(child);

    return "";
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
      await playback.playSong(item.id, {
        title: item.attributes.name,
        artist: item.attributes.artistName || "",
        album: item.attributes.albumName || "",
        artwork_url: getArtworkUrl(item.attributes.artwork, 600),
        duration_ms: item.attributes.durationInMillis,
      });
    } else if (type === "stations") {
      await playback.playStation(item.id, {
        name: item.attributes.name || "Station",
        subtitle: item.attributes.editorialNotes?.short || item.attributes.description?.short || "",
        artwork_url: getArtworkUrl(item.attributes.artwork, 600),
      });
    } else if (type === "playlists" || type === "library-playlists") {
      openPlaylist(item.id, type);
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
        "include[albums]": "artists,editorial-item,editorial-notes,editorial-artwork,editorial-video",
        "include[library-playlists]": "catalog",
        "include[personal-recommendation]": "primary-content,contents",
        "include[stations]": "radio-show,editorial-item,editorial-notes,editorial-artwork,editorial-video",
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
      
      // Store the resource map for resolving references
      if (data.resources) {
        resourceMap = data.resources;
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

{#snippet HeroCard(item: any)}
  {@const resolved = resolveResource(item)}
  {@const artwork = getHeroArtwork(resolved)}
  {@const videoUrl = getVideoUrl(resolved)}
  {@const eyebrow = getEyebrow(resolved)}
  {(() => {
    if (resolved.attributes) {
      console.log(`[HeroCard Debug] ${getItemTitle(resolved)}:`, {
        artworkUrl: artwork?.url,
        finalUrl: getArtworkUrl(artwork, 600, 800),
        videoUrl,
        eyebrow
      });
    }
    return '';
  })()}
  {#if resolved.attributes}
    <div class="flex-shrink-0 w-[64vw] sm:w-[50vw] md:w-[36vw] {heroWidthClass} max-w-none snap-start group cursor-pointer text-left">
      <div 
        class="product-lockup relative rounded-2xl overflow-hidden aspect-[3/4] mb-3 border border-white/5 shadow-2xl transition-all duration-500 group-hover:border-white/20"
        style="background-color: #{artwork?.bgColor || '18181b'}; --artwork-bg-color: #{artwork?.bgColor || '18181b'};"
        role="button"
        tabindex="0"
        onclick={() => handleItemClick(resolved)}
        onkeydown={(e) => activateOnKey(e, () => handleItemClick(resolved))}
      >
        <!-- Background Artwork/Video -->
        <div class="artwork-component w-full h-full absolute inset-0 bg-[var(--artwork-bg-color)]">
           {#if videoUrl}
             <video 
               use:bindHlsVideo={videoUrl}
               class="w-full h-full object-cover opacity-0 transition-opacity duration-1000" 
               autoplay 
               muted 
               loop 
               playsinline
               onloadeddata={(e) => (e.currentTarget.style.opacity = '1')}
             ></video>
           {/if}
           <picture class="{videoUrl ? 'absolute inset-0 z-[-1]' : ''}">
             <source 
               type="image/webp" 
               srcset={getArtworkSrcset(artwork, 'webp', true)}
               sizes="(max-width: 1679px) 450px, 600px"
             />
             <img
               src={getArtworkUrl(artwork, 600, 800)}
               alt={getItemTitle(resolved)}
               class="w-full h-full object-cover"
               loading="lazy"
               decoding="async"
             />
           </picture>
        </div>

        <!-- Legibility Gradient -->
        <div class="absolute inset-0 bg-gradient-to-t from-black/90 via-black/20 to-transparent opacity-70 group-hover:opacity-100 transition-opacity duration-500"></div>
        
        <!-- Metadata Overlay -->
        <div class="absolute inset-x-0 bottom-0 p-6 flex flex-col justify-end pointer-events-none">
           {#if eyebrow}
             <p class="text-[10px] font-bold uppercase tracking-[0.15em] text-white/70 mb-1.5 drop-shadow-md">{eyebrow}</p>
           {/if}
           <h3 class="text-2xl font-black text-white line-clamp-2 leading-tight drop-shadow-lg">
             {getItemTitle(resolved)}
           </h3>
           <p class="text-white/70 text-sm font-medium mt-1 truncate drop-shadow-md">
             {getItemSubtitle(resolved)}
           </p>
        </div>

        <!-- Platter Play Button -->
        <div class="absolute bottom-6 right-6 opacity-0 group-hover:opacity-100 translate-y-3 group-hover:translate-y-0 transition-all duration-300">
          <button 
            class="w-12 h-12 flex items-center justify-center bg-white text-zinc-950 rounded-full shadow-2xl transition-colors pointer-events-auto"
            onclick={(e) => { e.stopPropagation(); handleItemClick(resolved); }}
            title="Play"
          >
            <Play class="size-6 fill-current translate-x-0.5" />
          </button>
        </div>
      </div>
    </div>
  {/if}
{/snippet}

{#snippet Card(item: any, widthClass: string = "w-44")}
  {@const resolved = resolveResource(item)}
  {@const artwork = getItemArtworkObject(resolved)}
  {#if resolved.attributes}
    <div class="group cursor-pointer text-left {widthClass}">
      <div 
        class="product-lockup relative {isRoundArtwork(resolved) ? 'rounded-full' : 'rounded-xl'} overflow-hidden aspect-square mb-3 border border-white/5 shadow-2xl transition-all duration-500 group-hover:border-white/20"
        style="background-color: #{artwork?.bgColor || '18181b'}; --artwork-bg-color: #{artwork?.bgColor || '18181b'}; --aspect-ratio: 1;"
        role="button"
        tabindex="0"
        onclick={() => handleItemClick(resolved)}
        onkeydown={(e) => activateOnKey(e, () => handleItemClick(resolved))}
      >
        <div class="artwork-component w-full h-full">
           <picture>
             <source 
               type="image/webp" 
               srcset={getArtworkSrcset(artwork, 'webp', false)}
               sizes="(max-width: 640px) 176px, (max-width: 1024px) 220px, 400px"
             />
             <source 
               type="image/jpeg" 
               srcset={getArtworkSrcset(artwork, 'jpg', false)}
               sizes="(max-width: 640px) 176px, (max-width: 1024px) 220px, 400px"
             />
             <img
               src={getArtworkUrl(artwork, 400, 400)}
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
                class="w-10 h-10 flex items-center justify-center bg-white text-zinc-950 rounded-full shadow-xl transition-colors"
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
        class="mt-4 px-6 py-2 bg-red-600 hover:bg-red-500 text-white rounded-full font-bold text-sm transition-colors"
        onclick={fetchRecommendations}
      >
        Retry
      </button>
    </div>
  {:else}

    <!-- Render each recommendation shelf -->
    {#each recommendations as rec, shelfIndex}
      {@const items = getShelfItems(rec)}
      {@const title = getShelfTitle(rec)}
      {@const subtitle = getShelfSubtitle(rec)}
      {@const kind = rec.attributes?.display?.kind || ""}
      {@const isRecentlyPlayedShelf = title.toLowerCase().includes("recently played")}
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

          <!-- Hero Shelves (Top Picks etc.) -->
          {#if kind === "MusicNotesHeroShelf" || kind === "MusicSuperHeroShelf"}
            <div class="flex gap-6 overflow-x-auto no-scrollbar pb-4 snap-x snap-mandatory">
              {#each items as item}
                {@render HeroCard(item)}
              {/each}
            </div>

          <!-- Circle Cover Shelf (artists etc.) -->
          {:else if kind === "MusicCircleCoverShelf"}
            <div class="flex gap-7 overflow-x-auto no-scrollbar pb-4">
              {#each items as item}
                {@render Card(item, standardShelfWidth)}
              {/each}
            </div>

          <!-- Cover Grid — 2-column grid -->
          {:else if kind === "MusicCoverGrid"}
            <div class="flex gap-6 overflow-x-auto no-scrollbar pb-4 snap-x snap-mandatory">
              {#each items.slice(0, 8) as item}
                {@render Card(item, standardShelfWidth)}
              {/each}
            </div>

          <!-- Default: Horizontal Cover Shelf (most common) -->
          {:else}
            <div class="flex gap-5 overflow-x-auto no-scrollbar pb-4 snap-x snap-mandatory">
              {#each items as item}
                {@render Card(item, standardShelfWidth)}
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
