# Review status

Workflow: see `.agents/playbooks/reviewloop.md`. Reviewer harness: `codex`
(codex-cli 0.142.5, re-verified headless 2026-07-05 via `codex exec --json`).
Per-finding detail: see `.agents/review/findings/<id>.md`.
Closed prior loops: `.agents/review/2026-07-04-feature-batch-closed.md`
(rev-1..rev-6) and `.agents/review/2026-07-04-smb-native-closed.md`
(smb-1..smb-6).

Loop CLOSED 2026-07-05: cw-1..cw-3 all verified `[x]`, fixes on `main`.

Loop e2e-2 opened 2026-07-05 (standing instruction: reviewloop codex per
slice). Scope: E2E slice 2 + the app fix it surfaced — base `8ebbde1`,
head `d2be263` (`b4b4ebb` eh-5 hero fix; `d2be263` mpv-IPC playback
scenario). eh-5 was coder-filed and fixed before this loop opened; its
guard is the committed playback scenario (red/green + revert-check
transcribed in the finding doc). Same no-branches adaptation.

Loop e2e-1 CLOSED 2026-07-05: eh-1..eh-4 all verified `[x]`, fixes on
`main`. Scope was E2E harness slice 1 (base `23f6857`, head `34d3412`);
codex admitted eh-1/eh-2, and live diagnosis during eh-1 verification
surfaced two coder-filed findings (eh-3 unbounded requests, eh-4
Wayland-focus screenshot hangs — the root cause of every observed hang),
both fixed and verified in the same loop. Same no-branches adaptation as
the cw loop: one finding ↔ one commit ↔ one verdict.

Prior loop (cw, CLOSED): scope was the 2026-07-04 delegation batch
`ec94715..a055556` — SMB share-root auto-add (`f05919e`) and Continue
Watching curation slices 1-3 (`d2ea1a7`, `cf5af95`, `d259213`). Review
dispatches pinned (base = ec94715, head = a055556) for the batch pass, and
(base = pre-fix main head, head = fix commit) per finding.

## Legend
- `[ ]` Admitted, open (not yet started)
- `[~]` In progress / pending review
- `[x]` Verified
- `[!]` Contested — awaiting owner adjudication
- `[-]` Declined at intake

## Findings

| ID | Severity | Impact (one line) | Status | Fix commit |
|----|----------|-------------------|--------|------------|
| cw-1 | MEDIUM | Merged items (local front, server watch key) survive mark-watched/remove in the hero | `[x]` | `5ce26db` |
| cw-2 | LOW | Registry lock held across Plex removal await stalls unrelated UI up to 15s | `[x]` | `07167f1` |
| cw-3 | LOW | Failed play clears a removal tombstone; item wrongly returns to hero | `[x]` | `f767ae4` |
| eh-1 | MEDIUM | Ctrl-C orphans the driver/app process group and blocks the next run on port 4444 | `[x]` | `25757ea` |
| eh-2 | MEDIUM | Mixed valid+unknown scenario filter exits 0 without running the unknown one | `[x]` | `404f86a` |
| eh-3 | MEDIUM | Unbounded driver requests turn any stall into an opaque 300s hang | `[x]` | `0945104` |
| eh-4 | HIGH | Screenshots hang whenever the test window opens unfocused on the live desktop | `[x]` | `cfe6ee4` |
| eh-5 | HIGH | Local-only setups never see the Continue Watching hero (hub-gated render path) | `[~]` | `b4b4ebb` |

Review pass 2026-07-05 (codex, read-only, base `ec94715` head `a055556`):
3 candidates, 3 admitted, 0 declined.

Review pass 2026-07-05 (codex, read-only, base `23f6857` head `34d3412`,
loop e2e-1): 2 candidates, 2 admitted, 0 declined; plus 2 coder-filed
findings admitted during the loop (eh-3, eh-4). All 4 verdicts: accepted,
guard_confirmed (codex, manual-check mode — no JS unit runner in repo).
