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
- Token-bearing poster/stream URLs and SMB mount credential process arguments
  are an accepted local-only exposure. Avoid adding any new logs, errors, or UI
  copy that reveal token-bearing URLs or credentials.
- README status now reflects heuristic media-version/source selection and the
  lack of a manual version picker.
- Manual/live integration is still pending for Jellyfin/Emby, local folders, and
  SMB shares against real servers/shares.

## Next

- Smoke-test Jellyfin/Emby, local folders, and SMB shares against real
  servers/shares, or keep the live-integration caveat explicit.
- If updating broader governance metadata, refresh `.agents/repo-map.json` and
  `.agents/artifact-manifest.json` from their old `validated_against` commit.

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
