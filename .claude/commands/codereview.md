---
description: Run the codereview playbook with a named reviewer agent to verify the current finding against its record. Use when the owner says codereview <agent>.
# toolkit-owned; edits are drift — see AGENTS.md
---

Run the `codereview` playbook operator: read
`.agents/playbooks/codereview.md` and follow it to review the current finding
with the agent named in your request (e.g. `/codereview codex`,
`/codereview grok`, `/codereview agy`). The named agent is the reviewer
harness; it is dispatched headless and one-shot per the playbook. If the
playbook does not exist in this repo, say so rather than guessing. The
playbook is the authoritative definition; this file is only a pointer.
