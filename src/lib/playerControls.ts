// The controls of the player view: what the page may ask the player to do,
// and every rule the view follows that can be decided without a window.
//
// The page drives the player through ONE command, `player_command`, with a
// name from a closed list. The list is a type here, so the compiler refuses a
// control that sends anything else, and it is the same list Rust parses
// (src/lib/playerCommands.json; a Node test holds the two equal). Every other
// rule here is a plain function of its inputs, so plain Node can test it.
//
// This file imports nothing of Vela's but types: Node runs the tests on it
// with its type stripping and svelte-check checks it, and the two agree on
// erasable TypeScript and package imports only.
import { invoke } from "@tauri-apps/api/core";

/** The names `player_command` accepts, and no others. */
export const PLAYER_COMMANDS = [
  "toggle-pause",
  "seek-relative",
  "seek-absolute",
  "set-volume",
  "toggle-mute",
  "set-audio-track",
  "set-subtitle-track",
  "stop",
  "skip-active-marker",
  "toggle-autocrop",
] as const;

export type PlayerCommandName = (typeof PLAYER_COMMANDS)[number];

/** The arguments each command takes; the backend checks them again. */
export type PlayerCommandArgs = {
  "toggle-pause": [];
  "seek-relative": [seconds: number];
  "seek-absolute": [seconds: number];
  "set-volume": [volume: number];
  "toggle-mute": [];
  "set-audio-track": [id: number];
  "set-subtitle-track": [id: number | "no"];
  stop: [];
  "skip-active-marker": [];
  "toggle-autocrop": [];
};

/** One command with its arguments, as the key map returns it. */
export type PlayerCommand = {
  [N in PlayerCommandName]: { name: N; args: PlayerCommandArgs[N] };
}[PlayerCommandName];

/** Ask the play of `sessionId` to do `name`. The one way the page drives mpv. */
export function sendPlayerCommand<N extends PlayerCommandName>(
  sessionId: string,
  name: N,
  args: PlayerCommandArgs[N],
): Promise<void> {
  return invoke("player_command", { sessionId, name, args });
}

/**
 * Ask for Vela's window to be fullscreen (only for the running play) or not
 * (always). The answer is whether the page holds the player's fullscreen
 * after the request, as the backend keeps that claim (a refused "on" leaves
 * it as it was). It says nothing about what the window did: the
 * `window-fullscreen` event does.
 */
export function setPlayerFullscreen(sessionId: string, on: boolean): Promise<boolean> {
  return invoke<boolean>("player_fullscreen", { sessionId, on });
}

/** Sends one request for the player's fullscreen; see `fullscreenRequests`. */
export type FullscreenRequest = (sessionId: string, on: boolean, held: (held: boolean) => void) => void;

/**
 * The page's requests for the player's fullscreen, through `send`. Each is
 * sent only once the one before it has been answered, so the backend records
 * the claim and asks the window in the order the page asked: an "on" still
 * on its way can never land after the "off" that followed it and leave the
 * window fullscreen with nobody holding it. The answer of the LAST request
 * sent is handed to `held`, so the page's claim is the backend's (a refused
 * "on" drops it); an answer overtaken by a later request is dropped. A
 * request that fails outright is taken as holding nothing. (Review finding
 * pc-2.)
 */
export function fullscreenRequests(
  send: (sessionId: string, on: boolean) => Promise<boolean>,
): FullscreenRequest {
  let last: Promise<unknown> = Promise.resolve();
  let latest = 0;
  return (sessionId, on, held) => {
    const mine = ++latest;
    const answer = last.then(() => send(sessionId, on));
    last = answer.catch(() => undefined);
    answer.then(
      (holds) => {
        if (mine === latest) held(holds === true);
      },
      () => {
        if (mine === latest) held(false);
      },
    );
  };
}

// ---------------------------------------------------------------------------
// The state the player reports.

export type PlayerTrack = { id: number; title: string; lang: string; selected: boolean };
export type SkipKind = "intro" | "credits" | "commercial";
export type CropState = "off" | "measuring" | "on";

