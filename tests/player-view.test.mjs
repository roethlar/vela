// The page's side of a play that runs inside Vela's own window
// (.agents/plans/embedded-player.md, "Frontend: the player view", and plan
// review finding ep-2).
import assert from 'node:assert/strict';
import { test } from 'node:test';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { endLeavesPlayerFullscreen, nextPlayerSession } from '../src/lib/playerView.ts';

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
    /listen<\{ sessionId: string \}>\("player-opened", \(\{ payload \}\) => \{\s+playerSession = nextPlayerSession\(playerSession, \{\s+kind: "opened",\s+sessionId: payload\.sessionId,\s+\}\);/,
  );
  assert.match(
    page,
    /listen<\{ sessionId: string \}>\("player-closed", \(\{ payload \}\) => \{\s+playerSession = nextPlayerSession\(playerSession, \{\s+kind: "closed",\s+sessionId: payload\.sessionId,\s+\}\);/,
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
  // case by case below), with what the page knows now.
  assert.match(
    page,
    shape(`endsSinceViewClosed++;
      if (
        endLeavesPlayerFullscreen(event.payload ?? {}, {
          showing: playerSession,
          endsSinceViewChanged: endsSinceViewClosed,
          continuationPending: continuationPending(),
        })
      ) {
        leavePlayerFullscreen();
      }`),
  );
  assert.match(
    page,
    /function continuationPending\(\): boolean \{\s+return continuationsRunning > 0 \|\| continuationSession !== null \|\| sourceChoiceRequest !== null;\s+\}/,
  );
  assert.match(
    page,
    /async function handlePlaybackContinuation\(event: \{ payload: PlaybackContinuation \}\) \{\s+continuationsRunning\+\+;\s+try \{\s+await continueAfterPlayback\(event\.payload\);\s+\} finally \{\s+continuationsRunning--;\s+\}\s+\}/,
  );
  // Both lifecycle events keep the count and the backstop.
  assert.equal([...page.matchAll(/\}\);\s+playerViewChanged\(\);\s+\}\)\.then/g)].length, 2);
  assert.match(page, /const PLAYER_FULLSCREEN_BACKSTOP_MS = 20_000 \+ 30_000;/);
  // The bookkeeping itself, whole: a request for fullscreen that was refused
  // drops the claim; leaving clears the backstop.
  assert.match(
    page,
    shape(`function askPlayerFullscreen(on: boolean) {
      const sessionId = playerSession;
      if (on && sessionId === null) return;
      playerAskedFullscreen = on;
      if (!on) clearPlayerFullscreenBackstop();
      setPlayerFullscreen(sessionId ?? "", on).catch(() => {
        if (on) playerAskedFullscreen = false;
      });
    }`),
  );
  // A view change starts the count of ends again; with a view, no backstop;
  // with none and Vela's claim held, one backstop, which leaves only if
  // still no view shows when it runs out.
  assert.match(
    page,
    shape(`function playerViewChanged() {
      endsSinceViewClosed = 0;
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
// view opening or closing goes through `nextPlayerSession` and starts the
// count of ends again; each end goes through `endLeavesPlayerFullscreen`.
// Returns the index of every event at which fullscreen is left.
function fullscreenLeftAt(events) {
  let showing = null;
  let ends = 0;
  const left = [];
  events.forEach((event, index) => {
    if (event.kind === 'opened' || event.kind === 'closed') {
      showing = nextPlayerSession(showing, event);
      ends = 0;
      return;
    }
    ends++;
    const ctx = { showing, endsSinceViewChanged: ends, continuationPending: event.pending ?? false };
    if (endLeavesPlayerFullscreen(event.payload, ctx)) left.push(index);
  });
  return left;
}
// The tracker's report of an end: its session and whether the run is over.
const ended = (sessionId, runOver) => ({ kind: 'ended', payload: { sessionId, runOver, sourceId: 's', itemKey: 's:k' } });
// The completion dispatcher's second report of a clean end.
const endedAgain = (pending = false) => ({ kind: 'ended', payload: { sourceId: 's', itemKey: 's:k' }, pending });

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
  // A second report that names its session but does not say the run is
  // over keeps the old rule.
  const notOver = { kind: 'ended', payload: { sessionId: 'a', runOver: false, sourceId: 's', itemKey: 's:k' } };
  assert.deepEqual(fullscreenLeftAt([opened('a'), ended('a', false), notOver, opened('b'), closed('a')]), []);
  assert.deepEqual(fullscreenLeftAt([opened('a'), closed('a'), ended('a', false), notOver]), [3]);
});

test("the dispatcher's report of a clean end names its session and says whether the run is over", () => {
  // Over only when the playlist's successor could not be started.
  assert.match(
    lib,
    shape(`let after = commands::advance_playlist(&state, &completion.session_id).await;
      if after == commands::AfterCleanEnd::ContinuePlaying {`),
  );
  assert.match(
    lib,
    shape(`"playback-ended",
      serde_json::json!({
          "sourceId": source_id,
          "itemKey": completion.item_key.clone(),
          "sessionId": completion.session_id.clone(),
          "runOver": after == commands::AfterCleanEnd::RunOver,
      }),`),
  );
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
  // While the view still shows, the rule waits: there the page's own end of
  // the run (`finishPlaybackRun`, after `continue-playing`) leaves it.
  assert.deepEqual(fullscreenLeftAt([opened('a'), ended('a', false), endedAgain(false), closed('a')]), []);
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

// pc-2, reopened: a play that replaced the one showing (a step-down, a play
// the user or the page starts, a playlist's next play) fails to start. The
// old play's own report says "not over" when it comes while the replacement
// is still installed, and "over" when it comes after the failure was undone;
// the failed launch reports the old play's run over. The page leaves at the
// first report that says so, in every order of the three events.
const failedReplacement = (sessionId) => ({ kind: 'ended', payload: { sessionId, runOver: true } });

test('a play that replaced the one showing and fails to start leaves fullscreen at once, in every order, whoever started it', () => {
  const orders = [
    // The old play's report while the replacement is installed.
    [[opened('a'), ended('a', false), failedReplacement('a'), closed('a')], 2],
    [[opened('a'), ended('a', false), closed('a'), failedReplacement('a')], 3],
    [[opened('a'), closed('a'), ended('a', false), failedReplacement('a')], 3],
    // Its report after the failure was undone.
    [[opened('a'), failedReplacement('a'), ended('a', true), closed('a')], 1],
    [[opened('a'), failedReplacement('a'), closed('a'), ended('a', true)], 1],
    [[opened('a'), closed('a'), failedReplacement('a'), ended('a', true)], 2],
    // Its report read the state before the failure, and arrives after.
    [[opened('a'), failedReplacement('a'), ended('a', false), closed('a')], 1],
    [[opened('a'), closed('a'), failedReplacement('a'), ended('a', false)], 2],
  ];
  for (const [events, at] of orders) {
    const left = fullscreenLeftAt(events);
    assert.equal(left[0], at, JSON.stringify(events));
  }
});

test("a failed replacement's report leaves nothing while another play's view shows, and miscounts nothing after", () => {
  assert.deepEqual(fullscreenLeftAt([opened('a'), opened('c'), failedReplacement('a'), closed('a')]), []);
  // A later play's first report of a clean end is still only its first.
  assert.deepEqual(
    fullscreenLeftAt([
      opened('a'),
      ended('a', false),
      failedReplacement('a'),
      closed('a'),
      opened('b'),
      ended('b', false),
      closed('b'),
      ended('b', false),
    ]),
    [2],
  );
});

// pc-2: a launch whose play started in the embedded player (its view was
// announced) and then failed reports that play run over too, right after the
// replaced play's report. With the new play's view showing, the replaced
// play's report leaves nothing; the new play's own leaves, in every order.
test("a failed launch whose own view opened leaves fullscreen at once, in every order", () => {
  const orders = [
    [[opened('a'), opened('n'), closed('a'), ended('a', false), failedReplacement('a'), failedReplacement('n'), closed('n')], 5],
    [[opened('a'), opened('n'), ended('a', false), failedReplacement('a'), closed('n'), failedReplacement('n'), closed('a')], 5],
    [[opened('a'), opened('n'), closed('n'), closed('a'), ended('a', false), failedReplacement('a'), failedReplacement('n')], 5],
    [[opened('a'), ended('a', false), opened('n'), failedReplacement('a'), failedReplacement('n'), closed('a'), closed('n')], 4],
    [[opened('a'), opened('n'), failedReplacement('a'), failedReplacement('n'), ended('a', true), closed('a'), closed('n')], 3],
    // A first play: nothing replaced, only its own report.
    [[opened('n'), failedReplacement('n'), closed('n')], 1],
    [[opened('n'), closed('n'), failedReplacement('n')], 2],
  ];
  for (const [events, at] of orders) {
    assert.equal(fullscreenLeftAt(events)[0], at, JSON.stringify(events));
  }
});

test('a launch that fails after replacing a play reports that play run over, and no other path does', () => {
  // The sequence itself (the replaced play read before anything is
  // installed, the report given only when the launch fails and nothing is
  // playing) is run by the Rust tests of `install_and_launch`. What is held
  // here is the wiring around it: the play path starts every play through
  // it, hands it the function that really sends the report, and sends no
  // such report of its own anywhere else.
  const testsAt = commands.indexOf('\n#[cfg(test)]\nmod tests {');
  assert.notEqual(testsAt, -1, 'the command layer has its test module');
  const shipped = commands.slice(0, testsAt);
  const playPath = shipped.slice(shipped.indexOf('\nasync fn play_by_key_locked('));
  const playBody = playPath.slice(0, playPath.indexOf('\n}\n'));
  assert.match(
    playBody,
    shape(`let stop = install_and_launch(
        state,
        PlayInstall {`),
  );
  assert.match(
    playBody,
    shape(`launch,
        |end| report_playback_ended(state, end),
    )
    .await
    .map_err(PlayFailure::fatal)?;`),
  );
  // What sends it: `playback-ended`, with the value it is given.
  assert.match(
    shipped,
    shape(`fn report_playback_ended(state: &AppState, end: serde_json::Value) {
        if let Some(app) = state.app_handle.get() {
            use tauri::Emitter;
            let _ = app.emit("playback-ended", end);
        }
    }`),
  );
  // Nothing else in the command layer sends it or reads the play to report:
  // the play path's only `playback-ended` is its tracker's own report, and a
  // play that fails at its resolve, or starts, sends nothing more.
  assert.equal(shipped.split('report_playback_ended(').length - 1, 2, 'defined once and passed once');
  assert.equal(shipped.split('replaced_embedded_session(').length - 1, 2, 'defined once and read once');
  assert.match(
    shipped,
    shape(`let replaced = replaced_embedded_session(state, install.session_id).await;`),
  );
  assert.equal(shipped.split('"playback-ended"').length - 1, 2, "the tracker's report and the failed launch's");
  assert.equal(playBody.split('"playback-ended"').length - 1, 1, "the play path's own is the tracker's report");
  assert.match(playBody, /let run_over = tracker_end_is_run_over\([\s\S]{0,400}"playback-ended",\s+serde_json::json!\(\{/);
  // The tracker's verdict is the function the Rust tests run, which notes an
  // end told "the run goes on" for the launch to find: the tracker calls it,
  // and nothing else does.
  assert.equal(shipped.split('tracker_end_is_run_over(').length - 1, 2, 'defined once and called once, by the tracker');
  // What it reports: a session and the flag, nothing else.
  assert.match(
    commands,
    shape(`.map(|session| {
        serde_json::json!({
            "sessionId": session,
            "runOver": true,
        })
    })`),
  );
});

test('a report that does not say the run is over never leaves on its own', () => {
  assert.deepEqual(fullscreenLeftAt([opened('a'), closed('a'), endedAgain()]), []);
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
