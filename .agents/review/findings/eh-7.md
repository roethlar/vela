# eh-7: Playback guard cannot distinguish IPC quit from natural EOF

**Severity**: MEDIUM — the scenario claims to prove mid-clip quit/resume, but a broken quit can still pass (false-green in the guard itself)
**Status**: Open
**Branch**: n/a — no-branches adaptation; fix lands as a single commit on `main`
**Commit**: (filled in after commit)

## Evidence
`tests/e2e/scenarios/playback.mjs` — 10s clip; seek to 6s, sleep 1.5s,
fire-and-forget `mpv.quit()` (`tests/e2e/mpv.mjs:109-112`), socket closed
immediately; the recents poll then accepts any `viewOffsetMs` in
`[3000, 10000)`. Vela never learns the clip's duration (recents item
`durationMs: null`), so the finished-drop at 95% cannot fire either.
Filed by codex in the e2e-2 batch pass.

## Predicted observable failure
If the quit write is dropped or regresses to a no-op, mpv plays the short
clip to natural EOF inside the 15s poll window and stamps ~9.9s — inside
the accepted range, hero still shown — so the scenario PASSes without
quit/mid-clip behavior ever working.

## What
The guard's accepted outcome overlaps the failure mode it exists to rule
out.

## Approach
Make quit observable and the bound discriminating: after `quit()`, assert
the IPC socket disappears within a few seconds (proves the command acted
— natural EOF at ~10s cannot satisfy it in time), and tighten the offset
upper bound to 8000ms (seek 6s + ≤1.5s observation + margin; EOF stamps
~10s, reliably outside).

## Files changed
- `tests/e2e/scenarios/playback.mjs` — socket-gone assertion after quit;
  offset bound `[3000, 8000]`

## Guard proof
Deterministic red/green: with `quit()` temporarily made a no-op, the
scenario must FAIL (socket persists past the deadline / EOF stamp outside
the tightened bound); restored, it must PASS. Executed results transcribed
below once run.

## Coder dispute (if any)
None — admitted as filed.

## Known gaps
None.

## Reviewer comments
(pending)
