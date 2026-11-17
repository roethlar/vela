# Plan: end-to-end scenarios that hold on a slow runner

Status: DRAFT, 2026-11-17, awaiting the owner's approval; nothing is built.

## Why

The Windows suite at `9d44c5e` (run 44101184219) failed different
scenarios on each of two attempts, while nothing that runs on Windows had
changed since `74df55a`, where it passed in full (`.agents/state.md`,
Blockers). One cause was a real test race, repaired (`smoke`, `b871fcc`).
Two remain:

1. `curation` and `resume` (and, by the same helper, `multiplex`,
   `watchcurate`, `playverbs`): `playAndQuit` (tests/e2e/helpers.mjs) starts
   a play of a 10-second clip, waits for playback to pass 0.5 s, then waits
   for Vela's connections to mpv (`velaConnected`, up to 15 s; on Windows
   through a helper process), then asks mpv its position and seeks to 6 s.
   Both failure screenshots show the first play run to its end, so on the
   slow runner the clip ended during that wait, and the embedded player
   closed its pipe with the question unanswered. (Assumption supported by
   the screenshots; the run recorded no timings.)
2. `continuetv`: one WebDriver `execute/sync` got no response within 30 s.
   One occurrence; cause not known.

## Slices

- **S1, the play cannot end under the harness.** `playAndQuit` pauses the
  play once playback has passed 0.5 s, before waiting for Vela's
  connections, and resumes it after that wait; everything after (the
  position, the seek to 6 s, the stop) is as before. Recommended over
  longer clips, which would need the mock servers' runtimes changed with
  them and would move the proportions `resume` and `curation` judge (6 s
  of 10 is 60%). Red-proof: with the wait made longer than the clip and the
  pause removed, `resume` fails as on Windows; with the pause, it passes.
- **S2, an unanswered question says why.** `MpvIpc` rejects a pending
  command at once when mpv closes the connection ("mpv closed the
  connection before answering get_property"), instead of a 10-second "no
  reply", so the next failure of this kind names its cause.
- **S3, `continuetv`: evidence first.** No change until it recurs; the
  harness already logs the WebDriver call that hung. If it recurs, its own
  plan.

Each slice: the full check, both Linux suites in weston, a Windows run,
a codex hunt over it, records.

## The owner's part

Approve S1 and S2 (or say which).
