# fr-5: settings and the guide say a change of player waits for a restart; going back to mpv's own window does not

**Severity**: LOW — Settings and the guide say a change of player waits for a restart; going back to mpv's own window does not.
**Status**: Verified — codex wrote the repair; reviewed by the working agent, 2026-11-16
**Branch**: —
**Commit**: `861cc3f`

## Evidence

Reviewer, verbatim: "src/lib/Settings.svelte:927 and docs/usage.md:82 say a player change takes effect at the next Vela start. However, src-tauri/src/commands.rs:5472 reads the saved player for each new play. In a run started embedded, selecting external therefore makes the next play use mpv's own window without restarting."

## Predicted observable failure

Reviewer, verbatim: "The UI and documentation incorrectly promise that the current player remains selected until restart, and instruct users to restart unnecessarily."

Reviewer's suggested direction, verbatim: "Explain that entering embedded window mode requires a restart when this run lacks it, while switching to the external player affects the next play. Preserve the existing selection behavior."

## What

Admitted on the reviewer's reading of the play path (the saved player is
read for each new play): in a run that started in the embedded window mode,
choosing mpv's own window takes effect at the next play. Only the change TO
the embedded player can need a restart (when this run did not start in its
window mode). The sentence under the control and `docs/usage.md` say a
restart is needed either way. A statement on screen that the code
contradicts.

## Approach

What landed (`861cc3f`), in its commit's words:

Settings, the guide and the release notes said a change of video player
takes effect the next time Vela starts. That is true only one way. Which
player a video uses is read when the video starts, so going back to
mpv's own window applies to the next video; it is choosing Vela's window
that needs a restart, when this run did not start with it (review
finding fr-5).

The sentence under the control, docs/usage.md and RELEASE_NOTES.md now
say that. Nothing about the selection itself changed.

Written by codex; reviewed by the working agent: with the old sentence
put back in Settings.svelte its two guards fail; codex saw the same for
each of the two documents.

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `7b17f63..c093289`, 2026-11-16
(`.agents/review/codereview-2026-11-16-release.md`), the hunt for the slice
"Settings: the Video player control, and every setting saved as it is changed". Reviewer's title: "Restart guidance is false when switching back to the external player".
Reviewer's severity: LOW.
