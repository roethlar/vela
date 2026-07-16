# car-1: Continue Watching must not duplicate its Play/Resume control

**Severity**: LOW — the duplicate action does not launch the wrong item, but it
adds a superfluous button and vertical clutter to Vela's primary Home surface.
**Status**: Verified — primary Claude and independent Grok accepted with
`guard_confirmed:true`
**Branch**: `main` (owner-approved focused correction)
**Commits**: `507fb2fbd6d51ce2e73f6230f9c8806d18c559ce` (implementation),
`c753f45be7e3539cfef2d65bdac69b72b1608d8b` and
`a1b839b4a242ffdc4b03bdccec7d5828eedae3af` (reviewable source guard)

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
- The canonical source contract independently rejects the old action-row
  markup while requiring the centered card's playback handler and dynamic
  Play/Resume accessible label. Restoring the exact markup made only that
  focused Node contract fail; restoring head `a1b839b` returned it green.
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

**Primary implementation review — recorded 2026-07-16T18:38:33Z — accepted.**
Claude Code 2.1.211 (`claude-fable-5`, session
`c257183f-78d8-48ee-a1a4-8cfcefe9b97c`) reviewed exact head
`7c9046b0081f2fa249e5aa7857dce8c6523c9f76` against base
`7d7bc99547a0baf1fdd16f9bee3744d3c52333cd` in a disposable worktree.
It independently restored the base page's complete duplicate action row and
observed the focused source contract fail for the intended reason, then
restored the exact head and observed it pass. It separately removed the
centered card's `play(it)` handler and replaced its dynamic accessible label;
each positive guard failed on the exact missing behavior and returned green
after restoration. The full Node contract set passed 6/6, the worktree was
independently confirmed clean at the reviewed head, and the schema-valid
result carried `guard_confirmed:true`, verdict `accepted`, and no material
finding. Claude did not rerun the Linux-only E2E; the coder's focused and
25/25 Linux runs remain that execution evidence.

**Independent second review — recorded 2026-07-16T18:40:38Z — accepted.**
Grok 0.2.101 (`grok-4.5`, session
`019f6c3a-2ad5-7c01-8f7d-c25d4b220128`) reviewed the same exact head and base
in a separate disposable worktree without seeing the primary verdict. It
independently restored the deleted `.flowactions` row, observed the focused
source contract fail with the intended duplicate-control message, restored the
exact head, observed the guard pass, and left the worktree clean. The
schema-valid result carried the exact SHAs, `guard_confirmed:true`, verdict
`accepted`, and no material comments.
