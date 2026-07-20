---
description: Run the harness-update playbook to maintain the fleet-global model map. Use when the owner says harness-update.
# toolkit-owned; edits are drift — see AGENTS.md
---

Run the `harness-update` playbook operator: read
`.agents/playbooks/harness-update.md` and follow it. It is the sole write
path to the fleet-global model map (`.agents/model-map.json`) — normal
commit flow in the toolkit repo, never dispatch-time writes. If the
playbook does not exist in this repo, say so rather than guessing. The
playbook is the authoritative definition; this file is only a pointer.
