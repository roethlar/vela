# Codereview 2026-11-16 — the embedded player on Windows and macOS

Outcome: six codereview codex hunts: one per slice over the Windows host
(`8c40ff8..6b206ec`) and the macOS host (`4fe60b8..623ec17`), a third
over the fixes `c49be2a` and `ca07bfa`, a fourth over the macOS HDR slice
(`ad65c4d..49f2c3c`), a fifth over the macOS packaging and a sixth over
the Windows packaging (below). **findings**: thirteen candidates,
thirteen admitted (one HIGH, nine MEDIUM, three LOW), none declined.
All but pl-9 are repaired and closed: codex wrote pl-1 to pl-6's, pl-8's,
pl-11's and pl-12's repairs as coding tasks and the working agent's review
closed them (adding the Windows half of pl-5); the working agent wrote
pl-7's, pl-10's and pl-13's, which codex's next hunt over the fixes reads.
pl-9 is about tests alone and waits for the release's test pass
(`.agents/decisions.md`, 2026-11-14). pl-11 leaves one path open on
purpose, stated in its section.

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
| pl-8 | MEDIUM   | release: on a tag, the macOS DMG reaches the draft release before its checks run | `[x]` | `9dc0b0e` |
| pl-9 | LOW      | tests: no guard holds the mpv patch's `utils-mac.c` hunk | deferred | the release's test pass |
| pl-10 | LOW     | docs: the macOS audit and its CI job described as they were before the fixes | `[x]` | `e740bf4` |
| pl-11 | HIGH    | Windows: a later bare-name DLL load (libplacebo's d3d11.dll) can resolve beside Vela.exe | `[x]` | `4d511a2` |
| pl-12 | MEDIUM  | Windows set: the dependency checks miss delay imports and forwarders; the audit passes a foreign DLL | `[x]` | `2e3913d` |
| pl-13 | LOW     | docs: the embedded player described as Linux-only, Windows and macOS packages as without the library | `[x]` | `34fd3a3` |
| pl-14 | MEDIUM  | Windows set: the runtime audit trusts any module under %SystemRoot%, a third-party runtime included | `[ ]` | in repair (codex) |
| pl-15 | MEDIUM  | Windows set: the PE scanner refuses a valid DLL whose export directory spans sections | `[ ]` | in repair (codex) |
| pl-16 | MEDIUM  | macOS: DYLD_IMAGE_SUFFIX can load a renamed outside dependency past pl-4's check | `[ ]` | in repair (codex) |
| pl-17 | MEDIUM  | macOS: an image unloaded during pl-4's scan makes a cached, spurious refusal | `[ ]` | in repair (codex) |
| pl-18 | MEDIUM  | screens: Vela's own fallback screen overrides the user's numbered screen like a remembered one | `[ ]` | in repair (codex) |
| pl-19 | MEDIUM  | macOS: a conditional profile can undo video-timing-offset=0, so video runs ahead of audio | `[ ]` | in repair (codex) |

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

## A fifth hunt: the macOS packaging

Dispatched by the working agent over `9ec9991..bffc3a4`, limited to six
commits: the macOS set's build (`ba677b7`), its fixes (`c60a6d3`,
`485628c`, `ad65c4d`), Vela.app carrying the set (`2181dcb`) and the CI
job's hold removed (`bffc3a4`). Same harness and reviewer as above.
**findings**: three candidates, three admitted, as pl-8 (MEDIUM), pl-9
and pl-10 (LOW).

### pl-8 (mpkg)

Reviewer, verbatim, evidence: ".github/workflows/release.yml:383 supplies
tagName to tauri-action on tagged macOS builds, causing attachment during
the build step. The new packaged-libmpv check runs afterward, at line
432." Predicted failure: "If the packaged set fails its signature,
provenance, inventory or playback check, the rejected DMG remains
attached to the draft release. Failing the later job does not retract
that asset." Suggested: "Build without attaching through tauri-action.
Create the draft and upload the macOS DMG only after its checks pass,
alongside the other checked packages."

