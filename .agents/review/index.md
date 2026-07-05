# Review status

Workflow: see `.agents/playbooks/reviewloop.md`. Reviewer harness: `codex`
(codex-cli 0.142.5, re-verified headless 2026-07-05 via `codex exec --json`).
Per-finding detail: see `.agents/review/findings/<id>.md`.
Closed prior loops: `.agents/review/2026-07-04-feature-batch-closed.md`
(rev-1..rev-6) and `.agents/review/2026-07-04-smb-native-closed.md`
(smb-1..smb-6).

Loop CLOSED 2026-07-05: cw-1..cw-3 all verified `[x]`, fixes on `main`.

Loop e2e-5 opened 2026-07-05 (standing instruction: reviewloop codex per
slice). Scope: E2E slice 5, single commit — base `ec69de0`, head
`9274ac2` (queue auto-advance scenario + shared seedLocalMedia helper).
Same no-branches adaptation.

Loop e2e-4 CLOSED 2026-07-05: eh-10 verified `[x]`. Scope was E2E slice 4
+ the app fix it surfaced — base `e91cbcf`, head `2f5bba8` (`4527613`
eh-10 local-resume fix, coder-filed with the resume scenario as guard;
`2f5bba8` helpers + resume scenario). The codex batch pass over the slice
itself returned NO material issue — recorded as a clean pass. Same
no-branches adaptation.

Loop e2e-3 CLOSED 2026-07-05: eh-8..eh-9 verified `[x]`. Scope was E2E
slice 3 — base `ca0e9da`, head `ee01101` (curation scenario + ctx.restart
in the runner); codex admitted 2 guard-strength findings, both fixed and
verified. Same no-branches adaptation.

Review pass 2026-07-05 (codex, read-only, base `ca0e9da` head `ee01101`,
loop e2e-3): 2 candidates, 2 admitted (eh-8, eh-9), 0 declined.

Loop e2e-2 CLOSED 2026-07-05: eh-5..eh-7 all verified `[x]`, fixes on
`main`. Scope was E2E slice 2 + the app fix it surfaced — base `8ebbde1`,
head `d2be263` (`b4b4ebb` eh-5 hero fix, coder-filed with the playback
scenario as its guard; codex batch pass admitted eh-6 flaky-race and eh-7
quit-vs-EOF false-green, both fixed and verified). Same no-branches
adaptation.

Review pass 2026-07-05 (codex, read-only, base `8ebbde1` head `d2be263`,
loop e2e-2): 2 candidates, 2 admitted (eh-6, eh-7), 0 declined.

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
| eh-5 | HIGH | Local-only setups never see the Continue Watching hero (hub-gated render path) | `[x]` | `b4b4ebb` |
| eh-6 | MEDIUM | Playback scenario races the seeded source render — flaky false-red | `[x]` | `4f5abd9` |
| eh-7 | MEDIUM | Quit-vs-EOF indistinguishable in the playback guard — false-green | `[x]` | `dd5cec9` |
| eh-8 | LOW | Curation restart leg passes without exercising tombstone application | `[x]` | `ebf8162` |
| eh-9 | LOW | PID restart guard: overlap false-green, foreign-Vela false-red | `[x]` | `4b24550` |
| eh-10 | HIGH | Continue Watching restarted local-family items from 0:00 | `[x]` | `4527613` |

Review pass 2026-07-05 (codex, read-only, base `ec94715` head `a055556`):
3 candidates, 3 admitted, 0 declined.

Review pass 2026-07-05 (codex, read-only, base `23f6857` head `34d3412`,
loop e2e-1): 2 candidates, 2 admitted, 0 declined; plus 2 coder-filed
findings admitted during the loop (eh-3, eh-4). All 4 verdicts: accepted,
guard_confirmed (codex, manual-check mode — no JS unit runner in repo).
