# Codereview 2026-11-16 — the embedded player on Windows and macOS

Outcome: four codereview codex hunts: one per slice over the Windows
host (`8c40ff8..6b206ec`) and the macOS host (`4fe60b8..623ec17`), a
third over the fixes `c49be2a` and `ca07bfa`, and a fourth over the macOS
HDR slice (`ad65c4d..49f2c3c`) (below). **findings** in all four: seven
candidates, seven admitted (all MEDIUM), none declined. All seven are
repaired and closed: codex wrote pl-1 to pl-6's repairs as coding tasks
and the working agent's review closed them (adding the Windows half of
pl-5); the working agent wrote pl-7's, which codex's next hunt over the
fixes reads.

## Dispatch record

- Why: each slice that lands repo code gets a codereview codex defect hunt
  over its pinned range (`.agents/plans/embedded-player.md`, "Status",
  rules for the unattended run), and these two land code that no test runs
  on its platform (Windows and macOS code is type-checked from Linux and
  compiled by CI, never executed before the owner's checks).
- Reviewer: codex / host default / host default / standard, called plainly
  per `.agents/machines.md`: `codex exec --sandbox workspace-write --json
  --output-schema <schema> -o <outfile> <prompt>`, headless, stdin closed.
- Each hunt ran in a detached worktree of its own at the head SHA
  (`~/dev/vela-agent-keep/wt/rv-final-<slice>`), declared disposable; it
  could run the Node and Rust tests and `scripts/check-other-platforms.sh`.
- Mandates (the prompts list what to hunt for): the Windows hunt, the named
  pipe's overlapped I/O, DLL loading, the play's lifecycle without a
  surface, startup and its recovery marker, Settings, and Linux unchanged;
  the macOS hunt, the CAOpenGLLayer's locks and threads against mpv's
  render rules, AppKit on the main thread, placement, the library check,
  the protected SDR targets, and Linux and Windows unchanged.
- Both verdicts are schema-valid, `capability_ok` true, both pins echoed.

## Findings

| ID   | Severity | Finding (one line)                                                      | Status | Commit |
|------|----------|--------------------------------------------------------------------------|--------|--------|
| pl-1 | MEDIUM   | Windows: a user `gpu-context` survives the forced `gpu-api=d3d11`        | `[x]`  | this commit |
| pl-2 | MEDIUM   | Windows: own-config mode skips `<exe dir>/mpv.conf`, which mpv reads     | `[x]`  | this commit |
| pl-3 | MEDIUM   | macOS: a conditional mpv.conf profile can undo the protected SDR targets | `[x]`  | the HDR slice's commit |
| pl-4 | MEDIUM   | macOS: in a launch that honours DYLD_*, a set dependency can be redirected past the load check | `[x]` | `41d47c4` |
| pl-5 | MEDIUM   | Linux: a numeric `--fs-screen`/`--screen` from the user outranks the remembered name | `[x]` | `9833fc1` |
| pl-6 | MEDIUM   | Windows: an explicitly empty MPV_HOME is read as unset, so config files are read mpv would not read | `[x]` | `e93402e` |
| pl-7 | MEDIUM   | macOS: a draw begun under a colour hold can flush the old range's frame if the hold ends mid-render | `[x]` | `e1cf5f2` |

### pl-1 (whost)

Reviewer, verbatim, evidence: "src-tauri/src/player/launch.rs:565 forces
gpu-api=d3d11 but preserves the user's gpu-context. With Advanced options
--vo=gpu, --gpu-api=opengl and --gpu-context=win, switching to embedded
produces gpu-next/d3d11/win. In mpv 0.42, win is an OpenGL context, and the
context selector rejects this combination." Predicted failure: "Embedded
playback has no video output: audio can continue without a picture, or
playback ends. The successful loadfile command has already satisfied Vela's
startup check, so external fallback is not triggered."

Admitted. Repair: `gpu-context=d3d11` joins the protected tail beside
`gpu-api=d3d11` (both names defined in mpv 0.42.0's
`video/out/d3d11/context.c`; incompatible pairs filtered in
`video/out/gpu/context.c`). Tests seen failing with the rule taken out.

### pl-2 (whost)

Reviewer, verbatim, evidence: "src-tauri/src/player/embedded/options.rs:187
constructs the Windows configuration list from <exe>/mpv/mpv.conf and
RoamingAppData/mpv/mpv.conf, omitting <exe>/mpv.conf. mpv 0.42 also reads
that omitted location." Predicted failure: "Settings in the omitted file
are ignored by embedded playback."

Admitted after the working agent checked mpv 0.42.0's `options/path.c`
(`config_dirs`: home, old_home, osxbundle, exe_dir, global) and
`osdep/path-win.c` (`exe_dir` is returned in portable mode too; `global` is
`<exe dir>/mpv` only when not portable). Repair: a pure function, tested on
every platform, lists in mpv's reading order (lowest priority first)
`<exe dir>/mpv/mpv.conf`, `<exe dir>/mpv.conf`, AppData `mpv/mpv.conf`;
portable: `<exe dir>/mpv.conf`, `portable_config/mpv.conf`; a non-empty
`MPV_HOME` alone otherwise. Tests seen failing with each rule taken out.

### pl-3 (mhost)

Reviewer, verbatim, evidence: "macos_options.rs:12 sets the targets only
during configuration; macos_session.rs:52 neither observes nor repairs
subsequent changes. With an mpv.conf profile conditioned on p.pause ==
true, the pinned mpv 0.42 library changed the protected values from
srgb/bt.709/203 to pq/bt.2020/1000 after pausing. macos_gl.rs:487 keeps the
layer tagged sRGB." Predicted failure: "A runtime HDR profile produces
PQ/BT.2020 pixels interpreted as sRGB, giving visibly incorrect brightness
and colours instead of the promised SDR output." Suggested: "Observe and
restore the protected colour properties during playback, using
asynchronous updates and withholding mismatched frames until the targets
agree with the layer."

Admitted. Closed by the macOS HDR slice, which changes the targets during
a play on purpose: the five targets are observed for the whole play and
restored when anything rewrites them (a switch's black hold, asynchronous
sets, fresh reports of every value before the layer follows); five
restorations within ten seconds stop the play rather than flash forever.

### pl-4 (mhost)

Reviewer, verbatim, evidence: "src-tauri/src/player/embedded/libmpv.rs:325
uses ordinary dlopen, while :451 checks only the image exporting
mpv_create. In a non-hardened macOS launch that honors DYLD_LIBRARY_PATH, a
directory containing a compatible same-named FFmpeg dependency but no
libmpv can redirect that dependency while the bundled mpv_create still
passes the check." Predicted failure: "Vela accepts embedded playback
using libraries outside its shipped set."

Admitted, with this context: a released Vela.app is notarized with the
hardened runtime, which ignores DYLD_* variables, so the exposure is a
build without it (a development build). Repair (`41d47c4`): after the
load, every image dyld has loaded whose file name is one of the set
directory's dylibs must resolve, canonically, inside that directory, or
the library is unloaded and refused (a pure function tested on every
platform; the macOS part only gathers the paths). Tests seen failing with
the rule taken out.

## A third hunt: the fixes of `c49be2a` and `ca07bfa`

Dispatched by the working agent as each slice's hunt is (the plan's rule),
over `623ec17..ca07bfa` with the mandate limited to the two product commits
in that range: the Linux fullscreen screen (`c49be2a`) and the Windows
host's review repairs (`ca07bfa`). Same harness and reviewer as above.
**findings**: two candidates, both admitted (MEDIUM), as pl-5 and pl-6.
One codex task repaired pl-4, pl-5 and pl-6; each landed in a commit of
its own.

