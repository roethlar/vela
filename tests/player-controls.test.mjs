// The controls of the player view (.agents/plans/embedded-player.md, "Frontend:
// the player view", and its Slice 5 design): the closed list of commands the
// page may send, and the rules of the keys, the time, the bar and the seek bar.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  PLAYER_COMMANDS,
  PICTURE_CLICK_MS,
  SEEK_IDLE,
  WHEEL_IDLE,
  barHideAt,
  barShown,
  canSeek,
  commandNotice,
  focusKindOf,
  formatTime,
  fullscreenRequests,
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
  skipLabel,
  spokenTime,
  takeMouseCommand,
  wheelCommands,
} from '../src/lib/playerControls.ts';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const read = (file) => fs.readFileSync(path.join(repoRoot, file), 'utf8').replace(/\r\n/g, '\n');

function filesBelow(dir, suffixes) {
  return fs
    .readdirSync(dir, { withFileTypes: true })
    .flatMap((entry) => {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) return filesBelow(full, suffixes);
      return suffixes.some((suffix) => full.endsWith(suffix)) ? [full] : [];
    })
    .sort();
}

const sources = new Map(
  filesBelow(path.join(repoRoot, 'src'), ['.svelte', '.ts', '.js']).map((file) => [
    path.relative(repoRoot, file).split(path.sep).join('/'),
    read(path.relative(repoRoot, file)),
  ]),
);

