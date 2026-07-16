<!-- Installed by governance refresh; do not edit. Any change here is drift and is restored on the next refresh. Route changes through the toolkit owner. -->

Run the `codereview` playbook operator: read
`.agents/playbooks/codereview.md` and follow it to review the current finding
with the agent named in your request (e.g. `/codereview codex`,
`/codereview grok`, `/codereview agy`). The named agent is the reviewer
harness; it is dispatched headless and one-shot per the playbook. If the
playbook does not exist in this repo, say so rather than guessing. The
playbook is the authoritative definition; this file is only a pointer.
