# fr-9: the view's tests pass with several of its rules removed

**Severity**: LOW — The view's tests pass with several of its rules removed.
**Status**: In progress
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "Mutation checks individually removed the missing-track fallback at src/lib/playerControls.ts:245, the narrow-width guard at :536, timer cancellation at src/lib/PlayerView.svelte:245, and fullscreen delivery at :267. Every mutation still passed all 74 tests in tests/player-controls.test.mjs and tests/player-view.test.mjs. The view-handler tests beginning at tests/player-view.test.mjs:86 never exercise a successful double-click."

## Predicted observable failure

Reviewer, verbatim: "The Node checks accept regressions where fast double-clicks pause playback or never toggle fullscreen, alongside broken missing-track and narrow-width handling."

Reviewer's suggested direction, verbatim: "Exercise successful fast and slow double-click sequences through the actual handlers with controlled timers and replies. Add missing-track and narrow-width inputs, then verify each mutation fails."

## What

Admitted on the reviewer's four mutations (the missing-track fallback, the
narrow-width guard of the seek label, the cancelling of the click's timer,
and the delivery of fullscreen on a double-click): each left all 74 tests of
the two files green, and no test drives a successful double-click through
the handlers. A finding about tests; repaired with the others before the
release.

## Approach

The repair is being written by codex as a coding task (worktree
`~/dev/vela-agent-keep/wt/cx-rvview` on `continuum`, not on `main` until the
working agent has reviewed and landed it), under the owner's decision of
2026-11-14 that coding tasks may go to codex and that a repair codex wrote
is closed by the working agent's review. This section is replaced by what
landed.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `7b17f63..c093289`, 2026-11-16
(`.agents/review/codereview-2026-11-16-release.md`), the hunt for the slice
"The embedded player's view: in-page track lists, and the mouse". Reviewer's title: "The Node tests remain green when several control rules are removed".
Reviewer's severity: LOW.
