# br-1: resume scenario's recents-fallback guard satisfied by the server offset

**Severity**: MEDIUM — the scenario's stated purpose (hero resume position is
source-agnostic via Vela's recents stamp, independent of server thresholds)
is no longer what it proves; a recents-fallback regression stays green.
**Status**: Verified
**Branch**: n/a (no-branches adaptation — one finding ↔ one commit on `main`)
**Commit**: `8c596d0`

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
Run on the Linux VM 2026-07-09/10. The fallback under guard turned out to be
the BACKEND's: `commands.rs:2199` — when the source resolves `resume_ms == 0`,
`play_item` falls back to `recents::resume_stamp_ms`. (A first hack nulling
`view_offset_ms` in `recents::list()` left resume GREEN — the UI copy's
offset is not the resume driver; recorded here so nobody re-tries that layer.)
- RED: sever the backend fallback (`resume_stamp_ms` call → 0), rebuild →
  new-shape resume FAILS: "resume must start at the stamped 7500ms, got
  0.125s".
- VACUOUS-PASS: same severed build, OLD scenario shape (no `minResumeTicks`,
  server reflects Stopped) → resume PASSES — the exact weakening this
  finding closes.
- GREEN: hack reverted, fix in place → resume PASSES (and the full suite
  10/10).

## Reviewer comments
codex-cli 0.144.0 (read-only), reviewed_sha `8c596d0`, base_sha `6f3e0b1`,
`guard_confirmed:false` (Linux-only suite not runnable from the mac host —
the coder's recorded red/vacuous-pass/green run stands), verdict
**accepted**, 0 comments — 2026-07-10 (UTC).