Admitted: the Linux and Windows legs already attach nothing and leave it
to the `inventory` job; the macOS leg was left creating the draft. Repair
(`9dc0b0e`, codex): no leg of `bundle` gives tauri-action a tag; on a tag
push the `inventory` job requires one DMG, reuses or creates the draft,
refuses a published release, and uploads the checked DMG with the rest.
The `.app.tar.gz` that tauri-action used to add goes (the same app,
undocumented, never in SHA256SUMS). Guards seen failing with each of
eleven injected regressions.

### pl-9 (mpkg)

Reviewer, verbatim, evidence: "tests/dependency-policy.test.mjs:1190-1199
checks only the patch's target filename and SDK linker-path hunk.
Removing packaging/libmpv/macos/mpv-sdk-only.patch:14-19 left all 559
dependency-policy and libmpv-packaging tests passing in a local
regression probe." Predicted failure: "The portable checks accept removal
of the fix for the documented _cfstr_from_cstr load failure. With Cocoa
disabled, the Apple audio outputs again lack their utility definitions;
only the later macOS execution check detects it."

Admitted, about tests alone: under the owner's decision of 2026-11-14 it
is not repaired now and is written here for the release's test pass,
which should guard the patch's second hunk and its `cocoa` condition so
that removing either fails. Until then the macOS check (CI's
`libmpv-macos`, loading the set on a hosted Mac) is what catches it, as
it did on 2026-11-16.

### pl-10 (mpkg)

Reviewer, verbatim, evidence: "docs/development.md:177-178 and
packaging/libmpv/macos/NOTES.txt:70-74 say Xcode's Python libraries fail
the audit and no interpreter exemption exists. check-libmpv-macos.py:65-66
explicitly exempts sys.base_prefix. The guide at lines 185-187 also
describes CI as manual-only, although bffc3a4 removed that condition."
Predicted failure: "The development guide and shipped NOTES describe a
stricter audit than actually runs, and incorrectly tell maintainers that
pushes do not run the macOS set job."

Admitted. Repair (`e740bf4`, written by the working agent): both
documents state the one exemption and the job on every push; the guide
also names the AVFoundation audio output the set gained in `ad65c4d`.

## A sixth hunt: the Windows packaging

Dispatched by the working agent over `e54b302..4fe60b8`, limited to three
commits: the Windows set's cross build (`3dad7a8`), its container on
GitHub's runner (`8c40ff8`) and the installers carrying the set
(`4fe60b8`). Same harness and reviewer as above. **findings**: three
candidates, three admitted, as pl-11 (HIGH), pl-12 (MEDIUM) and pl-13
(LOW).

### pl-11 (wpkg)

Reviewer, verbatim, evidence: "scripts/build-libmpv-windows.sh:322 enables
libplacebo's D3D11 backend, whose pinned source later calls bare
LoadLibraryW for d3d11.dll and dxgi.dll.
src-tauri/src/player/embedded/libmpv.rs:342 restricts only the initial
load; the process-wide restrictions in
packaging/libmpv/check-libmpv-windows.py:202 are absent from Vela. A
headless Wine harness using the actual set and Vela's flags executed a
canary d3d11.dll beside the executable; applying the checker's process
restrictions prevented it." Predicted failure: "An unmanifested DLL beside
Vela.exe can execute inside Vela during embedded playback or break
rendering. The successful installer checks do not detect this because
they use a stricter loading policy and null video output."

