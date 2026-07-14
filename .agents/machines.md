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

## Live-server E2E (`npm run e2e:live`)

Recorded 2026-07-14. Owner-approved access (2026-07-14) to the boxes below.

- **Why it exists:** the owner's manual playtest found three defects in two sessions
  that 18 mock scenarios and 24 rounds of two-reviewer review all missed (the failure
  text, a leaked request url, an emptied library). They needed a real server.
- **Where it runs:** the same Linux VM as the hermetic suite — it is the ONLY host that
  can drive the app, because `tauri-driver` has no macOS support. Playwright cannot do
  it either: Vela is a Tauri app and everything goes through `invoke()`, which does not
  exist in a browser.
- **Jellyfin (COVERED):** the real server runs as `Jellyfin.app` on the Mac (launchd,
  `open -a Jellyfin` / kill the pid). The VM reaches it at `192.168.64.1:8096` — NOT
  `localhost`, which is the VM itself. The suite does not stop it: a TCP proxy in the
  runner forwards to it and killing the proxy is an instant, deterministic "server went
  away". Never touch the owner's running server to make a test fail.
- **Plex (NOT COVERED, and cannot be from here):** `michael@altiera` (10.1.10.59:32400,
  Arch, `plexmediaserver.service`). The VM can reach it by IP; there is no DNS for the
  name. But it CANNOT be scripted offline: `sudo` needs a password on both boxes, so
  neither a `systemctl stop` nor a VM-side firewall rule is available — and a
  `plex-watchdog.timer` restarts it every 5 minutes, which would race any test that got
  that far. It also cannot be proxied: it is HTTPS behind a `plex.direct` certificate,
  so interposing needs a hosts entry (root). **The Plex-only gaps — `sameSection`, the
  section binding, the rebind path — therefore remain inspection + owner playtest.**
- **Credentials:** extracted from `~/Library/Application Support/com.vela.vela/config.json`
  at run time by `scripts/e2e-live.sh`, written 0600 to the VM's `/tmp`, and deleted on
  exit. Gitignored. Never printed, never logged, never committed.
