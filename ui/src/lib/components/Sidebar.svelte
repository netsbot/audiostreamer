<script lang="ts">
  import Home from 'lucide-svelte/icons/home';
  import Compass from 'lucide-svelte/icons/compass';
  import Radio from 'lucide-svelte/icons/radio';
  import Library from 'lucide-svelte/icons/library';
  import PersonStanding from 'lucide-svelte/icons/person-standing';
  import { Button } from '$lib/components/ui/button/index.js';

  const navItems = [
    { icon: Home, label: 'Home', active: true },
    { icon: Compass, label: 'Browse' },
    { icon: Radio, label: 'Radio' },
    { icon: Library, label: 'Library' },
    { icon: PersonStanding, label: 'Made For You' },
  ];

  const playlists = [
    'Chill Electronic',
    'Late Night Driving',
    'Classical Study',
    'New Wave Essentials',
  ];
</script>

<aside class="relative h-full w-56 shrink-0 border-r border-white/5 bg-zinc-900/50 flex flex-col py-8 px-5 gap-y-4">
  <nav class="flex flex-col gap-y-1">
    {#each navItems as item}
      <a
        href="/"
        class="flex items-center gap-3 py-2 transition-all {item.active ? 'text-white font-bold translate-x-1' : 'text-zinc-400 hover:text-red-400'}"
      >
        <svelte:component this={item.icon} class="size-4 {item.active ? 'text-red-500' : ''}" />
        <span class="text-sm">{item.label}</span>
      </a>
    {/each}
  </nav>

  <div class="mt-8">
    <h3 class="px-3 text-[10px] uppercase tracking-widest text-zinc-500 font-bold mb-4">Playlists</h3>
    <nav class="flex flex-col gap-y-1">
      {#each playlists as playlist}
        <a href="/" class="px-3 py-2 text-sm text-zinc-400 hover:text-white transition-colors truncate">
          {playlist}
        </a>
      {/each}
    </nav>
  </div>


</aside>
