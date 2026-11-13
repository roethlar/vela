<script lang="ts">
  // The view shown while a play runs inside Vela's own window.
  //
  // The picture is not in the page. It is on a surface UNDER the window, and
  // shows wherever the page is transparent. So this view does two things: it
  // makes the page transparent for as long as it is mounted (the `player-open`
  // class, styled in app.css), and it tells the backend where its rectangle is
  // so the picture can be put behind it.
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";

  // The playback session this view belongs to. Close stops that session and
  // no other: a view cannot stop the play that replaced its own.
  let { sessionId }: { sessionId: string } = $props();

  let root: HTMLDivElement;

  function reportRect() {
    const r = root.getBoundingClientRect();
    invoke("player_set_rect", { x: r.left, y: r.top, width: r.width, height: r.height }).catch(
      () => {},
    );
  }

  function close() {
    invoke("player_close", { sessionId }).catch(() => {});
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      close();
    }
  }

  onMount(() => {
    document.documentElement.classList.add("player-open");
    reportRect();
    const observer = new ResizeObserver(reportRect);
    observer.observe(root);
    root.focus();
    return () => {
      observer.disconnect();
      document.documentElement.classList.remove("player-open");
    };
  });
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div class="player" bind:this={root} tabindex="-1" role="region" aria-label="Video" {onkeydown}>
  <button class="close" type="button" onclick={close}>Close</button>
</div>

<style>
  .player {
    position: fixed;
    inset: 0;
    z-index: 1000;
    /* Nothing is painted here: this is where the picture shows through. */
    background: transparent;
    outline: none;
  }
  .close {
    position: absolute;
    top: 0.75rem;
    right: 0.75rem;
    padding: 0.4rem 0.9rem;
    border: 1px solid rgb(255 255 255 / 0.35);
    border-radius: 0.5rem;
    background: rgb(0 0 0 / 0.6);
    color: #fff;
    font: inherit;
    cursor: pointer;
  }
  .close:hover,
  .close:focus-visible {
    background: rgb(0 0 0 / 0.85);
    border-color: #fff;
  }
</style>
