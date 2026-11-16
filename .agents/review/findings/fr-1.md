# fr-1: changing one Settings control can save over another setting that is stored with it

**Severity**: MEDIUM — Changing one Settings control can save over another setting that is stored with it.
**Status**: In progress
**Branch**: —
**Commit**: —

## Evidence

Reviewer, verbatim: "src/lib/Settings.svelte:126, :598 and :677 capture whole groups before queuing. Reproduced: delay the initial load, then change Skip intros; stored extraArgs='--profile=fast' becomes empty. Changing source policy similarly clears a stored resolution override. Also reproduced: submit useOwnConfig=true, close before application, reopen and change cropping; the second snapshot restores useOwnConfig=false after the first save succeeds."

## Predicted observable failure

Reviewer, verbatim: "Changing one control silently erases another saved or pending choice. Both writes can succeed and show Saved, despite the lost setting."

Reviewer's suggested direction, verbatim: "Queue the fields changed by each action, wait for an accepted initial load, and compose each grouped write with the latest confirmed sibling values after preceding writes finish."

## What

Admitted, on the reviewer's reproductions and on reading `createDraft` and
the grouped saves in `src/lib/Settings.svelte`: a save captures the whole
draft of its group (`capture()` copies every field) and writes it whole, so
a field this opening never loaded or never touched is written with whatever
the draft holds for it: the built-in default while the first load is
unanswered, or the value from before another opening's pending save. Both
writes succeed and the header says Saved. This loses a user's stored choice
without a word, which is the worst thing a settings page can do; the plan's
rule that a late load must not overwrite a change was held, and its mirror
(a change must not overwrite what was never loaded) was not.

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
"Settings: the Video player control, and every setting saved as it is changed". Reviewer's title: "Grouped autosaves overwrite untouched settings".
Reviewer's severity: MEDIUM.
