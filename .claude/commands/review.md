---
description: Pure alias of codereview. Use when the owner says review <harness> <model> <effort>.
# toolkit-owned; edits are drift — see AGENTS.md
---

`/review` is a pure alias of `/codereview`: run the `codereview` command
with the same arguments (grammar: `/codereview <harness> <model>
<effort>`), per `.agents/playbooks/codereview.md`. It never aliases
`openreview`.
