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
export type FocusKind = "none" | "button" | "volume" | "seek";

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
  /** A track list owns the keys, independently of DOM focus. */
  listOpen: boolean;
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
  | { kind: "track-list"; action: TrackListAction }
  | { kind: "fullscreen"; on: boolean };

export type TrackListMove = "previous" | "next" | "first" | "last";
export type TrackListAction = TrackListMove | "choose" | "close";
export type TrackId = number | "no";

/** Move the highlight without changing the selected track. */
export function nextTrackHighlight(
  ids: readonly TrackId[],
  current: TrackId | null,
  move: TrackListMove,
): TrackId | null {
  if (ids.length === 0) return null;
  const index = Math.max(0, ids.findIndex((id) => id === current));
  if (move === "first") return ids[0];
  if (move === "last") return ids[ids.length - 1];
  return ids[Math.min(ids.length - 1, Math.max(0, index + (move === "next" ? 1 : -1)))];
}

const PASS: KeyAction = { kind: "pass" };
const SWALLOW: KeyAction = { kind: "swallow" };

const SEEKS: Record<string, number> = {
  ArrowLeft: -5,
  ArrowRight: 5,
  ArrowUp: 60,
  ArrowDown: -60,
};

const TRACK_LIST_MOVES: Record<string, TrackListMove> = {
  ArrowUp: "previous", ArrowDown: "next", Home: "first", End: "last",
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

  // The in-page list gets first refusal, even if a button still has focus
  // during opening. Tab can leave it; q remains the player's close key.
  if (ctx.listOpen) {
    if (key === "Tab") return PASS;
    if (ctx.stopped) return SWALLOW;
    const move = TRACK_LIST_MOVES[key];
    if (move) return { kind: "track-list", action: move };
    if (input.repeat) return SWALLOW;
    if (key === "Escape") return { kind: "track-list", action: "close" };
    if (key === " " || key === "Enter") return { kind: "track-list", action: "choose" };
    if (key === "q") return { kind: "command", command: { name: "stop", args: [] } };
    return SWALLOW;
  }

  // What belongs to the focused control.
  if (ctx.focus === "button" && (key === " " || key === "Enter")) return PASS;
  if (ctx.focus === "volume" && key in SEEKS) return PASS;

  if (!isPlayerKey(key)) return PASS;

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
  // `C`, as it arrives with or without Shift held.
  if (ctx.crop === null) return PASS;
  if (ctx.crop === "measuring") return SWALLOW;
  return { kind: "command", command: { name: "toggle-autocrop", args: [] } };
}

// ---------------------------------------------------------------------------
// The picture.

export const PICTURE_CLICK_MS = 250;

type PictureInput = { button: number; target: unknown; currentTarget: unknown };
type PictureContext = {
  primaryPressOnPicture: boolean;
  listOpen: boolean;
  pressedWithList: boolean;
  stopped: boolean;
  hasState: boolean;
};

/** A primary click must begin with a primary press on the picture.
 * Otherwise ignore it completely, leaving any earlier click's wait alone.
 * Odd primary clicks wait 250 ms; even clicks belong to the native pair
 * and never pause. A right-click pauses immediately.
 * Right-click never dismisses a list. A primary press that dismissed one
 * belongs to that list, including the second click of a pair.
 */
export function pictureClickAction(
  input: PictureInput & { detail: number },
  ctx: PictureContext,
): "ignore" | "none" | "wait" | "pause" {
  if (input.button === 0 && !ctx.primaryPressOnPicture) return "ignore";
  if (input.target !== input.currentTarget || ctx.stopped || !ctx.hasState) return "none";
  if (input.button === 2) return "pause";
  if (input.button !== 0 || input.detail % 2 === 0 || ctx.listOpen || ctx.pressedWithList) return "none";
  return "wait";
}

