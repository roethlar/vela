# fr-2: a Settings save that is refused after the dialog has closed is never shown

**Severity**: MEDIUM — A Settings save that is refused after the dialog has closed is never shown.
**Status**: In progress
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "src/lib/Settings.svelte:100 keeps feedback in each dialog's draft, while :377 submits text during destruction and :570 reports failures only to that draft. Reproduced: edit the mpv path, close, reopen, then refuse the outstanding write. The destroyed dialog receives the error; the reopened section's error remains null. Conversely, a successful save in another opening does not clear an existing opening's obsolete error."

## Predicted observable failure

Reviewer, verbatim: "Text submitted on close can be rejected without any visible explanation or Not saved status. Another opening can also remain Not saved indefinitely after the setting has successfully saved."

Reviewer's suggested direction, verbatim: "Keep save outcomes alongside the shared setting queue and expose them to active dialogs. Clear an earlier failure when a later write to that setting succeeds."

## What

Admitted, on the reviewer's reproduction and on reading `createDraft`: the
error of a refused save is kept in the draft of the opening that made it
(`feedback`), and text edited when Settings closes is submitted while that
opening is being destroyed. A refusal that arrives afterwards is reported to
a draft nobody will see; the next opening shows nothing and the header never
says Not saved. The other half is also as described: an error shown in one
opening is not cleared by a later successful save from another.

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
"Settings: the Video player control, and every setting saved as it is changed". Reviewer's title: "Save errors do not survive closing Settings".
Reviewer's severity: MEDIUM.
