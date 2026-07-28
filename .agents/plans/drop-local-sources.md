# Plan: Drop local/SMB/SSH sources — Vela is a multi-server client

## Status
**COMPLETE 2026-07-09 — all three slices landed.** Slice 2 (E2E re-home)
landed `80dd8e6`+`b223951`+`b41703a` (0.1.40; loop `dls-s2` accepted clean
r1): full suite 10/10 on the owner's Linux VM (Ubuntu 25.10 aarch64), and
the re-home surfaced + fixed a real nav-flip regression (context-menu Play
threw — see the loop trail in `.agents/review/index.md`).

Landing history: **slices 1 and 3 landed first, slice 2 last (2026-07-09).**
Slice 1 (turn-off-and-delete) landed 2026-07-08 (0.1.33, `6855df5`; loop
`dls-s1` clean r1) and is owner-playtested. Slice 3 (docs sweep) landed
2026-07-09 — README/ISSUES/repo-guidance de-localed, six obsolete plans
bannered, decision closures recorded; the "repo-map refresh" item is moot
(`.agents/repo-map.json` was retired by the 2026-07-08 governance refresh).
Slices 2 and 3 were reordered (3 before 2) while the VM was provisioned; they
were independent, and Slice 2 later completed the plan.
Plan-review loop CLOSED accepted at r5 (five rounds, nine findings
dls-r1..r4 all resolved — see Review log).

## Goal
Remove local-file playback entirely: local folders, SMB shares, and SSH/SFTP
mounts stop existing as sources. Sources are media servers only (Plex now;
Jellyfin/Emby stay — the owner will eventually migrate off Plex to one of
them). The app must remain coherent, CI-green, and dead-end-free at every
slice boundary.

