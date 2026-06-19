# Agent State

This file is the first place future agents should read for current repo state.
Keep it short and update it when important repo facts change.

## Now

- Vela is a Tauri 2 + SvelteKit + Rust desktop media client for Plex,
  Jellyfin, Emby, local folders, SMB shares, and SSH/SFTP mounts. It plays media
  through the system `mpv` binary for HDR passthrough.
- The review hardening pass represented by `.review/deduped_action_list.md` is
  mostly complete. The known remaining product work is moving local filesystem
  browsing/search traversal out of async source methods.
- Token-bearing poster/stream URLs and SMB mount credential process arguments
  are an accepted local-only exposure. Avoid adding any new logs, errors, or UI
  copy that reveal token-bearing URLs or credentials.
- Possible doc drift: `README.md` still says multi-version items play the first
  available part, while current code scores Plex and Jellyfin/Emby media sources
  by directness, HDR, resolution, and bitrate. Verify the exact intended wording
  before editing the README status section.

## Next

- If continuing product hardening, move local listing/search work in
  `src-tauri/src/source/local.rs` off async worker threads, then run the relevant
  Rust and frontend verification.

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
