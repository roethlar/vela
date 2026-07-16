<!-- Installed by governance refresh; do not edit. Any change here is drift and is restored on the next refresh. Route changes through the toolkit owner. -->

Refresh this repo's governance from the AgentGovernanceBootstrap toolkit.

1. Locate the local toolkit clone (normally `~/dev/AgentGovernanceBootstrap`).
   If none exists, clone it:
   `git clone https://github.com/roethlar/AgentGovernanceBootstrap.git ~/dev/AgentGovernanceBootstrap`.
2. From this repo's root, run the refresh script:
   `py -3 <toolkit>/tools/refresh.py` (Windows) or
   `python3 <toolkit>/tools/refresh.py` (macOS/Linux).
   The script syncs the toolkit from its canonical remote itself (offline it
   proceeds on the local copy and says so), reconciles this repo to the
   shipped artifact set, and makes one scoped commit. Installed governance
   is toolkit-owned: content matching no shipped version is drift — reported
   as a DRIFT note naming the commits that introduced it, then restored to
   the shipped version. Uncommitted changes on paths it would touch make it
   refuse and change nothing.
3. Report the script's reconcile summary to the owner in plain English.
   Surface every DRIFT, FLAG, and LINT line; do not resolve a flagged file
   without an explicit owner decision (LINT lines are read-only hygiene
   findings — dead path references in governance prose, closed decisions
   awaiting archive).
4. If the script flags `AGENTS.md` as not a toolkit instance, this repo
   needs the bootstrap/migration procedure, not a refresh: read
   `<toolkit>/procedures/bootstrap.md` and follow it.

This wrapper adds no write authority: the script's scoped commit is the
refresh; anything beyond it (repo-guidance changes, migrations) goes through
the bootstrap procedure's approval gate.
