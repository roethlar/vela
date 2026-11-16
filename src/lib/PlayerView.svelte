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
  import { onMount, tick } from "svelte";
  import Icon from "$lib/Icon.svelte";
  import {
    PICTURE_CLICK_MS,
    SEEK_IDLE,
    WHEEL_IDLE,
    barHideAt,
    barShown,
    canSeek,
    commandNotice,
    focusKindOf,
    formatTime,
    keyAction,
    mergeWheelCommand,
    nextPlayerState,
    nextSeekBar,
    nextTrackHighlight,
    nextWheelSteps,
    pictureClickAction,
    pictureDoubleClick,
    queuePicturePause,
    seekBarValue,
    seekTimeAtPointer,
    sendPlayerCommand,
    skipLabel,
    spokenTime,
    takeMouseCommand,
    wheelCommands,
    type FocusKind,
    type QueuedPlayerCommand,
    type PlayerCommand,
    type PlayerState,
    type PlayerTrack,
    type SeekBar,
    type TrackId,
    type TrackListAction,
    type WheelTarget,
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
  let audioButton: HTMLButtonElement;
  let subtitleButton: HTMLButtonElement;
  let trackList = $state<HTMLDivElement>();

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
  let openList = $state<"audio" | "subtitles" | null>(null);
  // Remember the list before pointerdown can dismiss it. Click's detail
  // identifies the first click of a pair using the browser's own timing.
  let pressedWithList = false;
  let primaryPressOnPicture = false;
  let firstClickWithList = false;
  let firstClickTarget: EventTarget | null = null;
  // Whether this pair's first click requested pause when its wait expired.
  let firstClickPaused = false;
  let clickTimer: ReturnType<typeof setTimeout> | undefined;
  let disposed = false;
  let commandsPending = 0;
  let mousePending: readonly QueuedPlayerCommand[] = [];
  let wheelAccumulator = WHEEL_IDLE;
  let wheelTarget: WheelTarget = "none";
  let seekPointer = $state<{ x: number; y: number; left: number; width: number } | null>(null);
  let hoverTime = $derived(seekPointer === null ? null : seekTimeAtPointer(
    seekPointer.x, seekPointer.left, seekPointer.width, player?.duration ?? null,
  ));
  let highlightedTrack = $state<TrackId | null>(null);
  let listLeft = $state(0);
  let listBottom = $state(0);
  let listMaxHeight = $state(0);
  let shown = $derived(
    barShown({ now, lastActivity, paused: player?.paused ?? null, focusInBar, listOpen: openList !== null }),
  );

  let seekValue = $derived(seekBarValue(seekBar, player?.position ?? null, now));
  let volumeValue = $derived(seekBarValue(volumeBar, player?.volume ?? null, now));
  let selectedAudio = $derived(player?.audioTracks.find((t) => t.selected));
  let selectedSubtitle = $derived(player?.subtitleTracks.find((t) => t.selected));
  let audioLabel = $derived(selectedAudio ? trackLabel(selectedAudio) : "Audio track");
  let subtitleLabel = $derived(selectedSubtitle ? trackLabel(selectedSubtitle) : "Off");
  let listTracks = $derived<(Omit<PlayerTrack, "id"> & { id: TrackId })[]>(
    openList === "audio" ? player?.audioTracks ?? [] : [
      { id: "no", title: "Off", lang: "", selected: !selectedSubtitle },
      ...player?.subtitleTracks ?? [],
    ],
  );

  function reportRect() {
    const r = root.getBoundingClientRect();
    invoke("player_set_rect", { x: r.left, y: r.top, width: r.width, height: r.height }).catch(
      () => {},
    );
  }

  function send(command: PlayerCommand) {
    if (stopped || disposed) return;
    if (command.name === "stop") {
      stopped = true;
      clearPictureClick();
      mousePending = [];
      onstop();
    } else {
      mousePending = command.name === "toggle-pause" ? queuePicturePause(mousePending)
        : command.name === "set-volume" || command.name === "seek-relative"
          ? mergeWheelCommand(mousePending, command) : [...mousePending, command];
      flushMouse();
      return;
    }
    deliver(command);
  }

  // Only stop bypasses the dispatcher: the backend stops without its IPC gate.
  function deliver(command: PlayerCommand) {
    commandsPending++;
    sendPlayerCommand(sessionId, command.name, command.args).then(
      () => {
        if (!stopped) notice = "";
      },
      (error) => {
        if (!stopped) notice = commandNotice(error);
      },
    ).finally(() => {
      commandsPending--;
      flushMouse();
    });
  }

  function flushMouse() {
    const next = takeMouseCommand(mousePending, {
      busy: commandsPending !== 0, stopped: stopped || disposed, hasState: player !== null,
    });
    mousePending = next.pending;
    if (next.command) deliver(next.command);
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
      listOpen: openList !== null,
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
    else if (action.kind === "track-list") actOnTrackList(action.action);
  }

  // A pointer press must not leave focus on a button, or Space would click it
  // again. List options also keep focus on their listbox; ranges keep the
  // default press so that they can be dragged.
  function keepFocus(event: MouseEvent) {
    event.preventDefault();
  }

  function giveFocusBack() {
    root.focus();
  }

  function pictureClick(event: MouseEvent) {
    if (event.button !== 0) return;
    const context = {
      primaryPressOnPicture,
      listOpen: openList !== null,
      pressedWithList: pressedWithList || (event.detail % 2 === 0 && firstClickWithList),
      stopped: stopped || disposed, hasState: player !== null,
    };
    const action = pictureClickAction(event, context);
    if (action === "ignore") return;
    clearPictureClick();
    if (event.detail % 2 === 1) {
      firstClickWithList = pressedWithList;
      firstClickTarget = event.target;
      firstClickPaused = false;
    }
    activity();
    if (event.target === root && openList === null) giveFocusBack();
    if (action !== "wait") return;
    // MouseEvent.currentTarget is cleared after dispatch; save the inputs.
    const input = { button: event.button, detail: event.detail, target: event.target, currentTarget: root };
    clickTimer = setTimeout(() => {
      clickTimer = undefined;
      if (pictureClickAction(input, {
        ...context, stopped: stopped || disposed, hasState: player !== null, listOpen: openList !== null,
      }) === "wait") {
        firstClickPaused = true;
        mousePending = queuePicturePause(mousePending);
        flushMouse();
      }
    }, PICTURE_CLICK_MS);
  }

  function clearPictureClick() {
    clearTimeout(clickTimer);
    clickTimer = undefined;
  }

  function pictureDblClick(event: MouseEvent) {
    const action = pictureDoubleClick(event, {
      primaryPressOnPicture,
      firstClickTarget,
      firstClickPaused,
      listOpen: openList !== null,
      pressedWithList: firstClickWithList || pressedWithList,
      stopped: stopped || disposed,
      hasState: player !== null,
    });
    if (action === "none") return;
    clearPictureClick();
    activity();
    firstClickPaused = false;
    if (action === "restore-pause") {
      mousePending = queuePicturePause(mousePending);
      flushMouse();
    }
    onfullscreen(!fullscreen);
  }

  function pictureContextMenu(event: MouseEvent) {
    // WebKitGTK can follow this with a button-0 click, even when this
    // contextmenu had no pointerdown of its own.
    primaryPressOnPicture = false;
    event.preventDefault();
    event.stopPropagation();
    activity();
    if (openList === null) giveFocusBack();
    if (pictureClickAction(event, {
      primaryPressOnPicture,
      listOpen: openList !== null, pressedWithList: false,
      stopped: stopped || disposed, hasState: player !== null,
    }) === "pause") send({ name: "toggle-pause", args: [] });
  }

  function pointerActivity(event: PointerEvent) {
    primaryPressOnPicture = event.button === 0 && event.target === root;
    // A control that disappears after its press cannot inherit an older
    // picture click as the first click of a fullscreen pair.
    if (!primaryPressOnPicture) firstClickTarget = null;
    activity();
    // Keep secondary presses from focusing a range or dismissing a list;
    // middle/back/forward have no browser action in the player either.
    if (event.button !== 0) {
      event.preventDefault();
      pointerRange = null;
    }
  }

  function onwheel(event: WheelEvent) {
    activity();
    const target = event.target;
    const area: WheelTarget = target === root ? "picture"
      : target instanceof Element && target.closest(".track-list") ? "list"
      : target instanceof Element && target.closest(".volume, .mute") ? "volume"
      : target instanceof Element && target.closest(".seek") ? "seek" : "none";
    if (area !== wheelTarget) wheelAccumulator = WHEEL_IDLE;
    wheelTarget = area;
    if (area === "list") return;
    // Capture, non-passive: a native range must not also change itself.
    event.preventDefault();
    event.stopPropagation();
    // Moving focus out of a list would dismiss it through focusout.
    if (openList === null) giveFocusBack();
    if (area === "none") return;
    const next = nextWheelSteps(wheelAccumulator, event, now);
    wheelAccumulator = next.accumulator;
    const commands = wheelCommands(area, next.steps, seekBarValue(volumeBar, player?.volume ?? null, now), {
      hasState: player !== null, stopped: stopped || disposed,
    });
    for (const command of commands) {
      if (command.name === "set-volume") {
        volumeBar = nextSeekBar(volumeBar, { kind: "change", value: command.args[0], now });
      }
      mousePending = mergeWheelCommand(mousePending, command);
    }
    flushMouse();
  }

  function seekHover(event: PointerEvent) {
    const input = event.currentTarget as HTMLInputElement;
    const rect = input.getBoundingClientRect();
    const style = getComputedStyle(input);
    const left = parseFloat(style.borderLeftWidth) + parseFloat(style.paddingLeft);
    const right = parseFloat(style.borderRightWidth) + parseFloat(style.paddingRight);
    seekPointer = { x: event.clientX, y: event.clientY, left: rect.left + left, width: rect.width - left - right };
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

  function toggleTrackList(kind: "audio" | "subtitles") {
    if (openList === kind) {
      closeTrackList();
      return;
    }
    const tracks = kind === "audio" ? player?.audioTracks ?? [] : player?.subtitleTracks ?? [];
    highlightedTrack = tracks.find((t) => t.selected)?.id ?? (kind === "audio" ? tracks[0]?.id ?? null : "no");
    openList = kind;
    activity();
  }

  function closeTrackList() {
    openList = null;
    activity();
    giveFocusBack();
  }

  function chooseTrack(id: TrackId) {
    if (openList === "audio" && typeof id === "number") {
      send({ name: "set-audio-track", args: [id] });
    } else if (openList === "subtitles") {
      send({ name: "set-subtitle-track", args: [id] });
    }
    closeTrackList();
  }

  function revealHighlight() {
    tick().then(() => {
      trackList?.querySelector(".highlighted")?.scrollIntoView({ block: "nearest" });
    });
  }

  function actOnTrackList(action: TrackListAction) {
    if (action === "close") closeTrackList();
    else if (action === "choose") {
      if (highlightedTrack !== null) chooseTrack(highlightedTrack);
    } else {
      highlightedTrack = nextTrackHighlight(listTracks.map((t) => t.id), highlightedTrack, action);
      revealHighlight();
    }
  }

  function outsideTrackList(event: PointerEvent) {
    if (event.button !== 0) return;
    pressedWithList = openList !== null;
    if (openList === null) return;
    const target = event.target;
    if (target instanceof Node && (
      trackList?.contains(target) ||
      (!audioButton.disabled && audioButton.contains(target)) ||
      (!subtitleButton.disabled && subtitleButton.contains(target))
    )) return;
    // Dismissing by press leaves focus on the view, even over a range.
    event.preventDefault();
    closeTrackList();
  }

  function trackListFocusOut() {
    // Wait for the browser's focus transfer (including Tab) before returning
    // focus to the view. State updates never move focus out of the listbox.
    queueMicrotask(() => {
      if (openList !== null && !trackList?.contains(document.activeElement)) closeTrackList();
    });
  }

  function positionTrackList() {
    if (openList === null || !trackList) return;
    const button = openList === "audio" ? audioButton : subtitleButton;
    const top = bar.getBoundingClientRect().top - 6;
    listLeft = Math.max(8, Math.min(button.getBoundingClientRect().left, window.innerWidth - trackList.offsetWidth - 8));
    listBottom = window.innerHeight - top;
    listMaxHeight = Math.max(0, top - 8);
  }

  function barFocusOut(event: FocusEvent) {
    focusInBar = event.relatedTarget instanceof Node && bar.contains(event.relatedTarget);
  }

  function trackLabel(track: Omit<PlayerTrack, "id"> & { id: TrackId }): string {
    if (track.title && track.lang) return `${track.title} (${track.lang})`;
    return track.title || track.lang || `Track ${track.id}`;
  }

  // Focus and positioning change on opening, never on a playback tick.
  $effect(() => {
    const kind = openList;
    if (kind === null) return;
    const observer = new ResizeObserver(positionTrackList);
    observer.observe(root);
    observer.observe(bar);
    tick().then(() => {
      if (openList !== kind) return;
      positionTrackList();
      trackList?.focus();
      revealHighlight();
    });
    return () => observer.disconnect();
  });

  // Keep the same highlighted ID across state/selection updates and track
  // reordering. Only removal of that track needs a replacement highlight.
  $effect(() => {
    if (openList !== null && !listTracks.some((t) => t.id === highlightedTrack)) {
      highlightedTrack = listTracks[0]?.id ?? null;
      revealHighlight();
    }
  });

  // The bar hides when the time since the last activity runs out.
  $effect(() => {
    const at = barHideAt({ now, lastActivity, paused: player?.paused ?? null, focusInBar, listOpen: openList !== null });
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
    window.addEventListener("pointerdown", outsideTrackList, true);
    root.addEventListener("wheel", onwheel, { capture: true, passive: false });
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
      window.removeEventListener("pointerdown", outsideTrackList, true);
      root.removeEventListener("wheel", onwheel, true);
      clearPictureClick();
      disposed = true;
      mousePending = [];
      gone = true;
      unlistenState?.();
    };
  });
</script>

<!-- Fullscreen's keyboard equivalent is f, handled on the window above. -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
<div
  class="player"
  class:idle={!shown}
  bind:this={root}
  tabindex="-1"
  role="region"
  aria-label="Video"
  onpointermove={activity}
  onpointerdown={pointerActivity}
  onclick={pictureClick}
  ondblclick={pictureDblClick}
  oncontextmenu={pictureContextMenu}
  onauxclick={(event) => event.preventDefault()}
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
      onpointermove={seekHover}
      onpointerleave={() => (seekPointer = null)}
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
    <button
      class="audio"
      type="button"
      bind:this={audioButton}
      aria-label="Audio track"
      aria-haspopup="listbox"
      aria-expanded={openList === "audio"}
      aria-controls={openList === "audio" ? "player-track-list" : undefined}
      title={audioLabel}
      disabled={!player || player.audioTracks.length < 2}
      onmousedown={keepFocus}
      onclick={() => toggleTrackList("audio")}
    >
      <span class="track-label">{audioLabel}</span><span aria-hidden="true">▴</span>
    </button>
    <button
      class="subtitles"
      type="button"
      bind:this={subtitleButton}
      aria-label="Subtitles"
      aria-haspopup="listbox"
      aria-expanded={openList === "subtitles"}
      aria-controls={openList === "subtitles" ? "player-track-list" : undefined}
      title={subtitleLabel}
      disabled={!player || player.subtitleTracks.length === 0}
      onmousedown={keepFocus}
      onclick={() => toggleTrackList("subtitles")}
    >
      <span class="track-label">{subtitleLabel}</span><span aria-hidden="true">▴</span>
    </button>
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
  {#if shown && seekPointer && hoverTime !== null}
    <span
      class="seek-preview"
      aria-hidden="true"
      style:left="{seekPointer.x}px"
      style:top="{seekPointer.y - 12}px"
    >{formatTime(hoverTime, player?.duration ?? null).split(" / ")[0]}</span>
  {/if}
  {#if openList !== null}
    <div
      class="track-list"
      id="player-track-list"
      bind:this={trackList}
      role="listbox"
      tabindex="-1"
      aria-label={openList === "audio" ? "Audio track" : "Subtitles"}
      aria-activedescendant={highlightedTrack === null ? undefined : `player-track-${highlightedTrack}`}
      style:left="{listLeft}px"
      style:bottom="{listBottom}px"
      style:max-height="{listMaxHeight}px"
      onfocusout={trackListFocusOut}
    >
      {#each listTracks as track (track.id)}
        <div
          class="track-option"
          class:highlighted={highlightedTrack === track.id}
          id="player-track-{track.id}"
          role="option"
          aria-selected={track.selected}
          tabindex="-1"
          title={trackLabel(track)}
          onmousedown={keepFocus}
          onclick={() => chooseTrack(track.id)}
          onkeydown={onkey}
        >
          <span class="track-mark" aria-hidden="true">{track.selected ? "✓" : ""}</span>
          <span>{trackLabel(track)}</span>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .player {
    position: fixed;
    inset: 0;
    z-index: 1000;
    /* Nothing is painted here: this is where the picture shows through. */
    background: transparent;
    outline: none;
    /* Native ranges and the list's scrollbar render dark in every theme. */
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
  .seek-preview {
    position: fixed;
    transform: translate(-50%, -100%);
    pointer-events: none;
    padding: 0.2rem 0.4rem;
    border-radius: 0.25rem;
    background: rgb(0 0 0 / 0.85);
    color: #fff;
    font-size: 0.85rem;
    font-variant-numeric: tabular-nums;
  }
  .seek,
  .volume {
    accent-color: #fff;
  }
  .bar .audio,
  .bar .subtitles {
    width: 10rem;
    max-width: 10rem;
    min-width: 0;
    gap: 0.4rem;
    padding: 0.3rem;
    border: 1px solid rgb(255 255 255 / 0.35);
    border-radius: 0.4rem;
    background: rgb(0 0 0 / 0.6);
    color: #fff;
    font: inherit;
    font-size: 0.9rem;
  }
  .track-label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-align: left;
  }
  .track-list {
    position: fixed;
    z-index: 1;
    box-sizing: border-box;
    width: min(20rem, calc(100vw - 1rem));
    overflow-y: auto;
    overscroll-behavior: contain;
    padding: 0.25rem;
    border: 1px solid rgb(255 255 255 / 0.5);
    border-radius: 0.5rem;
    background: #181818;
    color: #fff;
    box-shadow: 0 0.3rem 1rem rgb(0 0 0 / 0.6);
    font-size: 0.9rem;
  }
  .track-option {
    display: flex;
    gap: 0.5rem;
    padding: 0.45rem;
    border: 1px solid transparent;
    border-radius: 0.25rem;
    overflow-wrap: anywhere;
    cursor: pointer;
  }
  .track-option:hover {
    background: #333;
  }
  .track-option.highlighted {
    border-color: #fff;
    background: #333;
  }
  .track-mark {
    flex: 0 0 1em;
  }
</style>
