# Review status

Workflow: see `.agents/playbooks/reviewloop.md`. Reviewer harness: `codex`
(codex-cli 0.142.5, re-verified headless 2026-07-06 via `codex exec --json`).
Per-finding detail: see `.agents/review/findings/<id>.md`.
Closed prior loops: `.agents/review/2026-07-04-feature-batch-closed.md`
(rev-1..rev-6) and `.agents/review/2026-07-04-smb-native-closed.md`
(smb-1..smb-6).

Loop sspf-12 CLOSED 2026-07-06: verified `[x]`, on `main`. Scope was **Bug 5 P1 —
Connected-tab triplication + erroring Remove** (base `ae9d2ff`, head `0a64cd0`,
two codex rounds: r1 reopened, r2 accepted). The slice: `9c3597a` excludes the
whole local family (`LOCAL_FAMILY_KINDS`) from the Connected registered-source loop
(drops the leaked smb/ssh source row + its erroring `remove_source` Remove);
`9379ec5` refuses to remove an SMB mount's last folder (a zombie zero-folder share)
with a guard-proven Rust test, and cascades a last-folder Remove to a full unmount
in the UI. r1 sspf-12 (MEDIUM): both frontend fixes shipped with no automated
guard, so the P1 dead-end could regress with CI green — and codex showed a hermetic
guard IS feasible (a **native** SMB mount, `mountpoint:""`, seeded in config renders
the Connected tab with no connection). Fix `0a64cd0` adds
`tests/e2e/scenarios/connectedtab.mjs` (asserts one SMB row, no leaked source row;
last-folder Remove cascades to unmount with no error), guard-proven headed by
reverting each frontend fix independently. Durable technique: the native mountless
SMB seed makes the Connected tab E2E-testable without SMB infra. Same no-branches
adaptation.

Loop sspf-10..11 CLOSED 2026-07-06: all verified `[x]`, fixes on `main`. Scope
was **Bug 3 — clicking a source dead-ends on empty Home** (frontend nav; code
`b9cca81`) — base `f8e6d81`, converged at head `6837157` after two codex rounds
(r1 reopened, r2 accepted clean). The b9cca81 slice put the empty-scoped-Home →
content auto-open at the tail of `selectSource()`; r1 found two real defects: sspf-10
(HIGH) — Home button and Back (`back()`→`goHome()` from a top-level section) still
dead-ended a scoped local source, and the selectSource early-return trapped the user
there; sspf-11 (MEDIUM) — reading `hubs`/`heroItems` right after `await
loadEverything()` could see a superseded Home load (concurrent `goHome()` bumps
`homeGen`), force-browsing a slow server source whose hubs hadn't arrived. Both
fixed in `6837157` by replacing the imperative check with a reactive `$effect`
that opens the first section when a scoped source's Home *settles* empty (no hubs
AND no hero/recents) with sections present, gated on `!loading` (covers source
click / Home / Back; never misfires mid-load or on a superseded load; keeps server
Home rails — the r1-finding-3 guarantee). Guard `tests/e2e/scenarios/sourcedeadend.mjs`
drives both directions plus the Home-button leg; guard-proven red/green (ran HEADED
— Xvfb absent on this host, owner-approved). sspf-11 is a superseded-load race,
covered by the `!loading` gate + analysis (the deterministic guards cover the
non-raced paths). Same no-branches adaptation.

Loop sspf-5..9 CLOSED 2026-07-06: all verified `[x]`, fixes on `main`. Scope was
**SMB seek Bug 1 sub-slice 3** (per-token SMB session reuse — the real seek fix;
code `05ed86b`) — base `21cd8909`, converged at head `ab3f74c` after **five codex
rounds** (r1-r4 reopened, r5 accepted). Each round banked a distinct, real,
guard-proven defect (a healthy converging loop, not a stall — the fixes built
toward a correct session-lifecycle model): r1 sspf-5 (a create after the play
released orphaned a session → generation-guarded commit, fix `c7211e6`) + sspf-6
(eviction freed a context under the registry lock → drop off-lock, fix `5a64172`);
r2 sspf-7 (sspf-5's release-bump left an ownerless generation a straggler could
store under → replaced with generation=which-play + active=is-it-live, fix
`dec0121`); r3 sspf-8 (a same-file replay keeps the session but play() installs the
owner only on success → release on play failure, fix `ada9f65`); r4 sspf-9 (that
on-failure release ran a blocking `smbc_free_context` on the async worker → moved
onto the blocking pool, fix `ab3f74c`). r5 **accepted** clean (guard_confirmed,
no comments) after a first attempt returned a fail-closed `invalid` (a codex
tooling/budget wrap-up, not a finding; re-prompted per the playbook). All
`Arc<SmbConnection>` drops verified off both the registry lock and async workers.
Same no-branches adaptation.

Loop sspf-4 CLOSED 2026-07-05: verified `[x]`, fix on `main`. Scope was **SMB
seek Bug 1 sub-slice 2** (write deadline on the proxy socket) — base `5c50044`,
head `8f41b90` after two codex rounds. r1 reopened sspf-4 (the 30s write
deadline broke a normal long mpv pause — ffmpeg reconnect is off by default, so
a mid-stream close hit premature EOF on resume); fix `8f41b90` enables ffmpeg
reconnect for the loopback proxy stream (`playback::proxy_reconnect_args`) and
raises the deadline default 30s→300s as a backstop; r2 **accepted** clean.
Same no-branches adaptation.

