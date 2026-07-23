<!-- toolkit-owned; edits are drift — see AGENTS.md -->

# Playbook: `drift` target scope and state-hygiene pass

The `drift` operator's first move — compare a doc, decision, or guidance claim
against repo evidence and fix the lower-authority source or report the
conflict — is defined in `AGENTS.md` (Operator Requests). This playbook carries
the rest of `drift`'s scope: its full target set and the deliberate
state-hygiene pass, read at invoke time so the detail costs no per-session
tokens fleet-wide.

The guidance files themselves — `AGENTS.md` and `.agents/*` — are in scope as drift targets, not just sources of truth, as is any out-of-repo memory the harness injects into sessions (inert where none exists). `drift` also owns the deliberate state-hygiene pass: rotate landed or superseded `## Now` entries verbatim to `docs/history/state-archive.md` (create on first use); re-verify the recorded basis of every parked or blocked item and move anything falsified into `## Blockers` with the new evidence; volatile facts (CI state, counts) carry `as of <commit>` and are re-verified or dropped; push status is never recorded in state files — git owns it, sessions check it live, and unpushed work is mentioned only in the moment it matters — so any recorded push-state line is deleted on sight, not refreshed; a count or enumeration another file owns is pointed to, never copied; machine-specific facts relocate to `.agents/machines.md`, and stale entries there are pruned.
