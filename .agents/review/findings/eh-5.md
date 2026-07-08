# eh-5: Continue Watching hero never renders for a hub-less (local-only) setup

**Severity**: HIGH — the hero is the product's core resume surface, and for local-only libraries it silently never appears; violates the recorded 2026-07-04 hero decision ("Vela's own recents, any source")
**Status**: Verified
**Branch**: n/a — no-branches adaptation; fix lands as a single commit on `main`
**Commit**: `b4b4ebb`

## Evidence
`src/routes/+page.svelte:1072-1076` (pre-fix) — the Home template renders
the `.home` container (which hosts `heroFlow()`) only when
`hubs.length > 0`; with zero hubs it shows "Nothing on your home screen
yet". `heroItems` derives from Vela's own recents, not hubs — but the
recents-fed hero is unreachable when no source contributes hubs. Trigger:
a setup whose only sources are local folders (local provider contributes
no home hubs), with an unfinished recent play. Found live by the slice-2
E2E playback scenario: after a mid-clip quit, recents carried the stamped
entry (`viewOffsetMs` ≈ 6000) while Home showed the empty-state message
and `[aria-label="Continue watching"]` matched nothing.

## Predicted observable failure
A local-only user plays a file, quits midway, returns Home: no Continue
Watching hero, no resume affordance — despite the recents entry existing
and the decided semantic "recently played and not finished = Continue
Watching, any source".

## What
The hero's render path is gated on hub presence although its data source
(recents) is independent of hubs.

## Approach
Gate the Home empty-state (and its loading skeleton) on
`hubs.length === 0 && heroItems.length === 0` instead of hubs alone, so
the `.home` container renders whenever either hubs or hero items exist;
`heroFlow()` already guards itself on `heroItems.length > 0`. Comment ties
the change to the 2026-07-04 decision.

## Files changed
- `src/routes/+page.svelte:1072-1079` — empty-state/skeleton conditions

## Guard proof
The slice-2 E2E playback scenario's final assertion (clip present in
`[aria-label="Continue watching"]` after a mid-clip quit and Home
navigation) is the automated guard: it fails against the pre-fix app
(observed red 2026-07-05, empty hero list + empty-state text in the
diagnostic dump) and passes after the fix. Revert-check executed 2026-07-05:
with the fix stashed (full rebuild), the scenario FAILS at the hero
assertion (exit 1); with the fix restored, it PASSES. The committed
scenario is the standing guard.

## Coder dispute (if any)
None — coder-filed.

## Known gaps
`loading` skeleton now also requires empty heroItems; recents load in the
same `loadHome` batch as hubs, so no flicker regression is expected.

## Reviewer comments
codex (codex-cli 0.142.5), manual-check mode, 2026-07-05 ~09:21 UTC.
Reviewed `b4b4ebbaf3afa5ac683a95284b6736a25a00a7c4` against base
`8ebbde1c62ccbac9de12682d345432efaaac7e47`. Verdict: **accepted**,
`guard_confirmed: true`. Comments: dual gate closes the root cause;
hub-only behavior unchanged (hubs still render `.home`, hero skipped
without items); no material loading regression (initial no-data still
shows the skeleton; a refresh with hero items keeps Home visible, matching
the recents-fed semantic); the committed scenario plus dd5cec9 hardening
is a sound guard.
