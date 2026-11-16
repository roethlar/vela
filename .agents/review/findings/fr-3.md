# fr-3: the certificate checkbox can become usable while showing a value that was never loaded

**Severity**: MEDIUM — The certificate checkbox can become usable while showing a value that was never loaded.
**Status**: In progress
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "src/lib/Settings.svelte:219 calls tlsSetting.loaded, which can discard an obsolete read at :63, but :220 unconditionally sets playerTls.loaded=true. Reproduced: close the first opening while its certificate read is pending, reopen, then answer the first read with verify=false while the second remains pending. The reopened checkbox becomes enabled at :968 but still shows skip=false."

## Predicted observable failure

Reviewer, verbatim: "The usable checkbox incorrectly indicates certificate checks are enabled when the stored setting skips them. This violates the requirement to keep the control disabled until its value has loaded."

Reviewer's suggested direction, verbatim: "Set readiness only when a load is accepted into shared confirmed state, or when a save confirms the value. A discarded read must not enable the control."

## What

Admitted on the reviewer's reproduction (the working agent did not re-run
it; the coder is to confirm it first): the load of the certificate setting
marks the control ready whether or not its answer was accepted, so an answer
that belongs to an earlier opening, and is rightly discarded, still enables
the checkbox, which then shows the default (checks on) over a stored choice
to skip them. The plan's rule is that this control stays disabled until its
value has loaded.

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
"Settings: the Video player control, and every setting saved as it is changed". Reviewer's title: "An obsolete certificate read enables an uninitialized checkbox".
Reviewer's severity: MEDIUM.
