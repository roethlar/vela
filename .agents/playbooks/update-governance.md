<!-- toolkit-owned; edits are drift — see AGENTS.md -->

# Playbook: `update-governance`

1. Read the target repo's guidance, including any owner-only refresh rule.
   Locate the toolkit clone from `.agents/machines.md`, then sibling
   directories containing `tools/shipped-set.json`. If absent, clone
   `https://github.com/roethlar/Bixi.git` as a sibling and record the path
   under the current machine. Do not assume a home-directory layout.
2. Follow the git playbook's branch setup for installation changes. Run
   `py -3 <toolkit>/tools/refresh.py` on Windows, or a probed Python 3.10+
   interpreter on macOS/Linux, from the target repo root. Refresh syncs
   its source, reconciles the shipped set and commits its changes.
3. Report the summary and every DRIFT, FLAG or remediation offer.
   DRIFT means restored content, not a request to edit an installed copy.
   A foreign AGENTS.md needs an explicit replacement decision:
   owner-requested replacement uses `--force`; otherwise follow
   `<toolkit>/procedures/bootstrap.md`. Do not resolve flags on inference.
   An ignore-rule skip instead needs a custody/ignore decision for the named
   rule; it is not foreign content and `--force` cannot override that rule.
4. Verify the result. For work branches, follow the git playbook through
   integration and local/remote deletion. Report pending steps honestly.

This operator adds no write authority beyond its defined refresh steps.
Repo-guidance edits and migration judgments require their own approved
scope. Refresh-installed governance remains toolkit-owned; installed
copies in the development toolkit are refreshed only by its owner.
