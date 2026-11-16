# fr-4: the Settings tests pass when a choice saves its previous value

**Severity**: LOW — The Settings tests pass when a choice saves its previous value.
**Status**: Verified — codex wrote the repair; reviewed by the working agent, 2026-11-16
**Branch**: —
**Commit**: `1f3c4f5`

## Evidence

Reviewer, verbatim: "tests/config-integrity.test.mjs:97 assigns the bound value before invoking the handler; tests/player-window-ui.test.mjs:540 similarly preassigns it. Removing src/lib/Settings.svelte:143, the assignment required because Svelte invokes the handler first, still passes all 61 focused tests. A probe using the actual event order passes unchanged code and fails that mutation: choosing Off saves the previous On value."

## Predicted observable failure

Reviewer, verbatim: "The tests certify autosave even when choices save their previous values. This is a demonstrated gap in the regression guard."

Reviewer's suggested direction, verbatim: "Invoke the parsed event handler with the new DOM value before updating the simulated binding, or exercise the compiled component. Retain the assignment-removal mutation as a red-proof."

## What

Admitted on the reviewer's mutation: the tests that drive the handlers set
the bound value before calling the handler, the reverse of what Svelte does,
so the one line that makes a choice save its NEW value (`changeDraft`
assigns it first) can be removed with every test green. A finding about
tests; a release is in view, so it is repaired.

## Approach

What landed (`1f3c4f5`), in its commit's words:

Svelte calls a control's change handler before it updates the bound
value, so the handler must take the new value from the event:
`changeDraft` assigns it first. The tests that drive the handlers set
the bound value themselves and then called the handler, which supplied
that assignment's effect: with the line removed, every choice in
Settings would have saved its PREVIOUS value and all 61 tests of the two
files stayed green (review finding fr-4).

Both drivers now call the parsed handler with the new value first and
update the binding after, and two tests keep the proof: with the
assignment taken out they fail for saving the wrong value.

Written by codex; reviewed by the working agent, who removed the
assignment again: 28 tests fail, the two named ones among them.

Tests only.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `7b17f63..c093289`, 2026-11-16
(`.agents/review/codereview-2026-11-16-release.md`), the hunt for the slice
"Settings: the Video player control, and every setting saved as it is changed". Reviewer's title: "Autosave tests mask the handler-before-binding requirement".
Reviewer's severity: LOW.
