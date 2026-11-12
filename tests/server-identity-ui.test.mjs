// Two servers can share a name (the owner runs a Plex and a Jellyfin both
// called "altiera"), so wherever the UI prints a server name it must also say
// which kind of server it is: a mark in rendered markup, the kind in words in
// plain strings. Plan: .agents/plans/server-identity-sidebar.md.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import {
  KIND_LABELS,
  flattenServerPlaylists,
  kindLabel,
  serverLabel,
} from "../src/lib/serverPlaylists.ts";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const read = (rel) => fs.readFileSync(path.join(repoRoot, rel), "utf8").replace(/\r\n/g, "\n");
const markup = (source) =>
  source
    .replaceAll(/<script(?:\s[^>]*)?>[\s\S]*?<\/script>/g, "")
    .replaceAll(/<style(?:\s[^>]*)?>[\s\S]*?<\/style>/g, "");

const logo = read("src/lib/ServerLogo.svelte");
const settings = read("src/lib/Settings.svelte");

test("ServerLogo draws a labelled mark for each supported server kind", () => {
  const body = markup(logo);
  for (const kind of ["plex", "jellyfin", "emby"]) {
    assert.match(body, new RegExp(`kind === "${kind}"`), `no drawing branch for ${kind}`);
    assert.match(KIND_LABELS[kind] ?? "", /^[A-Z][a-z]+$/, `no label for ${kind}`);
  }
  assert.match(body, /aria-label=\{label\}/, "the mark must carry its kind as an accessible name");
  assert.match(body, /\{#if label\}/, "an unknown kind must render nothing");
  assert.match(logo, /const label = \$derived\(KIND_LABELS\[kind\]\);/, "the label comes from the shared table");
});

test("ServerLogo is drawn inline, with no vendor artwork or external reference", () => {
  assert.doesNotMatch(logo, /<image\b/);
  assert.doesNotMatch(logo, /\bhref=/);
  assert.doesNotMatch(logo, /url\(\s*["']?(?:https?:|data:|\/)/);
  assert.doesNotMatch(logo, /import\s[^;]*\.(?:svg|png|jpe?g|webp)["']/);
});

test("the Settings server list shows each server's mark", () => {
  assert.match(settings, /import ServerLogo from "\$lib\/ServerLogo\.svelte";/);
  const row = /\{#each sources as s \(s\.id\)\}([\s\S]*?)\{\/each\}/.exec(markup(settings));
  assert.ok(row, "Settings must still list the connected sources");
  assert.match(row[1], /<ServerLogo kind=\{s\.kind\}/);
});

test("Settings labels Emby, and only Emby, as experimental", () => {
  const row = /\{#each sources as s \(s\.id\)\}([\s\S]*?)\{\/each\}/.exec(markup(settings));
  assert.ok(row);
  const labels = [...row[1].matchAll(/Experimental/g)];
  assert.equal(labels.length, 1, "exactly one Experimental label in the server row");
  assert.match(
    row[1],
    /\{#if s\.kind === "emby"\}\s*<span class="badge experimental">Experimental<\/span>\s*\{\/if\}/,
    "the label must be conditional on the server being Emby",
  );
});

const page = read("src/routes/+page.svelte");
const pageMarkup = markup(page);
const playlistsView = read("src/lib/PlaylistsView.svelte");
const serverPlaylistView = read("src/lib/ServerPlaylistView.svelte");
const serverName = read("src/lib/ServerName.svelte");

const group = (sourceId, sourceKind, available, titles) => ({
  sourceId,
  sourceName: `name-${sourceId}`,
  sourceKind,
  available,
  playlists: titles.map((title) => ({
    key: `${sourceId}:${title}`,
    title,
    itemCount: null,
    sourceId,
    sourceName: `name-${sourceId}`,
  })),
});

test("kind labels: known kinds in words, unknown kinds left as they are", () => {
  assert.equal(kindLabel("jellyfin"), "Jellyfin");
  assert.equal(kindLabel("future"), "future");
  assert.equal(serverLabel("altiera", "plex"), "altiera (Plex)");
  assert.equal(serverLabel("altiera", "jellyfin"), "altiera (Jellyfin)");
  assert.equal(serverLabel("altiera", "future"), "altiera");
  assert.equal(serverLabel("altiera", ""), "altiera");
  assert.equal(serverLabel("altiera", undefined), "altiera");
});

test("only available servers with playlists contribute sidebar entries, in order", () => {
  const entries = flattenServerPlaylists([
    group("a", "jellyfin", true, ["One", "Two"]),
    group("b", "plex", true, []),
    group("c", "emby", false, []),
    group("d", "plex", false, ["Stale"]),
    group("e", "plex", true, ["Three"]),
  ]);
  assert.deepEqual(
    entries.map((entry) => [entry.sourceId, entry.sourceKind, entry.sourceName, entry.playlist.title]),
    [
      ["a", "jellyfin", "name-a", "One"],
      ["a", "jellyfin", "name-a", "Two"],
      ["e", "plex", "name-e", "Three"],
    ],
  );
});

test("servers with nothing to show produce no entries at all", () => {
  assert.deepEqual(
    flattenServerPlaylists([group("b", "plex", true, []), group("c", "emby", false, []), group("d", "plex", false, ["Stale"])]),
    [],
  );
  assert.deepEqual(flattenServerPlaylists([]), []);
});

test("the sidebar's Server Playlists section is driven by the flattened entries", () => {
  assert.match(
    page,
    /const serverPlaylistEntries = \$derived\(flattenServerPlaylists\(serverPlaylistGroups\)\);/,
  );
  const section = /\{#if ([^}]+)\}\s*<div class="sidegroup">Server Playlists<\/div>([\s\S]*?)\{\/if\}\s*\{#if sources\.length > 1\}/.exec(pageMarkup);
  assert.ok(section, "the Server Playlists section must sit directly before Sources");
  assert.equal(section[1].trim(), "serverPlaylistEntries.length > 0", "the heading collapses with the list");
  assert.match(section[2], /\{#each serverPlaylistEntries as entry \(/);
  assert.match(section[2], /<ServerLogo kind=\{entry\.sourceKind\}/);
  assert.match(section[2], /serverLabel\(entry\.sourceName, entry\.sourceKind\)/, "the accessible name says which server");
  assert.doesNotMatch(pageMarkup, /serverPlaylistGroups/, "markup must not read the unfiltered groups");
});

test("the nested per-server playlist layout and its placeholders are gone", () => {
  for (const gone of ["serversourcename", "serverplayliststate", "serverplaylistgroup", "No video playlists", "Experimental"]) {
    assert.ok(!page.includes(gone), `+page.svelte still contains ${gone}`);
  }
});

test("the sidebar Sources list marks each server and names its kind", () => {
  const list = /<div class="sidegroup">Sources<\/div>([\s\S]*?)\{\/each\}/.exec(pageMarkup);
  assert.ok(list, "the Sources list must exist");
  assert.match(list[1], /\{#each sources as src \(src\.id\)\}/);
  assert.match(list[1], /<ServerLogo kind=\{src\.kind\}/);
  assert.match(list[1], /aria-label=\{serverLabel\(src\.name, src\.kind\)\}/);
});

test("ServerName pairs the mark with the name", () => {
  const body = markup(serverName);
  assert.match(body, /<ServerLogo \{kind\} \{size\} \/>/);
  assert.match(body, /\{name\}/);
});

test("no server name is rendered bare in the main window", () => {
  // Every rendered server name goes through ServerName (markup) or
  // serverLabel (strings). A new bare use fails here.
  // `(?<!=)` separates text interpolation from a prop value (`name={...}`).
  assert.doesNotMatch(pageMarkup, /(?<!=)\{sourceNameOf\(/, "sourceNameOf must not be rendered directly");
  assert.doesNotMatch(pageMarkup, /(?<!=)\{(?:hub|choice|playlist|entry|group)\.sourceName\}/);
  assert.equal(
    [...pageMarkup.matchAll(/(?<!=)\{src\.name\}/g)].length,
    [...pageMarkup.matchAll(/<ServerLogo kind=\{src\.kind\} size=\{15\} \/><span class="sourcename">\{src\.name\}<\/span>/g)].length,
    "a source name needs its mark beside it",
  );

  const rendered = [...pageMarkup.matchAll(/sourceNameOf\(/g)].length;
  const viaServerName = [...pageMarkup.matchAll(/<ServerName\s+name=\{sourceNameOf\(/g)].length;
  assert.equal(viaServerName, 6, "card tag, detail tag and four Play Version entries");
  assert.equal(rendered, viaServerName, "every rendered sourceNameOf() is a ServerName prop");
  assert.match(pageMarkup, /<ServerName name=\{hub\.sourceName\} kind=\{sourceKindOf\(hub\.sourceId\)\}/);
  assert.match(pageMarkup, /<ServerName name=\{choice\.sourceName\} kind=\{sourceKindOf\(choice\.sourceId\)\}/);
});

test("the playlist views name the server's kind too", () => {
  const serverBody = markup(serverPlaylistView);
  assert.doesNotMatch(serverBody, /(?<!=)\{playlist\.sourceName\}/);
  assert.match(serverBody, /<ServerName name=\{playlist\.sourceName\} kind=\{sourceKind\}/);
  assert.match(serverBody, /serverLabel\(playlist\.sourceName, sourceKind\)/);
  assert.match(pageMarkup, /sourceKind=\{sourceKindOf\(selectedServerPlaylist\.sourceId\)\}/);

  assert.doesNotMatch(playlistsView, /item\.year, entry\.sourceName\]/, "the joined subtitle must not use the bare name");
  assert.match(playlistsView, /serverLabel\(entry\.sourceName, sourceKindOf\(item\.sourceId \?\? ""\)\)/);
  assert.match(pageMarkup, /<PlaylistsView\s+sourceVersion=\{playlistVersion\}\s+\{sourceKindOf\}/);
});
