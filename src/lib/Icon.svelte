<script lang="ts">
  // Inline line icons drawn on a 24px grid, stroked with currentColor so they
  // inherit text color and theme automatically. Lucide-style geometry (MIT).
  let {
    name,
    size = 18,
    stroke = 2,
    class: cls = "",
  }: { name: string; size?: number; stroke?: number; class?: string } = $props();
</script>

<svg
  class={cls}
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width={stroke}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#if name === "settings"}
    <line x1="21" x2="14" y1="4" y2="4" />
    <line x1="10" x2="3" y1="4" y2="4" />
    <line x1="21" x2="12" y1="12" y2="12" />
    <line x1="8" x2="3" y1="12" y2="12" />
    <line x1="21" x2="16" y1="20" y2="20" />
    <line x1="12" x2="3" y1="20" y2="20" />
    <line x1="14" x2="14" y1="2" y2="6" />
    <line x1="8" x2="8" y1="10" y2="14" />
    <line x1="16" x2="16" y1="18" y2="22" />
  {:else if name === "queue"}
    <line x1="8" x2="21" y1="6" y2="6" />
    <line x1="8" x2="21" y1="12" y2="12" />
    <line x1="8" x2="21" y1="18" y2="18" />
    <line x1="3.5" x2="3.5" y1="6" y2="6" />
    <line x1="3.5" x2="3.5" y1="12" y2="12" />
    <line x1="3.5" x2="3.5" y1="18" y2="18" />
  {:else if name === "close"}
    <line x1="18" y1="6" x2="6" y2="18" />
    <line x1="6" y1="6" x2="18" y2="18" />
  {:else if name === "back"}
    <line x1="19" y1="12" x2="5" y2="12" />
    <polyline points="12 19 5 12 12 5" />
  {:else if name === "chevron"}
    <polyline points="9 18 15 12 9 6" />
  {:else if name === "check"}
    <polyline points="20 6 9 17 4 12" />
  {:else if name === "plus"}
    <line x1="12" y1="5" x2="12" y2="19" />
    <line x1="5" y1="12" x2="19" y2="12" />
  {:else if name === "search"}
    <circle cx="11" cy="11" r="8" />
    <line x1="21" y1="21" x2="16.65" y2="16.65" />
  {:else if name === "play"}
    <polygon points="6 3 20 12 6 21 6 3" />
  {:else if name === "film"}
    <rect x="2.5" y="3" width="19" height="18" rx="2.5" />
    <line x1="7.5" y1="3" x2="7.5" y2="21" />
    <line x1="16.5" y1="3" x2="16.5" y2="21" />
    <line x1="2.5" y1="12" x2="21.5" y2="12" />
    <line x1="2.5" y1="7.5" x2="7.5" y2="7.5" />
    <line x1="2.5" y1="16.5" x2="7.5" y2="16.5" />
    <line x1="16.5" y1="7.5" x2="21.5" y2="7.5" />
    <line x1="16.5" y1="16.5" x2="21.5" y2="16.5" />
  {/if}
</svg>
