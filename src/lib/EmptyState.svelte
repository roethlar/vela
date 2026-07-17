<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "$lib/Icon.svelte";

  let {
    icon,
    heading,
    hint,
    announce = false,
    children,
  }: {
    icon: "film" | "playlist";
    heading: string;
    hint: string;
    announce?: boolean;
    children?: Snippet;
  } = $props();
</script>

<div class="empty-state" role={announce ? "status" : undefined}>
  <div class="empty-state-icon" aria-hidden="true">
    {#if icon === "film"}
      <Icon name="film" size={46} stroke={1.5} />
    {:else}
      <Icon name="playlist" size={46} stroke={1.5} />
    {/if}
  </div>
  <h2>{heading}</h2>
  <p>{hint}</p>
  {#if children}
    {@render children()}
  {/if}
</div>

<style>
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.7rem;
    padding: 2.5rem 1.5rem;
    color: var(--text-muted);
    text-align: center;
  }
  .empty-state-icon {
    margin-bottom: 0.2rem;
    color: var(--accent);
    line-height: 0;
  }
  h2 {
    margin: 0;
    color: var(--text);
    font-size: 1.5rem;
    font-weight: 700;
    letter-spacing: -0.01em;
  }
  p {
    max-width: 24rem;
    margin: 0;
    line-height: 1.5;
  }
</style>
