# Codereview 2026-11-16 — the embedded player on Windows and macOS

Outcome: two codereview codex hunts, one per slice, each over one landed
commit: the Windows host (`8c40ff8..6b206ec`) and the macOS host
(`4fe60b8..623ec17`). **findings** in both: four candidates, four admitted
(all MEDIUM), none declined. pl-1 and pl-2 are repaired and closed (codex
wrote the repair as a coding task; the working agent's review closed it);
pl-3 and pl-4 are open, their repair queued after the macOS HDR slice,
which works on the same code.

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
| pl-3 | MEDIUM   | macOS: a conditional mpv.conf profile can undo the protected SDR targets | `[ ]`  | queued |
| pl-4 | MEDIUM   | macOS: in a launch that honours DYLD_*, a set dependency can be redirected past the load check | `[ ]` | queued |

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

Admitted. Open: the macOS HDR slice (in progress) makes the targets
change during a play on purpose; the repair (observe the colour targets,
restore the ones the current range requires) goes with it.

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
build without it (a development build). Open: the repair checks every
image dyld loaded against the set's directory after the load (the
reviewer's suggestion), queued with pl-3.
