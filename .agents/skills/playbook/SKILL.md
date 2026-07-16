---
name: playbook
description: Read the named playbook under .agents/playbooks/ and follow it. Use when the owner says playbook <name>.
---

<!-- Installed by governance refresh; do not edit. Any change here is drift and is restored on the next refresh. Route changes through the toolkit owner. -->

Run the `playbook` operator defined in this repo's `AGENTS.md` (Operator
Requests): read the named playbook at `.agents/playbooks/<name>.md` - the
name comes from the request - and follow it. If the named playbook does not
exist, say so rather than guessing. `AGENTS.md` is the authoritative
definition; this skill is only a pointer.
