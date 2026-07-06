# sspf-11: Bug 3 auto-open reads a superseded Home load → wrong force-browse

**Severity**: MEDIUM — a slow server source can be force-browsed (its Home rails
dropped) when its hub load is superseded mid-flight.
**Status**: Verified
**Branch**: (no-branches adaptation — landed on `main`)
**Commit**: `6837157` (fix); found reviewing slice `b9cca81`

## Evidence
`src/routes/+page.svelte` — the b9cca81 slice read `hubs`/`heroItems` immediately
after `await loadEverything()` without checking the load was still current.
`loadHome` only writes `hubs`/`recents` `if (gen === homeGen)`, and `loadEverything`
clears `hubs = []` at the start. A concurrent `goHome()` bumps `homeGen`, so the
awaited load resolves without writing — leaving `hubs` empty at the check.

## Predicted observable failure
Click a slow server source (real Home hubs), then click Home before its hubs
arrive: the first load is superseded and never writes, so the selectSource check
sees `hubs.length === 0 && heroItems.length === 0` and force-browses `sections[0]`
— replacing the server's Continue/On Deck/Recently Added rails with a forced
section grid. This is exactly the regression codex plan-review r1 finding 3
warned against (force-browsing a server source).

## What
The routing decision trusted post-`await` state that a concurrent navigation could
have invalidated, mistaking "hubs not yet loaded" for "source has no hubs".

## Approach
The reactive `$effect` (shared with [[sspf-10]]) is gated on `!loading`. A pending
or superseded Home load keeps `loading` true (a superseded `loadHome`'s `finally`
does not clear it — gen mismatch), so the effect waits until the *current* load
settles and `hubs` reflects the real result. A server source with hubs then has
`hubs.length > 0` and is never force-browsed; only a genuinely empty scoped Home
with sections auto-opens.

## Files changed
- `src/routes/+page.svelte` — the `!loading`-gated `$effect` (shared with
  [[sspf-10]]).

## Guard proof
Timing-dependent (a superseded-load race), so covered analytically + by the
`!loading` gate rather than a deterministic E2E: the effect cannot evaluate its
empty-Home condition while any Home load is in flight, so it never reads a
transient/superseded empty `hubs`. The deterministic [[sspf-10]] Home-button guard
plus the existing JF-keeps-its-Home regression leg exercise the non-raced paths.

## Reviewer comments
- **r1** 2026-07-06 `codex` (codex-cli 0.142.5), reviewed `b9cca81` base `f8e6d81`,
  `guard_confirmed: true`, verdict **reopened**. Comment (MEDIUM): "the auto-open
  decision does not verify that the loadEverything() Home result is still the
  current homeGen before treating hubs/heroItems as empty … clicking Home while
  the source load is pending can supersede the awaited hub load … causing
  selectSource() to force-browse sections[0] instead of keeping the server Home
  rails." Admitted; addressed by the `!loading` gate in `6837157`.