/**
 * A native primary double-click on the picture always toggles fullscreen,
 * however far apart its clicks were. If the first click already requested
 * pause, restore playback with another toggle, queued ahead of wheel input.
 * Do nothing if a track list is open, a press in the pair met an open list,
 * stop was sent, or no player state has arrived.
 * Both clicks must target the view, including when a control (such as Skip)
 * disappears after the first click.
 */
export function pictureDoubleClick(
  input: PictureInput,
  ctx: PictureContext & { firstClickTarget: unknown; firstClickPaused: boolean },
): "none" | "fullscreen" | "restore-pause" {
  if (!ctx.primaryPressOnPicture || input.button !== 0 || input.target !== input.currentTarget || ctx.firstClickTarget !== input.currentTarget ||
    ctx.listOpen || ctx.pressedWithList || ctx.stopped || !ctx.hasState) return "none";
  return ctx.firstClickPaused ? "restore-pause" : "fullscreen";
}

// ---------------------------------------------------------------------------
// Wheel input. Positive x is right; positive y is down.

export const WHEEL_STEP_PIXELS = 40;
export const WHEEL_RESET_MS = 500;
type WheelAxis = { remainder: number; direction: number; at: number };
export type WheelAccumulator = { x: WheelAxis; y: WheelAxis };
export const WHEEL_IDLE: WheelAccumulator = {
  x: { remainder: 0, direction: 0, at: 0 },
  y: { remainder: 0, direction: 0, at: 0 },
};
export type WheelSteps = { x: number; y: number };
export type WheelTarget = "picture" | "volume" | "seek" | "list" | "none";
export type WheelCommand = Extract<PlayerCommand, { name: "set-volume" | "seek-relative" }>;
/** Mouse, keyboard and controls share one queue; stop is always immediate. */
export type QueuedPlayerCommand = Exclude<PlayerCommand, { name: "stop" }>;

/** Picture pause and its reversal go before queued wheel input. Both wait
 * for the current command's answer, so a slow pair cannot be refused as busy.
 */
export function queuePicturePause(pending: readonly QueuedPlayerCommand[]): QueuedPlayerCommand[] {
  return [{ name: "toggle-pause", args: [] }, ...pending];
}

function wheelAxis(previous: WheelAxis, delta: number, now: number) {
  if (delta === 0) return { axis: previous, steps: 0 };
  const direction = Math.sign(delta);
  const remainder = now - previous.at >= WHEEL_RESET_MS || direction !== previous.direction
    ? 0 : previous.remainder;
  const total = remainder + delta;
  const steps = Math.trunc(total / WHEEL_STEP_PIXELS);
  return { axis: { remainder: total - steps * WHEEL_STEP_PIXELS, direction, at: now }, steps };
}

/** 40 pixels, 3 lines, or 1 page is one step. Each axis keeps its remainder
 * until 500 ms without input on that axis, or a reversal on that axis.
 */
export function nextWheelSteps(
  previous: WheelAccumulator,
  input: { deltaX: number; deltaY: number; deltaMode: number },
  now: number,
): { accumulator: WheelAccumulator; steps: WheelSteps } {
  const scale = input.deltaMode === 1 ? WHEEL_STEP_PIXELS / 3 : input.deltaMode === 2 ? WHEEL_STEP_PIXELS : 1;
  const x = wheelAxis(previous.x, input.deltaX * scale, now);
  const y = wheelAxis(previous.y, input.deltaY * scale, now);
  return { accumulator: { x: x.axis, y: y.axis }, steps: { x: x.steps, y: y.steps } };
}

/** The picture uses vertical volume and horizontal seek. On either slider,
 * up/right increases its value and down/left decreases it. Other bar targets
 * do nothing; a track list keeps its native scrolling.
 */
