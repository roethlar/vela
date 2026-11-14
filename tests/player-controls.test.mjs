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
  SEEK_IDLE,
  barHideAt,
  barShown,
  canSeek,
  commandNotice,
  focusKindOf,
  formatTime,
  fullscreenRequests,
  keyAction,
  nextPlayerState,
  nextSeekBar,
  seekBarValue,
  skipLabel,
  spokenTime,
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
    /function send\(command: PlayerCommand\) \{\s+if \(stopped\) return;\s+if \(command\.name === "stop"\) \{\s+stopped = true;\s+onstop\(\);\s+\}\s+sendPlayerCommand\(sessionId, command\.name, command\.args\)/,
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
  assert.deepEqual(keyAction(key('Unidentified'), ctx({ focus: 'select' })), PASS);
  for (const k of ['Shift', 'Control', 'Alt', 'Meta', 'AltGraph', 'CapsLock']) {
    assert.deepEqual(keyAction(key(k), ctx({ focus: 'select' })), PASS, k);
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
  // A select: Space, Enter and Tab; never a letter (type-ahead would pick a
  // track) and never an arrow (it would step the choice).
  assert.deepEqual(keyAction(key(' '), ctx({ focus: 'select' })), PASS);
  assert.deepEqual(keyAction(key('Enter'), ctx({ focus: 'select' })), PASS);
  assert.deepEqual(keyAction(key('Tab'), ctx({ focus: 'select' })), PASS);
  assert.deepEqual(keyAction(key('ArrowDown'), ctx({ focus: 'select' })), command('seek-relative', [-60]));
  assert.deepEqual(keyAction(key('f'), ctx({ focus: 'select' })), { kind: 'fullscreen', on: true });
  assert.deepEqual(keyAction(key('x'), ctx({ focus: 'select' })), SWALLOW);
  assert.deepEqual(keyAction(key('C'), ctx({ focus: 'select' })), SWALLOW);
  // The volume slider: the arrows step it.
  for (const k of ['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown']) {
    assert.deepEqual(keyAction(key(k), ctx({ focus: 'volume' })), PASS, k);
  }
  assert.deepEqual(keyAction(key(' '), ctx({ focus: 'volume' })), command('toggle-pause'));
  // The seek bar: the arrows seek as anywhere else, not by the bar's step.
  assert.deepEqual(keyAction(key('ArrowLeft'), ctx({ focus: 'seek' })), command('seek-relative', [-5]));
  assert.deepEqual(keyAction(key('ArrowUp'), ctx({ focus: 'seek' })), command('seek-relative', [60]));
  // f, m, q, C and Escape everywhere.
  for (const focus of ['button', 'select', 'volume', 'seek']) {
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
  assert.equal(focusKindOf(element('SELECT', 'audio')), 'select');
  assert.equal(focusKindOf(element('SELECT', 'subtitles')), 'select');
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
  const at = (now, extra = {}) => ({ now, lastActivity: 1000, paused: false, focusInBar: false, ...extra });
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
