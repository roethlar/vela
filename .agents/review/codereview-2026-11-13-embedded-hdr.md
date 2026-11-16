# Codereview 2026-11-13 — HDR in the embedded player (embedded player, Slice 4)

Outcome: codereview codex over `74f44b6..e8db400`: **findings**, three
candidates, three admitted (`hd-1`, `hd-2`, `hd-3`, all MEDIUM), none
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
- Thread `01a1d3ff-ebad-74b1-ae69-49ad6e9e324d`; the event stream ends in
  `turn.completed`; 39 tool calls. Dispatched 2026-11-13 08:22 UTC,
  finished 08:38 UTC.
- The prompt named the nine findings of the Slice 3 review as known and
  out of scope, so that they would not be reported a second time.

## Reviewed range

- base: `74f44b6e598f901b66f1c8f27e2c51ca39a10c56`
- head: `e8db400a734df3d22acbb40fcd086e1cac0df503`

One commit, `e8db400`.

## Acceptance (computed by the orchestrator)

The payload matches the schema; `verdict` is `findings` and `findings` is
non-empty; `reviewed_sha` and `base_sha` equal the dispatched pins;
`capability_ok` is literally `true`. Accepted as a valid findings verdict.

Payload, verbatim:

```json
{"verdict":"findings","capability_ok":true,"reviewed_sha":"e8db400a734df3d22acbb40fcd086e1cac0df503","base_sha":"74f44b6e598f901b66f1c8f27e2c51ca39a10c56","findings":[{"title":"Associate information replies with their output query","evidence":"surface.rs:202-208 replaces the shared `arriving` accumulator, while surface.rs:222-251 ignores which information object sent each event. Destroying the previous description at surface.rs:583-586 does not destroy its information child. A socket-pair test using the real bindings confirmed that old information arriving after the replacement's Ready consumed the accumulator: the result remained reference/max 265/465, and the subsequent 203/203 reply was discarded.","predicted_failure":"An output change is lost until another notification. HDR can remain selected after it is disabled, or playback can retain the previous brightness and tone-mapping target.","severity":"MEDIUM","better_approach":"Give each information object a query generation and its own accumulator. Publish only the current generation's completed description; ignore obsolete replies."},{"title":"Apply protected HDR targets after automatic file profiles","evidence":"session.rs:284-292 sets and tags the HDR target before `loadfile` at line 315. mpv applies automatic protocol/extension profiles during file loading. A user profile such as `[protocol.https]` with `target-trc=bt.1886` therefore overrides PQ after tagging. A headless libmpv test using the equivalent `[protocol.memory]` profile reproduced this override; no profile-contained include was involved.","predicted_failure":"mpv renders SDR-encoded pixels onto a surface still tagged PQ, producing visibly incorrect brightness and colour. With no compositor change, Vela never restores agreement.","severity":"MEDIUM","better_approach":"Apply and verify the protected targets after automatic file profiles execute but before presenting video, for example through a load hook. Keep subsequent target changes coordinated with the surface description."},{"title":"Avoid synchronous property writes on the render thread","evidence":"libmpv.rs:423 calls synchronous `mpv_set_property_string` through `follow_colour` in session.rs:356-361, on the thread responsible for rendering. `/usr/include/mpv/render.h` explicitly warns against this dependency. A headless software-renderer test of this setter with a pending frame blocked for approximately 399 ms.","predicted_failure":"A brightness change, monitor move or HDR toggle can stall rendering while mpv waits for that same thread to render, causing dropped frames and visible playback freezes until mpv's timeout releases the wait.","severity":"MEDIUM","better_approach":"Use asynchronous property updates or a separate control thread while continuing to service rendering. Gate presentation until the updates complete so the frame and surface description still change together."}]}
```

## Intake

| Candidate | Verdict | Why |
| --- | --- | --- |
| Information about the output is not tied to the question it answers | ADMITTED as `hd-1` | `State::arriving` is one slot filled by whichever information object speaks. The orchestrator had reasoned that the compositor answers in the order asked, so an old answer always arrives before the new question is acknowledged. That is how KWin behaves; the protocol does not promise it, and the reviewer showed with the real bindings that the other order loses the newer description. |
| A profile mpv applies by itself while loading a file can undo the HDR target | ADMITTED as `hd-2` | mpv applies the user's `[protocol.…]` and `[extension.…]` profiles when a file loads, after Vela has set its targets and tagged the surface. A profile that sets a target option leaves mpv drawing something other than what the surface says, and nothing notices. Needs own-configuration mode and such a profile. |
| The drawing thread waits for mpv when it sets the targets | ADMITTED as `hd-3` | mpv's `render.h` forbids exactly this: the thread that draws must not wait for the core, because the core may be waiting for that thread to draw. `mpv_set_property_string` waits. mpv breaks the tie with a timeout, so it is a stall, not a hang: the reviewer measured about 0.4 s per call, three calls per change. |

Per-finding records: `.agents/review/findings/hd-1.md` to `hd-3.md`.

## Notes

- None of the three is reached at the start of an ordinary play on this
  machine: the targets are set before mpv has a file, so there is nothing
  to wait for, and KWin answers in order. `hd-3` is reached when the
  display changes during a play (the brightness slider, HDR switched,
  another monitor); `hd-2` needs the user's own mpv configuration.
- `hd-2` and `hd-3` change the same code: once the targets are set
  without waiting, mpv's answer arrives later, and "both sides change
  together" has to hold across that gap. `hd-3` is repaired first, then
  `hd-2` on top of it.
- Not covered: the embedded player itself (no display, compositor or GPU
  in the reviewer's sandbox). The reviewer used a socket pair with the
  real Wayland bindings and a headless libmpv.
