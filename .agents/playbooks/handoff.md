<!-- toolkit-owned; edits are drift — see AGENTS.md -->

# Playbook: `handoff` — save my place

Finish in under 30 seconds. Update `.agents/state.md` with the task,
branch, verified head, current completion stage, next action, and live
blockers. Preserve pending merge/deletion work; the handoff ends the
session, not the task. Follow the state template's custody rules for
volatile facts, canonical pointers, and machine facts.

Commit only the records you changed, without another commit approval.
Push per `.agents/push-policy.md`; a policy requiring an ask still applies.
Do not rotate archives or rerun verification: the hygiene sweep belongs
to `catchup`.
