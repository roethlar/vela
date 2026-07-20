# Issue Queue

## Resolved - Owner-Reported (2026-07-18)

- Duplicate copies now use the explicit Player setting chosen by the user:
  Prefer Best, Prefer Compatible, Prefer Fastest Source, or Ask Every Time.
  Play Version supplies the documented automatic-mode override; Ask retains a
  choice only for the current Vela playlist or TV-continuation run; server
  playlists remain owner-bound. Manual watch edits and exact natural completion
  independently update every currently configured title backing, with safe
  partial-failure reporting and no offline queue. Implemented under
  `.agents/plans/playback-source-policy.md`; focused two-server production E2E
  and independent regression mutations pass. The canonical local gates, fresh
  Linux real-app suite, and Linux release package builds pass at version 0.1.61.

- Continue Watching missed a newly eligible next episode until manual Refresh
  because the automatic post-playback refresh ran before the clean-EOF server
  `mark_played` request settled. The authoritative Home refresh now runs after
  that settled attempt without delaying sequence release or adding reloads.
  Implemented under `.agents/plans/clean-eof-hub-refresh.md` (`chr-1`,
  implementation `6ec2ba6`); five production regressions were separately
  proven red/restored-green, the local gates and fresh-build Linux real-app
  suite 29/29 pass, and Claude accepted with no material issues. Merged to
  `main` at `5248fe6` on 2026-07-19. Detail:
  `.agents/review/findings/chr-1.md`. The real-Plex smoke remains deferred to
  the owner's next live session (non-fatal; observation only).

- Automatic playlist and Continue Playing successors now retain the completed
  mpv process's actual fullscreen and maximized state. Manual starts keep their
  configured defaults, and unknown state remains untouched. Implemented in Vela
  0.1.59 under `.agents/plans/playback-window-state-continuity.md`; eight
  production regressions were separately proven red/restored-green, the exact-
  source Linux suite passed 28/28, and Claude accepted exact reviewed head
  `8d4c7bc` with an independent guard proof. Detail:
  `.agents/review/findings/pws-1.md`.

- Marking an item watched from a library listing refreshed the entire page,
  returned the grid to page one, and lost the current scroll position. Vela now
  rebuilds the loaded range in a private server-authoritative buffer, publishes
  it once under exact root ownership, and restores the mounted grid's scroll
  after the DOM update. Failures retain the complete prior grid and confirmed
  local edit. Implemented in Vela 0.1.58 under
  `.agents/plans/watch-edit-position.md`; seven production regressions were
  separately proven red/restored-green, the exact-source Linux suite passed,
  and Claude accepted exact reviewed head `32b0777` with an independent guard
  proof. Detail: `.agents/review/findings/wsp-1.md`.

## Open - Agent-Found (2026-07-15)

- `scripts/build.sh --native` fails on macOS's Bash 3 after the toolchain check
  with `extra_args[@]: unbound variable`: `--native` leaves the array empty and
  the later Tauri invocation expands it under `set -u`. The default universal
  macOS path remains green. This pre-dates and is separate from the dependency
  refresh's Node/npm enforcement, so it was recorded rather than silently
  folded into that reviewed fix.

## Resolved - Owner-Reported (2026-07-04, Continue Watching curation)

Reported on Linux against a live Plex server; code-traced same day.
All three items implemented 2026-07-04 via
`.agents/plans/continue-watching-curation.md` (slices 2, 3, 1
respectively; commits `cf5af95`, `d259213`, `d2ea1a7`). Automated
checks pass and tests are guard-proven; live in-app confirmation of the
Plex server-side removal is pending first use (non-fatal by design —
the local tombstone alone hides the item). The On Deck supersession is
recorded in `.agents/decisions.md`.

- Mark watched/unwatched does not remove an item from the Continue Watching
  hero, and the owner expects it to. Root causes (both required):
  (a) `setWatched` (`src/routes/+page.svelte:707`) updates the card
  in place and never re-fetches hubs, so the server-side continue hub row
  keeps its stale copy until the next full refresh; (b) the hero also merges
  Vela's own recents (`recents.rs`), and a recents entry only leaves via the
  playback-end path (`finish()` past the watched threshold) — a right-click
  mark-watched never touches `cfg.recents`, so the item persists in the hero
  from the recents half even after the server drops it. Fix direction:
  `set_watched` should also drop the key from recents (backend) and trigger
  the same hub+recents re-fetch the `playback-ended` path uses (frontend).
- No "Remove from Continue Watching" action exists. Owner wants an explicit
  context-menu option on hero cards that removes the entry without changing
  watched state. Needs: a `remove_recent(rating_key)` command clearing the
  Vela recents entry, plus — for Plex-backed items — the server's
  remove-from-continue-watching action so the server hub copy goes too
  (Plex exposes this in its API; Jellyfin/Emby equivalents need
  investigation). Until the server side is wired, removal must at least
  stick locally (recents) and survive the merge with server hubs.
