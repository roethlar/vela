---
description: Run the codereview playbook with a named reviewer harness to verify the current finding against its record. Use when the owner says codereview <harness> <nickname> <effort>.
# toolkit-owned; edits are drift — see AGENTS.md
---

Run the `codereview` playbook operator: read
`.agents/playbooks/codereview.md` and follow it to review the current
finding. Grammar: `/codereview <harness> <nickname> <effort>` (e.g.
`/codereview codex <nickname> xhigh`); the nickname resolves to a model
slug through the fleet-global map (`.agents/model-map.json`) per the
playbook's "Model map and dispatch grammar" section, and `/review` is a
pure alias of this command. The named harness is the reviewer; it is
dispatched headless and one-shot per the playbook. If the
playbook does not exist in this repo, say so rather than guessing. The
playbook is the authoritative definition; this file is only a pointer.
