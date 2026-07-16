---
name: openreview
description: Run the openreview playbook with a named reviewer agent for an unprimed goal-first review of a whole change. Use when the owner says openreview <agent>.
---

<!-- Installed by governance refresh; do not edit. Any change here is drift and is restored on the next refresh. Route changes through the toolkit owner. -->

Run the `openreview` playbook operator: read
`.agents/playbooks/openreview.md` and follow it to get one unprimed,
goal-first review of a whole change from the reviewer agent named in the
request (for example `openreview codex`). The named agent is the reviewer
harness; it is dispatched headless and one-shot over a pinned base..head range
per the playbook. If the playbook does not exist in this repo, say so rather
than guessing. The playbook is the authoritative definition; this skill is
only a pointer.
