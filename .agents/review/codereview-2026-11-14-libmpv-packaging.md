# Codereview 2026-11-14 — packaging the mpv library set (embedded player, Slice 3, packaging)

Outcome: codereview codex over `2f0d191..4b8293e`: **findings**, six
candidates, six admitted (`pk-1` and `pk-2` HIGH, `pk-3` to `pk-6`
MEDIUM), none declined.

## Dispatch record

- Owner's dispatch word, verbatim (2026-11-12, the session goal):
  "codereview slices with codex". This is the generation half: a defect
  hunt over a pinned landed range.
- Reviewer: codex / host default / host default / standard
- Harness: codex-cli 0.167.2, CLI transport, headless one-shot, stdin
  closed: `codex exec --sandbox workspace-write --json --output-schema
  <schema> -o <outfile> <prompt>`. Called plainly, with no model and no
  effort argument, per `.agents/machines.md`. The `--json` stream names no
  model; the host's `~/.codex/config.toml` reads `model = "gpt-6-astra"`,
  `model_reasoning_effort = "max"`, so that is what a plain call resolves
  to on `continuum`.
- Run from `~/dev/vela-probe-wt`, a detached `git worktree` of this clone
  checked out at the head SHA, and declared disposable in the prompt.
- Tier: standard. T1 evaluated against the range before dispatch: no
  changed path matches a sensitive-path glob; the repo has no
  `.agents/review/sensitive-paths` override.
- Thread `01a1d78e-862a-7b01-819e-e90a4f936d45`; the event stream ends in `turn.completed`.
  Dispatched 2026-11-14 00:56 UTC, finished 01:05 UTC.
- The prompt said that the workflows and `tauri.conf.json` are untouched
  on purpose (released packages do not contain the set), and that tests
  skipped where `bsdtar` is missing are deliberate and documented.

## Reviewed range

- base: `2f0d191f21a74e28c1ec5d7306a46e35b8ee50a8`
- head: `4b8293ecb6485acb9014d5db5390d7765f8e14e5`

One commit, `4b8293e`.

## Acceptance (computed by the orchestrator)

The payload matches the schema; `verdict` is `findings` and `findings` is
non-empty; `reviewed_sha` and `base_sha` equal the dispatched pins;
`capability_ok` is literally `true`. Accepted as a valid findings verdict.

Payload, verbatim:

