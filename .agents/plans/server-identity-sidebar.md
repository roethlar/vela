# Plan: Server identity marks and a flat, collapsing Server Playlists list

## Status

**APPROVED 2026-11-12; implementation in progress.** Owner go, verbatim, in
answer to "Server-logo plan: go or not?": "go". All four slices are
implemented and pushed (`343f270`, `62b85e2`); awaiting owner acceptance in
the installed app. Gate G1 passed: shown the three drawings, the owner
answered "those logos are fine."

The version bump this plan makes is now 1.0.62 -> 1.0.63: 1.0.62 was taken
by the Windows mpv idle-exit fix (`bf1b1c3`) after this plan was drafted.

Owner rulings already given (2026-11-12), verbatim:

- **D1 (server marks): logos.** "logos. if you can approximate them with
  .svg, good. if not, then we can use actual icons. I'll take the risk that
  this won't get me sued, since we already use the plex and emby names in
  the settings." Meaning for this plan: the marks are hand-drawn simplified
  SVG approximations authored in this repo (no vendor asset files). The
  owner has accepted the trademark risk. Substituting the vendors' actual
  logo artwork is the owner-approved fallback, taken only if the owner
  rejects the approximations at gate G1.
- **D2 (server with nothing to list): hide.** "if there are no available
  server playlists, show nothing for that server". Applies equally to a
  server with zero video playlists and a server whose playlist request
  failed. Neither renders any sidebar element.
- **D3 (Emby "Experimental" label): Settings only.** "experimental doesn't
  need to be in the main UI at all. settings is enough." The label leaves
  the main window. Settings currently contains no "Experimental"
  wording at all (only an Emby note about converting), so this plan adds the
  word to the Emby row of the Settings source list, keeping decision
  `.agents/decisions.md` 2026-07-15 ("Emby support is labeled experimental")
  honoured. Owner confirmation, 2026-11-12, asked whether "Settings is
  enough" means adding the word there: "yes, if we need it anywhere".

- **D4 (scope of the marks): everywhere.** Asked whether the marks should
  also appear in the other places a bare server name is printed, the owner
  answered: "yes, anywhere a bare server name is used we need to
  differentiate." Slice 4 below implements it; it is part of this plan, not
  a follow-up.

Open, not yet ruled:
- **G1 (visual acceptance of the drawings).** A gate inside Slice 1, not a
  pre-approval ruling.


## Problem

Owner report with screenshot, 2026-11-12, on installed 1.0.61 with two
servers both named `altiera` (one Plex, one Jellyfin):

1. The sidebar "Sources" list prints only `src.name`
   (`src/routes/+page.svelte`, the `{#each sources as src}` block after the
   `Sources` heading), so the two entries are indistinguishable. The kind is
   already in the data: `type Source = { id; name; kind }`, with `kind` one
   of the lowercase strings `"plex"`, `"jellyfin"`, `"emby"`.
2. "Server Playlists" renders one block per server
   (`.serverplaylistgroup`): a sub-heading with the server name
   (`.serversourcename`), then either the playlists or a state line
   (`.serverplayliststate`: "No video playlists" / "Unavailable"). With two
   empty servers this is five lines of sidebar carrying no content, and the
   two sub-headings are again indistinguishable.
3. Settings shows the kind only as a lowercase text badge
   (`src/lib/Settings.svelte`, `<span class="badge">{s.kind}</span>`).

## Goal

1. Every server entry in the sidebar Sources list and in the Settings
   source list carries a per-kind mark (Plex, Jellyfin, Emby).
2. Server Playlists is one flat list directly under its heading: one button
   per playlist, each carrying its server's mark. No per-server sub-heading.
3. A server with no playlists, or whose playlists failed to load, renders
   nothing. If no server contributes a playlist, the "Server Playlists"
   heading itself is not rendered.
4. "Experimental" no longer appears in the main window. It appears on the
   Emby row in Settings.

