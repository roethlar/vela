# sspf-12: Bug 5 P1 frontend fixes shipped without an automated guard

**Severity**: MEDIUM — the Connected-tab leaked-row filter and the remove-last-folder
cascade are P1 dead-end fixes that could regress while CI stays green.
**Status**: Verified
**Branch**: (no-branches adaptation — landed on `main`)
**Commit**: `0a64cd0` (guard); found reviewing slice `9379ec5`

## Evidence
`src/lib/Settings.svelte` — the filter (`9c3597a`) and the `removeSmbFolder` cascade
(`9379ec5`) are frontend rendering/nav with no automated test. The backend
last-folder guard has a Rust test, but the frontend halves (which are what the user
actually hits) were verified only by inspection.

## Predicted observable failure
A future edit that reverts the filter re-leaks the smb/ssh source row whose Remove
calls `remove_source` and errors (a dead-end) — CI stays green. A future edit that
drops the cascade makes removing a share's last folder surface the backend
last-folder error instead of unmounting — also a dead-end — with CI green. Both are
P1 UX-ruling violations invisible to the existing suite.

## What
The slice's logic-bearing half (backend) was guarded; its user-facing frontend half
was not, despite a hermetic guard being feasible.

## Approach
codex showed the guard is hermetic: a **native** SMB mount (`mountpoint: ""` — the
Linux mountless marker) seeded directly in `config.json` makes the Connected tab
render from config (`get_sources` + `list_smb_mounts`) with **no SMB connection**.
`tests/e2e/scenarios/connectedtab.mjs` seeds one such mount with a single root
folder, opens Settings → Connected, asserts exactly one top-level SMB row (no leaked
source row), then removes the sole folder and asserts a clean cascade-to-unmount
(no `.err` alert, no rows left). (My r1 verification punted on this as "E2E
impractical"; codex correctly showed it is not — author-capitulation avoided by the
reviewer, not by me.)

## Files changed
- `tests/e2e/scenarios/connectedtab.mjs` — new hermetic guard. No product code
  changed (the fix was already correct).

## Guard proof
Ran headed (Xvfb absent). GREEN with both fixes. Reverting ONLY the filter
(`!LOCAL_FAMILY_KINDS.includes` → `!== "local"`) fails "exactly one top-level SMB
row" (two smb rows: leaked source + mount). Reverting ONLY the cascade fails "must
not surface an error" (the backend last-folder error surfaces). Both restored →
green. Each frontend fix is independently load-bearing.

## Reviewer comments
- **r1** 2026-07-06 `codex` (codex-cli 0.142.5), reviewed `9379ec5` base `ae9d2ff`,
  `guard_confirmed: false`, verdict **reopened**. Comment (MEDIUM): the frontend
  filter/cascade is left to inspection even though a hermetic Linux E2E can seed a
  native `smb_mounts` entry, open Settings/Connected, assert one SMB row + subrow,
  click the subrow Remove, and assert no `.err` and no SMB rows remain — "without
  9c3597a the top-level SMB row count is 2, and without the frontend cascade the
  click surfaces the backend last-folder error and leaves the mount, so this P1
  dead-end can regress while CI stays green." Admitted; guard added in `0a64cd0`.
