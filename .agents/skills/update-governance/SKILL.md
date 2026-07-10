---
name: update-governance
description: Refresh this repo's governance from the AgentGovernanceBootstrap toolkit. Use when the owner asks to update or refresh governance.
---

Refresh this repo's governance from the AgentGovernanceBootstrap toolkit:

1. Locate the local toolkit clone (normally `~/dev/AgentGovernanceBootstrap`);
   if none exists, clone
   `https://github.com/roethlar/AgentGovernanceBootstrap.git` there.
2. From this repo's root run `py -3 <toolkit>/tools/refresh.py` (Windows) or
   `python3 <toolkit>/tools/refresh.py` (macOS/Linux).
3. Report the reconcile summary in plain English; surface every FLAG and
   LINT line and resolve none without an explicit owner decision.
4. If `AGENTS.md` is flagged as not a toolkit instance, this repo needs the
   bootstrap procedure instead: read `<toolkit>/procedures/bootstrap.md` and
   follow it.

This skill adds no write authority; the script's scoped commit is the
refresh.