- QUEUED (owner direction 2026-07-04): collapse On Deck into the Continue
  Watching hero so both feed ONE cover-flow interface — no separate On Deck
  row. Background: the two are different lists by design (hero = Vela's
  recents ∪ the servers' continue/resume hubs; On Deck = Plex's "what's
  next" hub with next-up episodes) and overlap for in-progress episodes;
  the owner wants them merged rather than stacked. Direction: fold
  `hubPolicy`'s "landscape" (ondeck) bucket into "hero", dedupe by rating
  key against recents/continue items (recents-first ordering as today),
  and drop the 16:9 On Deck row. This refines the 2026-07-04 split-artwork
  decision's On Deck treatment; record the supersession when implemented.
  Needs a plan + owner approval of ordering semantics (where next-up
  episodes rank against in-progress items in the flow).

## Open - Owner-Reported (2026-07-04)

Owner-observed on macOS during live smoke testing. Recorded as reported;
untriaged, no code investigation yet.

Home screen, against a live Plex server (screenshot evidence; other backends
unchecked for these):

- Continue Watching does not refresh after playback. A video watched to
  completion or partway is only reflected in the Home hubs after an app
  restart.

- Card watch state is stale after playback. Progress bars and played
  checkmarks do not update after a video is finished or its resume position
  moves in either direction, until restart. Root cause confirmed 2026-07-04:
  the server is updated correctly on mpv exit; the frontend never re-fetches.
  Plan for this and the previous item:
  `.agents/plans/watch-state-refresh.md`. Implemented 2026-07-04 (backend
  `playback-ended` event after the final server check-in + frontend
  re-fetch); automated checks pass, owner playtest pending.

- Rows mix poster and content-frame artwork. The same row renders 2:3 posters
  next to 16:9 episode thumbnails at different heights (e.g. a movie poster
  beside an episode thumb in Continue Watching). The owner finds this
  distracting; a row should present one consistent artwork shape. Plan:
  `.agents/plans/row-artwork-consistency.md`. Implemented 2026-07-04 per the
  split policy: Continue Watching renders as the hero carousel (overlay
  arrows, backdrops for movies), On Deck as a uniform 16:9 row, catalog rows
  as uniform 2:3 with series posters for episodes (new seriesPoster/backdrop
  fields from Plex and Jellyfin/Emby; local series art deferred — that
  deferral died 2026-07-08 with the local-source removal).
  Unit-tested (guard-proven); owner playtest pending.

- Hero carousel reads as static/broken (reported 2026-07-04, v0.1.7: ignored
  the hero, played another video ~seconds, stopped — hero unchanged, "no
  arrows", no replacement). Diagnosis (code-traced): three stacked causes.
  (a) UX defect, real: the prev/next arrows are hover-revealed
  (`+page.svelte` `.heroarrow` opacity 0 until `.heroframe:hover`), so a
  single visible hero card shows no affordance at all — the approved plan's
  "proposed hover-reveal" default fails in practice. Fix direction: arrows
  (or an equivalent position indicator) always visible.
  (b) Not a Vela defect: the post-playback refresh did run (`playback-ended`
  fires even on early quit; hubs are refetched live), but Plex does not
  create a resume point for only a few seconds of playback (server-side
  minimum, ~60s — assumption from observed Plex behavior), so Continue
  Watching legitimately did not change. A >60s partial play is the correct
  retest.
  (c) By design (recorded in the artwork plan): if the played item was
  local/SMB, it can never enter Continue Watching — local items carry no
  watch state.
  Owner direction 2026-07-04: recency IS the desired semantic, and the hero
  becomes a cover-flow (foobar2000 reference; see `.agents/decisions.md`).
  Implemented same day: Vela-side recents (snapshot at play, position
  stamped at mpv exit, finished entries dropped at the watched threshold),
  ONE consolidated hero fed by recents ∪ server hubs, cover-flow capped at
  30% of window height with older items fanned behind-left / newer
  behind-right, side cards clickable, arrows always visible. Unit-tested
  (guard-proven); owner playtest pending.

Source setup:

> **Closed 2026-07-08:** local/SMB/SSH sources were removed from Vela
> entirely (decision `.agents/decisions.md` 2026-07-08 "Vela is a
> multi-server client"; removal landed in 0.1.33). Both entries below are
> moot and retained as history only.

- SMB shares surface labeled "Local" on the main screen. The share connects
  fine, but the UI presents it as "Local" instead of identifying it as an SMB
  source/share. Plausibly a consequence of the SMB-feeds-the-local-source
  design (selected share folders feed the local source; see
  `.agents/state.md`) — unverified — but as presented it is confusing.
  Screenshot evidence: source chips read "All | Plex | Nagatha | Local" and
  the nav lists the share's folders as "movies · Local" and
  "skippy/video/archive/tv · Local". Confirmed 2026-07-04: SMB/SSH folders
  are flattened into the single hardcoded "Local" source. Plan:
  `.agents/plans/smb-source-labeling.md`. Implemented 2026-07-04: each
  SMB/SSH mount now registers as its own source (`smb-<id>`/`ssh-<id>`)
  carrying the mount's human name; chips and nav tags pick it up
  automatically. Unit-tested (guard-proven); owner playtest pending.

- The `sshfs` requirement surfaces too late, and its install guidance is a
  dead end on macOS. Diagnosed 2026-07-04 on the owner's machine: the
  attempted `brew install sshfs` installed nothing — Homebrew core's `sshfs`
  depends on `libfuse`, which is Linux-only, so it cannot succeed on macOS.
  No sshfs binary exists in any checked location and macFUSE is absent;
  Vela's detection and its "sshfs was not found" error were correct.
  Remaining work: surface the dependency up front in the add-SSH UI rather
  than at add-failure time, and make the guidance platform-aware — on macOS
  the working route is the `macfuse` cask plus a macFUSE-compatible sshfs
  build (e.g. `gromgit/fuse/sshfs-mac`), including approving the macFUSE
  kernel/system extension (on Apple Silicon that means allowing third-party
  kexts via Recovery). Bare "install sshfs" sends macOS users into exactly
  this dead end. Decided 2026-07-04 (see `.agents/decisions.md`): keep the
  sshfs dependency and handle macOS with in-UI setup help/hint text (upfront
  requirement, platform-aware install route, known instability caveats);
  macOS SSH live testing is parked because the brew macFUSE/sshfs-mac builds
  segfault or act oddly on the owner's machine. Note for any eventual retest:
  mounts run ssh with `BatchMode=yes` and no password support, so first-time
  hosts need their host key trusted via plain `ssh` before Vela can mount
  them. Plan: `.agents/plans/ssh-macos-guidance.md`. Implemented 2026-07-04:
  `sshfs_status` command + upfront platform-aware guidance in the add-SSH
  panel, platform-aware mount error (unit-tested), host-key note in the form
  footer. Automated checks pass; owner visual check pending.

Library navigation and the "All" view (owner direction, 2026-07-04):

- The "All" view is not useful in its current form, and the library list
  needs rework. Per-source views break down by content type, but "All" breaks
  down by connection/share — as segregated as per-source browsing, only
  busier and sloppier. The nav sprawls one flat entry per library per source
  ("Movies · Plex", "Movies Archive · Plex", "TV Shows · Plex", "TV Shows
  Archive · Plex", "Shows · Nagatha", "movies · Local",
  "skippy/video/archive/tv · Local"). Owner direction: "All" should be a
  consolidated, deduped listing by content type — one entry per title backed
  by every source that carries it, defaulting playback to the most
  performant/reliable source, with an override to pick the source per title.
  Prerequisite: SMB/local metadata is not cached today, so those sources load
  slowly; likely needs persistent local metadata caching first (confirmed
  2026-07-04: the existing `metadata_cache.json` caches online-lookup results
  only; directory listings are re-walked live on every call). Plan, phased:
  `.agents/plans/library-all-view-rework.md`. All four phases implemented
  2026-07-04 (listing cache; consolidated type nav with merged listings;
  provider-id/title+year dedup with backing lists; kind ranking + per-title
  override via context menu). Unit-tested throughout (guard-proven); owner
  playtest pending.

## Kimi-K2.6 Review Triage (2026-05-23)

Review triage from the Kimi-K2.6 report against `vela-foundation` on 2026-05-23.
Items here are verified or worth tracking. Severity is adjusted from the report
where the original claim was overstated.

> **Status (addressed 2026-05-23):** All P0, P1, and P2 items below are
> implemented, with two deliberate exceptions noted inline: `serde-xml-rs` is
> kept (it's used for nested Plex XML; migration deferred, not dropped) and
> `bundle.targets = "all"` is left as-is (Tauri's `all` is host-native, not a
> cross-compile failure). The 5 pre-existing Plex dead-code warnings are now
> silenced with targeted `#[allow(dead_code)]`, so `clippy -D warnings` is clean
> and CI enforces it. Not-runtime-verified: the CSP needs confirming against a
> release build (it doesn't apply to the Vite dev server).

### P0 - Fix Before Merge

- Move blocking OS/process work out of async command bodies.
  `mount_smb`, `unmount_smb`, and `play_item` call OS mount/unmount or child
  process wait/spawn paths from async Tauri commands. Use
  `tauri::async_runtime::spawn_blocking` or make the commands synchronous.

- Fail closed on Plex stream preflight errors.
  `src-tauri/src/source/plex.rs` only rejects `404` during `HEAD` preflight.
  Non-2xx statuses and request errors should surface before launching mpv.

- Add HTTP status checks before parsing Plex XML.
  Several Plex library requests parse response bodies without
  `error_for_status()`, producing confusing XML parse errors for HTTP failures.

- Percent-encode Jellyfin/Emby stream and poster URL query values.
  `stream_url` and `poster_url` interpolate ids, tags, device ids, session ids,
  and tokens into URLs directly. Build these URLs with `url::Url` or
  `url::form_urlencoded`.

- Add keys to dynamic Svelte `{#each}` blocks.
  Source, section, hub, item, crumb, sort, folder, and SMB rows should use stable
  keys to avoid stale DOM state after source switches and list refreshes.

- Restore a restrictive Tauri CSP.
  `tauri.conf.json` currently has `"csp": null`. Add a CSP that allows the app,
  Tauri asset URLs, needed image sources, and no unnecessary script sources.

- Tighten local file exposure through the asset protocol.
  Folder and SMB commands currently expand the asset protocol scope to any path
  passed by the webview. Keep the intentional local-media behavior, but reduce
  XSS blast radius with stricter command validation or a narrower file-serving
  strategy.

### P1 - Security and Reliability Hardening

- Put mpv IPC sockets in a private runtime directory.
  The Unix IPC path is predictable under `/tmp`. Use a per-app private directory
  with owner-only permissions and a random path component.

- Replace the Plex PIN XML attribute helper with a real XML parser.
  The current helper is small and probably fine for Plex's current response, but
  the project already uses `quick-xml`; parse the PIN response with it instead
  of string searching.

- Remove `serde-xml-rs` if practical.
  The project already uses `quick-xml`, and some Plex paths switched to manual
  streaming because `serde-xml-rs` did not fit nested Plex XML well. Migrate the
  remaining struct parses before dropping the dependency. Treat any vulnerability
  claim as unverified until `cargo audit` is available.

- Add `rust-version` to `src-tauri/Cargo.toml`.
  The branch uses APIs such as `Result::inspect_err`; declare the supported Rust
  floor explicitly.

- Revisit `bundle.targets = "all"`.
  Native packaging should be platform-specific unless CI is prepared to build all
  Tauri bundle formats on every OS.

- Add release profile settings.
  Consider size-oriented release defaults such as LTO, `panic = "abort"`,
  fewer codegen units, and strip settings after confirming they do not hurt debug
  symbol needs.

- ~~Bound and decouple metadata cache writes.~~ **MOOT 2026-07-14 — the cache no
  longer exists.** `metadata_cache.json` and the listing cache died with the
  local/SMB/SSH source removal (decision `.agents/decisions.md` 2026-07-08;
  landed in 0.1.33). `config.json` is now the only persistent file in the app,
  and it is small and bounded (recents capped at 20, tombstones at 200). If a
  persistent metadata cache is ever reintroduced, this concern — unbounded
  growth, whole-file writes under a lock — comes back with it, and is the one
  thing in this repo that would justify an embedded database.

- Render the QR code without raw `{@html}` where possible.
  The current SVG is backend-generated, so this is low risk, but an `<img>` data
  URI or sanitized SVG keeps the UI safer if the data path changes later.

### P2 - UX, Accessibility, and Maintenance

- Fix Settings modal accessibility.
  Remove `role="button"` from the backdrop, move focus into the dialog on open,
  trap focus while open, and restore focus on close.

- Track and clear frontend timers.
  Store timeout ids for clipboard reset and Plex link polling; clear them when
  superseded or on component destroy.

- Add poster image fallback handling.
  Server and online poster URLs should fall back to the no-art placeholder on
  load failure.

- Add a playback option for borderless mpv windows.
  Expose a Vela setting that passes `--border=no` / `--no-border` when spawning
  mpv, so users can remove window-manager decorations from the playback window.

- Add CI.
  Minimum checks: `cargo check`, `cargo clippy --all-targets`, `cargo test`,
  `npm run check`, and dependency auditing once tooling is installed.

### Not Queued From The Report

- SMB credentials in process arguments: already documented and accepted as a
  local-only exposure for this branch. Reopen only if the threat model changes.

- Progress bar hidden when `viewOffsetMs === 0`: hiding a zero-progress bar is
  acceptable UI behavior.

- EDL parser empty URL segment: current EDL strings are generated internally from
  non-empty Plex part URLs; not a practical issue unless external EDL input is
  introduced.

- Broad component/CSS refactors and magic-number cleanup: useful later, but not
  merge-blocking for the current branch.
