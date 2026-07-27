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
  its transcript. Tier pairs live in the gitignored
  `.agents/review/harnesses.local.json`.

  **Reviews of this repo go to `codex`, never to a Claude harness** (decision
  `.agents/decisions.md` 2026-07-26): the working agent here IS Claude, so a
  Claude reviewer is not independent. The cache's owner-confirmed `claude` tiers
  (`claude-fable-5` @ xhigh / @ max, `competitive`) are therefore NOT selectable
  for review work on this repository, whatever the cache says.

  **Owner-directed `tr-12` exception (2026-07-26):** the owner explicitly
  required a one-off finding review by Claude with no model or effort override.
  Claude Code 2.1.220 resolved `claude-opus-5[1m]`; its transcript exposed no
  effort field. The repo hook redirected shell calls to PTK, and the reviewer
  used it to complete an independent HTTP-status red/restored-green proof in a
  disposable worktree. It accepted exact `a7d792e..9cde6b2` with
  `capability_ok:true`, `guard_confirmed:true`, and no comments. The worktree
  was removed. This one-off does not change the standing Codex routing above.

  **Codex is called plainly — no `--model`, no reasoning-effort override, and
  no model/effort pair recorded for it.** The whole dispatch, probed and used
  2026-07-26 against codex-cli 0.145.0 (cache records 0.142.5 — a note, not a
  gate):

  ```
  codex exec --sandbox workspace-write \
    --output-schema <schema.json> -o <outfile> "<prompt>"
  ```

  `workspace-write` is required so the reviewer can drive its own `git
  worktree`; `--output-schema` plus `-o` carry the structured verdict. Nothing
  else is passed. Do not add `--model` or `-c model_reasoning_effort=...`, and
  do not ask the owner to name either.
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

**NOT BROKEN. Diagnosed and cleared 2026-07-25.** The venue works; the binary
being run did not.

`--skip-build` reuses `src-tauri/target/debug/vela` **whatever produced it**, and
what had produced it was a plain `cargo build`. `run.mjs` says why that matters
in its own comment: "tauri build (unlike plain cargo build) embeds the built
frontend, so no dev server is involved." A `cargo build` binary has no embedded
frontend, so it falls back to `tauri.conf.json`'s `devUrl`
(`http://localhost:1420`) and waits for a Vite dev server that no E2E run
starts.

The evidence, from a throwaway scenario that asked the page for its state
instead of asserting one:

```
readyState: "complete", hasTauri: true, headings: [], href: "about:blank",
bodyHead: "Could not connect to localhost: Connection refused"
```

So WebKit, Xvfb, the driver stack and the Tauri IPC bridge were all healthy the
whole time — the webview had simply loaded a connection-refused error page.
`npm run e2e -- smoke` (no `--skip-build`) rebuilds with `tauri build --debug`
and PASSES.

**The lesson worth keeping: `cargo build` on this host produces a binary that
cannot pass E2E.** Never hand-build the app here and then run with
`--skip-build`; let the harness build, or run `npm run tauri -- build --debug
--no-bundle` yourself.

The prior investigation's five "not X" bullets were all correct and all beside
the point; one of them — "`cargo build` succeeds" — was in fact the cause,
recorded as evidence of health. Keeping that here as the caution it is: a
diagnostic that only rules things out never has to name the thing it ruled in.
`xdotool` was installed by the owner on 2026-07-25, but the marker pointer leg
is dead for a different reason (below).

**This venue never runs mpv with a real video output** (owner, 2026-07-25).
Scenarios drive mpv with `--vo=null`, so mpv publishes no `osd-dimensions` and
nothing that renders on the video surface can be asserted here — the marker skip
button, its hitbox, a pointer click on it, and its temporary Space binding are
all untestable in this suite at any effort. Behaviour that needs a real video
output is verified against real mpv on a desktop host instead; do not write a
scenario here that depends on an OSD overlay being drawn.

**Current-tree worktree (updated 2026-07-26):** `~/dev/vela-main` is a detached
worktree of the same clone, used to run E2E against current `main` without
disturbing the old tree or its stash. It is clean and aligned to `9cde6b2`
(1.0.57), re-verified after focused E2E 1/1, full E2E 39/39, and real-Plex
`live-transcode` 1/1 on 2026-07-26. The temporary bundle/patch were removed and
the VM returned to its prior powered-off state.
Refresh it with `git fetch origin && git checkout --detach <sha>` from inside
it. The original `~/dev/vela` remains at its old commit and is NOT the venue any
more.

The VM also has a `stash@{0}` (`codex-linux-validation-1a2bef5`) and its clone
sits at `95312fc` on `main`; the 2026-07-25 sync overwrote working-tree files
only after confirming every one of them matched a blob already in the mac
repo, so nothing original was lost.

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
  - **The rule is PRESENT as of 2026-07-26.** A prior check wrongly used
    `systemctl is-active`, which was never one of the four allowlisted commands
    and therefore could not prove the rule absent. A read-only `sudo -n -l`
    check against each exact start/stop command confirms all four remain
    allowed; no service command was executed during that verification.
  - **`plex-watchdog.timer` restarts Plex every 5 minutes.** It must be stopped for the
    window and restored after, or a test is racing a robot.
  - **Plex is restored on EVERY exit path** — scenario cleanup, the control server's
    signal handlers, and the launcher's trap. A crashed test must never leave the owner's
    server down. If you ever see `FAILED TO RESTORE PLEX`, start it by hand.
  - **Plex HLS header auth is live-proven (2026-07-26).** A one-off session
    requested its master playlist, child playlist, and first segment with the
    token only in `X-Plex-Token` headers: 200, 200, and 206 respectively. Neither
    generated child URI contained a token. Teardown returned 204 and a follow-up
    session-list check found none of the probe sessions; the independently active
    session already on the server was left untouched.
  - **`live-transcode` passes at 1.0.56 (2026-07-26).** Run it as
    `npm run e2e:live live-transcode`. At exact `ca15258`, candidate decisions
    created no server session; explicit-tier playback gave mpv a credential-free
    universal-transcode path and argv plus one exact token header in a regular
  0600 include; the play opened a fresh real session; and quitting mpv removed
  it. Post-run Plex/watchdog were active, the credentials file was absent, and
  there was no mpv process or live mpv socket listener. This closes `tr-10`.
  - **`live-transcode` also passes at 1.0.57 (2026-07-26).** At exact
    `9cde6b2`, candidate decision probes again created no fresh session;
    explicit-tier playback opened one fresh session and teardown removed it.
    Post-run Plex/watchdog were active, the credentials file was absent, and
    there was no mpv process or live mpv socket listener. This is the positive
    live regression gate for `tr-12`.
  - Historical E2E runs leave inert `mpv-*.sock` filesystem nodes under old
    owner-only `/tmp/vela-*` directories. Do not treat their presence as a live
    player and do not broad-delete `/tmp`; use `pgrep -x mpv` plus `ss -xl` to
    distinguish a live process/listener from an inert node.
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
- **Credentials:** extracted from
  `~/Library/Application Support/com.vela.vela/connections.json` at run time by
  `scripts/e2e-live.sh` (with legacy pre-split `config.json` fallback), written
  0600 to the VM's `/tmp`, and deleted on exit. Gitignored. Never printed,
  never logged, never committed.
