<!-- toolkit-owned; edits are drift — see AGENTS.md -->

# Playbook: model map maintenance (`harness-update`)

## What this operator is for

`harness-update` is the sole write path to the fleet-global model map
(`.agents/model-map.json`) — the single committed home for concrete model
slugs (see "Model map and dispatch grammar" in the `codereview`
playbook). Dispatch never writes the map; edits land here, as normal
commits in the toolkit repo, and reach governed repos on their next
refresh.

## Map edits

1. Edit `.agents/model-map.json` in the toolkit repo: add a nickname,
   rotate a slug when a vendor ships a successor, or delete a retired
   nickname. The schema is strict: `version` is `1`, `nicknames` is an
   object of objects, every nickname and slug matches
   `^[a-z0-9][a-z0-9._-]{0,63}$`, and harness keys come from the known
   set only.
2. Run the executable contract before committing:
   `python3 -m unittest tests/test_model_map.py`. The same validation runs
   at dispatch time; a map that fails it loud-stops every dispatch on
   every machine.
3. Commit with a message recording what moved and why (vendor rotation,
   new nickname, retirement). Slugs live in the map and nowhere else in
   committed text — the model-denylist lint enforces this across every
   template.

## What this operator never does

- Never writes the map at dispatch time or from a review worktree.
- Never touches machine-local state: harness presence, transports,
  capability grades, and tier confirmations live in
  `.agents/review/harnesses.local.json` (gitignored) and change only
  through the owner-confirmation flow in the `codereview` playbook.
- Never launders a session-only inline slug override into a pin. An
  override dies with its session; if the same slug keeps being asked
  for, that is the signal to add it to the map here, as its own commit.
- Never confers review eligibility: `openreview_confirmed` and grades
  are untouched by map edits.
