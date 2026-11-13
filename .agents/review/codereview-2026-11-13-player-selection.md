# Codereview 2026-11-13 — player selection and fallback (embedded player, Slice 2)

Outcome: codereview codex over `1cedb48..90db1a6`: **findings**, four
candidates, four admitted (`sel-1` MEDIUM, `sel-2`, `sel-3`, `sel-4` LOW),
none declined.

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
- Thread `01a1d39c-eae6-70a1-82db-33f1ff2a5dfb`; the event stream ends in
  `turn.completed`; 27 tool calls. Dispatched 2026-11-13 06:33 UTC,
  finished 06:47 UTC.

## Reviewed range

- base: `1cedb480e5e4db9765ffba12e0987af630fe4826`
- head: `90db1a6d959d23dee1fa939550a6426a2ced0fb9`

Two commits: `599c539` (the slice) and `90db1a6` (one wiring test
tightened after its own red-proof).

## Acceptance (computed by the orchestrator)

The payload matches the schema; `verdict` is `findings` and `findings` is
non-empty; `reviewed_sha` and `base_sha` equal the dispatched pins;
`capability_ok` is literally `true`. Accepted as a valid findings verdict.

Payload, verbatim:

```json
{"verdict":"findings","capability_ok":true,"reviewed_sha":"90db1a6d959d23dee1fa939550a6426a2ced0fb9","base_sha":"1cedb480e5e4db9765ffba12e0987af630fe4826","findings":[{"title":"The new settings-read failure bypasses session cleanup","evidence":"src-tauri/src/commands.rs:5044 propagates a settings error after installing the session at :4939 and registering its transcode at :5022. Reproduced by invalidating settings during stream resolution with start_from_beginning=true: the command returned an error, but the new session remained active, the player record still named the old session, and no transcode-stop call occurred. Removing only the new read restored cleanup.","predicted_failure":"A failed play leaves inconsistent playback state and skips the established transcode teardown and cursor restoration. This is a MEDIUM failure-path defect.","severity":"MEDIUM","better_approach":"Read the player setting before stream resolution and provisional session installation, or route this error through the existing launch-failure cleanup."},{"title":"A quality step-down rearms the fallback notice","evidence":"src-tauri/src/commands.rs:5208 records None for a successful step-down: apply_step_down supplies the actual External player, so choose reports no fallback. FallbackNotices::record at src-tauri/src/player/select.rs:107 then clears the previous notice. Exercising the command path with unchanged player=\"embedded\" produced IPC notices in this order: fallback, quality change, fallback on the next play.","predicted_failure":"The same fallback explanation appears twice within one Vela run despite unchanged settings and availability. Playback succeeds; the defect is repeated informational output.","severity":"LOW","better_approach":"Preserve fallback notice history across automatic relaunches of the same logical play. Add coverage for fallback followed by step-down followed by another play."},{"title":"A canceled notice is permanently counted as delivered","evidence":"src-tauri/src/commands.rs:5064 treats constructing the notice as telling the user, and :5213 records it immediately after launch succeeds. However, src-tauri/src/playback.rs:597 waits 1.5 seconds before sending and :598 cancels when the play is replaced. A command-path reproduction replaced the first play before that delay, then kept the replacement running: neither IPC connection received the fallback notice.","predicted_failure":"A quick replacement consumes the once-per-run explanation without displaying it; subsequent ordinary fallback plays silently suppress it. Playback itself remains functional.","severity":"LOW","better_approach":"Record successful notice delivery instead of successful process launch. Keep canceled or failed deliveries due, with session-scoped pending state to prevent duplicate delivery."},{"title":"The refusal test passes when the refusal is removed","evidence":"tests/player-selection.test.mjs:110 checks only that the conditional exists before four calls. Removing the return Err at src-tauri/src/playback.rs:301 while retaining the conditional left all five player-selection tests—and all 19 targeted Node tests—passing.","predicted_failure":"The test named 'play refuses a player it cannot start before it starts anything' falsely passes when an Embedded request can proceed into external-player side effects. This is a LOW regression-test defect; the current production refusal is correct.","severity":"LOW","better_approach":"Exercise playback::play with an Embedded PlaySpec and assert the expected refusal and absence of side effects. Prove that removing the return makes that test fail."}]}
```

## Intake

| Candidate | Verdict | Why |
| --- | --- | --- |
| A settings-read failure after the session is installed skips cleanup | ADMITTED as `sel-1` | The read is where the reviewer says, after `install_playback_session` and after the transcode is registered, and it returns with `?`. The launch-failure path a few lines below exists to undo exactly those two things. |
| A step-down relaunch re-arms the fallback notice | ADMITTED as `sel-2` | Read from the code: a relaunch is handed its player, so the choice reports no fallback, and the success path records "no fallback", which forgets what was told. |
| A notice cancelled before it is shown counts as shown | ADMITTED as `sel-3` | The notice is written 1.5 s after the player starts and is dropped if the play was replaced by then; the record was made when the player started. |
| The refusal test passes with the refusal removed | ADMITTED as `sel-4` | The test looks for the `if`, not for what the `if` does. |

Per-finding records: `.agents/review/findings/sel-1.md` to `sel-4.md`.

## Notes

- All four are in the code Slice 2 added. None changes what a user who
  has not set `player` gets, except `sel-1`: every play reads the setting,
  so every play has the new early return.
- While reading `sel-1` the orchestrator saw the same shape in code that
  predates this work: the settings read for Vela's own resume position, a
  few lines above, also returns with `?` after the session is installed
  (it runs only for a play that resumes and has no server position, and
  before any transcode is registered). It is outside the reviewed range
  and outside the approved plan; it is recorded in `.agents/state.md`
  (Blockers) for the owner and not changed here.
- Not covered: the E2E scenario (no display in the reviewer's sandbox).
