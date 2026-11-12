// Two servers can share a name (the owner runs a Plex and a Jellyfin both
// called "altiera"), so wherever the UI prints a server name it must also say
// which kind of server it is: a mark in rendered markup, the kind in words in
// plain strings. Plan: .agents/plans/server-identity-sidebar.md.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

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
    assert.match(logo, new RegExp(`${kind}: "[A-Z][a-z]+"`), `no label for ${kind}`);
  }
  assert.match(body, /aria-label=\{label\}/, "the mark must carry its kind as an accessible name");
  assert.match(body, /\{#if label\}/, "an unknown kind must render nothing");
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