Admitted. Repair (`4d511a2`, codex): immediately before the set's load,
the process's default DLL directories become System32 and user
directories (Vela adds none), as the checker's are; a failure refuses
the embedded player. Nothing else in Vela's process needs a bare-name
load from beside Vela.exe (WebView2's loader is linked statically).

Left open on purpose (the working agent's judgment, for the owner to
overrule): FFmpeg's own Windows loader (`compat/w32dlfcn.h`, lines 134-144
in 9.0.2) passes `LOAD_LIBRARY_SEARCH_APPLICATION_DIR |
LOAD_LIBRARY_SEARCH_SYSTEM32` explicitly, so with hardware decoding its
d3d11va and dxva2 paths still look beside Vela.exe first; moving the set
would not change that. Only something able to write into Vela's
installation directory can use it, and that can replace Vela.exe itself.
Closing it would take a patch to FFmpeg's loader in the Windows set.

### pl-12 (wpkg)

Reviewer, verbatim, evidence: "scripts/build-libmpv-windows.sh:357
extracts only objdump's 'DLL Name:' lines. Real PE fixtures containing
either a delayed libwinpthread-1.dll import or an export forwarded to
libwinpthread-1.pthread_create both produced an empty dependency list.
packaging/libmpv/check-libmpv-windows.py:139 also accepts an unlisted
libwinpthread-1.dll loaded outside the set. Replacing needed() with an
empty implementation still passed all 186 selected Windows policy tests."
Predicted failure: "The checks can certify a set with an unshipped runtime
dependency. Playback that subsequently reaches the affected function can
fail to resolve it or load a foreign DLL; the null-output playback check
need not exercise that function."

Admitted (the checks are release gates, not tests alone). Repair
(`2e3913d`, codex): the build reads each DLL with a standard-library PE
scanner (`packaging/libmpv/windows-pe.py`: ordinary imports, delay imports,
forwarded exports) and checks the closure over all three; the runtime
audit reports a module loaded after its baseline outside both the set and
Windows' own directories. The set CI built has no delay imports or
forwarders, and passes. Eighteen fixture tests, seen failing across 22
mutations.

### pl-13 (wpkg)

Reviewer, verbatim, evidence: "RELEASE_NOTES.md:142 still states that
Windows packages do not carry the library, contradicting the Windows
overlay and release workflow introduced by 4fe60b8. README.md:118 and
docs/development.md:227 also describe embedded playback as
Linux-on-Wayland-only." Predicted failure: "Users receive incorrect
information about the Windows installer's contents and the availability
of embedded playback."

Admitted. Repair (`34fd3a3`, written by the working agent): the release
notes, README, `docs/usage.md` and the development guide describe the
embedded player on all three platforms, per platform where they differ,
and every package carrying the library.

## A seventh hunt: the packaging and release fixes

Dispatched by the working agent over `ca07bfa..3061b52`, limited to the
fixes of pl-8 (`9dc0b0e`), pl-10 (`e740bf4`), pl-12 (`2e3913d`), pl-13
(`34fd3a3`) and the CI job's hold removed (`bffc3a4`). Same harness and
reviewer as above. **findings**: two candidates, both admitted (MEDIUM),
as pl-14 and pl-15, both against the repair of pl-12; repaired together
by one codex task, in two commits.

### pl-14 (fixpkg)

Reviewer, verbatim, evidence: "packaging/libmpv/check-libmpv-windows.py:147
exempts every new module beneath the directories listed at :135. A local
Wine harness loaded a freshly compiled third-party libwinpthread-1.dll from
System32 using the checker's exact SetDefaultDllDirectories flags.
check_loaded returned no problems although that module was absent from both
the manifest and baseline." Predicted failure: "The runtime gate can certify
an incomplete set when a dynamically loaded dependency happens to be
installed in the checking machine's System32. That package can then fail on
a clean Windows installation."

Admitted. In repair: the runtimes the set must never use are reported
wherever they load from, %SystemRoot% included.

### pl-15 (fixpkg)

Reviewer, verbatim, evidence: "packaging/libmpv/windows-pe.py:85 passes the
entire directory extent to location(), whose :62 requires one contiguous
raw section. A PE32+ fixture with its export header/EAT at RVA 0x1000 and
its forwarder at RVA 0x2000 in another section loads and resolves under
Wine, including after stripping. The production check_closure instead exits
1 with 'unmapped PE RVA: 0x1000'." Predicted failure: "A valid DLL with this
layout causes the Windows set build to reject an otherwise complete
dependency closure, blocking packaging."

