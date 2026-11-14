# Codereview 2026-11-14 — controls and cropping (embedded player, Slice 5)

Outcome: codereview codex over `0aeeff0..3d2fef6`: **findings**, two
candidates, two admitted (`pc-1`, `pc-2`, both MEDIUM), none declined.

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
- Thread `01a1d77a-92cd-75f2-ba08-c65470c73aec`; the event stream ends in
  `turn.completed`. Dispatched 2026-11-14 00:34 UTC, finished 00:45 UTC.
- The prompt named the findings of the earlier slices' reviews (`ec-*`,
  `hd-*`) as out of scope, and the four decisions the design leaves to the
  owner (Escape closing the view, no cropping of HDR video, the library
  showing between two plays in fullscreen, the crop script itself) as not
  defects.

## Reviewed range

- base: `0aeeff06064ef24264d06058820ae502ed425d71`
- head: `3d2fef64caa28672370bb37ad9fd5f2f06417195`

One commit, `3d2fef6`.

## Acceptance (computed by the orchestrator)

The payload matches the schema; `verdict` is `findings` and `findings` is
non-empty; `reviewed_sha` and `base_sha` equal the dispatched pins;
`capability_ok` is literally `true`. Accepted as a valid findings verdict.

Payload, verbatim:

```json
{"verdict":"findings","capability_ok":true,"reviewed_sha":"3d2fef64caa28672370bb37ad9fd5f2f06417195","base_sha":"0aeeff06064ef24264d06058820ae502ed425d71","findings":[{"title":"Partial or oversized IPC lines bypass the read deadline","evidence":"src-tauri/src/player/report.rs:313 loops internally until a complete accepted line or socket timeout; oversized lines restart that loop at line 341. Consequently, control.rs:430 cannot recheck its deadline, and report.rs:485 cannot recheck the stop flag. Socket probes showed a 100 ms command budget taking 602 ms with partial data and 446 ms with consecutive oversized events. A reporter also retained its notifier 500 ms after stop while partial data continued.","predicted_failure":"Continuous input can hold a command worker and its gate indefinitely, making subsequent controls return busy. The reporter can likewise stop emitting state and retain its thread and connection despite cancellation.","severity":"MEDIUM","better_approach":"Check elapsed time and cancellation between read chunks, including while discarding oversized lines. Preserve partial data across bounded returns so callers can enforce their deadlines and stop flags."},{"title":"Playback errors leave Vela fullscreen for 50 seconds","evidence":"src/routes/+page.svelte:112 requires two playback-ended events before leaving fullscreen. An error-ending play produces only one: playback.rs:681 excludes errors from the clean-completion dispatcher that emits the second event. Executing the page helpers for fullscreen → player-closed → playback-ended produced no fullscreen-off request, only the 50,000 ms timer from +page.svelte:94.","predicted_failure":"When an embedded play ends with a playback error, its view disappears but the library remains in player-owned fullscreen for 50 seconds despite there being no successor.","severity":"MEDIUM","better_approach":"Report an explicit session-scoped terminal outcome, including errors and whether a successor is pending, and leave player-owned fullscreen on that outcome. Reserve the backstop for missing events."}]}
```

## Intake

| Candidate | Verdict | Why |
| --- | --- | --- |
| A line that arrives in pieces, or one too long to keep, is read past the caller's limit | ADMITTED as `pc-1` | `Lines::next` reads until it has a whole line and only then returns, so neither a command's two-second limit nor a reporter's stop flag is looked at while pieces keep arriving or an over-long line is being thrown away. mpv does not normally send lines in dribbles, so this is not reached in an ordinary play; but the limit and the stop flag are the slice's own promises (one waiting thread per five seconds; no thread outliving its play), and here they do not hold. |
| A play that ends in an error leaves Vela's window in fullscreen for 50 seconds | ADMITTED as `pc-2` | The page leaves fullscreen at the second `playback-ended` of a play, because the first comes before the backend has decided on a successor. An end that is not clean is reported once only, so the page waits out its backstop. The builders knew and left it to the backstop (the page's own comment says so); the reviewer is right that 50 seconds of a fullscreen library is a defect a user of that player would see. |

Per-finding records: `.agents/review/findings/pc-1.md` and `pc-2.md`.

## Notes

- Neither changes a play in mpv's own window.
- Not covered: the embedded player itself and the E2E suite (no display,
  compositor or GPU in the reviewer's sandbox). The reviewer drove the
  line reader with sockets of its own and ran the page's helpers.
