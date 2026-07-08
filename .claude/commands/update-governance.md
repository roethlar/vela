Refresh this repo's governance from the AgentGovernanceBootstrap toolkit.

1. Confirm the canonical toolkit remote responds:
   `git ls-remote --exit-code https://github.com/roethlar/AgentGovernanceBootstrap.git HEAD`.
   If it does not and no reachable mirror or current local clone is available,
   report that in plain English and stop.
2. Obtain a current copy: shallow-clone the canonical remote into a scratch
   directory (`git clone --depth 1 <canonical-url> <scratch>/agb-toolkit`). A
   LAN mirror, when this repo's guidance names one, may serve as a faster
   fetch source — fast-forward to the canonical head only.
3. Read `<scratch>/agb-toolkit/procedures/bootstrap.md` and follow it top to
   bottom against this repo. The single approval gate is the approval summary;
   every change still goes through it — this command adds no write authority.

This wrapper is only a pointer; the synced `procedures/bootstrap.md` is the
authority for everything that follows.
