# Machines

Machine-specific facts (host layout, tool paths, local versions). Portable
truth belongs in `.agents/state.md` / `.agents/repo-guidance.md`; this file is
the only place allowed to know about a particular box.

## macOS dev host (`/Users/michael/Dev/vela`)

Relocated out of `.agents/state.md` 2026-07-14 (drift pass) — state.md stays
portable and may at most point here.

- The owner's Linux VM at `michael@192.168.64.5` is the standing E2E venue
  (Ubuntu 26.04 LTS aarch64, 12 CPU; OS re-verified 2026-07-15; provisioned
  with rustup, tauri-driver, Xvfb, bsdtar, webkit2gtk-4.1-dev, vendored arm64
  WebKitWebDriver, debug binary built). See the Linux VM section below.
- The mac clone has a `vm` remote. **The push policy is ASK, and that includes
  `vm`** (`.agents/push-policy.md`). To run E2E without pushing, `scp` the
  changed files into `~/dev/vela` and verify by checksum before running — no
  `git push`, no `git checkout -- .` on the VM's tree (that has silently
  reverted work).
- Reviewer CLIs: `codex exec --sandbox read-only -o <out.json> "$(cat
  <prompt>)" < /dev/null` — **stdin MUST be closed or it hangs** (it has hung
  once) — and `grok --sandbox read-only -p "$(cat <prompt>)"`. **grok has twice
  returned only its preamble with no JSON verdict; that is a FAILED run, not a
  clean pass. Re-dispatch it, and never read silence as agreement.**
- Reviewer MCP (verified 2026-07-18): the Claude Code server supports direct
  Workflow dispatch and records model, effort, version, and MCP entrypoint in
  its transcript. The owner-confirmed standard `codereview` pair lives in the
  gitignored `.agents/review/harnesses.local.json`; frontier and `openreview`
  mappings remain unconfirmed and therefore fail closed.
- Reviewer MCP execution blocker (proved 2026-07-18 on `wsp-1`, Claude Code
  2.1.214): the owner directed launching the server with `--allowedTools`.
  The registration now uses
  `--allowedTools=Read,Glob,Grep,Edit,Bash(*) mcp serve`, and the server's
  direct Bash tool ran the focused Node guard 5/5 without an approval prompt.
  A Workflow-launched reviewer is a second Claude process: it receives neither
  the ptk MCP tool nor the server's individual allowed-tool grants, so the same
  `node --test` call still reaches an unanswerable interactive approval gate.
  Server-level `bypassPermissions` also did not propagate; Workflow did not
  expose command-line or user custom agents with their own permission mode;
  and the MCP `Agent` endpoint reported no available agent type. Opus can read
  and statically review the exact head but cannot produce
  `guard_confirmed:true` through this MCP surface.
- Reviewer CLI fallback (verified 2026-07-18 on `wsp-1`, Claude Code 2.1.214):
  `claude --allowedTools=Read,Glob,Grep,Edit,Bash(*) --model=opus
  --effort=high --output-format=stream-json --verbose --json-schema=<schema>
  --no-session-persistence -p <prompt>` ran headlessly in an exact detached
  worktree. It resolved `claude-opus-4-8`, exposed the complete tool transcript,
  independently produced the required production red/restored-green proof,
  returned a schema-valid verdict, and left the worktree clean. The owner
  authorized this transport for `wsp-1` only; it is not a standing replacement
  for MCP.
- Temporary `chr-1` reviewer artifacts (2026-07-18): cleanup COMPLETE
  2026-07-19 with explicit owner approval. The four session-added allow
  entries (`Bash`, `Bash(*)`, `Bash(node *)`, `Bash(perl *)`) were removed
  from the ignored `.claude/settings.local.json` with every pre-existing
  permission preserved; `/tmp/claude-mcp-review.mjs` was already deleted; the
  `.claude/worktrees/chr-1-opus-review` worktree was confirmed clean at
  `fe8eebe` and removed during branch cleanup (`git worktree list` shows only
  main). No Claude review process is running.
- Multi-Plex worktree (2026-07-19): `.claude/worktrees/multi-plex`, review
  branch `fix/mpx-1-multi-plex`, landed on `main` at `3a1dd8b` by explicit
  owner direction (the implementation branch
  `worktree-multi-plex` remains at pre-review head `c24c132`), was recreated
  from current `main` at `34ad47c` after the prior worktree was found absent.
  Three detached reviewer worktrees remain after the review and are clean:
  `mpx-1-fable-r1` at `b90002a`, `mpx-1-fable-exact-r1` at `72628de`, and
  `mpx-1-fable-cli-r1` at accepted head `c32a59b`. They were not removed
  because cleanup is destructive and has not been separately authorized.

## Windows dev host (`F:\dev\vela`)

Relocated out of `.agents/state.md` 2026-07-14 (drift pass). Recorded
2026-07-09; **not re-verified since — treat as possibly stale.**

- The `ptk` MCP server (warm PowerShell runspace, `ptk_invoke`) is the DIRECT
  shell for agent harnesses on this host. Probe it before assuming there is no
  shell or delegating shell work to subagents (2026-07-09 lesson: an entire
  session ran shell through subagent indirection with ptk available the whole
  time).
