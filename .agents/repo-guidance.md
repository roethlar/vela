# Repo-Specific Guidance
<!-- Extends AGENTS.md; never overrides it. Rules and pointers only — state
     lives in .agents/state.md. -->

## Mission Detail

- Vela is a Tauri 2 desktop app with a SvelteKit/TypeScript frontend in `src/`
  and a Rust backend in `src-tauri/`.
- The app browses Plex, Jellyfin, Emby, local folders, SMB shares, and SSH/SFTP
  mounts through a common media-source abstraction.
- Playback is intentionally delegated to the system `mpv` binary in its own
  window for HDR passthrough. Do not embed video in the webview unless the
  owner explicitly changes that product decision.
- SvelteKit is configured as a static SPA for Tauri. Vite dev uses port `1420`
  with strict port behavior.
- Linux release packaging lives in `src-tauri/bundle/linux/` for Tauri bundles
  and `packaging/arch/` for the Arch package.

## Reading Order

1. `AGENTS.md` and this file.
2. `.agents/state.md` for current active work and blockers.
3. `.agents/decisions.md` for durable decisions and supersessions.
4. `.agents/repo-map.json` for repo shape and verification commands.
5. `README.md` and `ISSUES.md`.
6. `.review/deduped_action_list.md` and `.review/gpt_review.md` as historical
   evidence only — both carry a 2026-06-10 supersession banner; their current
   facts live in `.agents/state.md` and `.agents/decisions.md`, not in the
   `.review/` files themselves.

## Verification

Concrete commands and their working directories are recorded in
`.agents/repo-map.json`; this section adds rules the command list doesn't
carry on its own.

- Run Rust commands (`cargo check`, `cargo clippy`, `cargo test`) from
  `src-tauri/`, not the repo root.
- For changes that can affect both sides of the Tauri app, run the full CI
  command set: `npm run check`, `npm run build`, `cargo check --locked`,
  `cargo clippy --all-targets --locked -- -D warnings`, and
  `cargo test --locked`.
- Packaging changes should also run the affected packaging command when
  practical: `npm run build:linux` for Tauri Linux bundles or
  `npm run build:arch` for the Arch package.

## Remotes & Sync

- `origin`: `http://q.internal:3000/michael/vela.git` (fetch and push).
- Push policy lives in `.agents/push-policy.md`.

## Earned Practices

- Keep token and credential handling conservative. Plex/Jellyfin/Emby poster
  and stream URLs may carry tokens as an accepted local-only exposure. Do not
  add logs, errors, analytics, or copied UI text that expose token-bearing
  URLs, auth tokens, SMB passwords, or config contents. (See
  `.agents/decisions.md`, 2026-05-23.)
- Keep config persistence defensive. The config may contain Plex/Jellyfin/Emby
  tokens and SMB credentials; preserve owner-only Unix permissions, atomic
  saves, parse-error fail-closed behavior, and cross-process locking.
- Do not hold async runtime workers or shared locks across blocking OS,
  filesystem, process, or network work. Use the existing lock boundaries and
  `spawn_blocking` patterns.
- Local media roots must stay narrow. Continue rejecting filesystem roots and
  home roots, and keep symlink escape checks before listing, searching, or
  playing local files. (See `.agents/decisions.md`, 2026-05-23.)
- Linux SMB is native and mountless: in-process libsmbclient for
  browsing plus a loopback HTTP Range proxy for mpv playback — no OS
  mounts, no root, no gvfs/kio. Keep provider paths share-scoped (the
  normalize/containment rules in `source/smb_vfs.rs`) and never
  allow-list provider paths with the asset protocol. macOS/Windows still
  mount via the OS. (See `.agents/decisions.md`, 2026-07-04; plan
  `.agents/plans/smb-native-client.md`.) SSH/SFTP support uses `sshfs`
  with OpenSSH keys, agent, and config; Vela does not store SSH
  passwords. (See `.agents/decisions.md`, 2026-05-23.)
- Generated outputs and dependency/build directories are not source of truth.
  Do not edit `build/`, `.svelte-kit/`, `node_modules/`, `src-tauri/target/`,
  `src-tauri/gen/`, or packaged Arch output under `packaging/arch/pkg/`.
