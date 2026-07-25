<!-- toolkit-owned; edits are drift — see AGENTS.md -->

# Playbook: `catchup` — re-ground, tidy, report

Two jobs in one read: bring you back up to speed, and keep the record
honest while doing it. The hygiene sweep below is what became of the
retired `drift` operator's state checklist — it rides this pass, it is
never a separate owner word.

## Re-ground

Read `AGENTS.md` (the Prime Invariants in full), `.agents/repo-guidance.md`,
`.agents/state.md`, and any active repo docs (plans in flight, open
decisions). Note untracked or ignored agent-control files that affect the
work.

## Hygiene sweep (`.agents/` records only)

- Rotate landed or superseded `## Now` entries in `state.md` verbatim to
  `docs/history/state-archive.md` (create on first use).
- Re-verify the recorded basis of every parked or blocked item; move
  anything falsified into `## Blockers` with the new evidence.
- Volatile facts (CI state, counts) carry `as of <commit>` and are
  re-verified or dropped.
- Push status is never recorded in state files — git owns it, sessions
  check it live, and unpushed work is mentioned only in the moment it
  matters — so any recorded push-state line is **deleted on sight**, not
  refreshed (2026-07-11 ruling).
- A count or enumeration another file owns is pointed to, never copied.
- Machine-specific facts live in `.agents/machines.md`; prune stale
  entries there.
- A doc, decision, or guidance claim that disagrees with repo evidence:
  fix the lower-authority source — a repo-owned file in place, a
  refresh-installed copy is report-and-route, never edited — or report
  the unresolved conflict.

## Report

Summarize, bottom line first: current state, next action, blockers, and
one proposed first action — plus one line naming what the sweep cleaned
("tidied: archived 2 items, dropped a stale CI note", or "nothing
stale"). Make no other changes until the human responds.
