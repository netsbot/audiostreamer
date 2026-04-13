<script lang="ts">
  import Search from "lucide-svelte/icons/search";

  let { 
    searchQuery = $bindable(""), 
    isSearching = false, 
    handleSearch = () => {} 
  } = $props();
</script>

<header class="mb-12 flex items-end justify-between">
  <div>
    <h1 class="text-5xl font-black tracking-tighter mb-2">Listen Now</h1>
    <p class="text-zinc-500 font-medium">
      Curated daily for the refined ear.
    </p>
  </div>

  <!-- Search Bar -->
  <div class="relative w-80">
    <Search
      class="absolute left-3 top-1/2 -translate-y-1/2 size-4 text-zinc-500"
    />
    <input
      type="text"
      bind:value={searchQuery}
      onkeydown={(e) => e.key === "Enter" && handleSearch()}
      placeholder="Search Apple Music..."
      class="w-full bg-zinc-900/50 border border-white/5 rounded-full py-2 pl-10 pr-4 text-sm focus:outline-none focus:ring-1 focus:ring-red-500/50 transition-all placeholder:text-zinc-600"
    />
    {#if isSearching}
      <div class="absolute right-3 top-1/2 -translate-y-1/2">
        <div
          class="size-3 border-2 border-red-500/50 border-t-red-500 rounded-full animate-spin"
        ></div>
      </div>
    {/if}
  </div>
</header>
