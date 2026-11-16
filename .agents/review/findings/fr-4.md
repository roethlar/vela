# fr-4: the Settings tests pass when a choice saves its previous value

**Severity**: LOW — The Settings tests pass when a choice saves its previous value.
**Status**: In progress
**Branch**: —
**Commit**: —

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

The repair is being written by codex as a coding task (worktree
`~/dev/vela-agent-keep/wt/cx-rvsettings` on `continuum`, not on `main` until the
working agent has reviewed and landed it), under the owner's decision of
2026-11-14 that coding tasks may go to codex and that a repair codex wrote
is closed by the working agent's review. This section is replaced by what
landed.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `7b17f63..c093289`, 2026-11-16
(`.agents/review/codereview-2026-11-16-release.md`), the hunt for the slice
"Settings: the Video player control, and every setting saved as it is changed". Reviewer's title: "Autosave tests mask the handler-before-binding requirement".
Reviewer's severity: LOW.