Owner wording (2026-07-08): "no need for vela to play local files at all.
multiple servers stay." Rejected framings (do not resurrect): local library
with metadata scraping, file browser ("I already have file explorer, finder,
dolphin"), server↔file direct-play mapping ("zero value to accessing the same
file via ssh, smb, plex, emby, and jellyfin").

## What gets deleted (inventory, 2026-07-08 tree)
- **Rust modules (~200 KB src)**: `source/local.rs`, `source/vfs.rs`,
  `source/smb_vfs.rs`, `source/metadata.rs`, `source/listing_cache.rs`,
  `smb_client.rs`, `smb.rs`, `sshfs.rs`, `stream_proxy.rs`; the `velasmb:`
  scheme + loopback-proxy handling in `playback.rs` (incl.
  `proxy_reconnect_args`); local-family registration in `lib.rs`/registry.
- **lib.rs startup restore paths (r1 finding 1 — slice 1, not slice 2)**: the
  boot sequence clones `smb_mounts`/`ssh_mounts` from config and auto-remounts
  them (`lib.rs:144-145`, `:283-348`) and re-registers local-family sources via
  `refresh_local_source` (`:318`, `:348`, `:444-467`). These must be disabled in
  slice 1 or an old config keeps performing SMB/SSH side effects and
  resurrecting local sources despite the "tolerated but ignored" rule.
- **commands.rs**: the 15 local-family commands (`add/list/remove_local_folder`,
  `mount/unmount_smb`, `list_smb_mounts`, `list_smb_directories`,
  `add/remove_smb_folder`, `rename_smb_mount`, `mount/unmount_ssh`,
  `list_ssh_mounts`, `rename_ssh_mount`, **`sshfs_status`** — registered at
  `lib.rs:376`, missed by the first inventory) + their pure helpers/tests; the
  `local`/`smb`/`ssh` arms of `kind_rank`/`detail_rank` (both collapse to
  plex < jellyfin/emby < unknown).
- **Non-command references that must be pruned within the same
  turn-off-and-delete slice 1 for it to compile (r1 finding 2; renumbered per
  r2/r3)**: `remove_source`'s `source::local::is_local_family_id` gate
  (`commands.rs:211`); the playback-cleanup proxy-session machinery
  `proxy_session_key`/`release_proxy_session` and their call sites
  (`commands.rs:3516-3673`), which reference `stream_proxy`. Sweep for further
  cross-module references (`grep stream_proxy|sshfs|smb|local::` outside the
  deleted modules) before declaring the slice compilable.
- **Frontend**: Settings' local-folder / SMB / SSH add forms, the Connected
  tab's mount rows + rename/remove affordances, sshfs guidance panel; the
  unauthenticated empty-state copy "Connect Plex, Jellyfin, Emby, or a local
  folder" (`+page.svelte:1160` — r1 finding 5) loses its local-folder clause.
- **Packaging & build deps (r1 finding 4)**: PKGBUILD `smbclient` dep AND the
  `sshfs` optdepends block (`PKGBUILD:12-14`); deb/rpm `libsmbclient` deps;
  the Linux-target `pavao-sys`/`libc` dependency block in
  `src-tauri/Cargo.toml:39-41` (+ its comment). `license=('MIT' 'GPL2')`
  stays — that's the mpv autocrop script, unrelated.
- **Docs**: repo-guidance mission line + SMB/SSH earned-practice bullets,
  README/ISSUES mentions, `.agents/repo-map.json` refresh.
- **Plans CLOSED as obsolete** (banner, not deletion): `smb-native-client.md`,
  `smb-share-root-autoadd.md`, `smb-source-labeling.md`, `ssh-macos-guidance.md`,
  `local-metadata-revalidation.md`, `smb-ssh-playtest-fixes.md` (its Bug 4 +
  metadata rail die unworked); item-detail-view's deferred local slice is dead.

## What stays (explicitly)
Multi-server support and the merged All view + dedup/backing machinery
(server↔server overlap becomes real during the migration); `watch_key`/
`detail_key` (inert while only one server, live again with two); mpv
delegation + HDR stance; recents/Continue Watching (server-agnostic);
queue; the mock-JF E2E leg. NOT in scope: the Plex→JF/Emby watch-state
one-shot copy tool (separate future plan at migration time, per the
decision); any Trakt integration (rejected).

## Compatibility & safety rails
- **Old configs must keep loading.** `local_folders`/`smb_mounts`/`ssh_mounts`
  stay as tolerated serde fields (parsed, ignored, preserved on save — never
  stripped, so a rollback build still sees them). Config parse stays
  fail-closed on corruption, unchanged. **This requires disabling the legacy
  SMB migrator (r3 finding):** `AppConfig::normalize_legacy_smb_mounts`
  (`config.rs:183-228`) runs on every load (`:300`, `:376`) and REWRITES the
  legacy fields (moves `local_folder_id`-era data into `smb_mounts[].folders`,
  strips matching `local_folders` on the next save) — with it active, the
  preserve-on-save promise is false and rollback data is lost. Slice 1 deletes
  the migrator (its purpose is moot once mounts are inert), and the round-trip
  guard must include a legacy-shaped config (pre-migration `local_folder_id`
  form) proving the inert fields survive load→save byte-identical.
  **And the serde attrs must change with it (r4 finding):** `SmbMount.kind`
  and `SmbMount.local_folder_id` are `#[serde(default, skip_serializing)]`
  (`config.rs:96-99`) — they load but NEVER save, so even with the migrator
  gone, any save strips them. Slice 1 replaces `skip_serializing` with
  `skip_serializing_if = "String::is_empty"` on both (present legacy values
  round-trip; fresh configs stay clean), and the round-trip guard must fail
  if either field is dropped on save. Audit note (2026-07-08): these two
  attrs and the one migrator are the complete mutation-on-save surface for
  the inert fields — `grep skip_serializing config.rs` returns exactly
  lines 96/98, and `normalize_legacy_smb_mounts` is the only load-time
  normalizer.
- **Recents referencing removed sources**: dropped at load (a hero card whose
  Play can only error is the dead-end the UX rulings forbid). Tombstones and
  `merged_overrides` residue is harmless (keyed lookups miss) — leave.
- **SMB credentials in old configs**: left in place like the other inert
  fields (same local-only exposure class as today; stripping would break
  rollback). Note in README changelog that removing them is manual.

## Slices (each: own commit, reviewloop codex, full CI, version bump)
1. **Turn off AND delete, one slice (r2 finding: a split boundary cannot be
   CI-green).** Leaving the modules as dead code between an "off" slice and a
   "delete" slice fails Linux clippy `-D warnings` on `dead_code` (this
   crate demonstrably fires that lint — the Windows host's 13 baseline
   warnings are exactly unreferenced local-family helpers). So one slice
   does both, ordered internally as: registry stops constructing
   local/SMB/SSH sources; lib.rs startup restore/remount +
   `refresh_local_source` disabled (r1 finding 1 — no SMB/SSH side effects
   from an old config); the 15 commands + their Settings UI (forms,
   Connected tab) removed together so no affordance can call a missing
   command; empty-state copy loses its local-folder clause; recents
   load-filter for dead sources; rank arms pruned; then the module
   deletion in the same commit — Rust modules, playback proxy paths,
   `velasmb:` scheme, their tests, the non-command cross-references
   (`is_local_family_id` gate, proxy-session cleanup), the
   `pavao-sys`/`libc` Cargo target block, and the packaging deps. One
   large but mechanical deletion-heavy commit; CI green at its end; test
   count drops accordingly (Linux CI is the authoritative gate; the
   Windows host's 13 dead-code baseline warnings disappear with the
   modules).
2. **Re-home the E2E suite.** Port the local-seeded scenarios (playback,
   queue, search, curation, resume) to the mock-JF server's Range-capable
   HTTP streams; `mergedview` becomes two-server (second mock instance —
   mock Emby or a second mock JF on another port); delete `connectedtab`
   (its subject is gone). `sourcedeadend` (r1 finding 3): its two legs are
   a LOCAL source + mock JF (no SMB leg), and the Sources sidebar group it
   clicks only renders with >1 source (`+page.svelte` gates on
   `sources.length > 1`) — it must be rewritten around two mock servers
   (an empty-Home mock + a hubs-serving mock) or deleted with its Bug-3
   coverage noted as lost; a single-mock port would leave it permanently
   red. **The whole slice must be validated on the Linux host** (the
   harness does not run on the Windows dev host); it lands only from a
   session that can run it, or explicitly owner-run.
3. **Docs/guidance sweep.** Mission line, earned practices, README, ISSUES,
   repo-map, plan banners, decision status updates (2026-05-23 local-roots
   + 2026-07-04 SMB-native close as "code removed").

## Open decisions for owner
- Slice 2 (E2E) timing — **RESOLVED 2026-07-09**: slice 1 landed 2026-07-08
  with the e2e suite accepted temporarily broken; slice 2 is in progress
  from the owner's Linux VM (slices 2/3 reordered — see Status). No owner
  decision remains here.
- Nothing else — config tolerance and recents filtering above are the
  recommended defaults unless overridden.

## Verification
- Per slice: `npm run check`, `npm run build`, `cargo check --locked`,
  `cargo clippy --all-targets --locked -- -D warnings` (Linux) /
  baseline-compare (Windows host), `cargo test --locked`.
- Slice 1 guards: unit tests that an old config with populated
  local/SMB/SSH fields loads, registers zero local-family sources, and
  round-trips those fields unchanged on save; recents dead-source filter
  guard-proven red/green.
- Owner playtest after slice 1: old real config boots clean; sidebar shows
  servers only; hero has no dead cards; Plex playback/seek unaffected.

## Review log
Plan-review loop (playbook `reviewloop`, reviewer `codex` 0.142.5, read-only).

**r1 — 2026-07-08 — verdict `reopened`, 5 findings, all ADMITTED (each verified
against the tree before acceptance).**
- (HIGH) lib.rs startup restore/remount + `refresh_local_source` paths missing
  from slice 1 — old configs would keep performing SMB/SSH side effects. Fixed:
  inventory + slice 1 now disable them explicitly.
- (HIGH) Inventory incomplete for a compiling slice 2: `sshfs_status` (15th
  command), `remove_source`'s `is_local_family_id` gate, playback-cleanup
  proxy-session machinery in commands.rs. Fixed: all enumerated + a pre-delete
  cross-reference sweep required.
- (MEDIUM) `sourcedeadend` mis-inventoried (local+mockJF legs, no SMB leg; needs
  >1 source for the sidebar group) — a single-mock port would stay red. Fixed:
  slice 3 rewrites it around two mock servers or deletes it with coverage loss
  noted.
- (MEDIUM) Build/packaging cleanup missed `pavao-sys`/`libc` Cargo target block
  and the PKGBUILD `sshfs` optdepends. Fixed: added to inventory + slice 2.
- (LOW) Unauthenticated empty-state copy still offers "or a local folder".
  Fixed: added to the frontend sweep (slice 1).

**r2 — 2026-07-08 — verdict `reopened`, 1 finding, ADMITTED (r1 fixes all
confirmed resolved).**
- (MEDIUM) The off-then-delete two-slice split cannot be CI-green at its
  boundary: with registry/startup/command call sites gone, the retained
  modules become `dead_code` and Linux clippy `-D warnings` fails before the
  delete slice (this crate demonstrably fires the lint — the Windows host's
  13-warning baseline is unreferenced local-family helpers). Fixed: slices 1
  and 2 merged into ONE turn-off-and-delete slice (temporary
  `#[allow(dead_code)]` scaffolding rejected as churn); later slices
  renumbered (E2E re-home = 2, docs sweep = 3).

**r3 — 2026-07-08 — verdict `reopened`, 2 findings, both ADMITTED (r2 fix
confirmed resolved).**
- (MEDIUM) The preserve-on-save compatibility rail was contradicted by the
  live legacy migrator `normalize_legacy_smb_mounts` (`config.rs:183-228`,
  invoked at `:300`/`:376`), which rewrites legacy SMB/local fields on load —
  rollback data loss. Fixed: slice 1 deletes the migrator; the round-trip
  guard must cover a legacy-shaped (pre-migration) config byte-identically.
- (LOW) A stale "for slice 2 to compile" phrase survived the r2 renumbering
  in the inventory. Fixed: rephrased to the turn-off-and-delete slice 1.

**r4 — 2026-07-08 — verdict `reopened`, 1 finding, ADMITTED (r3 fixes
confirmed resolved).**
- (MEDIUM) The preserve-on-save rail was still incomplete: `SmbMount.kind` /
  `SmbMount.local_folder_id` are `#[serde(default, skip_serializing)]`
  (`config.rs:96-99`) — never written back, so saves strip rollback data even
  without the migrator. Fixed: slice 1 switches both to
  `skip_serializing_if = "String::is_empty"`; the round-trip guard must fail
  if either drops. Coder audit exhausted the class: those two attrs + the one
  migrator are the entire mutation surface for the inert fields.

**r5 — 2026-07-08 — verdict `accepted`, 0 comments** (reviewed_sha=base_sha
`2533f09`; `guard_confirmed:false` — read-only on a design doc). r4 fix
confirmed; no new material defect. **Plan-review loop CLOSED — awaiting owner
approval before implementation.** Healthy converging loop: r1 (5) → r2 (1) →
r3 (2) → r4 (1) → r5 (clean); every finding independently verified against
the tree before acceptance.
