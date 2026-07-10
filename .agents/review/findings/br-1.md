# br-1: resume scenario's recents-fallback guard satisfied by the server offset

**Severity**: MEDIUM — the scenario's stated purpose (hero resume position is
source-agnostic via Vela's recents stamp, independent of server thresholds)
is no longer what it proves; a recents-fallback regression stays green.
**Status**: Verified
**Branch**: n/a (no-branches adaptation — one finding ↔ one commit on `main`)
**Commit**: (filled at commit)

## Evidence
`tests/e2e/scenarios/resume.mjs:64` (replay assertion) +
`tests/e2e/mockjf.mjs` Stopped handler: the mock copies the first play's
Stopped position into the movie's `UserData.PlaybackPositionTicks`, so by the
hero replay BOTH stores (server offset and Vela recents stamp) carry ~6s.

## Predicted observable failure
Break the recents fallback (e.g. hero merge drops `viewOffsetMs` from the
recents copy) — the scenario still passes off the server offset, while a real
below-server-threshold item (Plex ~60s minimum) restarts at 0:00.

## What
The dls-s2 port moved the scenario from a local file (recents = only store)
to mock JF (two stores); the guard silently weakened.

## Approach
Real servers don't persist sub-threshold positions — model exactly that:
`startMockJellyfin` gains `minResumeTicks`; a Stopped position below it is
NOT reflected into UserData. The resume scenario seeds a threshold above the
whole 10s clip, so the server never stores a resume point and the recents
stamp is again the only store — faithful to real-server behavior AND the
original guard.

## Files changed
- `tests/e2e/mockjf.mjs` — `minResumeTicks` option gating the Stopped
  reflection
- `tests/e2e/scenarios/resume.mjs` — seed with the whole-clip threshold;
  comment states the guard

## Guard proof
Red/green on the Linux VM: with the fix in place, temporarily strip
`viewOffsetMs` from hero recents copies (app-side hack) → resume must FAIL;
restore → PASS. (Old scenario shape stays green under the same hack — the
weakening this closes.)

## Reviewer comments
(appended after the per-finding verdict)
