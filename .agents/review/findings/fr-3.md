# fr-3: the certificate checkbox can become usable while showing a value that was never loaded

**Severity**: MEDIUM — The certificate checkbox can become usable while showing a value that was never loaded.
**Status**: Verified — codex wrote the repair; reviewed by the working agent, 2026-11-16
**Branch**: —
**Commit**: `00693e5`

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

What landed (`00693e5`), in its commit's words:

"Skip server certificate checks" is disabled until its stored value has
loaded. But the read marked the control ready whether or not its answer
was accepted: an answer belonging to an opening that had since closed,
rightly discarded, still enabled the checkbox, which then showed
"checks on" over a stored choice to skip them (review finding fr-3).

The checkbox, and the guard in front of its save, now follow whether a
value is confirmed for the setting, which only an accepted read or a
save can make true. An accepted value also clears an earlier "could not
read" message; a refused save is still shown.

Written by codex; reviewed by the working agent: with Settings.svelte
put back, the three new tests fail (an obsolete read answering while the
current one is pending; an obsolete success after the current read
failed; an accepted value clearing a read error and not a save's); with
the repair, 64 of 64.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `7b17f63..c093289`, 2026-11-16
(`.agents/review/codereview-2026-11-16-release.md`), the hunt for the slice
"Settings: the Video player control, and every setting saved as it is changed". Reviewer's title: "An obsolete certificate read enables an uninitialized checkbox".
Reviewer's severity: MEDIUM.