export function wheelCommands(
  target: WheelTarget,
  steps: WheelSteps,
  shownVolume: number,
  ctx: { hasState: boolean; stopped: boolean },
): WheelCommand[] {
  if (!ctx.hasState || ctx.stopped) return [];
  const volume = target === "picture" ? -steps.y : target === "volume" ? steps.x - steps.y : 0;
  const seek = target === "picture" ? steps.x : target === "seek" ? steps.x - steps.y : 0;
  const commands: WheelCommand[] = [];
  if (volume !== 0) commands.push({ name: "set-volume", args: [Math.min(100, Math.max(0, shownVolume + volume * 2))] });
  if (seek !== 0) commands.push({ name: "seek-relative", args: [seek * 10] });
  return commands;
}

/** While a command is unanswered, keep the latest absolute volume target
 * and the sum of relative seek seconds. Keep the two kinds in arrival order.
 * Other commands separate bursts: a relative seek must not cross an absolute
 * seek (nor overtake a track choice or another explicit control).
 */
export function mergeWheelCommand(pending: readonly QueuedPlayerCommand[], command: WheelCommand): QueuedPlayerCommand[] {
  let start = pending.length;
  while (start > 0 && (pending[start - 1].name === "set-volume" || pending[start - 1].name === "seek-relative")) start--;
  const index = pending.findIndex((p, i) => i >= start && p.name === command.name);
  if (index < 0) return [...pending, command];
  const merged = [...pending];
  const previous = pending[index];
  const value = command.name === "seek-relative" && previous.name === "seek-relative"
    ? previous.args[0] + command.args[0] : command.args[0];
  if (command.name === "seek-relative" && value === 0) merged.splice(index, 1);
  else merged[index] = { name: command.name, args: [value] };
  return merged;
}

// The per-command bound in src-tauri/src/player/control.rs, not a burst limit.
const MAX_RELATIVE_SEEK = 600;

/** Take at most one command, only once the previous answer has arrived.
 * Split a large seek at dispatch, keeping its remainder queued for the next
 * reply. Later wheel input can still add to or cancel that remainder.
 */
export function takeMouseCommand(
  pending: readonly QueuedPlayerCommand[],
  ctx: { busy: boolean; stopped: boolean; hasState: boolean },
): { pending: readonly QueuedPlayerCommand[]; command: QueuedPlayerCommand | null } {
  if (ctx.stopped || !ctx.hasState) return { pending: [], command: null };
  if (ctx.busy || pending.length === 0) return { pending, command: null };
  const first = pending[0];
  if (first.name === "seek-relative" && Math.abs(first.args[0]) > MAX_RELATIVE_SEEK) {
    const seconds = Math.sign(first.args[0]) * MAX_RELATIVE_SEEK;
    return {
      command: { name: "seek-relative", args: [seconds] },
      pending: [{ name: "seek-relative", args: [first.args[0] - seconds] }, ...pending.slice(1)],
    };
  }
  return { pending: pending.slice(1), command: pending[0] };
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

/** Native WebKit range thumbs are assumed to be 16 CSS pixels wide. The
 * centre travels from half a thumb inside each end of the content box.
 */
export function seekTimeAtPointer(
  x: number,
  left: number,
  width: number,
  duration: number | null,
  thumbWidth = 16,
): number | null {
  const total = knownDuration(duration);
  if (total === null || width <= thumbWidth) return null;
  return total * Math.min(1, Math.max(0, (x - left - thumbWidth / 2) / (width - thumbWidth)));
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
  /** An open track list keeps both the bar and pointer visible. */
  listOpen: boolean;
};

/** Whether the bar shows: while paused or focused, else for a while after activity. */
export function barShown(input: BarInput): boolean {
  if (input.paused !== false || input.focusInBar || input.listOpen) return true;
  return input.now - input.lastActivity < BAR_LINGER_MS;
}

/** When the bar should be looked at again, or null when nothing will hide it. */
export function barHideAt(input: BarInput): number | null {
  if (!barShown(input) || input.paused !== false || input.focusInBar || input.listOpen) return null;
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
