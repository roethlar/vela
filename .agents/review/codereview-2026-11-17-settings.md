# Codereview 2026-11-17 — Settings restructure (S1–S3)

Outcome: a codereview codex hunt over `e29d3c6..ce6e96d` (the plan
`.agents/plans/settings-restructure.md`: S1 `0e4aef7`, S2 `47b2fc0`, S3
`ce6e96d`): **findings**, seven candidates, all verified and admitted.

- Reviewer: codex / host default / host default / standard, in a
  disposable worktree at the head (`~/dev/vela-agent-keep/wt/rv-final-set1`);
  schema-valid, `capability_ok` true, both pins echoed. Brief: the
  scratchpad's `review-final/add-set1.py`, slice `set1`.

| ID   | Severity | Finding (one line)                                                       | Status | Commit |
|------|----------|---------------------------------------------------------------------------|--------|--------|
| st-1 | MEDIUM   | The certificate warning said "off" under a checkbox whose "on" skips the checks | `[x]` | `cb89e51` |
| st-2 | MEDIUM   | A long heading's InfoTip ran past the Settings panel's edge              | `[x]` | `ffd3509` |
| st-3 | MEDIUM   | The documented one-start renderer workaround no longer holds              | `[x]` | `40b409b` |
| st-4 | LOW      | A start that could not be ready said nothing while mpv's window was chosen | `[x]` | `69a9076` |
| st-5 | LOW      | Messages and docs still sent users to the removed Player tab             | `[x]` | `6dc76b0` |
| st-6 | LOW      | Start-up comments still said the choice decides the window mode          | `[x]` | `5ec199a` |
| st-7 | LOW      | The layout guard passed with InfoTip's content never rendered             | `[x]` | `b66f2f7` |

Each verified by reading the code the reviewer cited before repair. Proof:
st-1 by the layout guard's corrected line; st-2, st-4, st-5 and st-7 each
red-proved (the old placement, the old note branch, a "Settings → Player"
put back, the explanation's render removed: each fails its test); st-3
and st-6 change documents and comments only.

The hunt over the repairs (slice `set2`, `ce6e96d..5ec199a`, same
reviewer setup in `rv-final-set2`; schema-valid, `capability_ok` true,
pins echoed) found one more: st-8 (LOW), `docs/development.md` still said
the setting decides how the window is created and to restart; repaired at
`a65f521` (a document only).
