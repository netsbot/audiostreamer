<script lang="ts">
  import Maximize2 from 'lucide-svelte/icons/maximize-2';

  const lyrics = [
    { text: 'Waiting in a car', focus: false },
    { text: 'Waiting for a ride in the dark', focus: false },
    { text: 'The night city grows', focus: true },
    { text: 'Look at the horizon line', focus: true },
    { text: 'The wind blows', focus: false },
    { text: 'In my hair', focus: false },
    { text: 'I\'m going to town', focus: false },
    { text: 'Searching for a place to go', focus: false },
    { text: 'A place to go', focus: false },
  ];
</script>

<aside class="fixed right-0 top-0 h-[calc(100vh-80px)] w-[400px] bg-zinc-900/80 backdrop-blur-3xl border-l border-white/5 p-12 overflow-y-auto no-scrollbar">
  <div class="flex items-center justify-between mb-12">
    <h2 class="text-xs font-black uppercase tracking-[0.2em] text-zinc-500">Lyrics</h2>
    <button class="text-zinc-500 hover:text-white">
      <Maximize2 class="size-4" />
    </button>
  </div>

  <div class="space-y-8 pb-32">
    {#each lyrics as line}
      <p class="leading-tight transition-all duration-500 {line.focus ? 'text-4xl font-black text-white' : 'text-3xl font-extrabold text-zinc-700'}">
        {line.text}
      </p>
    {/each}
  </div>

  <!-- Gradient Fade to bottom -->
  <div class="absolute bottom-0 left-0 w-full h-32 bg-gradient-to-t from-zinc-900 via-zinc-900/80 to-transparent pointer-events-none"></div>
</aside>
