<!-- toolkit-owned; edits are drift — see AGENTS.md -->

# Playbook: `plan` — the plan contract

An approved plan can be a short, explicit proposal. Write a durable plan
document when the owner invokes `plan`; do not turn every small change
into a document-approval ceremony.

Write for a cold implementing agent: scope, constraints, steps, evidence
and verification, pending decisions, and status. No conversation-dependent
references. Present unresolved owner decisions one at a time in plain
words; record each ruling durably and work only within approved scope.

Keep the current approved task scope, task-specific non-goals, assumptions
and pending choices at the top, before rationale and amendment history.
Apply an adopted amendment to that current contract; do not make readers
reconstruct it from a chronological log. Keep permanent product boundaries
in the already-read repo guidance rather than copying them into each plan.

A plan's status distinguishes verification, pending merge, pending branch
deletion, and completion. Use the git playbook's closeout before closing
implementation work. The owner's approval covers the named steps; do not
ask again for those steps or infer authority for unnamed ones.
