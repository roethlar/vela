# eh-8: Restart leg proves tombstone persistence, not application

**Severity**: LOW — the scenario's restart assertions can stay green even if the app stops applying `hidden_from_continue` at startup
**Status**: Open
**Branch**: n/a — no-branches adaptation; fix lands as a single commit on `main`
**Commit**: (filled in after commit)

## Evidence
`tests/e2e/scenarios/curation.mjs` (restart leg) — `hide()` removes the
recents entry AND writes the tombstone (`src-tauri/src/recents.rs`), so
after restart the hero is empty merely because no feed carries the item;
the tombstone's filtering is never exercised. The hero filter
(`src/routes/+page.svelte:293-306`) deliberately suppresses tombstoned
keys from BOTH feeds — recents included — so the application path is
locally testable. Filed by codex in the e2e-3 batch pass.

## Predicted observable failure
False-green: if the app stopped loading or applying tombstones on
startup, a feed item carrying the hidden key would reappear in the hero,
but this scenario would still pass because the only local feed entry was
removed alongside the tombstone.

## What
The guard's restart assertions don't depend on the mechanism they claim
to verify.

## Approach
`ctx.restart(between)` gains an optional between-sessions hook (app down,
no config-lock race). The scenario captures the stamped recents entry
before removal and reinserts it next to the surviving tombstone before
relaunch — after restart, recents carries the item and ONLY the tombstone
keeps it out of the hero (the documented both-feeds suppression), which
the existing hero-absent assertion now genuinely exercises.

## Files changed
- `tests/e2e/run.mjs` — `restart(between)` hook
- `tests/e2e/scenarios/curation.mjs` — capture + reinsert recents entry;
  post-restart assertions now depend on tombstone application

## Guard proof
Deterministic red/green: with the between-hook ALSO clearing
`hidden_from_continue` (entry present, no tombstone), the post-restart
hero-absent assertion must FAIL — proving it depends on the tombstone;
with the tombstone kept, it must PASS.

Executed 2026-07-05: green — scenario PASS with the entry reinserted and
the tombstone kept (hero stays empty across restart). Red — with the
between-hook also clearing `hidden_from_continue`, the run FAILS at
"empty home after restart" (exit 1): the reinserted entry brings the hero
back, proving the assertion depends on tombstone application.

## Coder dispute (if any)
None — admitted as filed. (The recents-side reinsertion is valid: the
frontend explicitly suppresses tombstoned keys from the recents feed too,
so the seeded state exercises documented behavior, not an artificial one.)

## Known gaps
Server-hub tombstone suppression (the On Deck path) remains server-gated
backlog — recorded in the plan's scenario backlog.

## Reviewer comments
(pending)