Unchanged: the `get_server_playlists` command and its payload, playlist
order (server order, then the server's own order), the read-only nature of
server playlists, opening/playing a server playlist, and every Rust file.

## Out of scope

Any change to the Rust backend or to command payloads. If a site in Slice 4
turns out to have a server name but no way to reach its kind from data
already in the frontend, stop and report it instead of adding a payload
field.

## Slice 1 — `ServerLogo` component, wired into Settings

New file `src/lib/ServerLogo.svelte`:

- Props: `kind: string`, `size?: number` (default 16), `class?: string`.
- Renders an inline `<svg viewBox="0 0 24 24" width={size} height={size}
  role="img" aria-label={label}>` where `label` is `Plex`, `Jellyfin`, or
  `Emby`. For any other `kind` it renders NOTHING (no placeholder), so an
  unknown future kind degrades to the plain name.
- Fixed brand colours, not `currentColor` (the purpose is telling servers
  apart at a glance). Starting geometry, to be refined by eye for G1:
  - `plex`: `<polygon points="7,3 12,3 18,12 12,21 7,21 13,12"
    fill="#E5A00D"/>` (gold chevron).
  - `jellyfin`: a triangle outline with a solid inner triangle, filled with
    a linear gradient `#AA5CC3` → `#00A4DC` (top-left to bottom-right):
    `<path fill-rule="evenodd" d="M12 2.5 22 20.5H2Z M12 8.6 16.9 17.6H7.1Z"/>`
    plus `<path d="M12 12.4 14.3 16.6H9.7Z"/>`. The gradient `id` must be
    unique per component instance (module-level counter in a
    `<script module>` block, or `$props.id()` if the installed Svelte
    supports it): the mark is rendered many times on one page, and a
    duplicated id resolves to the first instance, which breaks if that
    instance is not displayed.
  - `emby`: `<rect x="4.5" y="4.5" width="15" height="15" rx="2.5"
    transform="rotate(45 12 12)" fill="#52B54B"/>` plus a white play
    triangle `<path d="M10 8.2 16 12 10 15.8Z" fill="#fff"/>`.
- No `<image>`, no `href`, no `url(http...)`, no imported asset file: the
  drawing is entirely inline.

`src/lib/Settings.svelte`, the Connected list row
(`{#each sources as s (s.id)}`):

- Insert `<ServerLogo kind={s.kind} />` before the existing
  `<span class="badge">{s.kind}</span>`. Keep the text badge.
- After the name, when `s.kind === "emby"`, render
  `<span class="badge experimental">Experimental</span>`; style it with
  `color: var(--accent)` on top of `.badge`.
- Do not change the existing "Converting is unverified on Emby" note.

Version and notes (first shipped-code commit of this plan): run
`scripts/bump.sh` (1.0.61 → 1.0.62) and add one bullet under `# Unreleased`
in `RELEASE_NOTES.md` covering the whole plan (server marks; flat Server
Playlists that collapse when empty). Note: the unmerged `remove-sveltekit`
branch is also at 1.0.62; whichever lands second on `main` must re-bump.

**Gate G1 (owner, visual).** Before committing Slice 1's wiring as final,
show the owner the three marks at 14 px, 16 px and 24 px on the app's dark
surface (a screenshot of the running app's Settings row, or a standalone
HTML preview). The owner either accepts, asks for redraws, or invokes the
D1 fallback (actual vendor artwork). Record the outcome verbatim in the
implementation log. Do not start Slice 2 before G1 passes.

Tests: new file `tests/server-identity-ui.test.mjs`, added to the explicit
`node --test` list in the `package.json` `check` script. Source-text guards
in the style of `tests/ui-motion.test.mjs` (read the `.svelte` source,
normalize CRLF to LF, assert with regexes):

- S1a: `ServerLogo.svelte` has a branch for each of `"plex"`, `"jellyfin"`,
  `"emby"`, and an `aria-label` bound to the label.
- S1b: `ServerLogo.svelte` contains none of `<image`, `href=`, `url(http`,
  `import ` of an asset (`.svg`, `.png`).
- S1c: the Settings source row renders `<ServerLogo kind={s.kind}`.
- S1d: Settings renders the text `Experimental` inside a block conditioned
  on `s.kind === "emby"`.

Red-proofs, each separately, restored with `git checkout -- <file>` from the
committed state: delete the `emby` branch (S1a fails); add an `<image
href>` (S1b fails); remove the Settings `<ServerLogo` (S1c fails); remove
the emby condition so the label is unconditional (S1d fails); remove the
label entirely (S1d fails).

Commit: `ui: per-server marks in the Settings source list`.

## Slice 2 — marks in the sidebar Sources list

`src/routes/+page.svelte`, the block under `<div class="sidegroup">Sources</div>`:

- Each per-source button becomes
  `<button class="sideitem sourceitem" ... aria-label={`${src.name} (${kindLabel(src.kind)})`}>`
  containing `<ServerLogo kind={src.kind} size={15} />` then
  `<span class="sourcename">{src.name}</span>`.
- `kindLabel` maps `plex`/`jellyfin`/`emby` to `Plex`/`Jellyfin`/`Emby` and
  returns the raw kind otherwise. Export it from `ServerLogo.svelte`'s
  `<script module>` block so the label has one definition.
- The `All` button is unchanged and has no mark.
- CSS: `.sourceitem { display: flex; align-items: center; gap: 0.45rem; }`;
  `.sourcename { overflow: hidden; text-overflow: ellipsis; white-space:
  nowrap; min-width: 0; }`. The mark must not shrink:
  `.sourceitem :global(svg) { flex: none; }`.

Tests (same file): S2a the Sources loop renders
`<ServerLogo kind={src.kind}`; S2b the button's `aria-label` includes the
kind label. Red-proof each by removing it.

E2E: `tests/e2e/scenarios/` — grep for selectors that locate a Sources
button by text content or `aria-label` and update them to the new label.
List every changed selector in the implementation log.

Commit: `ui: per-server marks in the sidebar Sources list`.

## Slice 3 — flat Server Playlists that collapse when empty

New pure function in `src/lib/serverPlaylists.ts` (tests can import `.ts`
directly; `tests/image-reveal.test.mjs` already imports
`../src/lib/imageReveal.ts`):

```ts
import type { ServerPlaylist, ServerPlaylistGroup } from "./types";

export type ServerPlaylistEntry = {
  playlist: ServerPlaylist;
  sourceId: string;
  sourceName: string;
  sourceKind: string;
};

// D2: a server that is unavailable or has no playlists contributes nothing.
export function flattenServerPlaylists(groups: ServerPlaylistGroup[]): ServerPlaylistEntry[] {
  return groups
    .filter((group) => group.available)
    .flatMap((group) =>
      group.playlists.map((playlist) => ({
        playlist,
        sourceId: group.sourceId,
        sourceName: group.sourceName,
        sourceKind: group.sourceKind,
      })),
    );
}
```

`src/routes/+page.svelte`:

- `const serverPlaylistEntries = $derived(flattenServerPlaylists(serverPlaylistGroups));`
- Replace the whole `{#if serverPlaylistGroups.length > 0} ... {/if}` sidebar
  block with:

  ```svelte
  {#if serverPlaylistEntries.length > 0}
    <div class="sidegroup">Server Playlists</div>
    {#each serverPlaylistEntries as entry (`${entry.sourceId}:${entry.playlist.key}`)}
      <button
        class="sideitem sourceitem serverplaylistitem"
        class:active={selectedServerPlaylist?.key === entry.playlist.key}
        data-source-id={entry.sourceId}
        aria-label={`Open ${entry.playlist.title} from ${entry.sourceName} (${kindLabel(entry.sourceKind)})`}
        onclick={() => openServerPlaylist(entry.playlist)}
      >
        <ServerLogo kind={entry.sourceKind} size={15} />
        <span class="sourcename">{entry.playlist.title}</span>
      </button>
    {/each}
  {/if}
  ```

  Before finalizing the `class:active` expression and the each-key, read how
  `selectedServerPlaylist` and `playlist.key` are defined: if `key` is not
  unique across servers, compare `sourceId` as well.
- Leave `loadServerPlaylists` (including its catch branch that synthesizes
  `available: false` groups) unchanged: the flatten function is what hides
  them.
- Delete the now-dead CSS rules `.serverplaylistgroup`, `.serversourcename`
  (all three selectors), and `.serverplayliststate`. Change
  `.serverplaylistitem` to drop `padding-left: 1.15rem` (no nesting) and
  keep `font-size: 0.84rem`.
- The strings `No video playlists`, `Unavailable` (in this sidebar block
  only) and `Experimental` leave `+page.svelte`.

Existing guard that MUST change (approval of this plan is the owner's
approval to change it): `tests/ui-motion.test.mjs`, the assertion
`/class=["']serverplayliststate["'][^>]*>No video playlists<\/span>/`
("sidebar playlist metadata stays inline"). Replace it with an assertion
that `+page.svelte` does NOT contain `No video playlists`. Read the
surrounding test first: it enumerates empty-state categories, and the
sidebar entry may also be counted in a category list that needs the same
removal.

Tests:

- S3a (behaviour, imports the `.ts`): `flattenServerPlaylists` over a
  fixture of four groups — available with two playlists, available with
  zero, unavailable with zero, unavailable with one stale playlist — returns
  exactly the two playlists of the first group, in order, with that group's
  `sourceId`/`sourceName`/`sourceKind`.
- S3b: an all-empty/all-unavailable input returns `[]`.
- S3c (wiring): `+page.svelte` gates the `Server Playlists` heading on
  `serverPlaylistEntries.length > 0` and iterates `serverPlaylistEntries`;
  `serverPlaylistEntries` is derived from
  `flattenServerPlaylists(serverPlaylistGroups)`.
- S3d: `+page.svelte` contains none of `serversourcename`,
  `serverplayliststate`, `No video playlists`, `Experimental`.
- S3e: the playlist button renders `<ServerLogo kind={entry.sourceKind}`.

Red-proofs, each separately: drop the `.filter(available)` (S3a fails on
the stale playlist); return groups with empty playlists as placeholder
entries (S3a/S3b fail); gate the heading on `serverPlaylistGroups.length`
(S3c fails); iterate the groups instead of the entries (S3c fails);
reintroduce the `Experimental` small (S3d fails); remove the logo (S3e
fails).

E2E, `tests/e2e/scenarios/serverplaylists.mjs` (currently asserts the
nested layout and will fail unmodified):

- Replace the first `waitFor` (which reads
  `[data-source-id=...].dataset.playlistState`) with a wait for
  `button[aria-label="Open Server Night from Playlist Server (Jellyfin)"]`.
  One `get_server_playlists` call returns every group, so the healthy
  button's presence means the failing server's result has also arrived.
- Replace the `'Unavailable'` text assertion with: no element has
  `data-source-id="jf-offline"`, and the sidebar `nav` text contains neither
  `Unavailable Server` (outside the Sources list — scope the query to the
  elements between the `Server Playlists` heading and the `Sources` heading,
  or assert on `.serverplaylistitem` only) nor `No video playlists`.
- Update the `Open Server Night from Playlist Server` selector to the new
  label everywhere in the file.
- Grep the other scenarios for `serverplaylist`, `data-playlist-state`, and
  `from Playlist Server` and update likewise.

Commit: `ui: flat Server Playlists that collapse when empty`.

## Slice 4 — every other bare server name (D4)

Rule: wherever the UI prints a server's name, the reader can tell which
kind of server it is. Rendered markup gets the mark; plain strings
(`aria-label`, `title`, hint sentences, strings assembled with `.join`) get
the kind in words, as `name (Kind)`.

Shared pieces:

- `src/lib/ServerName.svelte`: props `name: string`, `kind: string`,
  `size?: number` (default 13). Renders `<span class="servername">` holding
  `<ServerLogo {kind} {size} />` and the name, laid out inline
  (`display: inline-flex; align-items: center; gap: 0.3em; vertical-align:
  bottom`). With an unknown kind it renders the name alone.
- In `+page.svelte`, next to `sourceNameOf(id)`, add `sourceKindOf(id)`
  (`sources.find((s) => s.id === id)?.kind ?? ""`) and
  `sourceLabelOf(id)` returning `name (Kind)` via `kindLabel`, or the bare
  name when the kind is unknown.

Sites as of `dd3049c` (re-grep `sourceNameOf\(|sourceName\b` across
`src/routes/+page.svelte` and `src/lib/*.svelte` at implementation time and
treat any site not listed here as in scope too):

- `+page.svelte`, card source tag
  (`sourceNameOf(item.backing?.[0]?.sourceId ?? item.sourceId)`): replace
  the text with `<ServerName>`. Read the surrounding ternary first; one arm
  prints something other than a server name and must stay as is.
- `+page.svelte`, detail `srctag` (`· {sourceNameOf(center.sourceId)}`):
  `<ServerName>` after the separator.
- `+page.svelte`, hub heading (`hub.sourceName`): `<ServerName
  name={hub.sourceName} kind={sourceKindOf(hub.sourceId)} />`.
- `+page.svelte`, version chooser (`choice.sourceName`): `<ServerName>`;
  confirm the choice object carries a source id, else resolve the kind from
  whatever key it does carry.
- `+page.svelte`, context-menu entries `Resume on …`, `Start over on …`,
  the bare `{sourceNameOf(b.sourceId)}` entry, and `Quality on …`:
  `<ServerName>` in place of the name text inside each button.
- `src/lib/PlaylistsView.svelte`, the entry metadata array ending in
  `entry.sourceName` joined into one string: use the `name (Kind)` text
  form. The component needs the kind; read its props/types to see whether
  an entry or item carries `sourceKind` or a source id plus a source list,
  and pass a `kindOf(sourceId)` function prop from `+page.svelte` if not.
- `src/lib/ServerPlaylistView.svelte`, header
  (`<p class="muted">{playlist.sourceName}</p>`): `<ServerName>`; and the
  empty-state hint (`Add videos on ${playlist.sourceName}, …`): the text
  form. Same kind-plumbing note as above.
- The `Open … from …` label of Slice 3 already uses the text form.

Tests (`tests/server-identity-ui.test.mjs`):

- S4a: `ServerName.svelte` renders `<ServerLogo` and the name.
- S4b: `+page.svelte` contains no `{sourceNameOf(` expression in rendered
  markup — every remaining call is inside `sourceLabelOf`, a script block,
  or an attribute string. Implement by asserting an exact, enumerated count
  of the allowed occurrences, so a newly added bare use fails the guard.
- S4c: `+page.svelte` contains no `{hub.sourceName}` and no
  `{choice.sourceName}` as bare text.
- S4d: `PlaylistsView.svelte` and `ServerPlaylistView.svelte` contain no
  `{playlist.sourceName}`/`entry.sourceName` outside a `ServerName` prop or
  a `name (Kind)` expression.

Red-proof each guard separately by restoring one bare use at one site per
guard. Existing source-text guards in `tests/ui-foundation.test.mjs`,
`tests/ui-motion.test.mjs`, `tests/image-reveal.test.mjs` and
`tests/transcoding-ui.test.mjs` match on markup near several of these
sites (the Quality menu in particular); run `npm run check` after each site
and adjust only an assertion whose literal text this slice changed,
recording each one in the implementation log.

E2E: grep `tests/e2e/scenarios/` for text matched against these sites
(`Resume on`, `Start over on`, `Quality on`, `srctag`, `choicename`, hub
titles with a ` · ` suffix). The mock servers are Jellyfin, so a text match
on `Quality on <name>` still passes if it is a prefix/contains match and
fails if it is an exact match; fix exact matches.

Commit: `ui: mark the server kind wherever a server name is shown`.
## Verification before claiming completion

Frontend-only change, so from the repo root: `node
scripts/check-js-toolchain.mjs`, `npm ci`,
`npm audit --audit-level=moderate`, `npm run check`, `npm run build`,
`node --test tests/`, `git diff --check`. `scripts/bump.sh` edits
`src-tauri/Cargo.toml` and `Cargo.lock`, so also run, from `src-tauri/`,
`cargo +stable check --locked` (and the 1.89.0 MSRV check on a host that
has that toolchain; `continuum` does not as of 2026-11-12).

E2E (`npm run e2e -- serverplaylists`, then the full suite) is required for
Slice 3 and has no ready venue as of 2026-11-12: the Linux VM is suspended
and its Node is below the `engines` floor (`.agents/machines.md`), and
`continuum` lacks `tauri-driver` and `Xvfb` (it has `ffmpeg`, `bsdtar`,
`mpv`). Either venue needs owner action or an owner go to install tools.
If E2E cannot run, say so plainly and do not claim Slice 3 verified.

Manual acceptance on `continuum` (the reporting host, two servers named
`altiera`): build and install the package (`scripts/build.sh`, `pacman -U`)
and confirm with the owner: the two Sources entries are distinguishable; no
Server Playlists section while both servers have none; Settings rows show
marks, and no "Experimental" appears in the main window.

## Interaction with `remove-sveltekit`

That unmerged branch moves `src/routes/+page.svelte` to `src/App.svelte`
and repoints tests. Landing this plan on `main` first means the branch's
next merge of `main` must carry these sidebar edits into `src/App.svelte`
and repoint `tests/server-identity-ui.test.mjs`. Record that obligation in
`.agents/plans/remove-sveltekit.md` when this lands.

## Paperwork on landing

- `.agents/state.md` "Now": one bullet with commit ids and what was and was
  not verified.
- `.agents/decisions.md`: a dated entry recording D1-D3 (and D4 once
  ruled), noting that D3 narrows where the 2026-07-15 "Emby is labeled
  experimental" decision is displayed, not the decision itself.
- This file: Status line with the owner's go verbatim; implementation log
  with every red-proof result and the G1 outcome.

## Implementation log

2026-11-12, host `continuum` (CachyOS; Node 26.13.1, npm 12.2.2).

- G1: preview rendered with `rsvg-convert` at large size and at 24, 16 and
  13 px on the app's dark surface and on a light strip, plus an HTML mock of
  the sidebar and Settings rows. Owner, verbatim: "those logos are fine."
- Slice 1, `343f270` (1.0.63): `src/lib/ServerLogo.svelte`, the Settings
  row mark, and the Emby `Experimental` badge. Gradient ids use
  `$props.id()` (Svelte 5.56.5).
- Slices 2-4, `62b85e2`, landed as ONE commit, not three: they were
  implemented together and all touch the same regions of `+page.svelte`.
  This departs from the plan's one-commit-per-slice layout.
- Departures from the plan's text, all in `62b85e2`:
  - Kind labels live in `src/lib/serverPlaylists.ts` (`KIND_LABELS`,
    `kindLabel`, `serverLabel`), not in a `<script module>` block of
    `ServerLogo.svelte`, so node tests can import them and there is one
    table. `ServerLogo` imports `KIND_LABELS`.
  - The plain-string form is produced by `serverLabel(name, kind)` at each
    site; no `sourceLabelOf` helper was added to `+page.svelte`.
    `sourceKindOf(id)` was added as planned.
  - The Server Playlists `class:active` compares both the playlist key and
    the source id.
  - `PlaylistsView` takes a `sourceKindOf` function prop and resolves the
    kind from `entry.item.sourceId`; an entry whose server is gone has no
    kind and keeps its bare name. `ServerPlaylistView` takes a `sourceKind`
    string prop.
- Existing guards changed, as the plan allowed: `tests/ui-motion.test.mjs`
  — the `No video playlists` inline assertion now asserts absence, and the
  exact `EmptyState` hint string for the empty server playlist follows the
  new `serverLabel(...)` expression. No other existing assertion changed.
- Tests: `tests/server-identity-ui.test.mjs`, 13 tests, in the `check` list.
- Red-proofs, each injected separately into the committed files and
  restored with `git checkout --`; every one failed the named test:
  - Slice 1 (6): emby branch deleted; an external `<image href>` added; an
    unknown kind rendering an empty svg; logo removed from the Settings
    row; `Experimental` made unconditional; `Experimental` removed.
  - Slices 2-4 (17): availability filter dropped from
    `flattenServerPlaylists`; empty servers given a placeholder entry;
    `serverLabel` dropping the kind; heading gated on the unfiltered
    groups; the derived list not built by `flattenServerPlaylists`;
    `Experimental` back in the main window; playlist entry without its
    mark; Sources entry without its mark; Sources `aria-label` without the
    kind; bare name back in the Quality entry; bare `hub.sourceName`; bare
    `choice.sourceName`; bare name in the server playlist header; hint
    without the kind; playlist subtitle without the kind; `ServerName`
    without the mark; `sourceKind` prop not passed to `ServerPlaylistView`.
- Verification at `62b85e2`: toolchain check, `npm ci`, `npm run check`
  (93 pass, 0 fail; svelte-check 306 files, 0 errors, 0 warnings),
  `npm run build`, `git diff --check`. Rust: `scripts/bump.sh` changed only
  version strings; the Cargo commands were not re-run locally for this
  plan (GitHub CI covers them on push).
- E2E on `continuum`, which became a working venue on 2026-11-12 (owner
  installed `tauri-driver` and `xorg-server-xvfb`; the pinned 2.52.3 driver
  works against system WebKitGTK 2.54.2). Run from a disposable worktree:
  - Baseline at `343f270`, before the layout change: `smoke` and the old
    `serverplaylists` passed.
  - At `62b85e2`: `serverplaylists` (rewritten), `mergedview`,
    `sourcepolicy`, `multiplex`, `playlistplay`, `playlistedit`, `smoke` —
    7 of 7 passed.
  - E2E red-proof of the rewritten scenario: re-adding an `Unavailable`
    line for the failing server made `serverplaylists` FAIL on
    `offlineInPlaylists: 1` and the extra entry; restored, it passed. A
    first attempted mutation (dropping the availability filter) did NOT
    fail it, correctly: the mock's failing server returns no playlists, so
    that filter is only reachable through the unit test, which does fail.
- Selectors changed in E2E: only `tests/e2e/scenarios/serverplaylists.mjs`
  (the nested-layout wait and `Unavailable` assertion replaced; the
  `Open Server Night from Playlist Server` label now ends ` (Jellyfin)`).
  Other scenarios match server names by text content, which the marks do
  not change.