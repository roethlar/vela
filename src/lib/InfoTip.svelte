<script lang="ts">
  // A setting's explanation behind an "i" button next to its heading, in place
  // of grey help text under every control (owner, 2026-11-17: "irrelevant
  // in-line help text that should be a popup or removed"). Adds no text to
  // the heading it sits in: the button is labelled for assistive technology
  // and drawn as an icon. Opens on click; closes on a second click, Escape,
  // or focus leaving it.
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  let { label, children }: { label: string; children: Snippet } = $props();
  const uid = $props.id();
  let open = $state(false);
  let root: HTMLSpanElement | undefined = $state();

  function onfocusout(e: FocusEvent) {
    if (!root?.contains(e.relatedTarget as Node | null)) open = false;
  }
</script>

<span class="infotip" bind:this={root} {onfocusout}>
  <button
    type="button"
    class="infobtn"
    aria-label={`About ${label}`}
    aria-expanded={open}
    aria-controls={`${uid}-tip`}
    onclick={() => (open = !open)}
    onkeydown={(e) => {
      if (e.key === "Escape" && open) {
        e.stopPropagation();
        open = false;
      }
    }}
  >
    <Icon name="info" size={15} stroke={2} />
  </button>
  {#if open}
    <span class="infopop" id={`${uid}-tip`} role="note">{@render children()}</span>
  {/if}
</span>

<style>
  /* Positioned against the heading it sits in (Settings positions its
     headings), so the explanation spans the heading's width below it and
     never runs past the panel's edge, however long the heading. */
  .infotip {
    position: static;
    display: inline-flex;
    vertical-align: middle;
    margin-left: 6px;
    font-weight: normal;
  }
  .infobtn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 2px;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
  }
  .infobtn:hover,
  .infobtn[aria-expanded="true"] {
    color: var(--accent);
  }
  .infobtn:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .infopop {
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    right: 0;
    z-index: 20;
    display: block;
    padding: 10px 12px;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    box-shadow: var(--shadow-md);
    color: var(--text-2);
    font-size: 0.85rem;
    font-family: var(--font-ui);
    line-height: 1.45;
    text-transform: none;
    letter-spacing: normal;
  }
  .infopop :global(.tipline) {
    display: block;
    margin: 0 0 6px;
  }
  .infopop :global(.tipline:last-child) {
    margin-bottom: 0;
  }
</style>
