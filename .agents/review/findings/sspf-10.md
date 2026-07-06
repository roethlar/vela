# sspf-10: Bug 3 empty-scoped-Home routing dead-ends via Home button / Back

**Severity**: HIGH — a scoped local/SMB/SSH source is trapped on the "Nothing on
your home screen yet" dead-end whenever its empty Home is reached via Home or Back.
**Status**: Verified
**Branch**: (no-branches adaptation — landed on `main`)
**Commit**: `6837157` (fix); found reviewing slice `b9cca81`

## Evidence
`src/routes/+page.svelte` — the b9cca81 slice put the auto-open only at the tail of
`selectSource()`. But `goHome()` (Home button, `+page.svelte:1018`) and `back()`
(`back()` → `goHome()` when `crumbs.length === 1`) also land on a scoped source's
Home. `selectSource()` also early-returns `if (activeSource === id && mode === "home")`,
so after the Home dead-end, re-clicking the same source does nothing.

## Predicted observable failure
Scope a hub-less source (Local/SMB/SSH), let it auto-open (b9cca81), then click
Home (or Back from the section): the render hits the dead-end branch
(`+page.svelte` home-scope render) showing "Nothing on your home screen yet", and
re-clicking the source early-returns — the user cannot click out of the dead-end.
E2E-detectable: after the Local source auto-opens, clicking Home shows the dead-end
text and no content grid.

## What
The dead-end-avoidance was tied to one navigation path (source click) instead of
the state it is meant to prevent, so every other path to an empty scoped Home
regressed, and the selectSource early-return made it a trap.

## Approach
Replace the imperative selectSource-tail check with a reactive `$effect`
(`+page.svelte`, after `heroClamp`) that opens the first section whenever a scoped
source's Home *settles* empty (no hubs AND no hero/recents) with sections present.
Being reactive, it covers source click, Home, and Back uniformly. `select()` sets
`mode = "browse"` synchronously so the effect cannot loop or double-open.

## Files changed
- `src/routes/+page.svelte` — removed the selectSource-tail block; added the
  `$effect` (shared with [[sspf-11]]).
- `tests/e2e/scenarios/sourcedeadend.mjs` — added the Home-button leg.

## Guard proof
`tests/e2e/scenarios/sourcedeadend.mjs` — after the Local source auto-opens,
clicking Home must land on content (clip grid present, dead-end text absent).
Reverting `+page.svelte` to the b9cca81 selectSource-only version fails this
assertion ("the Home button on a scoped local source must not dead-end (finding
1)"); restoring the effect passes. Ran headed (Xvfb absent on this host).

## Reviewer comments
- **r1** 2026-07-06 `codex` (codex-cli 0.142.5), reviewed `b9cca81` base `f8e6d81`,
  `guard_confirmed: true`, verdict **reopened**. Comment (HIGH): "empty scoped-Home
  routing only runs after selectSource(), while goHome()/Back reload a scoped local
  source's empty Home without auto-opening … clicking Home or Back renders 'Nothing
  on your home screen yet', and clicking Local again early-returns." Admitted;
  addressed by the reactive effect in `6837157`.
