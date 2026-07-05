# eh-9: PID restart guard accepts old+new overlap and foreign Vela processes

**Severity**: LOW — false-green on a teardown leak (two apps sharing one throwaway config) and false-red whenever the owner's own Vela is running
**Status**: Open
**Branch**: n/a — no-branches adaptation; fix lands as a single commit on `main`
**Commit**: (filled in after commit)

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
longer contaminate. Executed results transcribed below.

## Coder dispute (if any)
None — admitted as filed.

## Known gaps
None.

## Reviewer comments
(pending)
