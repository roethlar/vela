# Review status

Workflow: see `.agents/playbooks/reviewloop.md`. Reviewer harness: `codex`
(codex-cli 0.142.5, re-verified headless 2026-07-05 via `codex exec --json`).
Per-finding detail: see `.agents/review/findings/<id>.md`.
Closed prior loops: `.agents/review/2026-07-04-feature-batch-closed.md`
(rev-1..rev-6) and `.agents/review/2026-07-04-smb-native-closed.md`
(smb-1..smb-6).

Loop opened 2026-07-05 (owner: "playbook reviewloop codex"). Scope: the
2026-07-04 delegation batch, committed directly to `main` —
`ec94715..a055556`: SMB share-root auto-add (`f05919e`) and Continue
Watching curation slices 1-3 (`d2ea1a7`, `cf5af95`, `d259213`), plus their
docs/state commits. Adaptation to the owner's no-branches direction
(recorded 2026-07-04): findings are fixed as single commits on `main`
(one finding ↔ one commit ↔ one verdict); the Branch column records the
fix commit instead. Review dispatches pin (base = ec94715, head =
a055556) for the batch pass, and (base = pre-fix main head, head = fix
commit) per finding.

## Legend
- `[ ]` Admitted, open (not yet started)
- `[~]` In progress / pending review
- `[x]` Verified
- `[!]` Contested — awaiting owner adjudication
- `[-]` Declined at intake

## Findings

| ID | Severity | Impact (one line) | Status | Fix commit |
|----|----------|-------------------|--------|------------|
| cw-1 | MEDIUM | Merged items (local front, server watch key) survive mark-watched/remove in the hero | `[~]` | |
| cw-2 | LOW | Registry lock held across Plex removal await stalls unrelated UI up to 15s | `[~]` | |
| cw-3 | LOW | Failed play clears a removal tombstone; item wrongly returns to hero | `[~]` | |

Review pass 2026-07-05 (codex, read-only, base `ec94715` head `a055556`):
3 candidates, 3 admitted, 0 declined.
