# eh-9: PID restart guard accepts old+new overlap and foreign Vela processes

**Severity**: LOW — false-green on a teardown leak (two apps sharing one throwaway config) and false-red whenever the owner's own Vela is running
**Status**: Verified
**Branch**: n/a — no-branches adaptation; fix lands as a single commit on `main`
**Commit**: `4b24550`

## Evidence
`tests/e2e/scenarios/curation.mjs:121-129` — the restart check compares
raw `pgrep -x vela` output strings. Trigger 1: old app still alive when
the new session starts → output goes `old` → `old\nnew`, strings differ,
assertion passes while two apps share the config. Trigger 2: the owner's
real Vela runs during a harness run → the set is contaminated either way.
Filed by codex in the e2e-3 batch pass.

## Predicted observable failure
False-green: a session-teardown regression leaving the old app alive goes
unnoticed (two processes on one config dir). False-red: the scenario
fails on machines where a user-launched Vela is open — which is exactly
the owner's normal desktop state.

## What
A raw string compare over an unfiltered process list neither isolates the
scenario's app nor detects overlap.

## Approach
Filter candidate pids by `/proc/<pid>/environ` containing this scenario's
unique `XDG_CONFIG_HOME`; assert exactly one such pid before restart, and
after restart poll until the old pid is gone and exactly one NEW pid
remains.

## Files changed
- `tests/e2e/scenarios/curation.mjs` — environ-scoped pid helper +
  exactly-one/old-gone assertions

## Guard proof
The defect is in the predicate itself, so the proof is a direct predicate
check plus the live run: (a) scripted — the old predicate accepts
(`"100"` → `"100\n101"`) while the new logic rejects any state with the
old pid alive or ≠1 scoped pids; (b) the scenario stays green live, and
its pre-restart exactly-one assertion was run with a second (manually
launched, different-config) vela alive to confirm foreign processes no
longer contaminate.

Executed 2026-07-05: (a) predicate check — old logic returns true for
before `100` / after `100\n101` (accepts overlap); the new
exactly-one+different logic rejects it. (b) live — with a decoy vela
(separate config, own Xvfb) running throughout, the scenario PASSes; the
environ-scoped filter counts exactly one app on the scenario's config,
and the post-restart poll requires the old pid gone.

## Coder dispute (if any)
None — admitted as filed.

## Known gaps
None.

## Reviewer comments
codex (codex-cli 0.142.5), manual-check mode, 2026-07-05 ~09:50 UTC.
Reviewed `4b24550abd9015f9eab2cd8737a3f80a9daeebc6` against base
`ebf81625a5ac38a57288fa04d3b10bb2cf2fdcd6`. Verdict: **accepted**,
`guard_confirmed: true`. Comments: the environ match (exact
XDG_CONFIG_HOME entry + trailing NUL) includes same-config processes and
excludes foreign ones; exactly-one-before plus old-gone+exactly-one-after
closes both the overlap false-green and the foreign false-red; residuals
(unreadable environ, pid reuse) fail toward false-red, not missed leaks.
