<script lang="ts">
  // The view shown while a play runs inside Vela's own window.
  //
  // The picture is not in the page. It is on a surface UNDER the window, and
  // shows wherever the page is transparent. So this view does three things: it
  // makes the page transparent for as long as it is mounted (the `player-open`
  // class, styled in app.css), it tells the backend where its rectangle is so
  // the picture can be put behind it, and it draws the controls over the
  // picture. The player takes no input of its own: every key and control goes
  // through `sendPlayerCommand`, by a name from a closed list
  // ($lib/playerControls holds the list and every rule of the controls).
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import Icon from "$lib/Icon.svelte";
  import {
    SEEK_IDLE,
    barHideAt,
    barShown,
    canSeek,
    commandNotice,
    focusKindOf,
    formatTime,
    keyAction,
    nextPlayerState,
    nextSeekBar,
    seekBarValue,
    sendPlayerCommand,
    skipLabel,
    spokenTime,
    type FocusKind,
    type PlayerCommand,
    type PlayerState,
    type PlayerTrack,
    type SeekBar,
  } from "$lib/playerControls";

  // `sessionId`: the playback session this view belongs to; every command
  // names it, so a view cannot drive the play that replaced its own.
  // `fullscreen`: whether the window really is fullscreen, kept by the page.
  // `onfullscreen`: asks the page for fullscreen on or off.
  // `onstop`: tells the page the user stopped the play.
  let {
    sessionId,
    fullscreen,
    onfullscreen,
    onstop,
  }: {
    sessionId: string;
    fullscreen: boolean;
    onfullscreen: (on: boolean) => void;
    onstop: () => void;
  } = $props();

  let root: HTMLDivElement;
  let bar: HTMLDivElement;

  // The last `player-state` of this view's play; null until the first one.
  let player = $state<PlayerState | null>(null);
  // Set once `stop` is sent: nothing is sent after it.
  let stopped = $state(false);
  // What the notice line says: a command the player refused or did not answer.
  let notice = $state("");

  let seekBar = $state<SeekBar>(SEEK_IDLE);
  let volumeBar = $state<SeekBar>(SEEK_IDLE);
  // The range the pointer is on, so that only pointer use hands focus back.
  let pointerRange: HTMLInputElement | null = null;

  let now = $state(Date.now());
  let lastActivity = $state(Date.now());
  let focusInBar = $state(false);
  let shown = $derived(
    barShown({ now, lastActivity, paused: player?.paused ?? null, focusInBar }),
  );

  let seekValue = $derived(seekBarValue(seekBar, player?.position ?? null, now));
  let volumeValue = $derived(seekBarValue(volumeBar, player?.volume ?? null, now));
  let selectedSubtitle = $derived(
    player?.subtitleTracks.find((t) => t.selected)?.id ?? "no",
  );

  function reportRect() {
    const r = root.getBoundingClientRect();
    invoke("player_set_rect", { x: r.left, y: r.top, width: r.width, height: r.height }).catch(
      () => {},
    );
  }

  function send(command: PlayerCommand) {
    if (stopped) return;
    if (command.name === "stop") {
      stopped = true;
      onstop();
    }
    sendPlayerCommand(sessionId, command.name, command.args).then(
      () => {
        if (!stopped) notice = "";
      },
      (error) => {
        if (!stopped) notice = commandNotice(error);
      },
    );
  }

  function activity() {
    now = Date.now();
    lastActivity = now;
  }

  // Only a control of this view counts; focus anywhere else is none of its.
  function focusKind(element: Element | null): FocusKind {
    return focusKindOf(element !== null && root.contains(element) ? element : null);
  }

  // Capture phase on the window: the view sees every key first, and a key it
  // takes reaches nothing else (not the page's own handlers, not a focused
  // control's default action).
  function onkey(event: KeyboardEvent) {
    activity();
    const action = keyAction(event, {
      focus: focusKind(document.activeElement),
      hasState: player !== null,
      stopped,
      skip: player?.skip != null,
      crop: player?.crop ?? null,
      fullscreen,
    });
    if (action.kind === "pass") return;
    event.preventDefault();
    event.stopPropagation();
    if (action.kind === "command") send(action.command);
    else if (action.kind === "fullscreen") onfullscreen(action.on);
  }

  // A pointer press must not leave focus on a button, or Space would click it
  // again. Buttons only: on a range it would stop the drag, on a select the
  // popup.
  function keepFocus(event: MouseEvent) {
    event.preventDefault();
  }

  function giveFocusBack() {
    root.focus();
  }

  function seekInput(event: Event) {
    const value = Number((event.currentTarget as HTMLInputElement).value);
    seekBar = nextSeekBar(seekBar, { kind: "input", value });
  }

  // The one seek of a drag, on release.
  function seekChange(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const value = Number(input.value);
    seekBar = nextSeekBar(seekBar, { kind: "change", value, now: Date.now() });
    send({ name: "seek-absolute", args: [value] });
    rangeDone(input);
  }

  function volumeInput(event: Event) {
    const value = Number((event.currentTarget as HTMLInputElement).value);
    volumeBar = nextSeekBar(volumeBar, { kind: "input", value });
  }

  function volumeChange(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const value = Number(input.value);
    volumeBar = nextSeekBar(volumeBar, { kind: "change", value, now: Date.now() });
    send({ name: "set-volume", args: [value] });
    rangeDone(input);
  }

  function rangeDone(input: HTMLInputElement) {
    if (pointerRange === input) {
      pointerRange = null;
      giveFocusBack();
    }
  }

  function audioChange(event: Event) {
    const value = Number((event.currentTarget as HTMLSelectElement).value);
    send({ name: "set-audio-track", args: [value] });
    giveFocusBack();
  }

  function subtitleChange(event: Event) {
    const value = (event.currentTarget as HTMLSelectElement).value;
    send({ name: "set-subtitle-track", args: [value === "no" ? "no" : Number(value)] });
    giveFocusBack();
  }

  // A select that loses focus to nothing (its popup closed) gives it back;
  // one left by Tab keeps the order.
  function selectBlur(event: FocusEvent) {
    if (event.relatedTarget === null) giveFocusBack();
  }

  function barFocusOut(event: FocusEvent) {
    focusInBar = event.relatedTarget instanceof Node && bar.contains(event.relatedTarget);
  }

  function trackLabel(track: PlayerTrack): string {
    if (track.title && track.lang) return `${track.title} (${track.lang})`;
    return track.title || track.lang || `Track ${track.id}`;
  }

  // The bar hides when the time since the last activity runs out.
  $effect(() => {
    const at = barHideAt({ now, lastActivity, paused: player?.paused ?? null, focusInBar });
    if (at === null) return;
    const timer = setTimeout(() => (now = Date.now()), Math.max(at - Date.now(), 0) + 20);
    return () => clearTimeout(timer);
  });

  onMount(() => {
    document.documentElement.classList.add("player-open");
    reportRect();
    const observer = new ResizeObserver(reportRect);
    observer.observe(root);
    root.focus();
    window.addEventListener("keydown", onkey, true);
    // The view may be gone before `listen` resolves; the listener is then
    // dropped as soon as it exists.
    let gone = false;
    let unlistenState: (() => void) | undefined;
    listen("player-state", ({ payload }) => {
      player = nextPlayerState(player, sessionId, payload);
      if (player === null) return;
      const at = Date.now();
      seekBar = nextSeekBar(seekBar, { kind: "state", position: player.position, now: at });
      volumeBar = nextSeekBar(volumeBar, { kind: "state", position: player.volume, now: at });
    }).then((un) => {
      if (gone) un();
      else unlistenState = un;
    });
    return () => {
      observer.disconnect();
      document.documentElement.classList.remove("player-open");
      window.removeEventListener("keydown", onkey, true);
      gone = true;
      unlistenState?.();
    };
  });
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div
  class="player"
  class:idle={!shown}
  bind:this={root}
  tabindex="-1"
  role="region"
  aria-label="Video"
  onpointermove={activity}
