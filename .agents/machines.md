# Machines

Machine-specific facts (host layout, tool paths, local versions). Portable
truth belongs in `.agents/state.md` / `.agents/repo-guidance.md`; this file is
the only place allowed to know about a particular box.

## macOS dev host (`/Users/michael/Dev/vela`)

- AirPlay investigation baseline recorded 2026-08-20: macOS 27.0 build
  `26A5416b`; the tested system default output identified itself as AirPlay.
  `/opt/homebrew/bin/mpv` is 0.41.0 and reports automatic AO order
  `coreaudio`, `avfoundation`, `null`, `coreaudio_exclusive`, `pcm`. A muted
  direct probe opened CoreAudio on the AirPlay output and reported about
  11.6 ms device latency, not the observed roughly two-second physical
  endpoint delay.
- `/Applications/Plezy.app` is 2.16.0 build 142, matching sibling source tag
  `2.16.0`. That tag pins MPVKit 1.0.23 at
  `ee423a7e8727eeee4748f08cb7d3c00e935ed45e`, built from mpv 0.41.0, and
  configures `ao=coreaudio,avfoundation`. Source inspection found no macOS
  AirPlay detector or automatic audio-delay path; saved Plezy preferences did
  not contain a nonzero audio-sync override. Retained unified logs do not
  reveal which AO the synchronized Plezy play selected, so runtime AO remains
  an explicit diagnostic gap.
- Credential-free AirPlay A/B on 2026-08-20 used a 12-second 1280x720 60 fps
  stereo flash/click fixture. Forced CoreAudio selected `AirPlay (126)`,
  reported `avsync=-5.736 ms` and 11.610 ms device latency, swallowed the first
  click, then remained physically synchronized. Forced AVFoundation reported
  `avsync=-0.142 ms`, swallowed about half of the first click, then remained
  physically synchronized. Both backends therefore passed steady-state stereo
  sync; the difference was initial preroll only. An earlier mono fixture made
  CoreAudio fail with macOS `paramErr (-50)` and was excluded from sync
  comparison.
- Installed Vela 1.0.60's sanitized playback settings at the same diagnostic
  point were `mpv_extra_args=null`, `mpv_use_own_config=false`, and
  `mpv_autocrop=null`. No connection fields or media URLs were read or printed.
- Installed Vela 1.0.60 then reproduced roughly two seconds of physically late
  AirPlay audio on a network MKV with H.264/VideoToolbox video at 23.976025 fps
  and 48 kHz E-AC-3 `5.1(side)` decoded/downmixed to 48 kHz stereo. Live IPC
  reported CoreAudio, automatic device selection, `audio-delay=0`,
  `avsync=0.006 ms`, `video-sync=audio`, and 240 Hz display timing. No URL,
  title, headers, or credentials were queried. The local stereo AAC fixture
  remained synchronized through the same AO, making media/format/input path a
  controlled difference rather than proving a general CoreAudio defect.
- Owner-approved live IPC change of that exact Vela play to
  `ao=avfoundation` succeeded without an explicit reload or relaunch. IPC then
  reported `current-ao=avfoundation` and `avsync=0.359 ms`; the owner observed
  physical sync was correct. The write changed only the live mpv process and
  did not modify Vela or mpv settings.
- The first changed process ended naturally. Restarting the same item launched
  CoreAudio again and reproduced the offset; changing that new process to
  AVFoundation restored sync. With it still playing, AirPlay -> Mac mini
  Speakers kept AVFoundation/automatic-device selection, reported
  `avsync=0.011 ms`, moved sound to built-in output, and remained physically
  synchronized. Mac mini Speakers -> AirPlay kept the same AO/process,
  reported `avsync=0.007 ms`, moved sound back to AirPlay, and remained
  physically synchronized. No listener, polling, delay, or relaunch was used.