### pl-5 (fixes2)

Reviewer, verbatim, evidence: "src-tauri/src/player/launch.rs:992-994
appends fs-screen-name but preserves an Advanced option such as
--fs-screen=0. A Rust probe confirmed both survive. In mpv 0.42, Wayland's
find_output and X11's vo_x11_select_screen prioritize the numeric selector
regardless of argument order." Predicted failure: "If the remembered
screen is not screen 0, playback still opens fullscreen on screen 0. The
reader's subsequent fs-screen-name updates cannot override that numeric
selection either."

Admitted; it holds for `--screen` against `--screen-name` too, on macOS
as well (`getTargetScreen`), and on Windows for a fullscreen start (mpv
puts a fullscreen window on `fs-screen`'s monitor before its own,
`w32_common.c` `get_default_monitor`; the working agent's review found
that half). Repair (`9833fc1`): after the user's options,
`--screen=default` beside `--screen-name` (Linux, macOS), and for a
fullscreen start `--fs-screen=current` on every platform, with Linux's
`--fs-screen-name`. A windowed start leaves the user's `--fs-screen` for
later toggles. Tests seen failing with each rule taken out.

### pl-6 (fixes2)

Reviewer, verbatim, evidence: "src-tauri/src/player/embedded/options.rs:161
treats empty MPV_HOME as unset, allowing line 169 to add <exe
dir>/mpv.conf. mpv 0.42 preserves an empty environment value and disables
config discovery through options/path.c." Predicted failure: "With
own-config mode enabled, MPV_HOME empty, and only <exe dir>/mpv.conf
present, this commit newly applies that file's settings or profiles
although mpv would read no config."

Admitted. Repair (`e93402e`): unset, discovery; set and empty, no files;
set and non-empty, that directory's mpv.conf only, on every platform
(which also drops `/etc/mpv/mpv.conf` beside a non-empty `MPV_HOME` on
Linux and macOS: mpv reads only the first config directory then). Tests
seen failing with each rule taken out.

## A fourth hunt: the macOS HDR slice (`49f2c3c`)

Dispatched by the working agent as each slice's hunt is (the plan's rule),
over `ad65c4d..49f2c3c`, one commit: HDR in the macOS embedded player
through the layer's extended dynamic range. Same harness and reviewer as
above; the mandate named the decision, the switch read as three threads
(the play thread, Core Animation's drawing thread, the main thread), the
surface, and logs. **findings**: one candidate, admitted (MEDIUM), as
pl-7.

### pl-7 (mhdr)

Reviewer, verbatim, evidence: "macos_gl.rs:479-482 reads `held` only after
`draw_into` returns, while macos_host.rs:289-295 changes the layer and
calls `release`, which clears `held` without synchronizing with that draw
(macos_gl.rs:385-392). After the initial black acknowledgement, another
draw can snapshot the old or partially updated targets; the remaining
target confirmations and main-thread release can then finish before that
draw returns. mpv 0.42 snapshots renderer options before rendering
outside its core-facing mutex, so property confirmations do not exclude
this overlap. A headless harness using the production DrawingState
reproduced an old-encoding frame escaping; retaining the entry hold kept
that frame black." Predicted failure: "An overlapping HDR/SDR or gamut
switch can show a frame encoded for the previous targets under the new
layer colour space, producing a visible brightness or colour flash. This
is an intermittent rendering defect."

Admitted. Repair (`e1cf5f2`, written by the working agent): the hold is
read before mpv renders as well as after, and a draw that began or ended
under one is cleared to black (`draws_black`, `macos_hdr.rs`); the
release's redraw shows the picture with the new targets. The rule's unit
test and a guard of its wiring each seen failing with their part taken
out.