>
  <div class="over">
    <p class="notice" role="status">{notice}</p>
    {#if player?.skip}
      <button
        class="skip"
        type="button"
        onmousedown={keepFocus}
        onclick={() => send({ name: "skip-active-marker", args: [] })}
      >
        {skipLabel(player.skip)}
      </button>
    {/if}
  </div>
  <div
    class="bar"
    class:hidden={!shown}
    bind:this={bar}
    onfocusin={() => (focusInBar = true)}
    onfocusout={barFocusOut}
  >
    <button
      class="playpause"
      type="button"
      aria-label={player?.paused ? "Play" : "Pause"}
      disabled={!player}
      onmousedown={keepFocus}
      onclick={() => send({ name: "toggle-pause", args: [] })}
    >
      {#if player?.paused}
        <Icon name="play" />
      {:else}
        <Icon name="pause" />
      {/if}
    </button>
    <span class="time">{formatTime(player?.position ?? null, player?.duration ?? null)}</span>
    <input
      class="seek"
      type="range"
      min="0"
      max={player?.duration ?? 0}
      step="any"
      value={seekValue}
      aria-label="Seek"
      aria-valuetext={spokenTime(seekValue, player?.duration ?? null)}
      disabled={!canSeek(player?.duration ?? null)}
      onpointerdown={(e) => (pointerRange = e.currentTarget)}
      oninput={seekInput}
      onchange={seekChange}
    />
    <button
      class="mute"
      type="button"
      aria-label="Mute"
      aria-pressed={player?.muted ?? false}
      disabled={!player}
      onmousedown={keepFocus}
      onclick={() => send({ name: "toggle-mute", args: [] })}
    >
      {#if player?.muted}
        <Icon name="muted" />
      {:else}
        <Icon name="volume" />
      {/if}
    </button>
    <input
      class="volume"
      type="range"
      min="0"
      max="100"
      step="1"
      value={volumeValue}
      aria-label="Volume"
      disabled={!player}
      onpointerdown={(e) => (pointerRange = e.currentTarget)}
      oninput={volumeInput}
      onchange={volumeChange}
    />
    <select
      class="audio"
      aria-label="Audio track"
      disabled={!player || player.audioTracks.length < 2}
      onchange={audioChange}
      onblur={selectBlur}
    >
      {#each player?.audioTracks ?? [] as track (track.id)}
        <option value={track.id} selected={track.selected}>{trackLabel(track)}</option>
      {/each}
    </select>
    <select
      class="subtitles"
      aria-label="Subtitles"
      disabled={!player || player.subtitleTracks.length === 0}
      onchange={subtitleChange}
      onblur={selectBlur}
    >
      <option value="no" selected={selectedSubtitle === "no"}>Off</option>
      {#each player?.subtitleTracks ?? [] as track (track.id)}
        <option value={track.id} selected={track.selected}>{trackLabel(track)}</option>
      {/each}
    </select>
    {#if player && player.crop !== null}
      <button
        class="crop"
        type="button"
        aria-label="Crop"
        aria-pressed={player.crop === "on"}
        disabled={player.crop === "measuring"}
        onmousedown={keepFocus}
        onclick={() => send({ name: "toggle-autocrop", args: [] })}
      >
        <Icon name="crop" />
      </button>
    {/if}
    <button
      class="fullscreen"
      type="button"
      aria-label="Fullscreen"
      aria-pressed={fullscreen}
      onmousedown={keepFocus}
      onclick={() => onfullscreen(!fullscreen)}
    >
      {#if fullscreen}
        <Icon name="fullscreen-exit" />
      {:else}
        <Icon name="fullscreen" />
      {/if}
    </button>
    <button
      class="close"
      type="button"
      aria-label="Close"
      onmousedown={keepFocus}
      onclick={() => send({ name: "stop", args: [] })}
    >
      <Icon name="close" />
    </button>
  </div>
</div>

<style>
  .player {
    position: fixed;
    inset: 0;
    z-index: 1000;
    /* Nothing is painted here: this is where the picture shows through. */
    background: transparent;
    outline: none;
    /* Native ranges and selects render dark over the picture in every theme. */
    color-scheme: dark;
    color: #fff;
  }
  .player.idle {
    cursor: none;
  }
  .over {
    position: absolute;
    left: 1rem;
    right: 1rem;
    bottom: 4.5rem;
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 1rem;
    pointer-events: none;
  }
  .notice {
    margin: 0;
    max-width: 60ch;
    padding: 0;
    font-size: 0.95rem;
    text-shadow: 0 1px 3px rgb(0 0 0 / 0.9);
  }
  .skip {
    pointer-events: auto;
    margin-left: auto;
    padding: 0.55rem 1.1rem;
    border: 1px solid rgb(255 255 255 / 0.5);
    border-radius: 0.5rem;
    background: rgb(0 0 0 / 0.7);
    color: #fff;
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  .bar {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.75rem 1rem;
    background: linear-gradient(to top, rgb(0 0 0 / 0.8), rgb(0 0 0 / 0));
    opacity: 1;
    visibility: visible;
    /* Shown at once; hidden only when the fade has finished. */
    transition:
      opacity 200ms var(--ease),
      visibility 0s var(--ease) 0s;
  }
  .bar.hidden {
    opacity: 0;
    visibility: hidden;
    transition:
      opacity 200ms var(--ease),
      visibility 0s var(--ease) 200ms;
  }
  .bar button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0.4rem;
    border: 1px solid transparent;
    border-radius: 0.5rem;
    background: transparent;
    color: #fff;
    cursor: pointer;
  }
  .bar button:hover:not(:disabled),
  .skip:hover {
    background: rgb(255 255 255 / 0.15);
  }
  .bar button:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .bar button[aria-pressed="true"] {
    border-color: rgb(255 255 255 / 0.5);
  }
  .time {
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    font-size: 0.9rem;
  }
  .seek {
    flex: 1;
    min-width: 6rem;
  }
  .volume {
    width: 6rem;
  }
  .seek,
  .volume {
    accent-color: #fff;
  }
  .bar select {
    max-width: 10rem;
    padding: 0.3rem;
    border: 1px solid rgb(255 255 255 / 0.35);
    border-radius: 0.4rem;
    background: rgb(0 0 0 / 0.6);
    color: #fff;
    font: inherit;
    font-size: 0.9rem;
  }
</style>
