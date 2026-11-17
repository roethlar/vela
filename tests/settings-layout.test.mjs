// Settings' layout (owner, 2026-11-17: "the setting is buried under a ton of
// irrelevant in-line help text that should be a popup or removed. there are 4
// main settings tabs and a ton of stuff is crammed into the player one. fix
// that"; .agents/plans/settings-restructure.md, S2 and S3).
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';

const settings = fs.readFileSync('src/lib/Settings.svelte', 'utf8');
const infoTip = fs.readFileSync('src/lib/InfoTip.svelte', 'utf8');

test('Settings has six tabs, the old Player tab split in three', () => {
  const tabs = [...settings.matchAll(/\{ id: "([a-z]+)", label: "([^"]+)" \}/g)].map((m) => m[2]);
  assert.deepEqual(tabs, ['Connected', 'Servers', 'Playback', 'Picture', 'mpv', 'Appearance']);
  assert.doesNotMatch(settings, /activeTab === "player"/);
});

test('the Video player choice is the first thing on the Playback tab', () => {
  const start = settings.indexOf('{#if activeTab === "playback"}');
  assert.notEqual(start, -1);
  const first = settings.slice(start).match(/<h3>([^<]+)/);
  assert.equal(first[1], 'Video player');
});

test('each moved setting is on the tab the plan names', () => {
  const tabOf = (id) => {
    const at = settings.indexOf(`id="${id}"`);
    assert.notEqual(at, -1, id);
    const opens = [...settings.slice(0, at).matchAll(/\{#if activeTab === "([a-z]+)"\}/g)];
    return opens.at(-1)[1];
  };
  for (const [id, tab] of [
    ['video-player', 'playback'], ['continue-playing', 'playback'], ['skip-intros', 'playback'],
    ['playback-quality', 'playback'], ['playback-resolution-override', 'picture'],
    ['player-window', 'picture'], ['mpv-autocrop', 'picture'], ['mpv-path', 'mpv'],
    ['mpv-extra', 'mpv'], ['player-skip-certificate-checks', 'mpv'],
  ]) assert.equal(tabOf(id), tab, id);
  const policy = settings.indexOf('name="playback-source-policy"');
  const opens = [...settings.slice(0, policy).matchAll(/\{#if activeTab === "([a-z]+)"\}/g)];
  assert.equal(opens.at(-1)[1], 'servers', 'the duplicate-source choice is on Servers');
});

test('no grey help text sits under the controls: explanations are behind an InfoTip', () => {
  // What stays visible reports state or warns: why Vela's window cannot be
  // used here, mpv found or not, how mpv would be installed, the examples'
  // own lines (shown only when asked for), the Emby conversion warning, and
  // one line on turning certificate checks off.
  const kept = [
    /^\{videoPlayer\.note\}$/,
    /^\{#if mpv\.available\}/,
    /^\{mpv\.installDescription\}/,
    /^\{p\.help\}$/,
    /Converting is <b>unverified on Emby<\/b>/,
    /^Turning this on sends your sign-in token without checking who receives it\.$/,
  ];
  const paragraphs = [...settings.matchAll(/<p(?: id="[^"]+")? class="muted small">([\s\S]*?)<\/p>/g)]
    .map((m) => m[1].replace(/\s+/g, ' ').trim());
  const loose = paragraphs.filter((text) => !kept.some((rule) => rule.test(text)));
  assert.deepEqual(loose, [], 'inline help paragraphs left');
  assert.doesNotMatch(settings, /class="helpbox"/);
  assert.ok((settings.match(/<InfoTip label="/g) ?? []).length >= 10, 'the explanations moved into InfoTips');
});

test('an InfoTip adds no text to its heading and is labelled for assistive technology', () => {
  const button = infoTip.match(/<button[\s\S]*?<\/button>/)[0];
  assert.match(button, /aria-label=\{`About \$\{label\}`\}/);
  assert.match(button, /aria-expanded=\{open\}/);
  // The opening tag ends on a line of its own (its handlers contain "=>").
  const inner = button.split(/\n\s*>\n/)[1].replace(/<\/button>$/, '');
  assert.equal(inner.replace(/<Icon [^>]*\/>/, '').trim(), '', 'only an icon inside the button');
  assert.match(infoTip, /\{#if open\}/, 'the explanation is shown only when asked for');
});

test('an InfoTip renders the explanation it is given, when open', () => {
  // st-7: the guard above passed with the explanation never rendered.
  const shown = infoTip.match(/\{#if open\}([\s\S]*?)\{\/if\}/);
  assert.ok(shown, 'an open branch');
  assert.match(shown[1], /\{@render children\(\)\}/, 'the open branch renders the explanation');
});

test("an InfoTip's explanation stays inside its heading's width", () => {
  // st-2: a fixed-width popup beside a long heading ran past the panel's edge.
  const pop = infoTip.match(/\.infopop \{([\s\S]*?)\}/)[1];
  assert.match(pop, /left: 0;/);
  assert.match(pop, /right: 0;/);
  assert.doesNotMatch(pop, /max-content|width: \d/);
  assert.match(infoTip.match(/\.infotip \{([\s\S]*?)\}/)[1], /position: static;/);
  assert.match(settings.match(/\n  h3 \{([\s\S]*?)\}/)[1], /position: relative;/, 'Settings positions its headings');
});
