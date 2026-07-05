# eh-7: Playback guard cannot distinguish IPC quit from natural EOF

**Severity**: MEDIUM — the scenario claims to prove mid-clip quit/resume, but a broken quit can still pass (false-green in the guard itself)
**Status**: Verified
**Branch**: n/a — no-branches adaptation; fix lands as a single commit on `main`
**Commit**: `dd5cec9`

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
the IPC socket stops ACCEPTING within 4s (connectability probe — the
socket file itself is only unlinked when Vela cleans its runtime dir, so
`existsSync` was the wrong check and was replaced), and tighten the offset
upper bound to 8000ms (seek 6s + ≤1.5s observation + margin; EOF stamps
~10s, reliably outside).

## Files changed
- `tests/e2e/scenarios/playback.mjs` — socket-gone assertion after quit;
  offset bound `[3000, 8000]`

## Guard proof
Deterministic red/green, executed 2026-07-05 against the final probe:
with `quit()` temporarily a no-op, the scenario FAILS at "mpv socket to
stop accepting after quit" (exit 1); with quit restored it PASSES. (An
earlier `existsSync`-based probe failed green because mpv does not unlink
the socket file — replaced with the connectability probe.)

## Coder dispute (if any)
None — admitted as filed.

## Known gaps
None.

## Reviewer comments
codex (codex-cli 0.142.5), manual-check mode, 2026-07-05 ~09:28 UTC.
Reviewed `dd5cec95d951e8c488348f5507234804197a3f9a` against base
`4f5abd972e69ee3e5a43243b6c4e41712e715625`. Verdict: **accepted**,
`guard_confirmed: true`. Comments: the AF_UNIX connectability probe checks
the right observable with no socket leak; probe + tightened bound closes
the false-green (even an EOF that stopped accepting inside 4s would stamp
~10s, outside [3000,8000]); residual risk limited to ordinary false-red
flake under extreme scheduling delay.
