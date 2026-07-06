# sspf-14: Rename Save button errors on a blank name (Bug 5 UX ruling)

**Severity**: MEDIUM — a click terminates in an error-like state, violating the
Bug 5 "no click may terminate in an error/dead-end; every button either acts or
is not rendered" ruling.
**Status**: Verified
**Branch**: (no-branches adaptation — landed on `main`)
**Commit**: `5053d2b` (fixup); found reviewing slice `55a6852` (Bug 5 P2, base `8e4f140`)

## Evidence
`src/lib/Settings.svelte` — the inline rename Save button in the Connected tab
(both SMB and SSH mount rows) is `disabled={busy}` only. When the rename input is
cleared, Save stays enabled; clicking it calls `saveRename`, whose empty-name
guard set `err = "A name is required."` — a visible error message. The Enter key
in the field hit the same path.

## Predicted observable failure
User clears an SMB/SSH mount name and clicks Save (or presses Enter): the UI shows
the "A name is required." error banner instead of the button simply being
unavailable. This is the error-terminating click the Bug 5 UX ruling forbids
(`.agents/plans/smb-ssh-playtest-fixes.md` "Owner UX ruling").

## What
The rename affordance let an empty name reach an error state rather than making
the action unavailable.

## Approach
Make the action unavailable instead of erroring: disable Save when the trimmed
rename text is empty (`disabled={busy || !renameText.trim()}`, both rows), and
turn `saveRename`'s empty-name branch into a silent early return so the Enter-key
path also no-ops rather than showing the banner. The backend command still
rejects an empty name defensively (`rename_*_mount_in_config`), so the guard is
not the only line of defense.

## Files changed
- `src/lib/Settings.svelte` — both Save buttons gated on a non-blank name;
  `saveRename` empty guard returns silently.

## Guard proof
Frontend UX gating (no unit runner for Svelte in-repo). Verified by `npm run
check` + `npm run build` clean and by reasoning: with Save disabled on a blank
field and the Enter handler no-oping, there is no path from the rename affordance
to the error banner. Backend empty-name rejection remains unit-tested
(`rename_tests::*_rejects_empty_and_unknown`).

## Reviewer comments
- **r1** 2026-07-06 `codex` (codex-cli 0.142.5), reviewed `55a6852` base `8e4f140`,
  `guard_confirmed: false` (read-only sandbox — could not create the worktree/run
  the proof; the coder's red/green guard proof stands), verdict **reopened**.
  Comment (MEDIUM): "The inline rename Save action stays enabled when the rename
  field is blank and `saveRename` turns that click into the visible `A name is
  required.` error; clearing an SMB or SSH mount name and clicking Save terminates
  in an error-like state instead of an unavailable action, violating the Bug 5
  no-error-click UX ruling." Admitted — a real instance of the ruling. Fixed by
  disabling Save on a blank name + a silent Enter no-op.
- **r2** 2026-07-06 `codex` (codex-cli 0.142.5), reviewed `5053d2b` base `8e4f140`,
  `guard_confirmed: false` (read-only sandbox — `git worktree add` and `cargo
  test` both need writes it lacks; the coder's red/green guard proof on the pure
  helpers stands), verdict **accepted**: "No material observable defect found in
  the reviewed diff." Loop converged (r1 reopened → r2 accepted clean).
