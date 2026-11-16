# fr-9: the view's tests pass with several of its rules removed

**Severity**: LOW — The view's tests pass with several of its rules removed.
**Status**: Verified — codex wrote the repair; reviewed by the working agent, 2026-11-16
**Branch**: —
**Commit**: `b7e2caa`

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

What landed (`b7e2caa`), in its commit's words:

The reviewer removed four rules of the view one at a time and all 74
tests of its two files stayed green: the fallback when the highlighted
track is no longer in the list, the guard for a seek bar no wider than
its thumb, the cancelling of the click's timer, and the delivery of
fullscreen on a double-click. No test drove a successful double-click
through the view's own handlers (review finding fr-9).

Now: a missing highlighted track navigates from the first entry; a
track no wider than its thumb gives no time; and fast and slow
double-clicks are driven through the view's script with its timer and
its command replies controlled: fullscreen toggles both ways, a fast
pair never pauses, a slow pair restores what its first click changed,
answered or not, queued or not.

Written by codex; reviewed by the working agent, who injected the
reviewer's four mutations again: each now fails a test (the fallback,
the guard, `clearTimeout` in clearPictureClick, the fullscreen call).
Two calls that cancel the timer, in the second click's handler and in
the double-click's, each cover for the other; taking out either alone
changes nothing a user or a test can see, and that is not a gap.

Tests only.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `7b17f63..c093289`, 2026-11-16
(`.agents/review/codereview-2026-11-16-release.md`), the hunt for the slice
"The embedded player's view: in-page track lists, and the mouse". Reviewer's title: "The Node tests remain green when several control rules are removed".
Reviewer's severity: LOW.
