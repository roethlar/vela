<script lang="ts">
  // Per-kind server marks: Vela's own simplified drawings in each server's
  // colours, so two servers with the same name can be told apart at a glance.
  // Entirely inline — no vendor artwork, no asset files. An unknown kind
  // renders nothing and the caller's plain name stands alone.
  import { KIND_LABELS } from "$lib/serverPlaylists";

  let {
    kind,
    size = 16,
    class: cls = "",
  }: { kind: string; size?: number; class?: string } = $props();

  // The gradient is referenced by id, and this mark is rendered many times on
  // one page: a shared id would resolve to the first instance, which breaks
  // whenever that instance is not displayed.
  const uid = $props.id();
  const label = $derived(KIND_LABELS[kind]);
</script>

{#if label}
  <svg
    class={cls}
    width={size}
    height={size}
    viewBox="0 0 24 24"
    role="img"
    aria-label={label}
    focusable="false"
  >
    {#if kind === "plex"}
      <polygon points="6.5,2.5 12,2.5 18.5,12 12,21.5 6.5,21.5 13,12" fill="#E5A00D" />
    {:else if kind === "jellyfin"}
      <defs>
        <linearGradient id="{uid}-jf" x1="4" y1="6" x2="20" y2="21" gradientUnits="userSpaceOnUse">
          <stop offset="0" stop-color="#AA5CC3" />
          <stop offset="1" stop-color="#00A4DC" />
        </linearGradient>
      </defs>
      <path
        fill="none"
        stroke="url(#{uid}-jf)"
        stroke-width="3"
        stroke-linejoin="round"
        d="M12 4 21 19.5H3Z"
      />
      <path
        fill="url(#{uid}-jf)"
        stroke="url(#{uid}-jf)"
        stroke-width="1.2"
        stroke-linejoin="round"
        d="M12 12.6 14.6 17H9.4Z"
      />
    {:else if kind === "emby"}
      <rect x="4.6" y="4.6" width="14.8" height="14.8" rx="2.6" transform="rotate(45 12 12)" fill="#52B54B" />
      <path d="M9.9 8 16.2 12 9.9 16Z" fill="#fff" />
    {/if}
  </svg>
{/if}
