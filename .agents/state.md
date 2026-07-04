# Agent State

This file is the first place future agents should read for current repo state.
Keep it short and update it when important repo facts change.

## Now

- Vela is a Tauri 2 + SvelteKit + Rust desktop media client for Plex,
  Jellyfin, Emby, local folders, SMB shares, and SSH/SFTP mounts. It plays
  media through the system `mpv` binary for HDR passthrough.
- Version 0.1.9 at `864bdd0`. Remote `github` is current; remote `origin`
  (q:3000) is 3 commits behind — the owner pushes manually (push policy:
  ask, `.agents/push-policy.md`).
- 2026-07-04 landed a large batch, all owner-approved and verified:
  - All five approved plans implemented (see `.agents/plans/*`): post-playback
    watch-state refresh (`playback-ended` event); platform-aware sshfs
    guidance in the add-SSH UI; each SMB/SSH mount registered as its own
    named source; split artwork policy (16:9 resume surfaces, 2:3 catalog
    rows with series posters); library rework phases A-D (persistent listing
    cache for local/SMB, consolidated type-based All nav, cross-source dedup
    via provider ids with backing lists, kind-ranked playback with per-title
    override persisted in `merged_overrides`).
  - The batch then passed a cross-harness review loop (playbook
    `reviewloop`, reviewer codex): 5 findings fixed, guard-proven,
    independently verified, merged. Durable trail:
    `.agents/review/index.md` + `findings/`. Notable outcome: the merged
    All view pages from an immutable `MergedSnapshot` in `AppState`
    (stateless merged pagination was proven unsound in review).
  - Post-review owner-directed UI changes (decisions recorded 2026-07-04 in
    `.agents/decisions.md`): the Continue Watching hero is a cover-flow
    (~30% window height, older items fanned behind-left, newer behind-right,
    always-visible arrows) fed by Vela's OWN recents — semantic: "recently
    played and not finished = Continue Watching", any source, any duration
    (`src-tauri/src/recents.rs`; snapshot at play, position stamped at mpv
    exit via `EndNotify(u64)`, finished entries dropped at
    `watched_threshold_percent`, default 95%). Library nav moved to a left
    sidebar (Home / Library / Sources groups, Infuse reference
    `reference_screens/infuse-home-reference.png`).
- Token/credential stance unchanged: poster URLs (all backends),
  Jellyfin/Emby stream URLs, and SMB mount arguments are accepted local-only
  exposures; Plex stream auth rides as an `X-Plex-Token` header via an
  owner-only mpv include file. Add nothing new that logs or displays
  token-bearing URLs. Recents snapshots in `config.json` carry poster URLs —
  same exposure class as the config's stored tokens.
- macOS SSH live testing is parked (brew macFUSE/sshfs-mac unstable on the
  owner's machine); the shipped in-UI guidance is the decided handling.
- Known accepted v1 gaps: backend queue auto-advance plays are not
  snapshotted into recents; local-source series artwork deferred (portrait
  cards fall back to episode still/no-art); a merged card's progress bar can
  reflect server state while the ranked play target is a local copy (the
  per-title override is the escape hatch).
- `scripts/build.sh` takes ~2.5 min cold on macOS; the session's `!`
  foreground runner kills at 2 min mid-DMG (leaves a mounted staging volume
  and no final dmg) — run builds via the agent (no cap) or a real terminal.
  The `.app` exists only transiently during bundling; the DMG is the
  artifact.

## Next

- Owner playtest sweep of the whole 2026-07-04 batch (v0.1.9 dmg is built):
  sidebar nav; cover-flow hero — a few-seconds play should appear centered
  after mpv closes (recents semantic), and a >60s Plex play should also
  reach the server hub; watch-state refresh without restart; named SMB
  share in Sources; merged All view listing (scroll depth, "N sources"
  cards, context-menu "Play from" persisting an override); sshfs panel
  guidance.
- Finish live smoke tests: Emby, local folders, SMB browse/playback depth.
- Letterbox crop (decided 2026-07-03): next is the render-zoom safety spike
  (owner at the machine), then design approval, then code. Draft:
  `.agents/plans/letterbox-crop.md`.
- Plex stream header auth residuals: owner eyeball check on a real play
  (title bar / Shift+I clean), EDL split-file exercised only by unit tests,
  Jellyfin/Emby stream-URL parity follow-up.
- If updating broader governance metadata, refresh `.agents/repo-map.json`
  and `.agents/artifact-manifest.json` from their old `validated_against`
  commit.

## Blockers

- None recorded.

## Verification

- See `.agents/repo-map.json` for the current automated verification
  commands (npm check/build; cargo check/clippy/test from `src-tauri/`).
  Rust suite is at 56 tests, clippy `-D warnings` clean.
- Rust verification on Linux needs the Tauri/WebKitGTK system dependencies
  used by CI.

## Active Sources

- `AGENTS.md`
- `.agents/repo-guidance.md`
- `.agents/repo-map.json`
- `.agents/decisions.md`
- `.agents/plans/` (all five 2026-07-04 plans carry implementation notes)
- `.agents/review/index.md` (completed review loop, durable trail)
- `README.md`
- `ISSUES.md`
- `.review/deduped_action_list.md` and `.review/gpt_review.md` as historical
  evidence only

## Unrecorded Repo Memory

- None known.
