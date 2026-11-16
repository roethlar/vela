# Codereview 2026-11-14 — the parity tests (embedded player, Slice 6, the test side)

Outcome: codereview codex over `ab95c49..ac3e563`: **findings**, eight
candidates, eight admitted (`pa-1` to `pa-7` MEDIUM, `pa-8` LOW), none
declined.

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
- Thread `01a1d8e7-8ea4-7580-a1e8-1bff5869b505`; the event stream ends in `turn.completed`.
  Dispatched 2026-11-14 07:13 UTC, finished 07:27 UTC.
- The prompt said that the commit changes no product code, that the
  recorded known failures and exemptions are deliberate (to be reported
  only if what is recorded about them is false), and that the end-to-end
  suite cannot be run in the reviewer's sandbox, so the scenarios are to
  be read and each changed one compared with its version at the base.

## Reviewed range

- base: `ab95c4932f97b0f0029d9fa9e8b157d9868a2376`
- head: `ac3e5633a3b36522f997343d20344615a73a6c7d`

One commit, `ac3e563`.

## Acceptance (computed by the orchestrator)

The payload matches the schema; `verdict` is `findings` and `findings` is
non-empty; `reviewed_sha` and `base_sha` equal the dispatched pins;
`capability_ok` is literally `true`. Accepted as a valid findings verdict.

Payload, verbatim:

```json
{"verdict":"findings","capability_ok":true,"reviewed_sha":"ac3e5633a3b36522f997343d20344615a73a6c7d","base_sha":"ab95c4932f97b0f0029d9fa9e8b157d9868a2376","findings":[{"title":"Ordinary playback satisfies the autoskip proof","evidence":"tests/e2e/scenarios/markers.mjs:90 credits row 8a, but the assertions at :125 and :148 allow 14 and 16 seconds respectively for playback to reach 12 seconds. Neither leg pauses playback. A clock-controlled probe using the actual pollUntil implementation passed both predicates at 12 seconds without any skip.","predicted_failure":"If marker loading still works but automatic seeking stops working, both legs pass through natural playback and shown('8a') is reached. This is the only scenario credited for automatic skipping in either player, so the verdict can falsely certify that behaviour.","severity":"MEDIUM","better_approach":"Establish a known position inside the marker and require a jump across its end within a period too short for ordinary playback to cross it. Prove the assertion fails with automatic seeking disabled."},{"title":"Row 9 credits external crop modes that are never exercised","evidence":"tests/e2e/scenarios/autocrop.mjs:117 claims row 9, whose inventory includes off/manual/auto. Its configuration at :131 always selects auto, and its two legs at :148 and :159 test only fresh and resumed automatic cropping. It is the sole external-player coverage for row 9.","predicted_failure":"A broken external manual-crop binding, or an Off setting that still enables cropping, can leave the scenarios and verdict green. Manual cropping is exercised in embeddedcrop, so the declared parity conceals a one-sided proof.","severity":"MEDIUM","better_approach":"Add external manual and Off legs that verify both the absence of automatic cropping and the manual toggle's effect. Credit the complete row only after all its modes have been exercised."},{"title":"The inventory guard accepts removal of a split half","evidence":"tests/e2e/scenario-loader.mjs:285 accepts any nonempty collection of suffixed IDs for a plan row. Removing inventory entry 8b and both markerbutton.mjs and embeddedskip.mjs left all 13 parity tests passing in a local injection, with the plan unchanged. The 'half-split' test at tests/e2e-parity.test.mjs:212 tests whole-plus-split duplication instead.","predicted_failure":"The entire skip-button requirement can disappear while the inventory guard still reports that the plan's inventory is represented. The verdict subsequently has no 8b entry to enforce.","severity":"MEDIUM","better_approach":"Validate complete approved split groups, including every required subrow. Add a regression test that deletes one half and its scenarios."},{"title":"The import watch misses timers from Node's timer modules","evidence":"tests/e2e-parity.test.mjs:77 patches global timer functions only. A temporary scenario importing setTimeout from node:timers and scheduling an unreferenced timer passed all 13 tests; an awaited node:timers/promises delay also passed. The resource-count check at :94 does not expose the unreferenced timer.","predicted_failure":"Importing a scenario can schedule callbacks that later affect the runner or another scenario while the guard reports no import side effects. This contradicts the advertised timer coverage.","severity":"MEDIUM","better_approach":"Instrument the timer-module APIs as well as globals, or track timer creation through an appropriate runtime hook. Add negative tests for both promise-based timers and unreferenced timers."},{"title":"A skipped known failure is accepted as an observed failure","evidence":"tests/e2e/scenario-loader.mjs:201 checks the error text for every non-pass known-failure result without requiring status === 'fail'. With otherwise valid synthetic results, changing external embeddedownconfig to status 'skip' while retaining its error produced CLI exit 0, 'embeddedownconfig ... skip', and 'parity verdict: PASS'.","predicted_failure":"The verdict accepts evidence in which a required scenario was skipped instead of reproducing its recorded defect, undermining the full-run requirement.","severity":"MEDIUM","better_approach":"Require an explicit fail status before matching a known failure's error. Reject skipped, missing, and unknown statuses independently of their error text."},{"title":"The build flag does not establish that the executed binary was built","evidence":"tests/e2e/run.mjs:405 records built: !skipBuild. The build inherits CARGO_TARGET_DIR through :59, while :22 and :251 always select an existing src-tauri/target/debug/vela. Cargo metadata confirmed that CARGO_TARGET_DIR=/tmp/vela-review-target redirects the output directory.","predicted_failure":"With a custom Cargo target directory and an older binary remaining at the hard-coded path, a full run builds elsewhere but executes the older app. Its results nevertheless claim built: true and the current commit, allowing the verdict to accept evidence from another product build.","severity":"MEDIUM","better_approach":"Force the build to the directory used by the runner, or resolve and execute the actual build artifact. Record built only after establishing that this is the artifact the scenarios ran."},{"title":"The Plex coverage does not prove the final stopped timeline","evidence":"tests/e2e/scenarios/multiplex.mjs:72 claims row 2, but :243 accepts any /:/timeline request and :247 separately checks /:/progress. Neither assertion checks timeline state. If the final update_timeline('stopped') in src-tauri/src/playback.rs:1150 is omitted while periodic 'playing' reports and final progress remain, these assertions still pass.","predicted_failure":"The verdict can credit Plex's final-stop behaviour even though Plex receives no stopped timeline report. The other row-2 scenario, watchstate, exercises Jellyfin and cannot detect that omission.","severity":"MEDIUM","better_approach":"Assert a final Plex timeline request with state=stopped, the correct item, and the final position. Distinguish it from reports sent while playback was running."},{"title":"Inherited property names are accepted as test-only routes","evidence":"tests/e2e/scenario-loader.mjs:86 uses 'in' against the ordinary TEST_ONLY_ROUTES object; tests/e2e/proof.mjs uses the same membership check. A declaration with via: 'toString' passed scenarioProblems, and testOnlyRoute('toString') followed by proofProblems also returned no problems.","predicted_failure":"The loader and runtime recorder accept a nonexistent route inherited from Object.prototype, despite promising to reject routes outside the three registered helpers.","severity":"LOW","better_approach":"Check own-property membership, such as Object.hasOwn, in both places. Add rejection cases for inherited names such as toString and constructor."}]}
```