```json
{"verdict":"findings","capability_ok":true,"reviewed_sha":"4b8293ecb6485acb9014d5db5390d7765f8e14e5","base_sha":"2f0d191f21a74e28c1ec5d7306a46e35b8ee50a8","findings":[{"title":"Extract AppImages without executing the package","evidence":"packaging/libmpv/check-package.sh:185-187 copies the supplied AppImage, makes it executable, and runs it. A harmless shell script named untrusted.AppImage wrote an external marker and created squashfs-root/usr/bin/vela; --without-libmpv returned RESULT PASS.","predicted_failure":"Checking a package executes arbitrary package-controlled commands with the builder's privileges, even in absence-checking mode. Removing APPIMAGE_EXTRACT_AND_RUN does not prevent this.","severity":"HIGH","better_approach":"Extract the AppImage filesystem using a trusted external parser without invoking its runtime. Reject unsupported formats before executing anything from them."},{"title":"Reject invalid sets before loading their libraries","evidence":"packaging/libmpv/check-package.sh:270 invokes check-libmpv.py, whose lines 207-217 continue to ctypes.CDLL after manifest validation fails. Replacing the packaged libmpv.so.2 with a symlink to the external reference set produced manifest failures, followed by a successful load and an isolation failure naming the external library.","predicted_failure":"The checker loads libraries outside the unpacked package and can execute modified library constructors before reporting failure. Its eventual rejection does not undo that execution.","severity":"HIGH","better_approach":"Complete source, checksum, and containment validation before loading any library. Abort the load phase immediately when those checks fail."},{"title":"Keep earlier opt-in Arch packages out of default-build output","evidence":"scripts/build.sh:338 restricts makepkg --packagelist collection to opt-in builds; lines 355-364 collect every matching older package for default builds. Using the build fixture with an earlier real opt-in package in packaging/arch, a build without the flag exited 0 and copied that package into dist/.","predicted_failure":"After an opt-in build and a version change, a default build distributes an older package containing the complete library set despite the flag being absent.","severity":"MEDIUM","better_approach":"Collect only the current makepkg outputs for default Arch builds too. Add a regression case with an earlier opt-in package beside the current output."},{"title":"Check permissions on every ancestor of the packaged set","evidence":"packaging/libmpv/check-package.sh:234-235 checks usr/lib/Vela and its libmpv subtree, omitting usr and usr/lib. Repacking the real set into a deb with usr/lib mode 0700 still returned RESULT PASS and 'every user may read every file of the set'.","predicted_failure":"The checker approves packages whose directory permissions prevent ordinary users from reaching or loading the libraries. Its load succeeds because extraction makes the checking account the owner.","severity":"MEDIUM","better_approach":"Validate traversal permissions on every packaged directory between the installation root and the set, as well as permissions within the set."},{"title":"Validate containment and accessibility of Arch licence copies","evidence":"packaging/libmpv/check-package.sh:293 only compares licence contents with cmp, which follows symlinks. Replacing all Arch licence copies with absolute symlinks to the external reference directory still returned RESULT PASS. A licence copy with mode 0600 also passed.","predicted_failure":"An approved Arch package can have dangling licence links on the destination machine, or licence texts ordinary users cannot read, under usr/share/licenses/vela/libmpv.","severity":"MEDIUM","better_approach":"Before comparing contents, verify that each licence copy and its ancestors remain inside the package and are accessible to ordinary users. Reject external symlinks."},{"title":"Handle spaces in the temporary library path","evidence":"packaging/libmpv/check-package.sh:154 honors TMPDIR and passes the extracted path to check-libmpv.py at line 270. That helper parses /proc/self/maps using line.split()[-1] at line 338. With TMPDIR containing spaces, the unchanged real Arch package failed isolation because the path prefix was discarded.","predicted_failure":"A valid package is rejected, and an opt-in build fails, solely because the builder's temporary directory contains spaces.","severity":"MEDIUM","better_approach":"Parse the fixed fields of /proc/self/maps while preserving the complete pathname remainder. Exercise the package checker with a temporary directory containing spaces."}]}
```

## Intake

| Candidate | Verdict | Why |
| --- | --- | --- |
| Extract AppImages without executing the package | ADMITTED as `pk-1` | The check copies the AppImage, makes it executable and runs it with `--appimage-extract`. That runs the file's own runtime, or, for a file that only calls itself an AppImage, anything at all. The reviewer's shell script named `untrusted.AppImage` ran, wrote a marker outside, and was then PASSED as a package without the set. |
| Reject invalid sets before loading their libraries | ADMITTED as `pk-2` | The manifest check and the load are two steps and the second does not depend on the first. With the packaged `libmpv.so.2` replaced by a link to a library outside the package, the manifest check failed, and the library outside was loaded all the same. |
| Keep earlier opt-in Arch packages out of default-build output | ADMITTED as `pk-3` | Only the opt-in path takes its packages from `makepkg --packagelist`. The default path still collects by file name pattern, as it always did; what is new is that a package with the set can be lying there. |
| Check permissions on every ancestor of the packaged set | ADMITTED as `pk-4` | The check's own load succeeds because unpacking makes the checker the owner of everything. |
| Validate containment and accessibility of Arch licence copies | ADMITTED as `pk-5` | `cmp` follows links, and nothing looks at type or mode. |
| Handle spaces in the temporary library path | ADMITTED as `pk-6` | `check-libmpv.py` takes the last whitespace-separated field of a maps line as the path. |

Per-finding records: `.agents/review/findings/pk-1.md` to `pk-6.md`.

## Notes

- All six are in the package check and the build script. None changes
  what a released package contains: the release workflow does not use any
  of this, and a plain build is affected only by `pk-3`.
- The helpers that built and attacked this before it landed raised 19
  findings of their own and missed these six. Two of them (`pk-1`,
  `pk-2`) are about the check running or loading what it is checking,
  which nobody had asked of it until the reviewer's prompt did.
- Not covered: a real AppImage (none can be built on this host), and any
  build (the reviewer's sandbox has no network).
