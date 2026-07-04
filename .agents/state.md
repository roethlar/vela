# Agent State

This file is the first place future agents should read for current repo state.
Keep it short and update it when important repo facts change.

## Now

- Vela is a Tauri 2 + SvelteKit + Rust desktop media client for Plex,
  Jellyfin, Emby, local folders, SMB shares, and SSH/SFTP mounts. It plays media
  through the system `mpv` binary for HDR passthrough.
- Local filesystem browsing/search traversal has been moved off async source
  workers through `spawn_blocking`; older governance notes that list this as
  deferred are stale.
- SMB shares are mounted first, then one or more selected folders inside the
  share can feed the local source. Legacy SMB config records are normalized into
  that selected-folder shape on load.
- mpv discovery now validates runnable candidates, supports a configured custom
  executable, and reports the detected installer path/description to the UI.
- Token-bearing poster URLs (all backends), Jellyfin/Emby stream URLs, and SMB
  mount credential process arguments are an accepted local-only exposure. Plex
  stream URLs are credential-free as of 2026-07-03: the token rides as an
  `X-Plex-Token` header delivered to mpv via an owner-only include file. Avoid
  adding any new logs, errors, or UI copy that reveal token-bearing URLs or
  credentials.
- README status now reflects heuristic media-version/source selection and the
  lack of a manual version picker.
- Live smoke tests 2026-07-04: Jellyfin passed against a real server; SMB
  connected but surfaces labeled "Local"; the SSH add failure was diagnosed —
  `sshfs` was not actually installed (Homebrew core's formula depends on
  Linux-only libfuse, so `brew install sshfs` cannot work on macOS). The tap
  route (macFUSE + sshfs-mac 2.10) installs and runs but segfaults or acts
  oddly in use, so macOS SSH live testing is parked; decided 2026-07-04:
  handle macOS SSH with in-UI setup guidance (see `.agents/decisions.md`).
  Findings queued in `ISSUES.md`, alongside an owner-direction
  rework of the library list and "All" view (consolidated, deduped,
  cross-source, metadata caching for SMB/local first). Still pending live:
  Emby, local folders, and SMB browse/playback depth.

## Next

- All five plans were approved by the owner on 2026-07-04, with each plan's
  "proposed" defaults adopted. Implementation order: watch-state-refresh →
  ssh-macos-guidance → smb-source-labeling → row-artwork-consistency →
  library-all-view-rework (phases A-D). The plans:
  `.agents/plans/watch-state-refresh.md`,
  `.agents/plans/row-artwork-consistency.md`,
  `.agents/plans/smb-source-labeling.md`,
  `.agents/plans/library-all-view-rework.md` (phased A-D; largest; its
  ranking phase depends on smb-source-labeling landing first), and
  `.agents/plans/ssh-macos-guidance.md` (implements the 2026-07-04
  macOS-SSH decision; live mount testing stays parked). A 2026-07-04
  design-language decision (Infuse reference,
  `reference_screens/infuse-home-reference.png`) resolved the artwork
  plan's poster-vs-content question as the split policy and shapes the
  rework's nav phase.
- Finish live smoke tests: Emby, local folders, SMB browse/playback; or keep
  the live-integration caveat explicit.
- If updating broader governance metadata, refresh `.agents/repo-map.json` and
  `.agents/artifact-manifest.json` from their old `validated_against` commit.
- Letterbox crop (decided 2026-07-03, see `.agents/decisions.md`): next step is
  the render-zoom safety spike (needs the owner at the machine — it plays video
  on the real HDR stack and probes the known D-state wedge), then an approved
  design plan, then code. Draft plan: `.agents/plans/letterbox-crop.md`.
- Plex stream header auth landed 2026-07-03 (decision and implementation, see
  `.agents/decisions.md`). Verified live: header-authed HEAD on a real part
  URL 200, no-auth 401, and a windowless 10-frame mpv decode over header auth
  passed. Remaining: owner eyeball check on the next real play (title bar and
  `Shift+I` stats should show no token), EDL split-file media exercised only
  by unit tests, and Jellyfin/Emby stream-URL parity as a follow-up.

## Blockers

- None recorded.

## Verification

- See `.agents/repo-map.json` for the current automated verification commands.
- Rust verification on Linux needs the Tauri/WebKitGTK system dependencies used
  by CI.

## Active Sources

- `AGENTS.md`
- `.agents/repo-map.json`
- `.agents/decisions.md`
- `README.md`
- `ISSUES.md`
- `.review/deduped_action_list.md` and `.review/gpt_review.md` as historical
  evidence only

## Unrecorded Repo Memory

- None known.
