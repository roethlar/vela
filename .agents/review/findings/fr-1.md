# fr-1: changing one Settings control can save over another setting that is stored with it

**Severity**: MEDIUM — Changing one Settings control can save over another setting that is stored with it.
**Status**: Verified — codex wrote the repair; reviewed by the working agent, 2026-11-16
**Branch**: —
**Commit**: `72270d8`

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

What landed (`72270d8`), in its commit's words:

Seven mpv options are stored by one command and the source policy with
the two display overrides by another. A change captured the whole group
as the dialog held it at that moment and wrote it whole. So a field this
opening had not loaded yet was written with its built-in default (change
Skip intros before the first load answered, and the stored extra mpv
arguments were erased), and a field another opening's save was still
writing was written back to its old value. Both writes succeeded and the
header said Saved (review finding fr-1).

Now a change queues only the field that changed. Its write waits for the
settings to have been loaded, and is put together, when its turn in the
queue comes, from what is confirmed stored at that moment plus that one
field. A load that fails refuses the write and says so; nothing is ever
written from defaults. A choice no longer submits text that is still
being typed in the same group, and a refused text is not sent again by a
sibling's save.

Written by codex; reviewed by the working agent: with Settings.svelte
put back, seven tests fail (for each group: a change before the load
answers, changes queued across two openings with the first accepted or
refused, a failed load; and a choice made beside unblurred text); with
the repair, 35 of 35. End to end with the four repairs after it, in both
players: sourcepolicy, continueoff, windowsetting, autocrop,
embeddedcrop and the reviewer's own autosave checks (one of them 40
bursts of 13 changes, each stored and shown).

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `7b17f63..c093289`, 2026-11-16
(`.agents/review/codereview-2026-11-16-release.md`), the hunt for the slice
"Settings: the Video player control, and every setting saved as it is changed". Reviewer's title: "Grouped autosaves overwrite untouched settings".
Reviewer's severity: MEDIUM.
