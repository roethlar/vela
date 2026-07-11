Run the `review` playbook operator: read
`.agents/playbooks/reviewloop.md` and follow it to review the current finding
with the agent named in your request (e.g. `/review codex`, `/review grok`,
`/review agy`). The named agent is the reviewer harness; it is dispatched
headless and one-shot per the playbook. If the playbook does not exist in this
repo, say so rather than guessing. The playbook is the authoritative
definition; this file is only a pointer.
