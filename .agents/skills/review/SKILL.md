---
name: review
description: Run the reviewloop playbook with a named reviewer agent to review the current finding. Use when the owner says review <agent>.
---

Run the `review` playbook operator: read `.agents/playbooks/reviewloop.md`
and follow it to review the current finding with the reviewer agent named
in the request (for example `review codex`). The named agent is the
reviewer harness; it is dispatched headless and one-shot per the playbook.
If the playbook does not exist in this repo, say so rather than guessing.
The playbook is the authoritative definition; this skill is only a pointer.
