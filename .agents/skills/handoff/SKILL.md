---
name: handoff
description: Fast save-my-place snapshot of .agents/state.md so the next session resumes without chat context. Use when the owner says handoff or a session is wrapping up.
---

<!-- Installed by governance refresh; do not edit. Any change here is drift and is restored on the next refresh. Route changes through the toolkit owner. -->

Run the `handoff` operator defined in this repo's `AGENTS.md` (Operator
Requests): a fast save-my-place snapshot — update `.agents/state.md` so the
next session resumes without chat context; machine-specific facts go to the
tracked `.agents/machines.md`, keyed by machine and dated. The slow
document-hygiene pass belongs to the `drift` operator, not here.
`AGENTS.md` is the authoritative definition; this skill is only a pointer.
