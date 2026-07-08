# Plan: Drop local/SMB/SSH sources — Vela is a multi-server client (DRAFT)

## Status
DRAFT 2026-07-08 — decision recorded (`.agents/decisions.md` 2026-07-08 "Vela is
a multi-server client"); plan not yet codex-reviewed or owner-approved. No code
change until approval.

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
- **commands.rs**: the 14 local-family commands (`add/list/remove_local_folder`,
  `mount/unmount_smb`, `list_smb_mounts`, `list_smb_directories`,
  `add/remove_smb_folder`, `rename_smb_mount`, `mount/unmount_ssh`,
  `list_ssh_mounts`, `rename_ssh_mount`) + their pure helpers/tests; the
  `local`/`smb`/`ssh` arms of `kind_rank`/`detail_rank` (both collapse to
  plex < jellyfin/emby < unknown).
- **Frontend**: Settings' local-folder / SMB / SSH add forms, the Connected
  tab's mount rows + rename/remove affordances, sshfs guidance panel.
- **Packaging**: PKGBUILD `smbclient` dep (+ `license=('MIT' 'GPL2')` stays —
  that's the mpv autocrop script, unrelated); deb/rpm `libsmbclient` deps.
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
  fail-closed on corruption, unchanged.
- **Recents referencing removed sources**: dropped at load (a hero card whose
  Play can only error is the dead-end the UX rulings forbid). Tombstones and
  `merged_overrides` residue is harmless (keyed lookups miss) — leave.
- **SMB credentials in old configs**: left in place like the other inert
  fields (same local-only exposure class as today; stripping would break
  rollback). Note in README changelog that removing them is manual.

## Slices (each: own commit, reviewloop codex, full CI, version bump)
1. **Turn the family off at the surface.** Registry stops constructing
   local/SMB/SSH sources; the 14 commands + their Settings UI (forms,
   Connected tab) go together so no affordance can call a missing command;
   recents load-filter for dead sources; rank arms pruned. App = servers
   only; all local-family Rust below the registry is now dead code but
   still compiles. CI green.
2. **Delete the corpse.** Remove the Rust modules, playback proxy paths,
   `velasmb:` scheme, their tests, and the packaging deps. CI green; test
   count drops accordingly (Linux CI is the authoritative gate — the
   Windows dev host skips the unix-gated tests anyway, and its 13
   dead-code baseline warnings should disappear with the modules).
3. **Re-home the E2E suite.** Port the local-seeded scenarios (playback,
   queue, search, curation, resume) to the mock-JF server's Range-capable
   HTTP streams; `mergedview` becomes two-server (second mock instance —
   mock Emby or a second mock JF on another port); delete `connectedtab`
   (its subject is gone); `sourcedeadend` keeps its mock-JF leg, loses the
   SMB leg. **Must be validated on the Linux host** (the harness does not
   run on the Windows dev host); this slice lands only from a session that
   can run it, or explicitly owner-run.
4. **Docs/guidance sweep.** Mission line, earned practices, README, ISSUES,
   repo-map, plan banners, decision status updates (2026-05-23 local-roots
   + 2026-07-04 SMB-native close as "code removed").

## Open decisions for owner
- Slice 3 timing: port E2E in the same push (needs a Linux session) or
  accept a temporarily red/thinner e2e suite? Recommendation: slices 1-2
  land now; slice 3 immediately next from the Linux host, before any
  further feature work.
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
- Owner playtest after slice 2: old real config boots clean; sidebar shows
  servers only; hero has no dead cards; Plex playback/seek unaffected.

## Review log
(plan-review rounds recorded here)
