# Codereview 2026-11-13 — the player seam (embedded player, Slice 1)

Outcome: codereview codex over `b19d7d1..b719a65`, limited to the Slice 1
paths: **findings**, two candidates, two admitted (`ps-1` MEDIUM, `ps-2`
LOW), none declined. Neither is a change in what the external player does:
both are about tests.

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
  checked out at the head SHA, and declared disposable in the prompt, so
  the reviewer's own experiments could not touch the working tree.
- Tier: standard. T1 evaluated against the range before dispatch: no
  changed path matches a sensitive-path glob; the repo has no
  `.agents/review/sensitive-paths` override.
- Thread `01a1d376-8a1b-7f50-8c7e-96f48ac76301`; the event stream ends in
  `turn.completed`; 27 tool calls. Dispatched 2026-11-13 05:52 UTC,
  finished 06:00 UTC.

## Reviewed range

- base: `b19d7d1c1f2c5b31e4fed224f30ca00c87524739`
- head: `b719a65d32f253ba463d00a6ce26d7531d66c330`

The range also holds the Slice 0b repair commits, which have their own
review (`.agents/review/codereview-2026-11-13-libmpv-set.md`). The
reviewer was told so and given the exact comparison to evaluate:

```
git diff b19d7d1..b719a65 -- src-tauri tests/mpv-lifecycle.test.mjs \
  tests/config-integrity.test.mjs tests/e2e/scenarios/markerpayload.mjs
```

That is three commits of Slice 1: `38cb850` (records the external command
line on the old code), `3e91c63` (the refactor), `b719a65` (the marker
script takes its payload path as an option).

## Acceptance (computed by the orchestrator)

The payload matches the schema; `verdict` is `findings` and `findings` is
non-empty; `reviewed_sha` and `base_sha` equal the dispatched pins;
`capability_ok` is literally `true`. Accepted as a valid findings verdict.

Payload, verbatim:

```json
{"verdict":"findings","capability_ok":true,"reviewed_sha":"b719a65d32f253ba463d00a6ce26d7531d66c330","base_sha":"b19d7d1c1f2c5b31e4fed224f30ca00c87524739","findings":[{"title":"Isolate the payload-writing tests from each other's pruning","evidence":"src-tauri/src/player/external.rs:436 adds a concurrent caller of try_write_marker_payload. That helper prunes every same-process payload at src-tauri/src/player/launch.rs:576, including the file used by marker_payload_is_written_owner_only_and_absent_for_no_markers. Running those two tests with --test-threads=2 failed on the third repetition: launch.rs:1044 panicked with NotFound.","predicted_failure":"Normal parallel Cargo test runs can intermittently fail because either test deletes the other's payload before its read or permission assertion.","severity":"MEDIUM","better_approach":"Give these tests independent runtime directories, or share a test mutex covering payload creation through assertions and cleanup in both tests."},{"title":"Update the credential guard for structured launch options","evidence":"tests/config-integrity.test.mjs:315 still rejects only the literal source text '--http-header-fields='. Options now use LaunchArg::set. Temporarily adding LaunchArg::set(\"http-header-fields\", ...) populated from spec.http_headers left all 14 JavaScript checks passing, including the private-authentication test.","predicted_failure":"The JavaScript security guard misses a credential-in-argv regression expressed through the new option API. This weakens the existing guard; the current production code still uses the private include.","severity":"LOW","better_approach":"Extend the assertion to recognize structured header options, or check rendered arguments for credential exposure. Prove the guard rejects the demonstrated mutation."}]}
```

## Intake

| Candidate | Verdict | Why |
| --- | --- | --- |
| Payload-writing tests prune each other's files | ADMITTED as `ps-1` | Reproduced here before any fix: the two tests side by side failed 2 runs in 60 (`external.rs:459`, the payload read, `NotFound`). The suite is the gate every slice has to pass. |
| The credential guard misses structured options | ADMITTED as `ps-2` | The assertion named is exactly as described: it looks for the text `--http-header-fields=`, and since the refactor an option is written as `LaunchArg::set("http-header-fields", ...)`, which that text does not match. The shipped code is correct; the guard is not. |

Per-finding records: `.agents/review/findings/ps-1.md`, `ps-2.md`.

## Notes

- What the reviewer reported nothing on, having been pointed at it: the
  order of operations in `playback::play`, the shutdown and failed-start
  paths, the ownership of the auth include and the payload, the reaper
  holding `PlayerHandle`, the moved unit tests, and the marker script's
  option-over-environment precedence. Silence is not proof; it is what a
  bounded defect hunt returns.
- `main` moved on to Slice 2 (`599c539`, `90db1a6`) while the review ran.
  The reviewer was pinned to explicit SHAs; its verdict covers the range
  above and nothing later.
- Not covered: the E2E suite (the reviewer's sandbox has no display and no
  network). It ran 39 of 39 at `3e91c63`, recorded in the plan's log.