// A call of `invoke`, typed or not: `invoke(` and `invoke<T>(` alike.
const INVOKE = /\binvoke\s*(?:<[^>()]*>)?\s*\(/g;

test('the names the page may send are the list Rust parses', () => {
  const names = JSON.parse(read('src/lib/playerCommands.json'));
  assert.deepEqual([...PLAYER_COMMANDS], names);
  assert.equal(new Set(names).size, names.length);
});

test('every Node test file is run by the check script', () => {
  // The check lists its files by name; one left out would stop running
  // without a word. (tests/player-command.test.mjs holds the same.)
  const check = JSON.parse(read('package.json')).scripts.check;
  for (const file of fs.readdirSync(path.join(repoRoot, 'tests')).filter((name) => name.endsWith('.test.mjs'))) {
    assert.ok(check.split(/\s+/).includes(`tests/${file}`), `tests/${file} is in the check script`);
  }
});

test('the notice line shows every refusal the backend gives as it is', () => {
  // The backend's five texts; a Rust test holds this list equal to them.
  const refusals = JSON.parse(read('src/lib/playerRefusals.json'));
  assert.equal(refusals.length, 5);
  for (const refusal of refusals) assert.equal(commandNotice(refusal), refusal);
});

test('one function sends player commands, and only player_command', () => {
  const controls = sources.get('src/lib/playerControls.ts');
  const calls = [...controls.matchAll(INVOKE)].map((m) => controls.slice(m.index, m.index + 60));
  assert.deepEqual(
    calls.map((call) => call.match(/\(\s*"([a-z_]+)"/)?.[1]),
    ['player_command', 'player_fullscreen'],
    'the module invokes those two commands and no others',
  );
  assert.match(
    controls,
    /export function sendPlayerCommand<N extends PlayerCommandName>\(\s+sessionId: string,\s+name: N,\s+args: PlayerCommandArgs\[N\],\s+\): Promise<void> \{\s+return invoke\("player_command", \{ sessionId, name, args \}\);\s+\}/,
  );
  // Nothing else in the page reaches the command by its name.
  for (const [file, source] of sources) {
    if (file === 'src/lib/playerControls.ts') continue;
    assert.doesNotMatch(source, /player_command|player_fullscreen/, `${file} names the command itself`);
  }
});

test('the player view calls invoke only to report where it is', () => {
  const view = sources.get('src/lib/PlayerView.svelte');
  const calls = [...view.matchAll(INVOKE)].map((m) => view.slice(m.index, m.index + 40));
  assert.equal(calls.length, 1, `the view calls invoke once: ${calls.join(' | ')}`);
  assert.match(calls[0], /^invoke\("player_set_rect",/);
  // Every command of the view goes through the one wrapper, in one place.
  assert.equal(view.split('sendPlayerCommand(').length - 1, 1);
  assert.match(
    view,
    /function deliver\(command: PlayerCommand\) \{\s+commandsPending\+\+;\s+sendPlayerCommand\(sessionId, command\.name, command\.args\)/,
  );
});

// ---------------------------------------------------------------------------
// Keys.

const key = (k, extra = {}) => ({
  key: k,
  ctrlKey: false,
  altKey: false,
  metaKey: false,
  repeat: false,
  ...extra,
});
const ctx = (extra = {}) => ({
  focus: 'none',
  listOpen: false,
  hasState: true,
  stopped: false,
  skip: false,
  crop: null,
  fullscreen: false,
  ...extra,
});
const command = (name, args = []) => ({ kind: 'command', command: { name, args } });
const PASS = { kind: 'pass' };
const SWALLOW = { kind: 'swallow' };

test("the page maps mpv's own keys", () => {
  assert.deepEqual(keyAction(key(' '), ctx()), command('toggle-pause'));
  assert.deepEqual(keyAction(key('ArrowLeft'), ctx()), command('seek-relative', [-5]));
  assert.deepEqual(keyAction(key('ArrowRight'), ctx()), command('seek-relative', [5]));
  assert.deepEqual(keyAction(key('ArrowUp'), ctx()), command('seek-relative', [60]));
  assert.deepEqual(keyAction(key('ArrowDown'), ctx()), command('seek-relative', [-60]));
  assert.deepEqual(keyAction(key('m'), ctx()), command('toggle-mute'));
  assert.deepEqual(keyAction(key('q'), ctx()), command('stop'));
  assert.deepEqual(keyAction(key('f'), ctx()), { kind: 'fullscreen', on: true });
  assert.deepEqual(keyAction(key('f'), ctx({ fullscreen: true })), { kind: 'fullscreen', on: false });
  // Shifted letters are other keys in mpv: not the page's.
  assert.deepEqual(keyAction(key('M'), ctx()), PASS);
  assert.deepEqual(keyAction(key('F'), ctx()), PASS);
  assert.deepEqual(keyAction(key('Q'), ctx()), PASS);
  assert.deepEqual(keyAction(key('x'), ctx()), PASS);
  assert.deepEqual(keyAction(key('Tab'), ctx()), PASS);
  assert.deepEqual(keyAction(key('Enter'), ctx()), PASS);
});

test('Escape leaves fullscreen when the window is fullscreen, and stops otherwise', () => {
  assert.deepEqual(keyAction(key('Escape'), ctx({ fullscreen: true })), { kind: 'fullscreen', on: false });
  assert.deepEqual(keyAction(key('Escape'), ctx()), command('stop'));
  // A held Escape cannot leave fullscreen and then stop.
  assert.deepEqual(keyAction(key('Escape', { repeat: true }), ctx()), SWALLOW);
});

test('Space skips while the skip button shows, and pauses otherwise', () => {
  assert.deepEqual(keyAction(key(' '), ctx({ skip: true })), command('skip-active-marker'));
  assert.deepEqual(keyAction(key(' '), ctx({ skip: false })), command('toggle-pause'));
});

test('C crops only with a crop script, whether or not Shift is held', () => {
  assert.deepEqual(keyAction(key('C'), ctx({ crop: 'off' })), command('toggle-autocrop'));
  assert.deepEqual(keyAction(key('C'), ctx({ crop: 'on' })), command('toggle-autocrop'));
  // The key does not carry Shift through `shiftKey` reliably; it is not read.
  assert.deepEqual(keyAction(key('C', { shiftKey: true }), ctx({ crop: 'off' })), command('toggle-autocrop'));
  assert.deepEqual(keyAction(key('C', { shiftKey: false }), ctx({ crop: 'off' })), command('toggle-autocrop'));
  assert.deepEqual(keyAction(key('C'), ctx({ crop: 'measuring' })), SWALLOW);
  assert.deepEqual(keyAction(key('C'), ctx({ crop: null })), PASS);
  assert.deepEqual(keyAction(key('c'), ctx({ crop: 'off' })), PASS);
});

test('keys with Ctrl, Alt or Meta, Unidentified keys and bare modifiers are not acted on', () => {
  for (const k of [' ', 'ArrowLeft', 'm', 'f', 'q', 'Escape', 'C']) {
    for (const modifier of ['ctrlKey', 'altKey', 'metaKey']) {
      assert.deepEqual(keyAction(key(k, { [modifier]: true }), ctx({ crop: 'off' })), PASS, `${modifier}+${k}`);
    }
  }
  // Typing a shifted C delivers `C` and then a key named Unidentified.
  assert.deepEqual(keyAction(key('Unidentified'), ctx({ listOpen: true })), PASS);
  for (const k of ['Shift', 'Control', 'Alt', 'Meta', 'AltGraph', 'CapsLock']) {
    assert.deepEqual(keyAction(key(k), ctx({ listOpen: true })), PASS, k);
  }
});

test('auto-repeat acts only for the seek keys', () => {
  assert.deepEqual(keyAction(key('ArrowLeft', { repeat: true }), ctx()), command('seek-relative', [-5]));
  assert.deepEqual(keyAction(key('ArrowUp', { repeat: true }), ctx()), command('seek-relative', [60]));
  for (const k of [' ', 'm', 'f', 'q', 'Escape', 'C']) {
    assert.deepEqual(keyAction(key(k, { repeat: true }), ctx({ crop: 'off', fullscreen: true })), SWALLOW, k);
  }
});

test('a focused control keeps its own keys and no others', () => {
  // A button: Space and Enter click it.
  assert.deepEqual(keyAction(key(' '), ctx({ focus: 'button' })), PASS);
  assert.deepEqual(keyAction(key('Enter'), ctx({ focus: 'button' })), PASS);
  assert.deepEqual(keyAction(key('ArrowLeft'), ctx({ focus: 'button' })), command('seek-relative', [-5]));
  // The volume slider: the arrows step it.
  for (const k of ['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown']) {
    assert.deepEqual(keyAction(key(k), ctx({ focus: 'volume' })), PASS, k);
  }
  assert.deepEqual(keyAction(key(' '), ctx({ focus: 'volume' })), command('toggle-pause'));
  // The seek bar: the arrows seek as anywhere else, not by the bar's step.
  assert.deepEqual(keyAction(key('ArrowLeft'), ctx({ focus: 'seek' })), command('seek-relative', [-5]));
  assert.deepEqual(keyAction(key('ArrowUp'), ctx({ focus: 'seek' })), command('seek-relative', [60]));
  // f, m, q, C and Escape everywhere.
  for (const focus of ['button', 'volume', 'seek']) {
    assert.deepEqual(keyAction(key('m'), ctx({ focus })), command('toggle-mute'), focus);
    assert.deepEqual(keyAction(key('q'), ctx({ focus })), command('stop'), focus);
    assert.deepEqual(keyAction(key('Escape'), ctx({ focus })), command('stop'), focus);
    assert.deepEqual(keyAction(key('C'), ctx({ focus, crop: 'off' })), command('toggle-autocrop'), focus);
    assert.deepEqual(keyAction(key('f'), ctx({ focus })), { kind: 'fullscreen', on: true }, focus);
  }
});

test("the key map is told which of the view's controls has focus", () => {
  const element = (tagName, ...classes) => ({
    tagName,
    classList: { contains: (name) => classes.includes(name) },
  });
  assert.equal(focusKindOf(null), 'none');
  assert.equal(focusKindOf(element('BUTTON', 'playpause')), 'button');
  assert.equal(focusKindOf(element('BUTTON', 'close')), 'button');
  assert.equal(focusKindOf(element('BUTTON', 'audio')), 'button');
  assert.equal(focusKindOf(element('BUTTON', 'subtitles')), 'button');
  assert.equal(focusKindOf(element('INPUT', 'volume')), 'volume');
  assert.equal(focusKindOf(element('INPUT', 'seek')), 'seek');
  assert.equal(focusKindOf(element('DIV', 'player')), 'none');
  assert.equal(focusKindOf(element('SPAN', 'time')), 'none');
  // The view hands over only its own controls: focus elsewhere is none of its.
  const view = read('src/lib/PlayerView.svelte');
  assert.match(
    view,
    /function focusKind\(element: Element \| null\): FocusKind \{\s+return focusKindOf\(element !== null && root\.contains\(element\) \? element : null\);\s+\}/,
  );
  assert.equal(view.split('focusKind(').length - 1, 2, 'defined, and asked by the key handler');
});

test('before the first state only stop and fullscreen act', () => {
  const early = ctx({ hasState: false, crop: 'off' });
  assert.deepEqual(keyAction(key('q'), early), command('stop'));
  assert.deepEqual(keyAction(key('Escape'), early), command('stop'));
  assert.deepEqual(keyAction(key('f'), early), { kind: 'fullscreen', on: true });
  for (const k of [' ', 'ArrowLeft', 'ArrowUp', 'm', 'C']) {
    assert.deepEqual(keyAction(key(k), early), SWALLOW, k);
  }
});

test('after stop nothing acts', () => {
  const gone = ctx({ stopped: true, crop: 'off' });
  for (const k of [' ', 'ArrowLeft', 'm', 'f', 'q', 'Escape', 'C']) {
    assert.deepEqual(keyAction(key(k), gone), SWALLOW, k);
  }
  assert.deepEqual(keyAction(key('Escape'), { ...gone, fullscreen: true }), SWALLOW);
});

test('an open track list owns navigation, selection and Escape before focused controls', () => {
  const actions = {
    ArrowUp: 'previous', ArrowDown: 'next', Home: 'first', End: 'last',
    Enter: 'choose', ' ': 'choose', Escape: 'close',
  };
  for (const focus of ['none', 'button', 'seek', 'volume']) {
    for (const fullscreen of [false, true]) {
      const open = ctx({ focus, fullscreen, listOpen: true, skip: true });
      for (const [k, action] of Object.entries(actions)) {
        assert.deepEqual(keyAction(key(k), open), { kind: 'track-list', action }, `${focus}: ${k}`);
      }
      assert.deepEqual(keyAction(key('Tab'), open), PASS, 'Tab may move focus out');
      for (const k of ['ArrowUp', 'ArrowDown', 'Home', 'End']) {
        assert.deepEqual(keyAction(key(k, { repeat: true }), open), {
          kind: 'track-list', action: actions[k],
        });
      }
      for (const k of ['Enter', ' ', 'Escape']) {
        assert.deepEqual(keyAction(key(k, { repeat: true }), open), SWALLOW);
      }
    }
  }
});

test('an open track list suppresses player shortcuts except q', () => {
  for (const crop of [null, 'off', 'on', 'measuring']) {
    const open = ctx({ listOpen: true, crop, fullscreen: true, skip: true });
    for (const k of ['ArrowLeft', 'ArrowRight', 'm', 'f', 'C', 'x']) {
      assert.deepEqual(keyAction(key(k), open), SWALLOW, k);
    }
    assert.deepEqual(keyAction(key('q'), open), command('stop'));
    assert.deepEqual(keyAction(key('q', { repeat: true }), open), SWALLOW);
    assert.deepEqual(keyAction(key('q'), { ...open, stopped: true }), SWALLOW);
  }
});

test('closing a track list restores the ordinary Escape and Space rules', () => {
  for (const fullscreen of [false, true]) {
    const open = ctx({ listOpen: true, fullscreen });
    assert.deepEqual(keyAction(key('Escape'), open), { kind: 'track-list', action: 'close' });
    const closed = { ...open, listOpen: false };
    assert.deepEqual(keyAction(key('Escape', { repeat: true }), closed), SWALLOW);
    assert.deepEqual(keyAction(key('Escape'), closed), fullscreen
      ? { kind: 'fullscreen', on: false } : command('stop'));
    assert.deepEqual(keyAction(key(' '), closed), command('toggle-pause'));
  }
});

test('track highlights move within the live list without selecting a track', () => {
  const ids = ['no', 4, 7];
  assert.equal(nextTrackHighlight(ids, 4, 'previous'), 'no');
  assert.equal(nextTrackHighlight(ids, 4, 'next'), 7);
  assert.equal(nextTrackHighlight(ids, 4, 'first'), 'no');
  assert.equal(nextTrackHighlight(ids, 'no', 'last'), 7);
  assert.equal(nextTrackHighlight(ids, 'no', 'previous'), 'no');
  assert.equal(nextTrackHighlight(ids, 7, 'next'), 7);
  assert.equal(nextTrackHighlight([7, 4, 'no'], 4, 'next'), 'no', 'use the current order and track ID');
  assert.equal(nextTrackHighlight([], null, 'next'), null);
  assert.deepEqual(ids, ['no', 4, 7]);
});

// ---------------------------------------------------------------------------
// The picture.

test('picture clicks wait 250 ms, right-clicks pause immediately, and other targets/buttons do nothing', () => {
  const picture = {};
  const input = { button: 0, detail: 1, target: picture, currentTarget: picture };
  const context = { primaryPressOnPicture: true, listOpen: false, pressedWithList: false, stopped: false, hasState: true };
  assert.equal(PICTURE_CLICK_MS, 250);
  assert.equal(pictureClickAction(input, context), 'wait');
  assert.equal(pictureClickAction({ ...input, detail: 2 }, context), 'none', 'second click cancels the wait');
  assert.equal(pictureClickAction({ ...input, detail: 3 }, context), 'wait', 'third click is a first click again');
  assert.equal(pictureClickAction({ ...input, detail: 4 }, context), 'none', 'the next pair also ignores its even click');
  assert.equal(pictureClickAction({ ...input, button: 2 }, context), 'pause');
  for (const button of [1, 3, 4]) {
    assert.equal(pictureClickAction({ ...input, button }, context), 'none');
  }
  for (const control of ['bar', 'button', 'seek', 'volume', 'time', 'skip', 'track-list', 'track-option']) {
    for (const button of [0, 2]) {
      assert.equal(pictureClickAction({ ...input, target: { control }, button }, context), 'none', `${control}, ${button}`);
    }
  }
  for (const extra of [{ listOpen: true }, { pressedWithList: true }]) {
    assert.equal(pictureClickAction(input, { ...context, ...extra }), 'none', 'a list dismissal owns the click');
  }
  assert.equal(pictureClickAction({ ...input, button: 2 }, { ...context, listOpen: true }), 'pause',
    'right-click on the picture pauses without dismissing the list');
  for (const extra of [{ stopped: true }, { hasState: false }]) {
    for (const button of [0, 2]) {
      assert.equal(pictureClickAction({ ...input, button }, { ...context, ...extra }), 'none');
    }
  }
});

test('picture clicks without a primary picture press are ignored, including even clicks', () => {
  const picture = {};
  const context = { primaryPressOnPicture: false, listOpen: false, pressedWithList: false, stopped: false, hasState: true };
  for (const detail of [1, 2, 3, 4]) {
    assert.equal(pictureClickAction({ button: 0, detail, target: picture, currentTarget: picture }, context), 'ignore');
  }
  assert.equal(pictureClickAction({ button: 2, detail: 1, target: picture, currentTarget: picture }, context), 'pause',
    'contextmenu still pauses without a primary press');
});

test('only a primary double-click on the picture toggles fullscreen, without a list press or stop', () => {
  const picture = {};
  const input = { button: 0, target: picture, currentTarget: picture };
  const context = { primaryPressOnPicture: true, firstClickTarget: picture, firstClickPaused: false, listOpen: false, pressedWithList: false, stopped: false, hasState: true };
  assert.equal(pictureDoubleClick(input, context), 'fullscreen', 'the picture toggles');
  assert.equal(pictureDoubleClick(input, { ...context, firstClickPaused: true }), 'restore-pause',
    'a slow native pair restores pause and toggles fullscreen regardless of elapsed time');
  for (const control of ['bar', 'button', 'seek', 'volume', 'time', 'audio', 'subtitles', 'skip', 'track-list', 'track-option']) {
    assert.equal(
      pictureDoubleClick({ ...input, target: { control } }, context),
      'none',
      `${control} does not toggle`,
    );
    assert.equal(
      pictureDoubleClick(input, { ...context, firstClickTarget: { control } }),
      'none',
      `${control} disappearing after the first click does not turn the pair into a picture double-click`,
    );
  }
  for (const firstClickPaused of [false, true]) {
    for (const extra of [{ primaryPressOnPicture: false }, { listOpen: true }, { pressedWithList: true }, { stopped: true }, { hasState: false }]) {
      assert.equal(pictureDoubleClick(input, { ...context, firstClickPaused, ...extra }), 'none',
        `the same gates apply before and after the first pause: ${JSON.stringify(extra)}`);
    }
  }
  for (const button of [1, 2, 3, 4]) {
    assert.equal(pictureDoubleClick({ ...input, button }, context), 'none', `button ${button} does not toggle`);
  }
});

test('a slow native pair reverses the 250 ms pause before queued wheels once the answer arrives', (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const picture = {};
  const input = { button: 0, detail: 1, target: picture, currentTarget: picture };
  const context = {
    primaryPressOnPicture: true,
    listOpen: false, pressedWithList: false, stopped: false, hasState: true,
    firstClickTarget: picture, firstClickPaused: false,
  };
  const ready = { busy: false, stopped: false, hasState: true };
  const pause = { name: 'toggle-pause', args: [] };
  let paused = false;
  let fullscreen = false;
  assert.equal(pictureClickAction(input, context), 'wait');
  setTimeout(() => {
    const first = takeMouseCommand(queuePicturePause([]), ready);
    assert.deepEqual(first.command, pause);
    paused = !paused;
    context.firstClickPaused = true;
  }, PICTURE_CLICK_MS);
  t.mock.timers.tick(249);
  assert.equal(paused, false, 'the first click still waits at 249 ms');
  t.mock.timers.tick(1);
  assert.equal(paused, true, 'the first click pauses at 250 ms');
  t.mock.timers.tick(70);
  assert.equal(pictureClickAction({ ...input, detail: 2 }, context), 'none', 'the slow second click never pauses');
  const action = pictureDoubleClick(input, context);
  assert.equal(action, 'restore-pause');
  const wheel = { name: 'set-volume', args: [42] };
  const pending = queuePicturePause([wheel]);
  fullscreen = !fullscreen;
  assert.equal(fullscreen, true, 'the native dblclick toggles fullscreen even while pause awaits its answer');
  const waiting = takeMouseCommand(pending, { ...ready, busy: true });
  assert.equal(waiting.command, null, 'the first toggle may still be unanswered');
  assert.deepEqual(waiting.pending, [pause, wheel], 'restoration is retained ahead of wheel input');
  const answer = takeMouseCommand(waiting.pending, ready);
  assert.deepEqual(answer.command, pause, 'the next answer releases the restoration');
  paused = !paused;
  assert.equal(paused, false, 'the double-click leaves playback as it was');
  assert.deepEqual(takeMouseCommand(answer.pending, ready).command, wheel, 'only then can the wheel run');
  assert.deepEqual(takeMouseCommand(pending, { ...ready, busy: true, stopped: true }),
    { pending: [], command: null }, 'a stopped or disposed view drops the restoration and wheels');
});

// ---------------------------------------------------------------------------
// Wheel steps and unanswered commands.

const wheel = (previous, deltaX, deltaY, now, deltaMode = 0) =>
  nextWheelSteps(previous, { deltaX, deltaY, deltaMode }, now);

test('wheel notches are 40 pixels; small deltas accumulate and keep their remainder', () => {
  assert.deepEqual(wheel(WHEEL_IDLE, 0, -40, 0).steps, { x: 0, y: -1 });
  let next = wheel(WHEEL_IDLE, 0, 15, 0);
  assert.equal(next.steps.y, 0);
  next = wheel(next.accumulator, 0, 15, 100);
  assert.equal(next.steps.y, 0);
  next = wheel(next.accumulator, 0, 15, 200);
  assert.equal(next.steps.y, 1);
  assert.equal(next.accumulator.y.remainder, 5);
  next = wheel(next.accumulator, 0, 115, 300);
  assert.equal(next.steps.y, 3, 'all steps in a large event count');
  assert.equal(next.accumulator.y.remainder, 0);
});

test('wheel remainders expire after 500 ms or reversal, independently on each axis', () => {
  const start = wheel(WHEEL_IDLE, 20, 30, 100).accumulator;
  assert.equal(wheel(start, 0, 10, 599).steps.y, 1);
  const expired = wheel(start, 0, 10, 600);
  assert.equal(expired.steps.y, 0);
  assert.equal(expired.accumulator.y.remainder, 10);
  const reversed = wheel(start, 0, -20, 200);
  assert.equal(reversed.accumulator.y.remainder, -20);
  const done = wheel(reversed.accumulator, 20, -20, 300);
  assert.deepEqual(done.steps, { x: 1, y: -1 }, 'vertical reversal did not discard horizontal remainder');
  const otherAxis = wheel(start, 0, 10, 550).accumulator;
  assert.equal(wheel(otherAxis, 20, 0, 600).steps.x, 0, 'vertical activity did not extend horizontal lifetime');
});

test('three wheel lines or one page make one step', () => {
  assert.deepEqual(wheel(WHEEL_IDLE, 3, -3, 100, 1).steps, { x: 1, y: -1 });
  let next = wheel(WHEEL_IDLE, 0, 1, 100, 1);
  next = wheel(next.accumulator, 0, 1, 200, 1);
  assert.equal(next.steps.y, 0);
  next = wheel(next.accumulator, 0, 1, 300, 1);
  assert.equal(next.steps.y, 1);
  assert.deepEqual(wheel(WHEEL_IDLE, -1, 1, 100, 2).steps, { x: -1, y: 1 });
});

test('wheel targets map to absolute volume or relative seek, with state and stop gates', () => {
  const context = { hasState: true, stopped: false };
  const volume = (value) => ({ name: 'set-volume', args: [value] });
  const seek = (value) => ({ name: 'seek-relative', args: [value] });
  assert.deepEqual(wheelCommands('picture', { x: 1, y: -1 }, 40, context), [volume(42), seek(10)]);
  assert.deepEqual(wheelCommands('picture', { x: -1, y: 1 }, 40, context), [volume(38), seek(-10)]);
  for (const steps of [{ x: 0, y: -1 }, { x: 1, y: 0 }]) {
    assert.deepEqual(wheelCommands('seek', steps, 40, context), [seek(10)]);
    assert.deepEqual(wheelCommands('volume', steps, 40, context), [volume(42)]);
  }
  for (const steps of [{ x: 0, y: 1 }, { x: -1, y: 0 }]) {
    assert.deepEqual(wheelCommands('seek', steps, 40, context), [seek(-10)]);
    assert.deepEqual(wheelCommands('volume', steps, 40, context), [volume(38)]);
  }
  assert.deepEqual(wheelCommands('volume', { x: 0, y: -10 }, 95, context), [volume(100)]);
  assert.deepEqual(wheelCommands('volume', { x: 0, y: 10 }, 5, context), [volume(0)]);
  for (const target of ['none', 'list']) {
    assert.deepEqual(wheelCommands(target, { x: 1, y: -1 }, 40, context), []);
  }
  for (const extra of [{ hasState: false }, { stopped: true }]) {
    assert.deepEqual(wheelCommands('picture', { x: 1, y: -1 }, 40, { ...context, ...extra }), []);
  }
});

test('wheel bursts wait for the answer, merge volume targets and seek seconds, and stop drains nothing', () => {
  const ready = { busy: false, stopped: false, hasState: true };
  let pending = [];
  let bar = SEEK_IDLE;
  const enqueueVolume = () => {
    const [command] = wheelCommands('picture', { x: 0, y: -1 }, seekBarValue(bar, 40, 100), ready);
    bar = nextSeekBar(bar, { kind: 'change', value: command.args[0], now: 100 });
    pending = mergeWheelCommand(pending, command);
  };
  enqueueVolume();
  const first = takeMouseCommand(pending, ready);
  assert.deepEqual(first.command, { name: 'set-volume', args: [42] });
  pending = first.pending;
  for (let i = 0; i < 4; i++) {
    enqueueVolume();
    const busy = takeMouseCommand(pending, { ...ready, busy: true });
    assert.equal(busy.command, null, 'no second unanswered command');
    pending = busy.pending;
  }
  assert.equal(seekBarValue(bar, 40, 100), 50, 'all five notches show before any state reports them');
  assert.deepEqual(pending, [{ name: 'set-volume', args: [50] }]);
  for (const seconds of [10, 10, -10, 20]) {
    pending = mergeWheelCommand(pending, { name: 'seek-relative', args: [seconds] });
  }
  const volumeAnswer = takeMouseCommand(pending, ready);
  assert.deepEqual(volumeAnswer.command, { name: 'set-volume', args: [50] });
  const seekAnswer = takeMouseCommand(volumeAnswer.pending, ready);
  assert.deepEqual(seekAnswer.command, { name: 'seek-relative', args: [30] });
  assert.deepEqual(seekAnswer.pending, []);
  assert.deepEqual(mergeWheelCommand(volumeAnswer.pending, { name: 'seek-relative', args: [-30] }), []);
  for (const extra of [{ stopped: true }, { hasState: false }]) {
    assert.deepEqual(takeMouseCommand(pending, { ...ready, ...extra }), { pending: [], command: null });
  }
});

test('queued seeks split at the backend limit and retain every remaining second', () => {
  const ready = { busy: false, stopped: false, hasState: true };
  const seek = (seconds) => ({ name: 'seek-relative', args: [seconds] });
  const volume = { name: 'set-volume', args: [42] };
  for (const sign of [1, -1]) {
    for (const [total, chunks] of [[600, [600]], [610, [600, 10]], [1210, [600, 600, 10]], [600.5, [600, 0.5]]]) {
      let pending = [seek(sign * total), volume];
      for (const chunk of chunks) {
        const waiting = takeMouseCommand(pending, { ...ready, busy: true });
        assert.deepEqual(waiting, { pending, command: null }, 'waiting never consumes a remainder');
        const next = takeMouseCommand(pending, ready);
        assert.deepEqual(next.command, seek(sign * chunk), `${sign * total} seconds`);
        pending = next.pending;
      }
      assert.deepEqual(pending, [volume], 'the following command keeps its place');
    }
  }
});

test('seek hover time uses thumb centres at both ends and at the middle', () => {
  assert.equal(seekTimeAtPointer(100, 100, 216, 180), 0);
  assert.equal(seekTimeAtPointer(108, 100, 216, 180), 0);
  assert.equal(seekTimeAtPointer(208, 100, 216, 180), 90);
  assert.equal(seekTimeAtPointer(308, 100, 216, 180), 180);
  assert.equal(seekTimeAtPointer(316, 100, 216, 180), 180);
  assert.equal(seekTimeAtPointer(158, 100, 216, 180), 45, 'a quarter of thumb travel');
  for (const duration of [null, 0, Number.NaN, -1]) {
    assert.equal(seekTimeAtPointer(208, 100, 216, duration), null);
  }
});

// ---------------------------------------------------------------------------
// Time.

test('time reads m:ss, h:mm:ss from an hour, and --:-- without a duration', () => {
  assert.equal(formatTime(0, 90), '0:00 / 1:30');
  assert.equal(formatTime(65.9, 600), '1:05 / 10:00');
  assert.equal(formatTime(59, 3599), '0:59 / 59:59');
  assert.equal(formatTime(59, 3600), '0:00:59 / 1:00:00');
  assert.equal(formatTime(3725, 7200), '1:02:05 / 2:00:00');
  assert.equal(formatTime(5, null), '0:05 / --:--');
  assert.equal(formatTime(5, 0), '0:05 / --:--');
  assert.equal(formatTime(5, Number.NaN), '0:05 / --:--');
  assert.equal(formatTime(null, null), '0:00 / --:--');
  // Positions that cannot be shown are clamped.
  assert.equal(formatTime(-3, 90), '0:00 / 1:30');
  assert.equal(formatTime(Number.NaN, 90), '0:00 / 1:30');
  assert.equal(formatTime(Number.POSITIVE_INFINITY, 90), '0:00 / 1:30');
  assert.equal(spokenTime(65, 600), '1:05 of 10:00');
  assert.equal(spokenTime(65, null), '1:05');
  assert.equal(canSeek(90), true);
  assert.equal(canSeek(0), false);
  assert.equal(canSeek(null), false);
  assert.equal(canSeek(Number.NaN), false);
});

// ---------------------------------------------------------------------------
// The bar.

test('the bar hides three seconds after the last activity, only while playing', () => {
  const at = (now, extra = {}) => ({ now, lastActivity: 1000, paused: false, focusInBar: false, listOpen: false, ...extra });
  assert.equal(barShown(at(1000)), true);
  assert.equal(barShown(at(3999)), true);
  assert.equal(barShown(at(4000)), false);
  assert.equal(barHideAt(at(1000)), 4000);
  assert.equal(barHideAt(at(4000)), null);
  // Paused, before the first state, or with a control focused, it stays.
  for (const extra of [{ paused: true }, { paused: null }, { focusInBar: true }]) {
    assert.equal(barShown(at(60_000, extra)), true, JSON.stringify(extra));
    assert.equal(barHideAt(at(1000, extra)), null, JSON.stringify(extra));
  }
  // Activity brings it back.
  assert.equal(barShown(at(60_000, { lastActivity: 59_000 })), true);
});

test('an open track list keeps the bar shown with no hide deadline', () => {
  for (const now of [1000, 4000, 60_000, 3_600_000]) {
    const open = { now, lastActivity: 1000, paused: false, focusInBar: false, listOpen: true };
    assert.equal(barShown(open), true);
    assert.equal(barHideAt(open), null);
    const closed = { ...open, listOpen: false };
    assert.equal(barShown(closed), now < 4000);
    assert.equal(barHideAt(closed), now < 4000 ? 4000 : null);
  }
});

// ---------------------------------------------------------------------------
// The seek bar.

test('the seek bar follows the play, the drag, then its target until the play gets there', () => {
  let bar = SEEK_IDLE;
  assert.equal(seekBarValue(bar, 10, 0), 10);
  bar = nextSeekBar(bar, { kind: 'input', value: 40 });
  // While dragged, the state does not move the thumb.
  bar = nextSeekBar(bar, { kind: 'state', position: 11, now: 100 });
  assert.equal(seekBarValue(bar, 11, 100), 40);
  bar = nextSeekBar(bar, { kind: 'input', value: 50 });
  bar = nextSeekBar(bar, { kind: 'change', value: 50, now: 1000 });
  assert.deepEqual(bar, { dragging: null, pending: { target: 50, since: 1000 } });
  // Released, it holds the target while the play is still elsewhere ...
  bar = nextSeekBar(bar, { kind: 'state', position: 12, now: 1500 });
  assert.equal(seekBarValue(bar, 12, 1500), 50);
  // ... until a state lands within a second of it.
  bar = nextSeekBar(bar, { kind: 'state', position: 49.2, now: 1800 });
  assert.equal(bar.pending, null);
  assert.equal(seekBarValue(bar, 49.2, 1800), 49.2);

  // Or until three seconds have passed.
  let late = nextSeekBar(SEEK_IDLE, { kind: 'change', value: 80, now: 0 });
  late = nextSeekBar(late, { kind: 'state', position: 5, now: 2999 });
  assert.equal(seekBarValue(late, 5, 2999), 80);
  late = nextSeekBar(late, { kind: 'state', position: 5, now: 3000 });
  assert.equal(late.pending, null);
  assert.equal(seekBarValue(late, 5, 3000), 5);
  // Even without a state, the held target does not outlive its time.
  const held = nextSeekBar(SEEK_IDLE, { kind: 'change', value: 80, now: 0 });
  assert.equal(seekBarValue(held, 5, 3000), 5);
  assert.equal(seekBarValue(SEEK_IDLE, null, 0), 0);
});

// ---------------------------------------------------------------------------
// The state.

const payload = (extra = {}) => ({
  sessionId: 'a',
  position: 12.5,
  duration: 90,
  paused: false,
  volume: 80,
  muted: false,
  audioTracks: [{ id: 1, title: 'Main', lang: 'en', selected: true }],
  subtitleTracks: [],
  skip: null,
  crop: null,
  ...extra,
});

test("a view takes only its own play's state", () => {
  const mine = nextPlayerState(null, 'a', payload());
  assert.equal(mine.position, 12.5);
  assert.equal(nextPlayerState(mine, 'a', payload({ sessionId: 'b', position: 99 })), mine);
  assert.equal(nextPlayerState(null, 'a', payload({ sessionId: 'b' })), null);
  assert.equal(nextPlayerState(mine, 'a', null), mine);
  assert.equal(nextPlayerState(mine, 'a', 'a'), mine);
});

test('the state keeps only the closed sets and well-formed values', () => {
  const s = nextPlayerState(null, 'a', payload({
    position: Number.NaN,
    duration: 'long',
    paused: 'yes',
    volume: 250,
    skip: 'https://example.invalid/x',
    crop: 'cropping',
    audioTracks: [
      { id: 1, title: 'x'.repeat(500), lang: 'en', selected: true },
      { id: 0, title: 'bad id', lang: '', selected: false },
      { id: 2.5, title: 'bad id', lang: '', selected: false },
      'junk',
      { id: 3, selected: 'yes' },
    ],
  }));
  assert.equal(s.position, null);
  assert.equal(s.duration, null);
  assert.equal(s.paused, false);
  assert.equal(s.volume, 100);
  assert.equal(s.skip, null);
  assert.equal(s.crop, null);
  assert.deepEqual(s.audioTracks.map((t) => t.id), [1, 3]);
  assert.equal(s.audioTracks[0].title.length, 200);
  assert.deepEqual(s.audioTracks[1], { id: 3, title: '', lang: '', selected: false });
  for (const kind of ['intro', 'credits', 'commercial']) {
    assert.equal(nextPlayerState(null, 'a', payload({ skip: kind })).skip, kind);
  }
  for (const crop of ['off', 'measuring', 'on']) {
    assert.equal(nextPlayerState(null, 'a', payload({ crop })).crop, crop);
  }
});

test("the skip button uses the script's own wording", () => {
  assert.equal(skipLabel('intro'), 'Skip Intro');
  assert.equal(skipLabel('credits'), 'Skip Credits');
  assert.equal(skipLabel('commercial'), 'Skip Commercials');
  const script = read('src-tauri/resources/mpv-scripts/vela-markers.lua');
  for (const label of ['Skip Intro', 'Skip Credits', 'Skip Commercials']) {
    assert.ok(script.includes(`label = "${label}"`), label);
  }
});

test('the notice line shows the fixed sentences and nothing that came otherwise', () => {
  assert.equal(commandNotice('The player did not answer.'), 'The player did not answer.');
  assert.equal(commandNotice("The player isn't running."), "The player isn't running.");
  for (const odd of [
    'failed: http://server/stream?token=abc',
    '/home/user/file.mkv',
    new Error('The player did not answer.'),
    undefined,
    '',
    'x'.repeat(81),
  ]) {
    assert.equal(commandNotice(odd), 'The player could not do that.', String(odd));
  }
});

// Review finding pc-2: the page's requests for the player's fullscreen. A
// fake backend whose answers the test releases by hand, in any order.
function fakeBackend() {
  const sent = [];
  const send = (sessionId, on) =>
    new Promise((resolve, reject) => {
      sent.push({ sessionId, on, resolve, reject });
    });
  return { sent, send };
}
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

test('fullscreen requests reach the backend one at a time, in the order asked', async () => {
  const backend = fakeBackend();
  const request = fullscreenRequests(backend.send);
  const claims = [];
  request('a', true, (held) => claims.push(held));
  request('a', false, (held) => claims.push(held));
  await settle();
  // The "off" is not sent while the "on" is unanswered: it cannot overtake
  // it and land first, leaving the window fullscreen with nobody holding it.
  assert.deepEqual(backend.sent.map((r) => r.on), [true]);
  backend.sent[0].resolve(true);
  await settle();
  assert.deepEqual(backend.sent.map((r) => r.on), [true, false]);
  backend.sent[1].resolve(false);
  await settle();
  // Only the last request's answer becomes the claim.
  assert.deepEqual(claims, [false]);
});

test("the page's claim becomes what the backend answers it holds", async () => {
  // A refused "on" gives the claim up.
  let backend = fakeBackend();
  let request = fullscreenRequests(backend.send);
  let claims = [];
  request('a', true, (held) => claims.push(held));
  await settle();
  backend.sent[0].resolve(false);
  await settle();
  assert.deepEqual(claims, [false]);
  // A refused "on" while an earlier one is held keeps it (the backend says so).
  backend = fakeBackend();
  request = fullscreenRequests(backend.send);
  claims = [];
  request('a', true, (held) => claims.push(held));
  await settle();
  backend.sent[0].resolve(true);
  await settle();
  request('b', true, (held) => claims.push(held));
  await settle();
  backend.sent[1].resolve(true);
  await settle();
  assert.deepEqual(claims, [true, true]);
  // A request that fails outright holds nothing, and the next is still sent.
  backend = fakeBackend();
  request = fullscreenRequests(backend.send);
  claims = [];
  request('a', true, (held) => claims.push(held));
  await settle();
  backend.sent[0].reject(new Error('gone'));
  await settle();
  assert.deepEqual(claims, [false]);
  request('a', false, (held) => claims.push(held));
  await settle();
  assert.equal(backend.sent.length, 2);
  // Only `true` holds.
  backend = fakeBackend();
  request = fullscreenRequests(backend.send);
  claims = [];
  request('a', true, (held) => claims.push(held));
  await settle();
  backend.sent[0].resolve('yes');
  await settle();
  assert.deepEqual(claims, [false]);
});