export type PlayerState = {
  position: number | null;
  duration: number | null;
  paused: boolean;
  volume: number;
  muted: boolean;
  audioTracks: PlayerTrack[];
  subtitleTracks: PlayerTrack[];
  /** The marker whose skip button shows, if any. */
  skip: SkipKind | null;
  /** Where cropping stands; null when the play has no crop script. */
  crop: CropState | null;
};

function finiteOrNull(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function text(value: unknown): string {
  return typeof value === "string" ? value.slice(0, 200) : "";
}

function tracks(value: unknown): PlayerTrack[] {
  if (!Array.isArray(value)) return [];
  const out: PlayerTrack[] = [];
  for (const entry of value) {
    if (typeof entry !== "object" || entry === null) continue;
    const t = entry as Record<string, unknown>;
    if (typeof t.id !== "number" || !Number.isInteger(t.id) || t.id < 1) continue;
    out.push({ id: t.id, title: text(t.title), lang: text(t.lang), selected: t.selected === true });
  }
  return out;
}

/**
 * The state after a `player-state` event: the view of `sessionId` takes only
 * its own play's events, and only the closed sets for `skip` and `crop`.
 */
export function nextPlayerState(
  current: PlayerState | null,
  sessionId: string,
  payload: unknown,
): PlayerState | null {
  if (typeof payload !== "object" || payload === null) return current;
  const p = payload as Record<string, unknown>;
  if (p.sessionId !== sessionId) return current;
  const volume = finiteOrNull(p.volume);
  return {
    position: finiteOrNull(p.position),
    duration: finiteOrNull(p.duration),
    paused: p.paused === true,
    volume: volume === null ? 100 : Math.min(Math.max(volume, 0), 100),
    muted: p.muted === true,
    audioTracks: tracks(p.audioTracks),
    subtitleTracks: tracks(p.subtitleTracks),
    skip: p.skip === "intro" || p.skip === "credits" || p.skip === "commercial" ? p.skip : null,
    crop: p.crop === "off" || p.crop === "measuring" || p.crop === "on" ? p.crop : null,
  };
}

/** The skip button's text: the wording `vela-markers.lua` draws in mpv's window. */
export function skipLabel(kind: SkipKind): string {
  if (kind === "intro") return "Skip Intro";
  if (kind === "credits") return "Skip Credits";
  return "Skip Commercials";
}

// ---------------------------------------------------------------------------
// Keys.

/** What has keyboard focus inside the view when a key arrives. */
export type FocusKind = "none" | "button" | "select" | "volume" | "seek";

/** Of an element, what the key map needs: its tag (upper case, as HTML has it) and classes. */
export type FocusedElement = {
  tagName: string;
  classList: { contains(name: string): boolean };
};

/**
 * Which of the view's controls has focus. `element` is null when focus is
 * on nothing, or on anything outside the view.
 */
export function focusKindOf(element: FocusedElement | null): FocusKind {
  if (element === null) return "none";
  if (element.tagName === "BUTTON") return "button";
  if (element.tagName === "SELECT") return "select";
  if (element.tagName === "INPUT") return element.classList.contains("volume") ? "volume" : "seek";
  return "none";
}

export type KeyInput = {
  key: string;
  ctrlKey: boolean;
  altKey: boolean;
  metaKey: boolean;
  repeat: boolean;
};

export type KeyContext = {
  focus: FocusKind;
  /** Whether a `player-state` has arrived yet. */
  hasState: boolean;
  /** Whether this view has sent `stop`. */
  stopped: boolean;
  /** The skip button shows (the last state names a marker). */
  skip: boolean;
  /** The last state's crop, null without a crop script. */
  crop: CropState | null;
  /** The window really is fullscreen, whoever made it so. */
  fullscreen: boolean;
};

export type KeyAction =
  /** Not the page's key: the browser and the focused control keep it. */
  | { kind: "pass" }
  /** The page's key, with nothing to do now: kept from everyone else. */
  | { kind: "swallow" }
  | { kind: "command"; command: PlayerCommand }
  | { kind: "fullscreen"; on: boolean };

const PASS: KeyAction = { kind: "pass" };
const SWALLOW: KeyAction = { kind: "swallow" };

const SEEKS: Record<string, number> = {
  ArrowLeft: -5,
  ArrowRight: 5,
  ArrowUp: 60,
  ArrowDown: -60,
};

const MODIFIERS = new Set([
  "Shift",
  "Control",
  "Alt",
  "AltGraph",
  "Meta",
  "OS",
  "Super",
  "Hyper",
  "CapsLock",
  "NumLock",
  "ScrollLock",
  "Fn",
  "FnLock",
  "Symbol",
  "SymbolLock",
]);

/** The keys the page acts on (mpv's own keys for these). */
function isPlayerKey(key: string): boolean {
  return key === " " || key in SEEKS || key === "m" || key === "f" || key === "q" ||
    key === "Escape" || key === "C";
}

/** What the page does with a key, given where focus is and what it knows. */
export function keyAction(input: KeyInput, ctx: KeyContext): KeyAction {
  const { key } = input;
  if (input.ctrlKey || input.altKey || input.metaKey) return PASS;
  if (key === "Unidentified" || key === "Dead" || MODIFIERS.has(key)) return PASS;

  // What belongs to the focused control.
  if (ctx.focus === "button" && (key === " " || key === "Enter")) return PASS;
  if (ctx.focus === "select" && (key === " " || key === "Enter" || key === "Tab")) return PASS;
  if (ctx.focus === "volume" && key in SEEKS) return PASS;

  if (!isPlayerKey(key)) {
    // A closed select picks a track by type-ahead: no letter reaches it.
    if (ctx.focus === "select" && key.length === 1) return SWALLOW;
    return PASS;
  }

  if (ctx.stopped) return SWALLOW;
  const seek = SEEKS[key];
  if (input.repeat && seek === undefined) return SWALLOW;

  if (key === "f") return { kind: "fullscreen", on: !ctx.fullscreen };
  if (key === "Escape") {
    if (ctx.fullscreen) return { kind: "fullscreen", on: false };
    return { kind: "command", command: { name: "stop", args: [] } };
  }
  if (key === "q") return { kind: "command", command: { name: "stop", args: [] } };

  if (!ctx.hasState) return SWALLOW;
  if (seek !== undefined) {
    return { kind: "command", command: { name: "seek-relative", args: [seek] } };
  }
  if (key === " ") {
    return ctx.skip
      ? { kind: "command", command: { name: "skip-active-marker", args: [] } }
      : { kind: "command", command: { name: "toggle-pause", args: [] } };
  }
  if (key === "m") return { kind: "command", command: { name: "toggle-mute", args: [] } };
  // `C`, as it arrives with or without Shift held. Without a crop script it
  // is not the page's key, except that a select must not type-ahead on it.
  if (ctx.crop === null) return ctx.focus === "select" ? SWALLOW : PASS;
  if (ctx.crop === "measuring") return SWALLOW;
  return { kind: "command", command: { name: "toggle-autocrop", args: [] } };
}

// ---------------------------------------------------------------------------
// Time.

function clock(seconds: number, hours: boolean): string {
  const whole = Math.floor(seconds);
  const h = Math.floor(whole / 3600);
  const m = Math.floor((whole % 3600) / 60);
  const s = whole % 60;
  const ss = String(s).padStart(2, "0");
  if (hours || h > 0) return `${h}:${String(m).padStart(2, "0")}:${ss}`;
  return `${m}:${ss}`;
}

function knownDuration(duration: number | null): number | null {
  return duration !== null && Number.isFinite(duration) && duration > 0 ? duration : null;
}

function shownPosition(position: number | null): number {
  return position !== null && Number.isFinite(position) && position > 0 ? position : 0;
}

/** `m:ss / m:ss`, both sides `h:mm:ss` from an hour, `--:--` for no duration. */
export function formatTime(position: number | null, duration: number | null): string {
  const total = knownDuration(duration);
  const hours = total !== null && total >= 3600;
  const left = clock(shownPosition(position), hours);
  return `${left} / ${total === null ? "--:--" : clock(total, hours)}`;
}

/** What the seek bar says to a screen reader. */
export function spokenTime(position: number | null, duration: number | null): string {
  const total = knownDuration(duration);
  const hours = total !== null && total >= 3600;
  const left = clock(shownPosition(position), hours);
  return total === null ? left : `${left} of ${clock(total, hours)}`;
}

/** Whether the seek bar can seek: only with a known, non-zero duration. */
export function canSeek(duration: number | null): boolean {
  return knownDuration(duration) !== null;
}

// ---------------------------------------------------------------------------
// The bar's visibility.

/** How long the bar stays after the last pointer movement or key press. */
export const BAR_LINGER_MS = 3000;

export type BarInput = {
  now: number;
  /** When the pointer last moved or a key was last pressed. */
  lastActivity: number;
  /** Whether the play is paused; null before the first state. */
  paused: boolean | null;
  /** Whether a control in the bar has keyboard focus. */
  focusInBar: boolean;
};

/** Whether the bar shows: while paused or focused, else for a while after activity. */
export function barShown(input: BarInput): boolean {
  if (input.paused !== false || input.focusInBar) return true;
  return input.now - input.lastActivity < BAR_LINGER_MS;
}

/** When the bar should be looked at again, or null when nothing will hide it. */
export function barHideAt(input: BarInput): number | null {
  if (!barShown(input) || input.paused !== false || input.focusInBar) return null;
  return input.lastActivity + BAR_LINGER_MS;
}

// ---------------------------------------------------------------------------
// The seek bar.

/** How long a released seek bar holds its target while the player gets there. */
export const SEEK_HOLD_MS = 3000;
/** How close the reported position has to come to count as arrived. */
export const SEEK_ARRIVED_S = 1;

export type SeekBar = {
  /** The position under the thumb while it is dragged. */
  dragging: number | null;
  /** The position released at, until the player reports it. */
  pending: { target: number; since: number } | null;
};

export const SEEK_IDLE: SeekBar = { dragging: null, pending: null };

export type SeekEvent =
  | { kind: "input"; value: number }
  | { kind: "change"; value: number; now: number }
  | { kind: "state"; position: number | null; now: number };

/** The seek bar after an event. A `change` is where the view sends its one seek. */
export function nextSeekBar(bar: SeekBar, event: SeekEvent): SeekBar {
  if (event.kind === "input") return { dragging: event.value, pending: null };
  if (event.kind === "change") {
    return { dragging: null, pending: { target: event.value, since: event.now } };
  }
  if (bar.pending === null) return bar;
  const arrived =
    event.position !== null && Math.abs(event.position - bar.pending.target) <= SEEK_ARRIVED_S;
  if (arrived || event.now - bar.pending.since >= SEEK_HOLD_MS) {
    return { dragging: bar.dragging, pending: null };
  }
  return bar;
}

/** Where the seek bar's thumb sits. */
export function seekBarValue(bar: SeekBar, position: number | null, now: number): number {
  if (bar.dragging !== null) return bar.dragging;
  if (bar.pending !== null && now - bar.pending.since < SEEK_HOLD_MS) return bar.pending.target;
  return shownPosition(position);
}

// ---------------------------------------------------------------------------
// The notice line.

/**
 * What the notice line says when a command fails. The backend answers with
 * one of a few fixed sentences; anything else (which would be a fault of
 * Vela's own) is not shown as it came.
 */
export function commandNotice(error: unknown): string {
  const said = typeof error === "string" ? error.trim() : "";
  if (said.length > 0 && said.length <= 80 && /^[A-Za-z][A-Za-z ,.']*$/.test(said)) return said;
  return "The player could not do that.";
}
