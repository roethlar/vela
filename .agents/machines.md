# Machines

Machine-specific facts (host layout, tool paths, local versions). Portable
truth belongs in `.agents/state.md` / `.agents/repo-guidance.md`; this file is
the only place allowed to know about a particular box.

## Linux VM (E2E host)

Recorded 2026-07-13.

- The E2E suite (`npm run e2e`) is Linux-only and runs here, not on the macOS
  dev box: `michael@192.168.64.5`, clone at `~/dev/vela`, reachable from the
  dev box as the `vm` git remote (`receive.denyCurrentBranch=updateInstead`, so
  a plain `git push vm main` updates its working tree).
- **`cargo` is on the LOGIN shell PATH only** (`~/.cargo/bin`). A plain
  `ssh michael@192.168.64.5 'cd ~/dev/vela && npm run e2e'` gets a non-login
  shell and dies in the Tauri build with `failed to run 'cargo metadata'
  command ... No such file or directory`. That error names cargo but means
  PATH. Wrap the whole command: `ssh michael@... 'bash -lc "cd ~/dev/vela &&
  npm run e2e"'`.
- The suite drives the real debug binary, so the first run after a push pays
  for a `tauri build --debug`. `npm run e2e -- --skip-build` reuses the
  existing binary; `npm run e2e -- <name>` runs a single scenario.