- Local Developer ID signing/notarization configured 2026-08-20. The login
  Keychain holds Apple's Developer ID G2 intermediate and `Developer ID
  Application: MICHAEL COELHO (27R2KCAHN7)` (leaf expires 2031-08-08).
  App Store Connect API metadata and the private `.p8` remain canonical in
  `../Certificates/AppStoreConnectCertInfo.csv`; the `.p12`, `.p8`, password
  list, and API metadata file are owner-only (`0600`). Never copy credential
  values into this repo or command output. Use
  `scripts/build-signed-macos.sh` for a fail-closed local signed build; it loads
  the non-secret metadata from that CSV and uses the installed identity. Its
  2026-08-20 real-path proof submitted `f6c85900-9550-4d71-99bf-2485e7f1d165`,
  Apple accepted and stapled it, Gatekeeper accepted the mounted app as
  `Notarized Developer ID`, and both universal architectures were present.
  Resulting DMG SHA-256:
  `7cb333e33fbfbffac749cd7c1b5364f4e0443a2caefbc7e37c06a7ab370a977c`.

- JS and Rust toolchain for this repo, observed 2026-10-24 over
  `ssh michael@nagatha` (macOS 27.0.1 build `26A434`, arm64):
  - Homebrew Node is 26.12.1 with its bundled npm 11.20.0, which is below
    `engines.npm` (`>=12`). Don't change the Homebrew npm. Install a
    scratch-prefix npm instead (`npm install --prefix <dir> --global
    npm@12.0.2`) and put `<dir>/bin` first on `PATH`. With that,
    `node scripts/check-js-toolchain.mjs` passes, and so does every npm
    command.
  - rustup lives in `~/.cargo/bin` (stable 1.99.1, plus 1.89.0 and other
    pinned toolchains). That directory is not on `PATH` for
    `ssh ... 'zsh -l -s'`, so prefix `PATH=$HOME/.cargo/bin:$PATH` before any
    Cargo or Tauri command.
  - The zsh builtin `log` shadows the unified-log tool, so call
    `/usr/bin/log show ...` explicitly.
  - While the console is locked, an SSH session has no screen access:
    `screencapture` fails with "could not create image from display", and
    sshd has no Automation (Apple Events) grant. A GUI app launched over SSH
    does run, but nothing visual can be checked from SSH. Visual acceptance
    needs the owner at the desktop.

Relocated out of `.agents/state.md` 2026-07-14 (drift pass) — state.md stays
portable and may at most point here.

- The owner's Linux VM at `michael@192.168.64.5` is the standing E2E venue
  (Ubuntu 26.04 LTS aarch64, 12 CPU; OS re-verified 2026-07-15; provisioned
  with rustup, tauri-driver, Xvfb, bsdtar, webkit2gtk-4.1-dev, vendored arm64
  WebKitWebDriver, debug binary built). See the Linux VM section below.
- The mac clone has a `vm` remote. **The push policy is ASK, and that includes
  `vm`** (`.agents/push-policy.md`). Corrected 2026-10-24: the Mac's
  `~/Dev/vela` has only `origin` (magneto Gitea) and `github`, with no `vm`
  remote. Reach the VM with the git-bundle-plus-`scp` route (for example
  `.agents/plans/remove-sveltekit.md`, Host constraints). To run E2E without pushing, `scp` the
  changed files into `~/dev/vela` and verify by checksum before running — no
  `git push`, no `git checkout -- .` on the VM's tree (that has silently
  reverted work).
- Reviewer CLIs: `codex exec --sandbox read-only -o <out.json> "$(cat
  <prompt>)" < /dev/null` — **stdin MUST be closed or it hangs** (it has hung
  once) — and `grok --sandbox read-only -p "$(cat <prompt>)"`. **grok has twice
  returned only its preamble with no JSON verdict; that is a FAILED run, not a
  clean pass. Re-dispatch it, and never read silence as agreement.**
