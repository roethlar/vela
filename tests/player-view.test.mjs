// The page's side of a play that runs inside Vela's own window
// (.agents/plans/embedded-player.md, "Frontend: the player view", and plan
// review finding ep-2).
import assert from 'node:assert/strict';
import { test } from 'node:test';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { endLeavesPlayerFullscreen, nextPlayerSession, showingPlayEnded } from '../src/lib/playerView.ts';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const read = (file) => fs.readFileSync(path.join(repoRoot, file), 'utf8').replace(/\r\n/g, '\n');
const page = read('src/routes/+page.svelte');
const view = read('src/lib/PlayerView.svelte');
const css = read('src/app.css');
const commands = read('src-tauri/src/commands.rs');
const lib = read('src-tauri/src/lib.rs');
const playerMod = read('src-tauri/src/player/mod.rs');
const session = read('src-tauri/src/player/embedded/session.rs');
const host = read('src-tauri/src/player/embedded/host.rs');

/**
 * Every opening tag `<name ...>` in Svelte markup, whole: a `>` inside an
 * attribute's `{...}` (an arrow function's `=>`, a comparison) does not end it.
 */
function openingTags(markup, name) {
  const tags = [];
  for (const start of markup.matchAll(new RegExp(`<${name}\\b`, 'g'))) {
    let depth = 0;
    for (let i = start.index + 1; i < markup.length; i++) {
      const c = markup[i];
      if (c === '{') depth++;
      else if (c === '}') depth--;
      else if (c === '>' && depth === 0) {
        tags.push(markup.slice(start.index, i + 1));
        break;
      }
    }
  }
  return tags;
}