Loop sspf-1..sspf-3 CLOSED 2026-07-05: all verified `[x]`, fixes on `main`.
Scope was **SMB seek Bug 1 sub-slice 1** (`.agents/plans/smb-ssh-playtest-fixes.md`)
— base `adbeb867`, converged at head `401fd1bc` after four codex rounds (three
reopens, each a real distinct defect, all guard-proven). r1 reopened sspf-1
(token reuse serves a stale cached length → fix `08fef74`); r2 reopened sspf-2
(a late `store_len` repopulates a length a replay cleared, TOCTOU → per-token
generation guard, fix `79f3979`); r3 reopened sspf-3 (env-gated live probe
panics after connect went lazy → fix `401fd1b`); r4 **accepted** clean, no
comments. Each round banked a verifiable delta (healthy converging loop, not a
stall). Same no-branches adaptation.

Loop CLOSED 2026-07-05: cw-1..cw-3 all verified `[x]`, fixes on `main`.

Loop e2e-10 CLOSED 2026-07-05: eh-15 verified `[x]`. Scope was E2E slice
11 (mark-unwatched round-trip; base `d307494`, head `7c899be`); codex
admitted 1 guard-strength finding (eh-15) at intake, extended to both
badge legs. Fix `6db391c` gates each badge assertion on a later
`/Users/{u}/Items` refetch then asserts a present card; guard-proven with
a `drop-after-unwatch` mock (old scenario PASSES the dropped card, fixed
scenario FAILS), and accepted by codex (analytical guard-confirm). An
independent 3-lens adversarial pre-review (all `refuted:false`) refined
the rationale: the optimistic *watched* card never paints (batched Svelte
flush), so the load-bearing hole is the unwatch leg's missing-card wait.
Same no-branches adaptation.

Loop e2e-9 CLOSED 2026-07-05: eh-14 verified `[x]`. Scope was E2E slice
10 — base `7c7a394`, head `5742789` (merged All view scenario); codex
admitted 1 guard-strength finding, fixed and verified. Same no-branches
adaptation.

Loop e2e-8 CLOSED 2026-07-05: eh-13 verified `[x]` after the loop's first
reopen→fix→accept round-trip (reviewer caught a reversed-range crash path
the first fix missed). Scope was E2E slice 9 — base `d3a79de`, head
`ccc6270` (watch-state scenario + mock stream/check-in routes). Same
no-branches adaptation.

Loop e2e-7 CLOSED 2026-07-05: eh-12 verified `[x]`. Scope was E2E slice 8
— base `4ffc272`, head `c706228` (mock-Jellyfin leg + mark-watched
scenario + cleanup hook + plan extension); codex admitted 1 mock-fidelity
finding, fixed and verified. Same no-branches adaptation.

Loop docs-2 CLOSED 2026-07-05: clean pass, no findings. Scope was the
artifact-manifest refresh — base `c1f2b65`, head `7e08272` (docs only).

Loop docs-1 CLOSED 2026-07-05: clean pass, no findings. Scope was the
README test-workflow section — base `b6063e8`, head `36b0a6f` (docs only;
every documented command was live-verified the same day).

Loop app-1 CLOSED 2026-07-05: clean pass, no findings. Scope was slice 7
— base `24de4ee`, head `e7c5231` (resolve_stream onto the blocking pool +
repo-map P0-audit note). Same no-branches adaptation.

Loop e2e-6 CLOSED 2026-07-05: clean pass, no findings. Scope was E2E
slice 6 — base `ee757e2`, head `fc902f4` (search scenario + driver
type()). Same no-branches adaptation.

Loop e2e-5 CLOSED 2026-07-05: eh-11 verified `[x]`. Scope was E2E slice 5
— base `ec69de0`, head `9274ac2` (queue auto-advance scenario + shared
seedLocalMedia helper); codex admitted 1 flakiness finding, fixed and
verified. Same no-branches adaptation.

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
| eh-11 | MEDIUM | Queue scenario: clip A's EOF races the UI window — flaky false-red | `[x]` | `2eabf26` |
| eh-12 | MEDIUM | Mock Jellyfin ignores the query contract — client regressions pass silently | `[x]` | `32c01e2` |
| eh-13 | MEDIUM | Mock stream Range edges crash the runner / send invalid 206s | `[x]` | `526f511`+`d5e1b04` |
| eh-14 | LOW | Merged-view override assertion accepts any key/value — wrong persist stays green | `[x]` | `2b8becb` |
| eh-15 | MEDIUM | Watched-badge waits satisfied by optimistic UI, not post-refetch state | `[x]` | `6db391c` |

Review pass 2026-07-05 (codex, read-only, base `ec94715` head `a055556`):
3 candidates, 3 admitted, 0 declined.

Review pass 2026-07-05 (codex, read-only, base `23f6857` head `34d3412`,
loop e2e-1): 2 candidates, 2 admitted, 0 declined; plus 2 coder-filed
findings admitted during the loop (eh-3, eh-4). All 4 verdicts: accepted,
guard_confirmed (codex, manual-check mode — no JS unit runner in repo).