Admitted. In repair: a directory's extent is checked in virtual-address
space and each structure it references is mapped on its own.

## An eighth hunt: the runtime fixes

Dispatched by the working agent over `ca07bfa..3061b52`, limited to the
runtime repairs: pl-4 (`41d47c4`), pl-5 (`9833fc1`), pl-6 (`e93402e`),
pl-7 (`e1cf5f2`), pl-11 (`4d511a2`), the Windows privacy check
(`96c0f4d`) and the macOS draw driving (`cf23d90`, `e3b4005`, `a45999c`).
Same harness and reviewer as above. **findings**: four candidates, all
admitted (MEDIUM), as pl-16 to pl-19; repaired by two codex tasks (the
macOS three, the screens one).

### pl-16 (fixrt)

Reviewer, verbatim, evidence: "src-tauri/src/player/embedded/libmpv.rs:425-428
checks an image only when its loaded basename exactly matches a set
filename. In a non-hardened launch, DYLD_IMAGE_SUFFIX=_debug with
DYLD_LIBRARY_PATH can select an outside libavcodec.63_debug.dylib while
libmpv itself remains bundled. A headless probe confirmed that the
production policy accepts this outside image." Predicted failure: "Embedded
playback accepts an FFmpeg dependency outside Vela's shipped set. This
leaves pl-4 reachable in development launches that honor DYLD variables."

Admitted. In repair: a launch whose environment holds a dyld redirection
variable is refused before the load (a released app's hardened runtime
ignores them anyway).

### pl-17 (fixrt)

Reviewer, verbatim, evidence: "src-tauri/src/player/embedded/libmpv.rs:467-475
samples dyld's image count once, then enumerates its changing image list.
If another thread unloads an unrelated image during the scan, a later index
can return NULL, triggering refusal. src-tauri/src/player/embedded/host.rs:393
caches that failure." Predicted failure: "A valid bundled set is reported
unavailable, and subsequent embedded plays fall back to the external player
until Vela restarts."

Admitted (the working agent had noted the race when reviewing pl-4 and
judged it rare; the cached result makes it last for the run). In repair:
the scan repeats until the image list is stable, and a transient failure is
not cached.

### pl-18 (fixrt)

Reviewer, verbatim, evidence: "src-tauri/src/player/launch.rs:623-628 treats
every screen_name as remembered, but commands.rs:5157-5159 supplies Vela's
current screen even without a saved or previous play. A production-path
probe with no remembered screen, Vela on DP-2, and Advanced --screen=1
produced [--screen=1, --screen-name=DP-2, --screen=default]." Predicted
failure: "On the first play of a fresh run, Linux and macOS ignore the
user's numeric monitor selection and open playback on Vela's monitor. The
numeric choice survived before this fix."

Admitted; on Windows Vela's fallback index already overrode the user's
`--screen`. In repair, the same on every platform: a remembered screen
stays protected after the user's options; Vela's own fallback becomes a
default before them, which the user's options override.

### pl-19 (fixrt)

Reviewer, verbatim, evidence: "src-tauri/src/player/embedded/macos_options.rs:14
sets video-timing-offset=0 only during configuration, while
libmpv.rs:1202-1218 disables target-time waiting for every render. An
own-config profile containing profile-cond=true and video-timing-offset=0.5
reapplies the offset after initialization. With shipped mpv 0.42 and a
headless software renderer, nonblocking renders of a 5-fps audio/video clip
finished a median 209 ms before target time; the blocking baseline
finished on time." Predicted failure: "macOS presents video ahead of audio
when a conditional profile changes this supposedly protected option. The
existing runtime protection observes only the five colour properties."

Admitted. In repair: the option is observed for the whole play and set
back to 0 whenever it changes.
