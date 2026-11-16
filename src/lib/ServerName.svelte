<script lang="ts">
  // A server's name with its kind mark, for anywhere the name is rendered as
  // markup. Plain strings (aria-labels, hints, joined metadata) use
  // `serverLabel` instead, which says the kind in words.
  import ServerLogo from "$lib/ServerLogo.svelte";

  let { name, kind, size = 13 }: { name: string; kind: string; size?: number } = $props();
</script>

<span class="servername"><ServerLogo {kind} {size} /><span class="servername-text">{name}</span></span>

<style>
  .servername {
    display: inline-flex;
    align-items: center;
    gap: 0.3em;
    vertical-align: bottom;
    min-width: 0;
    max-width: 100%;
  }
  .servername :global(svg) {
    flex: none;
  }
  .servername-text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }
</style>
