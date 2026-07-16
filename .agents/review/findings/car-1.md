# car-1: Continue Watching must not duplicate its Play/Resume control

**Severity**: LOW — the duplicate action does not launch the wrong item, but it
adds a superfluous button and vertical clutter to Vela's primary Home surface.
**Status**: In progress — implementation and guard proof complete; external
reviews pending
**Branch**: `main` (owner-approved focused correction)
**Commit**: `507fb2fbd6d51ce2e73f6230f9c8806d18c559ce`

## Evidence

At base `7d7bc99`, `src/routes/+page.svelte` rendered a centered `.flowcard`
button whose default action and accessible name were already Play or Resume,
then rendered `.flowactions button.primary` beneath it with the same action.
The owner reported the newly visible Play button as superfluous on 2026-07-16.

## Predicted observable failure

A fresh Continue Watching item shows a second visible Play control below the
carousel even though activating the centered card already plays it. An
in-progress item similarly duplicates Resume, consumes vertical space, and
makes the carousel read as two equivalent primary actions.

## What

Remove the entire below-carousel action row. Keep the centered card as the sole
visible Play/Resume control, and keep the distinct Play from Beginning choice
in its existing context menu.

## Approach

Delete `.flowactions` markup and component CSS without changing the centered
card's button semantics, click handler, or dynamic accessible label. Drive all
continuation E2E paths through that actual card. The `playverbs` scenario now
requires the duplicate row to be absent and proves the hero context menu still
contains Resume and Play from Beginning; the UI-foundation scenario no longer
inventories a primary button that no longer exists.

## Files changed

- `src/routes/+page.svelte` — delete the redundant action row and dead styles.
- `tests/e2e/scenarios/playverbs.mjs` — guard row absence and retained context
  verbs.
- `tests/e2e/scenarios/continueoff.mjs`, `continueon.mjs`, and
  `continuetv.mjs` — activate the centered card instead of the removed button.
- `tests/e2e/scenarios/uifoundation.mjs` — retain primary-style comparison only
  across real remaining primary-button surfaces.

## Guard proof

- A disposable copy restored the exact `.flowactions` Play/Resume markup.
  Fresh-build Linux `playverbs` failed at `the carousel must not duplicate its
  Play/Resume card with an action row`; restoring committed
  `src/routes/+page.svelte` returned the same scenario green.
- The four other selector-dependent Linux scenarios passed together, and the
  final real-app suite passed 25/25.
- Local Node syntax checks, the canonical frontend check, and the production
  frontend build passed before the implementation commit.

## Coder dispute (if any)

None.

## Known gaps

The owner is unavailable to playtest this track. The guard drives the compiled
Tauri app through WebKit on Linux and captured the corrected carousel; no owner
playtest is required.

## Reviewer comments

Primary Claude and independent Grok reviews pending.
