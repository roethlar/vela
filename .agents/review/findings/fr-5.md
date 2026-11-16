# fr-5: settings and the guide say a change of player waits for a restart; going back to mpv's own window does not

**Severity**: LOW — Settings and the guide say a change of player waits for a restart; going back to mpv's own window does not.
**Status**: In progress
**Branch**: —
**Commit**: —

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
"Settings: the Video player control, and every setting saved as it is changed". Reviewer's title: "Restart guidance is false when switching back to the external player".
Reviewer's severity: LOW.