## Intake

Each candidate was checked against the code by the coordinating agent
before it was admitted.

| Candidate | Verdict | Why |
| --- | --- | --- |
| Ordinary playback satisfies the autoskip proof | ADMITTED as `pa-1` | Read: the intro and the commercial run from second 2 to second 12, neither leg pauses the play, and the waits allow 14 and 16 seconds. The scenario's own comment says crossing the range "is only explicable as that seek"; with those limits it is not. The weakness is older than Slice 6, but Slice 6 is what counts the row as shown on it. |
| Row 9 credits external crop modes that are never exercised | ADMITTED as `pa-2` | Read: `autocrop` seeds `mpv_autocrop: 'auto'` and has two legs, a fresh and a resumed play, both automatic. |
| The inventory guard accepts removal of a split half | ADMITTED as `pa-3` | Read: `inventoryProblems` complains only when NO part of a plan row is present. The test named for a "half-split row" tests a row present both whole and split, which is something else. |
| The import watch misses timers from Node's timer modules | ADMITTED as `pa-4` | Read: the watch replaces `globalThis.setTimeout` and its siblings; the timer modules' own exports are other bindings of the same functions and are not replaced. |
| A skipped known failure is accepted as an observed failure | ADMITTED as `pa-5` | Read: `verdictProblems` tests `result.status === 'pass'` and otherwise only the text. The runner does not write such a result today (a skip carries its own reason), so this needs a results file that says so; the rule is still wrong. |
| The build flag does not establish that the executed binary was built | ADMITTED as `pa-6` | Read: `run.mjs` passes its environment to the build and takes the binary from a fixed path. This is in the rule the coordinating agent added last, before landing (a run made with `--skip-build` does not count): it asked whether a build was requested, not whether the binary run is the one built. |
| The Plex coverage does not prove the final stopped timeline | ADMITTED as `pa-7` | Read: the two waits match on the request path alone. |
| Inherited property names are accepted as test-only routes | ADMITTED as `pa-8` | Read: both places use `in` on a plain object. |

Per-finding records: `.agents/review/findings/pa-1.md` to `pa-8.md`.

## Notes

- All eight are about tests: a check that can pass without the thing it
  stands for. None is a defect of Vela itself, and none changes what
  ships. What they change is how far the parity verdict of
  `ac3e563` can be trusted: rows 2, 8a and 9 are counted as shown on
  proofs that do not hold all of what the row says (`pa-7`, `pa-1`,
  `pa-2`), and the guard has four holes (`pa-3`, `pa-4`, `pa-5`, `pa-8`).
- `pa-6` is in the rule the coordinating agent added just before landing.
- `pa-1` and `pa-2` are older than Slice 6: the scenarios were written for
  the external player long before and were only given their row. Slice 6
  is what makes a row's proof count for something.
- The helpers that built and attacked this before it landed raised
  twenty-one findings of their own and missed these eight.
- Not covered by the reviewer: the end-to-end suite itself was read, not
  run (no display, no network in its sandbox).