- cargo/rustc need valid stdin: `cmd /c "cargo ... < nul"` (rustup shim quirk).
- codex lives at `%APPDATA%\npm\codex.cmd`; headless via
  `codex exec --json --sandbox read-only` with the prompt on stdin.
- Unix-cfg-gated cargo tests are excluded here — **Linux CI is authoritative;
  do not record the local test count, it rots.**
- clippy baseline = 4 pre-existing cfg-dead mpv-installer warnings
  (post-removal; was 13).
- The E2E harness does NOT run here (needs Linux WebKitWebDriver).
- Checkout is `autocrlf=true`: empty-diff "modified" files are line-ending
  noise.

## Windows validation host (`netwatch-01`)

Recorded and verified 2026-07-20 through `ssh michael@netwatch-01`.

- Windows x64 build 26200, PowerShell 7.6.3, Visual Studio Build Tools 18.7.3,
  and the Windows SDK can compile, test, and package Vela natively. The Linux
  WebKitDriver E2E harness remains Linux-only.
- The host-wide Node 24/npm 11 pair was left unchanged. Playback-policy
  validation used checksum-verified Node 26.5.0, npm 12.0.1, and Rust 1.89 in
  task-isolated temporary locations, then the host's stable Rust for rolling
  checks.
- With owner approval, the checksum-verified unsigned 1.0.0 NSIS package
  replaced Vela 0.1.62 in place on 2026-07-20. The current per-user
  installation is `C:\Users\michael\AppData\Local\Vela\Vela.exe`; the
  installer exited successfully, and both file metadata and the uninstall
  registration report 1.0.0. Vela was not running before the silent upgrade,
  so no process needed to be stopped or relaunched.

## Linux VM (E2E host)

Recorded 2026-07-13; OS/toolchain re-verified 2026-07-15.

- Current observed baseline after the owner-approved Slice 1 alignment
  (2026-07-15): Ubuntu 26.04 LTS; user-local Node 26.5.0/npm 12.0.1 at
  `~/.local/bin`; Rust 1.97.0 stable; WebKitGTK 2.52.3; mpv 0.41.0; FFmpeg
  8.0.1; tauri-driver 2.0.6. The crates.io registry and upstream release log
  still identified tauri-driver 2.0.6 (published 2026-05-06) as current on
  2026-07-15. Node came from the checksum-verified official
  arm64 archive under `~/.local/opt/node-v26.5.0`; npm's registry integrity was
  pinned and verified. Ubuntu's `/usr/bin/node` 22.22.1 and `/usr/bin/npm`
  9.2.0 packages remain installed and unchanged. Removing only the three
  user-local `node`/`npm`/`npx` symlinks and that versioned directory rolls the
  alignment back. `bash -lc` resolves the user-local pair, matching the E2E
  launcher; a clean install and the real-app `smoke` scenario passed.

- The fetched E2E driver now uses Ubuntu's SHA-pinned
  `webkitgtk-webdriver` 2.52.3 package, exactly matching this VM's WebKitGTK;
  the previous Debian 2.50.6/ICU72 cache is invalidated by package identity.
  The ARM64 package passed an isolated Vela session/IPC/UI/screenshot probe and
  the full suite. The AMD64 package URL, checksum, and payload were inspected,
  but no current AMD64 runtime venue was available.

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
- **Plex (COVERED as of 2026-07-14):** `michael@altiera` (10.1.10.59:32400, Arch,
  `plexmediaserver.service`). The VM reaches it over its `plex.direct` HTTPS name; there
  is no DNS for the short name, so use the IP or the plex.direct host.
  - It CANNOT be proxied (HTTPS behind a `plex.direct` certificate), so a live test stops
    the REAL service. The owner installed a NOPASSWD sudoers rule scoped to FOUR literal
    commands — start/stop `plexmediaserver.service` and `plex-watchdog.timer`, nothing
    else (`/etc/sudoers.d/vela-e2e`; remove with `sudo rm` to revoke).
  - **`plex-watchdog.timer` restarts Plex every 5 minutes.** It must be stopped for the
    window and restored after, or a test is racing a robot.
  - **Plex is restored on EVERY exit path** — scenario cleanup, the control server's
    signal handlers, and the launcher's trap. A crashed test must never leave the owner's
    server down. If you ever see `FAILED TO RESTORE PLEX`, start it by hand.
  - The VM was deliberately NOT given an SSH key on the Plex box: that is persistent
    access to the owner's server, granted for a test. The Mac (which already has access)
    runs `scripts/live-control.mjs` for the length of one run — host-only address,
    ephemeral port, per-run secret in the path, two argument-less verbs.
- **STILL NOT COVERED, anywhere:** a Plex REBIND. It needs a SECOND Plex server, which
  does not exist here — so `sameSection` and the section-binding comparison remain
  inspection-only. `live-plex` now exercises real XML browse/movie detail/show-season-
  episode detail, direct HTTPS paused playback, successful watched/unwatched readback,
  real section keys/provenance/scan, and the stopped-server edit/restart path. Its one
  clean watch fixture is restored directly on normal failure and handled signals; the
  Mac control process independently restores the Plex service and watchdog.
- **Credentials:** extracted from `~/Library/Application Support/com.vela.vela/config.json`
  at run time by `scripts/e2e-live.sh`, written 0600 to the VM's `/tmp`, and deleted on
  exit. Gitignored. Never printed, never logged, never committed.
