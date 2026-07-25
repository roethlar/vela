---
name: codereview
description: Run the codereview playbook with a named reviewer harness to verify the current finding against its record. Use when the owner says codereview <harness> <model> <effort>.
# toolkit-owned; edits are drift — see AGENTS.md
---

Run the `codereview` playbook operator: read
`.agents/playbooks/codereview.md` and follow it to review the current finding
with the reviewer harness named in the request. Grammar:
`codereview <harness> <model> <effort>` (for example `codereview codex
<model> xhigh`), with `review` as a pure alias; the playbook's "Dispatch
grammar" section defines how `<model>` is handled — the owner's literal
word, used verbatim and checked against no list. The named harness is
the reviewer; it is dispatched headless and one-shot per the playbook. If the
playbook does not exist in this repo, say so rather than guessing. The playbook
is the authoritative definition; this skill is only a pointer.
