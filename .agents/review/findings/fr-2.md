# fr-2: a Settings save that is refused after the dialog has closed is never shown

**Severity**: MEDIUM — A Settings save that is refused after the dialog has closed is never shown.
**Status**: Verified — codex wrote the repair; reviewed by the working agent, 2026-11-16
**Branch**: —
**Commit**: `88aa965`

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

What landed (`88aa965`), in its commit's words:

The reason for a refused save was kept by the opening of Settings that
made it. Text edited when Settings closes is saved as it closes, so a
refusal of that save arrived for a dialog that was gone: the next
opening showed the old value and nothing else, and the header never said
Not saved. And an error shown in one opening stayed after another
opening had saved the same setting successfully (review finding fr-2).

The outcome of a save now belongs to the setting's queue, which outlives
the dialog: a refusal is shown in that setting's section by whichever
opening is there, now or next, and the next successful save of that
setting clears it everywhere. It is kept in memory only, so it is gone
when Vela restarts.

Written by codex; reviewed by the working agent: with Settings.svelte
put back, 26 tests fail (for each of thirteen controls: a save refused
after closing reaches the current and the next opening; a later success
clears it in every opening); with the repair, 61 of 61.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `7b17f63..c093289`, 2026-11-16
(`.agents/review/codereview-2026-11-16-release.md`), the hunt for the slice
"Settings: the Video player control, and every setting saved as it is changed". Reviewer's title: "Save errors do not survive closing Settings".
Reviewer's severity: MEDIUM.