- Grok 0.2.112 was probed on 2026-07-27. Its built-in writable profile is named
  `workspace` (not Codex's `workspace-write`). Two no-model `tr-13` dispatches
  failed HTTP 401 after built-in auth recovery, before inference or tools; do
  not retry unless the owner explicitly returns to Grok and first
  reauthenticates with `grok login`.
- Kimi Code CLI is `/Users/michael/.kimi-code/bin/kimi`, version 0.29.2 as of
  2026-07-27. Headless `-p` mode already uses the auto permission policy and
  rejects explicit `--auto` or `--yolo`. Use
  `KIMI_MODEL_THINKING_EFFORT=<effort>` for a launch-scoped effort and
  `-m <alias> --output-format stream-json`; the saved session wire records the
  alias, server model, and effective effort.
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

  **Owner-directed `tr-13` Claude transport (2026-07-27):** Claude Code 2.1.220 was
  called with no model or effort override over exact `735b591..81d5497`. The
  process created `/tmp/vela-tr13-claude-review-20260727`, reached two focused
  Rust guard invocations around its caller-only mutation/restoration, and
  removed the worktree. Its final JSON contained only zero-exit execution
  metadata—no result payload, tool transcript, resolved model, pins, or proof
  booleans. The one allowed re-emission could not resume because the dispatch
  used `--no-session-persistence`. The main worktree remained clean, no review
  verdict counts, and future schema-bound CLI reviews must retain a resumable
  session until their result payload has been validated.

  **Owner-directed `tr-13` final Kimi review (2026-07-27):** after the Claude
  payload failure and Grok HTTP 401, the owner selected Kimi Code CLI 0.29.2,
  alias `kimi-code/k3`, at `max`. A bare launch with incompatible `--auto`
  failed before Kimi started; the real prompt-mode dispatch used its implicit
  auto policy. Session `bcc9531b-d82a-44e1-bce3-d79df9f73ebf` records alias
  `kimi-code/k3`, server model `k3`, and thinking effort `max`. It accepted
  exact `735b591..81d5497` with matching pins, `capability_ok:true`,
  `guard_confirmed:true`, and no comments after a decision-only
  missing-`copyts` red/restored-green proof. The disposable worktree
  `/tmp/vela-tr13-kimi-review-20260727` was clean at `81d5497` and removed.
  This one-off does not change standing Codex routing.

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
  from the ignored `.claude/settings.local.json` <!-- lint: allow (owner ruled leave-it, 2026-08-02: historical cleanup of gitignored Claude local settings; path is per-machine) --> with every pre-existing
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
- Re-observed locally on 2026-10-23. The toolchain has changed since the
  2026-07-20 record, so the native-build claim above no longer holds:
  - Windows Server 2025 Standard, build 26100, PowerShell 7.6.7.
  - **No Visual Studio Build Tools:** no `vswhere`, and no Visual Studio
    install directory. Cargo check, clippy and test, and native packaging,
    cannot link here until the Build Tools are reinstalled. The per-user
    Vela at the path above is still present.
  - Node 26.12.1 is installed host-wide from the official MSI
    (`winget install OpenJS.NodeJS`), with its bundled npm 11.20.0. No
    npm 12 is installed globally, so the toolchain check (`engines.npm`
    `>=12`) fails here. Run npm through `npx -y npm@12.0.2` (CI's baseline)
    instead, or put a scratch-prefix npm 12 first on `PATH` for the check.
  - rustup is installed (`winget install Rustlang.Rustup`) with
    stable-x86_64-pc-windows-msvc only; 1.89.0 is not installed.
  - This clone has `core.autocrlf=true`, from the system gitconfig
    (`C:/Program Files/Git/etc/gitconfig`), with no repo-local override, so it
    checks out CRLF. Since 2026-10-24 the node tests normalize CRLF at read
    time, and `node --test tests/` and `npm run check` pass in this checkout
    directly; no LF worktree is needed. If you still want one, pass
    `-c core.autocrlf=false` on each git command that writes its files.
    Never run `git -C <worktree> config`: worktrees share `.git/config`, so
    that command rewrites the main clone's setting.
  - As of 2026-10-24 this clone's remotes follow `.agents/repo-guidance.md`
    (Remotes & Sync):
    - `origin` is the canonical Gitea, `http://magneto:3001/michael/vela.git`
      (Gitea 28.1.0). It is reachable from here, and the stored Git
      credentials can push to it.
    - `github` is `https://github.com/roethlar/vela`.
    Host `q` and its q:3000 Gitea are retired (owner, 2026-10-24). Use
    `GIT_TERMINAL_PROMPT=0 GCM_INTERACTIVE=never` for unattended Gitea calls.

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

**1.0.58 validation worktree (last verified 2026-07-27):** `~/dev/vela-main`
is a detached worktree of the same clone, used to run E2E against the release
code without disturbing the old tree or its stash. It is clean and aligned to
`81d5497`
(1.0.58); the checksum-verified temporary bundle and both copies were removed.
The `tr-13`-specific `plexdecision` scenario passed, but the required full
39/39 did not: the first run reached 34/39, the second reached 20 pass / 4 fail
before a 38-minute `refresh` WebDriver stall, and a graceful reboot plus
focused runs isolated repeated pre-existing continuation/mpv and `refresh`
harness races. Logs:
`/Users/michael/.ptk/jobs/job-7397-{49,50,51,56,57}.log`. Under the owner's
proportionate-verification ruling, real-Plex `live-transcode` then passed 1/1
on 2026-07-27. The venue was checked for process, listener, credential, and
worktree residue, Plex and `plex-watchdog.timer` were active, and the VM
returned to its prior powered-off state.
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
- Pending (2026-10-23, restated 2026-10-24 for the merged toolchain policy):
  `engines.node` is now `>=26.12.1 <27` (a security floor for the Oct 2026
  Node security release; `engines.npm` stays `>=12`), so
  `scripts/check-js-toolchain.mjs` rejects this VM's Node 26.5.0 until it is
  re-aligned; its npm 12.0.1 is still inside the range. Re-align with the same
  reversible recipe: verify the official arm64 26.12.1 (or any newer 26.x)
  archive against its SHASUMS, extract to `~/.local/opt/node-v26.12.1`,
  repoint only the three `~/.local/bin` symlinks, install npm 12.0.1 (or CI's
  12.0.2 baseline) into that tree with its own Node, and prove `bash -lc`
  reports the new pair and the checker passes. Keep the 26.5.0 directory as
  the rollback. Update the baseline above once done.
  Still pending at 2026-10-24: the VM could not be started (next bullet).
- How the VM is hosted (observed 2026-10-24): it is the UTM 4.7.5 QEMU guest
  named `Ubuntu`, bundle `/Volumes/Apps/VMs/Linux.utm`, app
  `/Volumes/Apps/Applications/UTM.app`. It was suspended, not powered off,
  and 192.168.64.5 refused SSH. It can't be started from an SSH session on
  the Mac:
  - `utmctl` fails with OSStatus -1743 and prints "utmctl does not work from
    SSH sessions or before logging in". sshd has no Apple Events grant.
  - `open 'utm://start?name=Ubuntu'` exits 0, but while the console is locked
    LaunchServices only hands the request to CoreServicesUIAgent, and UTM
    never receives it. No QEMU process starts.

  Starting it needs the owner at the Mac's desktop: unlock the Mac and start
  `Ubuntu` in UTM. Requests queued by `open` may then start it on their own
  (assumption, not observed).

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