/** A pattern for `text` as written, whatever its whitespace and indentation. */
function shape(text) {
  const words = text.trim().split(/\s+/).map((word) => word.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'));
  return new RegExp(words.join('\\s+'));
}

const opened = (sessionId) => ({ kind: 'opened', sessionId });
const closed = (sessionId) => ({ kind: 'closed', sessionId });
const after = (events) => events.reduce(nextPlayerSession, null);

test("a play's view opens with it and closes with it", () => {
  assert.equal(after([]), null);
  assert.equal(after([opened('a')]), 'a');
  assert.equal(after([opened('a'), closed('a')]), null);
});

test("a replaced play's late close does not dismiss the view of the play that replaced it", () => {
  // Auto-advance, a replacement play, an Automatic step-down: the new play
  // opens first, and only then does the old one finish shutting down.
  assert.equal(after([opened('a'), opened('b'), closed('a')]), 'b', "b's view stays open");
  assert.equal(after([opened('a'), opened('b'), closed('a'), closed('b')]), null);
  // The old close may also arrive twice, or for a play whose view never showed.
  assert.equal(after([opened('a'), opened('b'), closed('a'), closed('a')]), 'b');
  assert.equal(after([opened('b'), closed('never-shown')]), 'b');
  assert.equal(after([closed('a')]), null);
});

test('the page shows the player view for exactly the session the events name', () => {
  // Both events go through the rule above, and nothing else decides which
  // session's view is showing.
  assert.match(
    page,
    /listen<\{ sessionId: string \}>\("player-opened", \(\{ payload \}\) => \{\s+const previous = playerSession;\s+playerSession = nextPlayerSession\(playerSession, \{\s+kind: "opened",\s+sessionId: payload\.sessionId,\s+\}\);/,
  );
  assert.match(
    page,
    /listen<\{ sessionId: string \}>\("player-closed", \(\{ payload \}\) => \{\s+const previous = playerSession;\s+playerSession = nextPlayerSession\(playerSession, \{\s+kind: "closed",\s+sessionId: payload\.sessionId,\s+\}\);/,
  );
  assert.match(page, /let playerSession = \$state<string \| null>\(null\);/);
  assert.equal(
    [...page.matchAll(/(?<!let )playerSession = /g)].length,
    2,
    'after its declaration, the showing session is assigned only by those two listeners',
  );
  // `playback-ended` fires twice after a clean end; it never decides which
  // view shows. Besides the watch-state refresh, it can end fullscreen for a
  // run that is over (the tests below).
  assert.match(page, /listen<PlaybackEnd>\("playback-ended", playbackEnded\)/);
  assert.match(
    page,
    /function playbackEnded\(event: \{ payload: PlaybackEnd \}\): Promise<unknown> \{[\s\S]*?\n\s+return refreshWatchState\(\);\s+\}/,
  );
  // The view is keyed by session, so a successor gets a fresh one; what it
  // knows of fullscreen comes from the page, which outlives it.
  assert.match(
    page,
    /\{#if playerSession\}\s+\{#key playerSession\}\s+<PlayerView\s+sessionId=\{playerSession\}\s+fullscreen=\{windowFullscreen\}\s+onfullscreen=\{askPlayerFullscreen\}\s+onstop=\{leavePlayerFullscreen\}\s+\/>\s+\{\/key\}\s+\{\/if\}/,
  );
});

test('the player view tells the backend where it is, and drives the play only through the command list', () => {
  assert.match(
    view,
    /function reportRect\(\) \{\s+const r = root\.getBoundingClientRect\(\);\s+invoke\("player_set_rect", \{ x: r\.left, y: r\.top, width: r\.width, height: r\.height \}\)/,
  );
  // On mount, and whenever its size changes.
  assert.match(view, /onMount\(\(\) => \{[\s\S]{0,200}reportRect\(\);\s+const observer = new ResizeObserver\(reportRect\);\s+observer\.observe\(root\);/);
  // `invoke(` and `invoke<T>(` alike: one call, the rectangle. Every other
  // command goes through $lib/playerControls (tests/player-controls.test.mjs).
  const invokes = [...view.matchAll(/\binvoke\s*(?:<[^>()]*>)?\s*\(/g)];
  assert.equal(invokes.length, 1, 'the view invokes one command itself');
  assert.equal(view.split('sendPlayerCommand(').length - 1, 1, 'and sends player commands from one place');
});

test('every key reaches the key map first, and the view acts on what it says', () => {
  // Capture phase on the window while the view is mounted, removed with it.
  assert.match(view, /onMount\(\(\) => \{[\s\S]*?window\.addEventListener\("keydown", onkey, true\);[\s\S]*?return \(\) => \{[\s\S]*?window\.removeEventListener\("keydown", onkey, true\);/);
  assert.equal(view.split('addEventListener(').length - 1, 1);
  assert.match(
    view,
    /function onkey\(event: KeyboardEvent\) \{\s+activity\(\);\s+const action = keyAction\(event, \{\s+focus: focusKind\(document\.activeElement\),\s+hasState: player !== null,\s+stopped,\s+skip: player\?\.skip != null,\s+crop: player\?\.crop \?\? null,\s+fullscreen,\s+\}\);\s+if \(action\.kind === "pass"\) return;\s+event\.preventDefault\(\);\s+event\.stopPropagation\(\);\s+if \(action\.kind === "command"\) send\(action\.command\);\s+else if \(action\.kind === "fullscreen"\) onfullscreen\(action\.on\);\s+\}/,
  );
  // Nothing is sent after stop, and stopping tells the page.
  assert.match(
    view,
    /function send\(command: PlayerCommand\) \{\s+if \(stopped\) return;\s+if \(command\.name === "stop"\) \{\s+stopped = true;\s+onstop\(\);\s+\}/,
  );
  assert.equal([...view.matchAll(/stopped = true/g)].length, 1);
});

test("the controls keep focus off buttons only, and the view takes only its own play's state", () => {
  const markup = view.slice(view.indexOf('</script>'), view.indexOf('<style>'));
  const buttons = openingTags(markup, 'button');
  // Skip, play/pause, mute, crop, fullscreen, close.
  assert.equal(buttons.length, 6, `the view's buttons: ${buttons.length}`);
  for (const button of buttons) {
    assert.ok(button.endsWith('>') && !button.endsWith('=>'), button);
    assert.match(button, /onmousedown=\{keepFocus\}/, button);
    assert.match(button, /aria-label=|class="skip"/, `${button} has a name`);
  }
  const others = [...openingTags(markup, 'input'), ...openingTags(markup, 'select')];
  assert.equal(others.length, 4, 'the seek bar, the volume, and the two track lists');
  for (const other of others) {
    assert.doesNotMatch(other, /onmousedown/, `${other} keeps its pointer press`);
  }
  // And nowhere else: one per button.
  assert.equal(markup.split('onmousedown').length - 1, buttons.length);
  assert.match(view, /function keepFocus\(event: MouseEvent\) \{\s+event\.preventDefault\(\);\s+\}/);
  // A select hands focus back on change; a range when the pointer moved it.
  assert.equal(markup.split('onchange={audioChange}').length - 1, 1);
  assert.match(
    view,
    /function audioChange\(event: Event\) \{\s+const value = [^;]+;\s+send\(\{ name: "set-audio-track", args: \[value\] \}\);\s+giveFocusBack\(\);\s+\}/,
  );
  assert.match(
    view,
    /function subtitleChange\(event: Event\) \{\s+const value = [^;]+;\s+send\(\{ name: "set-subtitle-track", args: \[[^\]]+\] \}\);\s+giveFocusBack\(\);\s+\}/,
  );
  assert.match(
    view,
    /function seekChange\(event: Event\) \{\s+const input = [^;]+;\s+const value = [^;]+;\s+seekBar = [^;]+;\s+send\(\{ name: "seek-absolute", args: \[value\] \}\);\s+rangeDone\(input\);\s+\}/,
  );
  // A drag only moves the thumb; the one command of a range goes on release.
  assert.match(
    view,
    shape(`function seekInput(event: Event) {
      const value = Number((event.currentTarget as HTMLInputElement).value);
      seekBar = nextSeekBar(seekBar, { kind: "input", value });
    }`),
  );
  assert.match(
    view,
    shape(`function volumeInput(event: Event) {
      const value = Number((event.currentTarget as HTMLInputElement).value);
      volumeBar = nextSeekBar(volumeBar, { kind: "input", value });
    }`),
  );
  assert.match(
    view,
    shape(`function volumeChange(event: Event) {
      const input = event.currentTarget as HTMLInputElement;
      const value = Number(input.value);
      volumeBar = nextSeekBar(volumeBar, { kind: "change", value, now: Date.now() });
      send({ name: "set-volume", args: [value] });
      rangeDone(input);
    }`),
  );
  assert.equal(view.split('send({ name: "seek-absolute"').length - 1, 1, 'one seek, in seekChange');
  assert.equal(view.split('send({ name: "set-volume"').length - 1, 1, 'one volume, in volumeChange');
  for (const [handler, range] of [
    ['oninput={seekInput}', 'seek'],
    ['onchange={seekChange}', 'seek'],
    ['oninput={volumeInput}', 'volume'],
    ['onchange={volumeChange}', 'volume'],
  ]) {
    const tag = openingTags(markup, 'input').find((input) => input.includes(`class="${range}"`));
    assert.ok(tag.includes(handler), `${range}: ${handler}`);
  }
  assert.match(
    view,
    /function rangeDone\(input: HTMLInputElement\) \{\s+if \(pointerRange === input\) \{\s+pointerRange = null;\s+giveFocusBack\(\);\s+\}\s+\}/,
  );
  assert.match(view, /function giveFocusBack\(\) \{\s+root\.focus\(\);\s+\}/);
  assert.match(view, /function selectBlur\(event: FocusEvent\) \{\s+if \(event\.relatedTarget === null\) giveFocusBack\(\);\s+\}/);
  assert.equal(markup.split('onblur={selectBlur}').length - 1, 2, 'both selects');
  // Close stops this view's play.
  assert.match(markup, /class="close"[\s\S]*?onclick=\{\(\) => send\(\{ name: "stop", args: \[\] \}\)\}/);
  // The skip button sends the script's skip, and nothing else.
  assert.match(
    markup,
    /\{#if player\?\.skip\}\s+<button\s+class="skip"\s+type="button"\s+onmousedown=\{keepFocus\}\s+onclick=\{\(\) => send\(\{ name: "skip-active-marker", args: \[\] \}\)\}\s+>\s+\{skipLabel\(player\.skip\)\}\s+<\/button>\s+\{\/if\}/,
  );
  // The notice line is always there, so what it says is announced.
  assert.match(markup, /<div class="over">\s+<p class="notice" role="status">\{notice\}<\/p>/);
  // `player-state`: this session's only, and a listener that resolves after
  // the view has gone is dropped at once.
  assert.match(
    view,
    /let gone = false;\s+let unlistenState: \(\(\) => void\) \| undefined;\s+listen\("player-state", \(\{ payload \}\) => \{\s+player = nextPlayerState\(player, sessionId, payload\);[\s\S]*?\}\)\.then\(\(un\) => \{\s+if \(gone\) un\(\);\s+else unlistenState = un;\s+\}\);/,
  );
  assert.match(view, /return \(\) => \{[\s\S]*?gone = true;\s+unlistenState\?\.\(\);\s+\};/);
});

test('each control is wired to what it shows and what it sends', () => {
  const markup = view.slice(view.indexOf('</script>'), view.indexOf('<style>'));
  const tag = (name, cls) => {
    const found = openingTags(markup, name).filter((t) => t.includes(`class="${cls}"`));
    assert.equal(found.length, 1, `one ${name}.${cls}`);
    return found[0];
  };
  // "Off" is "no"; a track is its number.
  assert.match(
    view,
    shape(`send({ name: "set-subtitle-track", args: [value === "no" ? "no" : Number(value)] });`),
  );
  // The crop control: only with a crop script, and not while measuring.
  assert.match(markup, shape('{#if player && player.crop !== null} <button class="crop"'));
  assert.match(tag('button', 'crop'), /disabled=\{player\.crop === "measuring"\}/);
  assert.match(tag('button', 'crop'), /aria-pressed=\{player\.crop === "on"\}/);
  assert.match(tag('button', 'mute'), /aria-pressed=\{player\?\.muted \?\? false\}/);
  assert.match(tag('input', 'seek'), /disabled=\{!canSeek\(player\?\.duration \?\? null\)\}/);
  assert.match(tag('input', 'seek'), /value=\{seekValue\}/);
  assert.match(tag('input', 'volume'), /value=\{volumeValue\}/);
  // What a refusal says goes through the notice filter.
  assert.match(
    view,
    shape(`sendPlayerCommand(sessionId, command.name, command.args).then(
      () => {
        if (!stopped) notice = "";
      },
      (error) => {
        if (!stopped) notice = commandNotice(error);
      },
    );`),
  );
  // Each state feeds both ranges.
  assert.match(
    view,
    shape(`const at = Date.now();
      seekBar = nextSeekBar(seekBar, { kind: "state", position: player.position, now: at });
      volumeBar = nextSeekBar(volumeBar, { kind: "state", position: player.volume, now: at });`),
  );
  // Native controls render dark over the picture; the keys start at the view.
  assert.match(view, /\.player \{[^}]*color-scheme: dark;/);
  assert.match(view, shape('observer.observe(root); root.focus();'));
});

test('the bar and the pointer hide by the rule, and a hidden control takes no click', () => {
  const markup = view.slice(view.indexOf('</script>'), view.indexOf('<style>'));
  const [root] = openingTags(markup, 'div');
  assert.match(root, /class="player"/);
  assert.match(root, /class:idle=\{!shown\}/, 'the pointer hides with the bar');
  assert.match(root, /onpointermove=\{activity\}/, 'a pointer move shows the bar');
  const bar = openingTags(markup, 'div').find((tag) => tag.includes('class="bar"'));
  assert.match(bar, /class:hidden=\{!shown\}/);
  assert.match(
    view,
    /let shown = \$derived\(\s+barShown\(\{ now, lastActivity, paused: player\?\.paused \?\? null, focusInBar \}\),\s+\);/,
  );
  // Faded out, then out of reach: `visibility: hidden` after the fade.
  assert.match(
    view,
    shape(`.bar.hidden {
      opacity: 0;
      visibility: hidden;
      transition:
        opacity 200ms var(--ease),
        visibility 0s var(--ease) 200ms;
    }`),
  );
  assert.match(view, shape('.player.idle { cursor: none; }'));
});

test('fullscreen for the player is held by the page and left on events', () => {
  // The real state, and Vela's claim on it cleared when the window leaves
  // fullscreen by any other way.
  assert.match(
    page,
    /listen<\{ on: boolean \}>\("window-fullscreen", \(\{ payload \}\) => \{\s+windowFullscreen = payload\.on === true;\s+if \(!windowFullscreen\) \{\s+playerAskedFullscreen = false;/,
  );
  // What the window is comes from that event alone, never from a request
  // (Tauri's own answer is only the last request).
  assert.match(page, /let windowFullscreen = \$state\(false\);/);
  assert.equal(
    [...page.matchAll(/(?<!let )\bwindowFullscreen\s*=[^=]/g)].length,
    1,
    'after its declaration, the real state is assigned only by that listener',
  );
  // Only what Vela asked for is left.
  assert.match(
    page,
    /function leavePlayerFullscreen\(\) \{\s+clearPlayerFullscreenBackstop\(\);\s+if \(!playerAskedFullscreen\) return;\s+askPlayerFullscreen\(false\);\s+\}/,
  );
  assert.match(
    page,
    /function askPlayerFullscreen\(on: boolean\) \{\s+const sessionId = playerSession;\s+if \(on && sessionId === null\) return;\s+playerAskedFullscreen = on;/,
  );
  assert.equal([...page.matchAll(/playerAskedFullscreen = true/g)].length, 0, 'set only by askPlayerFullscreen');
  // The run's own end, with no successor.
  assert.match(
    page,
    /function finishPlaybackRun\(sessionId: string\) \{\s+if \(playerSession === null \|\| playerSession === sessionId\) leavePlayerFullscreen\(\);/,
  );
  // Every report of an end goes through the rule in $lib/playerView (tested
  // case by case below), with what the page knows now: first whether it
  // names the view that shows, then whether it leaves.
  assert.match(
    page,
    shape(`const end = event.payload ?? {};
      playerSessionEnded = showingPlayEnded(playerSession, playerSessionEnded, playerSession, end);
      if (
        endLeavesPlayerFullscreen(end, {
          showing: playerSession,
          showingEnded: playerSessionEnded,
          continuationPending: continuationPending(),
        })
      ) {
        leavePlayerFullscreen();
      }`),
  );
  assert.equal(
    [...page.matchAll(/(?<!let )playerSessionEnded = /g)].length,
    2,
    'after its declaration, set only on a report and on a view change',
  );
  assert.match(
    page,
    /function continuationPending\(\): boolean \{\s+return continuationsRunning > 0 \|\| continuationSession !== null \|\| sourceChoiceRequest !== null;\s+\}/,
  );
  assert.match(
    page,
    /async function handlePlaybackContinuation\(event: \{ payload: PlaybackContinuation \}\) \{\s+continuationsRunning\+\+;\s+try \{\s+await continueAfterPlayback\(event\.payload\);\s+\} finally \{\s+continuationsRunning--;\s+\}\s+\}/,
  );
  // Both lifecycle events keep the view's state and the backstop, from the
  // view that showed before them.
  assert.equal(
    [...page.matchAll(/const previous = playerSession;\s+playerSession = nextPlayerSession\(playerSession, \{[^}]*\}\);\s+playerViewChanged\(previous\);\s+\}\)\.then/g)].length,
    2,
  );
  assert.match(page, /const PLAYER_FULLSCREEN_BACKSTOP_MS = 20_000 \+ 30_000;/);
  // The bookkeeping itself, whole: requests go one at a time, in order, and
  // the claim becomes what the backend answers it holds (a refused request
  // drops it); leaving clears the backstop.
  assert.match(
    page,
    shape(`function askPlayerFullscreen(on: boolean) {
      const sessionId = playerSession;
      if (on && sessionId === null) return;
      playerAskedFullscreen = on;
      if (!on) clearPlayerFullscreenBackstop();
      // One request at a time, in order; the claim then becomes what the
      // backend holds (a refused request gives it up).
      requestPlayerFullscreen(sessionId ?? "", on, (held) => {
        playerAskedFullscreen = held;
      });
    }`),
  );
  assert.match(page, shape(`const requestPlayerFullscreen = fullscreenRequests(setPlayerFullscreen);`));
  assert.equal(page.split('setPlayerFullscreen(').length - 1, 0, 'the request is sent only through the queue');
  assert.equal(page.split('requestPlayerFullscreen(').length - 1, 1, 'and only from askPlayerFullscreen');
  // A view change keeps whether the view showing is of a play that ended
  // (only while that view still shows); with a view, no backstop; with none
  // and Vela's claim held, one backstop, which leaves only if still no view
  // shows when it runs out.
  assert.match(
    page,
    shape(`function playerViewChanged(previous: string | null) {
      playerSessionEnded = showingPlayEnded(previous, playerSessionEnded, playerSession);
      if (playerSession !== null) {
        clearPlayerFullscreenBackstop();
      } else if (playerAskedFullscreen && !playerFullscreenBackstop) {
        playerFullscreenBackstop = setTimeout(() => {
          playerFullscreenBackstop = undefined;
          if (playerSession === null) leavePlayerFullscreen();
        }, PLAYER_FULLSCREEN_BACKSTOP_MS);
      }
    }`),
  );
  assert.match(
    page,
    shape(`function clearPlayerFullscreenBackstop() {
      if (playerFullscreenBackstop) clearTimeout(playerFullscreenBackstop);
      playerFullscreenBackstop = undefined;
    }`),
  );
  // The window leaving fullscreen by any way ends the claim and the backstop,
  // and the listener goes with the page.
  assert.match(
    page,
    shape(`listen<{ on: boolean }>("window-fullscreen", ({ payload }) => {
      windowFullscreen = payload.on === true;
      if (!windowFullscreen) {
        playerAskedFullscreen = false;
        clearPlayerFullscreenBackstop();
      }
    }).then((un) => (unlistenWindowFullscreen = un));`),
  );
  assert.match(
    page,
    shape(`onDestroy(() => {
      unlistenPlayerOpened?.();
      unlistenPlayerClosed?.();
      unlistenWindowFullscreen?.();
      clearPlayerFullscreenBackstop();`),
  );
  assert.equal([...page.matchAll(/playerFullscreenBackstop = setTimeout\(/g)].length, 1);
});

// pc-2. The page's events for a run, replayed as the page takes them: a
// view opening or closing goes through `nextPlayerSession`, and whether the
// play of the view showing has ended through `showingPlayEnded`; each end
// goes through `showingPlayEnded` and then `endLeavesPlayerFullscreen`.
// Returns the index of every event at which fullscreen is left.
function fullscreenLeftAt(events) {
  let showing = null;
  let showingEnded = false;
  const left = [];
  events.forEach((event, index) => {
    if (event.kind === 'opened' || event.kind === 'closed') {
      const previous = showing;
      showing = nextPlayerSession(showing, event);
      showingEnded = showingPlayEnded(previous, showingEnded, showing);
      return;
    }
    showingEnded = showingPlayEnded(showing, showingEnded, showing, event.payload);
    const ctx = { showing, showingEnded, continuationPending: event.pending ?? false };
    if (endLeavesPlayerFullscreen(event.payload, ctx)) left.push(index);
  });
  return left;
}
// A play's own report of its end: its session and whether the run is over.
const ended = (sessionId, runOver) => ({ kind: 'ended', payload: { sessionId, runOver, sourceId: 's', itemKey: 's:k' } });
// The completion dispatcher's second report of a clean end, saying neither
// that the run is over nor that a successor is coming (Continue Playing
// decides).
const endedAgain = (pending = false, sessionId = 'a') => ({
  kind: 'ended',
  payload: { sessionId, runOver: false, successor: false, sourceId: 's', itemKey: 's:k' },
  pending,
});

test('a clean end whose successor the backend starts keeps fullscreen across the gap (case 1)', () => {
  // As the embeddedfullscreenend scenario saw it: the old view closes only
  // after the successor's has opened.
  assert.deepEqual(fullscreenLeftAt([opened('a'), ended('a', false), opened('b'), closed('a'), endedAgain()]), []);
  // And with the old view gone before the successor's opens.
  assert.deepEqual(fullscreenLeftAt([opened('a'), closed('a'), ended('a', false), opened('b'), endedAgain()]), []);
});

test("a playlist whose next play cannot be started ends the run at the dispatcher's report (case 1)", () => {
  // The dispatcher's report names the session and says the run is over. As
  // the app sends it, it comes while the ended play's view still shows.
  const over = (sessionId) => ({ kind: 'ended', payload: { sessionId, runOver: true, sourceId: 's', itemKey: 's:k' } });
  assert.deepEqual(fullscreenLeftAt([opened('a'), ended('a', false), over('a'), closed('a')]), [2]);
  assert.deepEqual(fullscreenLeftAt([opened('a'), closed('a'), ended('a', false), over('a')]), [3]);
  // A report without `successor` is a play's own, not the dispatcher's: it
  // leaves only when it says the run is over, however many come.
  const notOver = { kind: 'ended', payload: { sessionId: 'a', runOver: false, sourceId: 's', itemKey: 's:k' } };
  assert.deepEqual(fullscreenLeftAt([opened('a'), ended('a', false), notOver, opened('b'), closed('a')]), []);
  assert.deepEqual(fullscreenLeftAt([opened('a'), closed('a'), ended('a', false), notOver]), []);
});

test("the dispatcher's report of a clean end names its session and says whether the run is over", () => {
  // Over only when the playlist's successor could not be started.
  assert.match(
    lib,
    shape(`let after = commands::advance_playlist(&state, &completion.session_id).await;
      if after == commands::AfterCleanEnd::ContinuePlaying {`),
  );
  // The payload is `clean_end_report`'s (tested in Rust, and the one the
  // Rust enumeration of orders sends to its page): ids, whether the run is
  // over, and whether a successor is coming.
  assert.match(
    lib,
    shape(`let _ = app_handle.emit(
          "playback-ended",
          commands::clean_end_report(&completion, after),
      );`),
  );
  assert.equal(lib.split('"playback-ended"').length - 1, 1, 'the dispatcher sends it once, there');
  assert.match(
    commands,
    shape(`"runOver": after == AfterCleanEnd::RunOver,
      "successor": after == AfterCleanEnd::Successor,
    })`),
  );
});

test('a report that says a successor is coming keeps fullscreen, whatever else the page sees', () => {
  const coming = (sessionId) => ({ kind: 'ended', payload: { sessionId, runOver: false, successor: true, sourceId: 's', itemKey: 's:k' } });
  // The successor's view opens only after the report (still starting, or
  // announced after its launch returned).
  assert.deepEqual(fullscreenLeftAt([opened('a'), closed('a'), ended('a', false), coming('a'), opened('b')]), []);
  // The successor plays in mpv's own window: no view ever opens.
  assert.deepEqual(fullscreenLeftAt([opened('a'), closed('a'), ended('a', false), coming('a')]), []);
  // Saying no successor is coming, with nothing pending here, that same
  // report leaves.
  assert.deepEqual(fullscreenLeftAt([opened('a'), closed('a'), ended('a', false), endedAgain()]), [3]);
  // Only true says a successor is coming, and only false that none is: any
  // other value is no dispatcher's report, and leaves nothing.
  const maybe = { kind: 'ended', payload: { sessionId: 'a', runOver: false, successor: 'yes' } };
  assert.deepEqual(fullscreenLeftAt([opened('a'), closed('a'), ended('a', false), maybe]), []);
  // A report that says the run is over still leaves, whatever else it says.
  const both = { kind: 'ended', payload: { sessionId: 'a', runOver: true, successor: true } };
  assert.deepEqual(fullscreenLeftAt([opened('a'), closed('a'), both]), [2]);
});

test("a clean end whose successor the page decides keeps fullscreen while it decides (case 2)", () => {
  assert.deepEqual(fullscreenLeftAt([opened('a'), closed('a'), ended('a', false), endedAgain(true), opened('b')]), []);
  // The play the page started fails: the page is still "pending" for the
  // whole of that play, and its end in an error leaves all the same.
  assert.deepEqual(
    fullscreenLeftAt([
      opened('a'),
      closed('a'),
      ended('a', false),
      endedAgain(true),
      opened('b'),
      closed('b'),
      { ...ended('b', true), pending: true },
    ]),
    [6],
  );
  assert.deepEqual(fullscreenLeftAt([opened('b'), { ...ended('b', true), pending: true }, closed('b')]), [1]);
});

test('a clean end with no successor leaves fullscreen at its second report (case 3)', () => {
  assert.deepEqual(fullscreenLeftAt([opened('a'), closed('a'), ended('a', false), endedAgain(false)]), [3]);
  // The view that still shows is the ended play's own: it is only waiting
  // to close, and the report leaves all the same.
  assert.deepEqual(fullscreenLeftAt([opened('a'), ended('a', false), endedAgain(false), closed('a')]), [2]);
  // While another play's view shows, nothing leaves: the page's own end of
  // the run (`finishPlaybackRun`, after `continue-playing`) decides.
  assert.deepEqual(fullscreenLeftAt([opened('b'), endedAgain(false, 'a')]), []);
});

test("the user's stop leaves fullscreen from the view, and its end then changes nothing (case 4)", () => {
  // The view's `onstop` leaves at once (pinned above); the report after it
  // finds Vela's claim gone, so leaving again does nothing.
  assert.deepEqual(fullscreenLeftAt([opened('a'), closed('a'), ended('a', true)]), [2]);
});

test('an end in an error leaves fullscreen at its one report (case 5)', () => {
  assert.deepEqual(fullscreenLeftAt([opened('a'), closed('a'), ended('a', true)]), [2]);
  // The report may come while the view still shows.
  assert.deepEqual(fullscreenLeftAt([opened('a'), ended('a', true), closed('a')]), [1]);
  // A playlist's next play that fails, as the scenario saw it.
  assert.deepEqual(
    fullscreenLeftAt([opened('a'), ended('a', false), opened('b'), ended('b', true), closed('b'), closed('a')]),
    [3],
  );
});

test('mpv quitting by another hand, or dying, leaves fullscreen at its one report (case 6)', () => {
  assert.deepEqual(fullscreenLeftAt([opened('a'), closed('a'), ended('a', true)]), [2]);
});

test('a step-down keeps fullscreen: the old play is replaced, not over (case 7)', () => {
  assert.deepEqual(fullscreenLeftAt([opened('a'), opened('b'), ended('a', false), closed('a')]), []);
  // Even an end of the old play said to be over leaves nothing while the
  // new play's view shows.
  assert.deepEqual(fullscreenLeftAt([opened('a'), opened('b'), ended('a', true), closed('a')]), []);
});

test('a play the backend replaces keeps fullscreen (case 8)', () => {
  assert.deepEqual(fullscreenLeftAt([opened('a'), closed('a'), ended('a', false), opened('b')]), []);
  assert.deepEqual(fullscreenLeftAt([opened('a'), opened('b'), closed('a'), ended('a', false)]), []);
});

// pc-2. Every distinct sequence of events the page receives in the
// orders the Rust test `in_every_order_the_page_leaves_fullscreen_exactly_when_the_run_is_over`
// (src-tauri/src/run_over_tests.rs) runs through the real backend functions:
// launches that fail before or after stopping the play they replace, with or
// without a view of their own, one after another; the old play's own report,
// clean or not; the dispatcher's report; the old view closing; the reaper; a
// play that fell back to mpv's own window. Each event says whether anything
// was playing or coming when it was sent. Replayed as `+page.svelte` takes
// them, through the real helpers: the page leaves the player's fullscreen at
// a report once nothing plays or is coming, and never while something does.
// What plays or is coming at each event is the Rust test's model (its
// `World`), carried in the fixture: this replay checks the page's real
// helpers against that model, not the model itself. The page's backstop is
// not replayed (no time passes here).
const orders = JSON.parse(read('tests/fixtures/run-over-orders.json'));

// An end's code: `1`/`0` a play's own report (over or not), `R`/`s`/`n` the
// dispatcher's (over, a successor coming, neither).
function parseToken(token) {
  if (token[0] === 'e') {
    const code = token[2];
    assert.ok('10Rsn'.includes(code), token);
    const payload = { sessionId: token[1], runOver: code === '1' || code === 'R', sourceId: 's', itemKey: 's:k' };
    if ('Rsn'.includes(code)) payload.successor = code === 's';
    return { kind: 'ended', payload, busy: token[3] === '+', pending: token[4] === 'p' };
  }
  return { kind: token[0] === 'o' ? 'opened' : 'closed', sessionId: token[1], busy: token[2] === '+', pending: token[3] === 'p' };
}

// The page's handlers for these events (pinned above): `player-opened` and
// `player-closed` go through `nextPlayerSession` and `showingPlayEnded`;
// `playback-ended` goes through `showingPlayEnded`, asks
// `endLeavesPlayerFullscreen`, and leaves only fullscreen it asked for. The
// page has asked for it when the run's first view showed.
function replayPage(events) {
  let showing = null;
  let showingEnded = false;
  let asked = true;
  let leftAt = null;
  events.forEach((event, index) => {
    if (event.kind !== 'ended') {
      const previous = showing;
      showing = nextPlayerSession(showing, { kind: event.kind, sessionId: event.sessionId });
      showingEnded = showingPlayEnded(previous, showingEnded, showing);
      return;
    }
    showingEnded = showingPlayEnded(showing, showingEnded, showing, event.payload);
    const leaves = endLeavesPlayerFullscreen(event.payload, {
      showing,
      showingEnded,
      continuationPending: event.pending,
    });
    if (leaves && asked) {
      asked = false;
      leftAt = index;
    }
  });
  return leftAt;
}

test('in every order the backend produces, the page leaves fullscreen at a report exactly when the run is over', () => {
  assert.ok(orders.length > 700, `${orders.length} sequences`);
  // Among them, a source choice for the next play that ends without a play
  // (the dispatcher says a successor is coming, then the choice's end says
  // the run is over), and a successor in mpv's own window followed by
  // another, or replaced by a step-down.
  assert.ok(orders.some((line) => /eAs\+\. .*eA1-/.test(line)));
  assert.ok(orders.some((line) => /eBs\+/.test(line)));
  assert.ok(orders.some((line) => /eB0\+.*eD/.test(line)));
  // Among them, a successor coming with no view of its own showing yet.
  assert.ok(orders.some((line) => / cA[+-]. .*eAs\+/.test(line) && !/oB/.test(line.split('eAs')[0])));
  let left = 0;
  let kept = 0;
  for (const line of orders) {
    const [body, end] = line.split(' |');
    const events = body.split(' ').map(parseToken);
    const busyAtEnd = end === '+';
    const leftAt = replayPage(events);
    if (busyAtEnd) {
      assert.equal(leftAt, null, `left while something still plays: ${line}`);
      kept++;
    } else {
      assert.notEqual(leftAt, null, `nothing plays or is coming, and nothing told the page: ${line}`);
      assert.equal(events[leftAt].busy, false, `left at ${leftAt} while something played or was coming: ${line}`);
      left++;
    }
  }
  assert.ok(left > 0 && kept > 0, `${left} left, ${kept} kept`);
});

test("a launch that fails reports the run over only from the state of the app, and nothing else sends that report", () => {
  // The sequence itself (the player in the slot read before anything is
  // installed, the report given only when nothing plays, no play is active
  // and the page holds the player's fullscreen) is run by the Rust tests.
  // What is held here is the wiring around it that a unit test cannot reach.
  const testsAt = commands.indexOf('\n#[cfg(all(test, target_os = "linux"))]');
  assert.notEqual(testsAt, -1, 'the command layer has its tests after the shipped code');
  const shipped = commands.slice(0, testsAt);
  const playPath = shipped.slice(shipped.indexOf('\nasync fn play_by_key_locked('));
  const playBody = playPath.slice(0, playPath.indexOf('\n}\n'));
  // The play path starts every play through `install_and_launch` and hands it
  // the function that really sends the report.
  assert.match(
    playBody,
    shape(`launch,
        |end| report_playback_ended(state, end),
    )
    .await
    .map_err(PlayFailure::launch_failed)?;`),
  );
  assert.match(
    shipped,
    shape(`fn report_playback_ended(state: &AppState, end: serde_json::Value) {
        if let Some(app) = state.app_handle.get() {
            use tauri::Emitter;
            let _ = app.emit("playback-ended", end);
        }
    }`),
  );
  assert.equal(
    shipped.split('report_playback_ended(').length - 1,
    4,
    'defined once, passed to the launch, and to the two ends of a source choice',
  );
  assert.equal(shipped.split('"playback-ended"').length - 1, 2, "the tracker's report, and the one the backend owes from the state of the app");
  // What it sends: a session and the flag, nothing else, under the view
  // record's lock.
  assert.match(
    shipped,
    shape(`if failed_launch_ends_run(nothing_playing, play_active, fullscreen_held) {
        state.player_view.report(session_id, |session| {
            report_end(serde_json::json!({
                "sessionId": session,
                "runOver": true,
            }))
        });
    }`),
  );
});

test("the page's claim on fullscreen is kept where it is asked for and where the window leaves it", () => {
  // `player_fullscreen` records the claim through `claim_player_fullscreen`
  // (unit-tested in Rust) and asks the window only when that says so.
  assert.match(
    commands,
    shape(`let state = app.state::<AppState>();
        let claim = claim_player_fullscreen(&state, &session_id, on);
        if claim.ask_window {
            if let Some(window) = app.webview_windows().into_values().next() {
                let _ = window.set_fullscreen(on);
            }
        }
        claim.held`),
  );
  // The window leaving fullscreen, however that was asked for, gives the
  // claim up before the page hears of it.
  assert.match(
    host,
    shape(`let on = event.new_window_state().contains(fullscreen);
        // The page gives up its claim on fullscreen when it hears the
        // window has left it; so does the backend's copy of that claim
        // (review finding pc-2).
        if !on {
            app.state::<crate::AppState>()
                .player_fullscreen_held
                .store(false, std::sync::atomic::Ordering::SeqCst);
        }
        let _ = app.emit("window-fullscreen", serde_json::json!({ "on": on }));`),
  );
  assert.equal(host.split('player_fullscreen_held').length - 1, 1, 'and only there');
});

test('the view record and the view events are one step', () => {
  // The embedded play's lifecycle sends `player-opened` and `player-closed`
  // only through the record (`ViewRecord::announce`), with the send inside.
  assert.match(
    host,
    shape(`app.state::<crate::AppState>().player_view.announce(
        &session_id,
        event == Lifecycle::Opened,
        || {
            let _ = app.emit(name, serde_json::json!({ "sessionId": session_id }));
        },
    );`),
  );
  assert.equal(host.split('app.emit(name').length - 1, 1, 'the view events are sent nowhere else');
  for (const source of [commands, lib, playerMod, session]) {
    assert.equal(source.indexOf('"player-opened"'), -1);
    assert.equal(source.indexOf('"player-closed"'), -1);
  }
});

test("the dispatcher's every answer that a successor is coming checks that a play is active", () => {
  const advance = commands.slice(commands.indexOf('\npub(crate) async fn advance_playlist('));
  const body = advance.slice(0, advance.indexOf('\n}\n'));
  assert.equal(body.split('AfterCleanEnd::Successor').length - 1, 1, 'only for a choice the user is making');
  assert.match(body, shape(`if cursor.session_id != ended_session {
      return replaced_or_over(state).await;
  };`));
  assert.equal(body.split('Ok(result) => after_successor(state, result).await,').length - 1, 2, 'both playlists');
  assert.match(commands, shape(`PlayCommandResult::Superseded => replaced_or_over(state).await,`));
  assert.match(commands, shape(`} else {
        replaced_or_over(state).await
    }`));
});

test('a report that does not say the run is over never leaves on its own', () => {
  assert.deepEqual(fullscreenLeftAt([opened('a'), closed('a'), ended('a', false)]), []);
  assert.deepEqual(fullscreenLeftAt([opened('a'), closed('a'), ended('a', false), ended('b', false), ended('c', false)]), []);
  assert.deepEqual(
    fullscreenLeftAt([opened('a'), closed('a'), { kind: 'ended', payload: { runOver: true } }]),
    [],
    'over, but naming no session',
  );
  assert.deepEqual(fullscreenLeftAt([opened('a'), closed('a'), ended('a', 'yes')]), [], 'only true is over');
});

test("the tracker's report of an end names its session and says whether the run is over", () => {
  // The one emit of the tracker's end, whole: ids and a flag, the flag from
  // the rule `end_is_run_over` (tested in Rust) on what the backend knows
  // there.
  assert.match(
    commands,
    shape(`let run_over = tracker_end_is_run_over(
        &app.state::<AppState>(),
        advance.saw_eof(&session_id),
        &session_id,
    );
    let _ = app.emit(
        "playback-ended",
        serde_json::json!({
            "sourceId": source_id,
            "itemKey": item_key.clone(),
            "sessionId": session_id.clone(),
            "runOver": run_over,
        }),
    );
    }
    advance.mark_ended(PlaybackCompletion {`),
  );
  assert.match(
    commands,
    shape(`fn end_is_run_over(clean_eof: bool, active_session: Option<&str>, ended_session: &str) -> bool {
      !clean_eof && active_session.is_none_or(|active| active == ended_session)
    }`),
  );
});

test('the page gets out of the way of the picture only while the view is mounted', () => {
  // The picture is on a surface under the window: it shows where the page
  // paints nothing.
  assert.match(
    view,
    /onMount\(\(\) => \{\s+document\.documentElement\.classList\.add\("player-open"\);[\s\S]{0,1200}return \(\) => \{\s+observer\.disconnect\(\);\s+document\.documentElement\.classList\.remove\("player-open"\);/,
  );
  assert.match(
    css,
    /html\.player-open,\nhtml\.player-open body \{\n  background: transparent !important;\n\}\nhtml\.player-open \.app \{\n  visibility: hidden;\n\}/,
  );
  assert.equal(css.split('player-open').length - 1, 3, 'and that is all the class does');
  assert.match(view, /\.player \{[^}]*background: transparent;/);
});

// pc-2. The dispatcher asks the user to choose the source of the
// run's next play; the page keeps fullscreen while the dialog is up. The
// user cancels it (or it expired, or the play chosen fails before it is
// installed): the backend then says the run is over (Rust:
// `a_successor_being_chosen_keeps_fullscreen_and_a_choice_ended_without_a_play_ends_the_run`).
test('a successor being chosen keeps fullscreen, and a choice that ends without a play leaves it', () => {
  const coming = { kind: 'ended', payload: { sessionId: 'a', runOver: false, successor: true, sourceId: 's', itemKey: 's:k' } };
  // The dialog is not up yet when the dispatcher's report comes, and is
  // when the choice ends.
  assert.deepEqual(fullscreenLeftAt([opened('a'), closed('a'), ended('a', false), coming]), []);
  assert.deepEqual(
    fullscreenLeftAt([opened('a'), closed('a'), ended('a', false), coming, { ...ended('a', true), pending: true }]),
    [4],
  );
  // The view closes only after the choice has ended.
  assert.deepEqual(fullscreenLeftAt([opened('a'), ended('a', false), coming, ended('a', true), closed('a')]), [3]);
});

test("a source choice's end is reported from the state of the app, through the real commands", () => {
  const testsAt = commands.indexOf('\n#[cfg(all(test, target_os = "linux"))]');
  const shipped = commands.slice(0, testsAt);
  // Both commands go through the functions the Rust tests run, with the
  // real play and the function that really sends.
  assert.match(
    shipped,
    shape(`Ok(cancel_choice(state, &request_id, |end| report_playback_ended(state, end)).await)`),
  );
  const resolve = shipped.slice(shipped.indexOf('\npub async fn resolve_playback_source_choice('));
  const resolveBody = resolve.slice(0, resolve.indexOf('\n}\n'));
  assert.match(resolveBody, shape(`resolve_choice(
      state,
      &request_id,
      &source_id,
      |pending, source_id| async move {
          play_by_key_locked(`));
  assert.match(resolveBody, shape(`|end| report_playback_ended(state, end),
    )
    .await`));
  // What it sends: a session and the flag, under the view record's lock.
  assert.match(
    shipped,
    shape(`if choice_end_ends_run(nothing_playing, choice_pending, fullscreen_held) {
        state.player_view.report(own, |session| {
            report_end(serde_json::json!({
                "sessionId": session,
                "runOver": true,
            }))
        });
    }`),
  );
  // Every way a choice ends without a play reaches it: no longer held, a
  // source not offered, and a play that neither started nor was launched.
  const choose = shipped.slice(shipped.indexOf('\nasync fn resolve_choice<'));
  const chooseBody = choose.slice(0, choose.indexOf('\n}\n'));
  assert.equal(chooseBody.split('report_choice_end(').length - 1, 3);
  assert.match(chooseBody, shape(`Ok(PlayCommandResult::Started { .. }) => true,
      Err(failure) => failure.launched,
      Ok(_) => false,`));
  const cancel = shipped.slice(shipped.indexOf('\nasync fn cancel_choice('));
  const cancelBody = cancel.slice(0, cancel.indexOf('\n}\n'));
  assert.equal(cancelBody.split('report_choice_end(').length - 1, 1);
});

// pc-2. A successor the dispatcher starts in mpv's own window has
// no view. Its own reports do not end the run unless they say so: it ends
// cleanly and its dispatcher starts the next play, or a step-down replaces
// it (Rust: `a_successor_in_mpv_own_window_keeps_fullscreen_until_the_run_is_over`).
test('a successor in mpv own window keeps fullscreen until a report says the run is over', () => {
  const coming = (sessionId) => ({ kind: 'ended', payload: { sessionId, runOver: false, successor: true } });
  const over = (sessionId) => ({ kind: 'ended', payload: { sessionId, runOver: true, successor: false } });
  const start = [opened('a'), closed('a'), ended('a', false), coming('a')];
  // B ends cleanly; its dispatcher starts C.
  assert.deepEqual(fullscreenLeftAt([...start, ended('b', false), coming('b')]), []);
  // ... or cannot.
  assert.deepEqual(fullscreenLeftAt([...start, ended('b', false), over('b')]), [5]);
  // B is stepped down to D; D plays on.
  assert.deepEqual(fullscreenLeftAt([...start, ended('b', false)]), []);
  // A's view, its play long ended, closes only after all of that.
  assert.deepEqual(
    fullscreenLeftAt([opened('a'), ended('a', false), coming('a'), ended('b', false), over('b'), closed('a')]),
    [4],
  );
  // While a play that still plays has its view showing, nothing leaves.
  assert.deepEqual(fullscreenLeftAt([opened('c'), over('b')]), []);
});

test("whether the view showing is of a play that ended follows the reports and the view's changes", () => {
  assert.equal(showingPlayEnded(null, false, null, { sessionId: 'a' }), false, 'no view shows');
  assert.equal(showingPlayEnded('a', false, 'a', { sessionId: 'a' }), true, 'a report names it');
  assert.equal(showingPlayEnded('a', false, 'a', { sessionId: 'b' }), false, 'a report names another');
  assert.equal(showingPlayEnded('a', true, 'a', { sessionId: 'b' }), true, 'it stays ended');
  assert.equal(showingPlayEnded('a', true, 'a'), true, "another play's view closed");
  assert.equal(showingPlayEnded('a', true, 'b'), false, 'another view opened');
  assert.equal(showingPlayEnded('a', true, null), false, 'the view closed');
});
