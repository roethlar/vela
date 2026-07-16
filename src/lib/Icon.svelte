<script lang="ts">
  // Inline line icons drawn on a 24px grid, stroked with currentColor so they
  // inherit text color and theme automatically. Lucide-style geometry (MIT).
  type IconName =
    | "settings"
    | "playlist"
    | "close"
    | "back"
    | "chevron"
    | "check"
    | "star"
    | "heart"
    | "alert"
    | "plus"
    | "play"
    | "refresh"
    | "film";

  let {
    name,
    size = 18,
    stroke = 2,
    class: cls = "",
  }: { name: IconName; size?: number; stroke?: number; class?: string } = $props();
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
  focusable="false"
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
  {:else if name === "playlist"}
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
  {:else if name === "star"}
    <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
  {:else if name === "heart"}
    <path d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78L12 21.23l8.84-8.84a5.5 5.5 0 0 0 0-7.78z" />
  {:else if name === "alert"}
    <path d="m21.73 18-8-14a2 2 0 0 0-3.46 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3" />
    <line x1="12" y1="9" x2="12" y2="13" />
    <line x1="12" y1="17" x2="12.01" y2="17" />
  {:else if name === "plus"}
    <line x1="12" y1="5" x2="12" y2="19" />
    <line x1="5" y1="12" x2="19" y2="12" />
  {:else if name === "play"}
    <polygon points="6 3 20 12 6 21 6 3" />
  {:else if name === "refresh"}
    <path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8" />
    <path d="M21 3v5h-5" />
    <path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16" />
    <path d="M8 16H3v5" />
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
