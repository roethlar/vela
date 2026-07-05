# eh-1: Detached driver process group is orphaned on SIGINT/SIGTERM

**Severity**: MEDIUM — an interrupted run leaves a live app window + drivers and blocks every subsequent run on the occupied port
**Status**: Verified
**Branch**: n/a — no-branches adaptation; fix lands as a single commit on `main`
**Commit**: `25757ea`

## Evidence
`tests/e2e/run.mjs:65-80` — `tauri-driver` is spawned with `detached: true`
(own process group, deliberately, so the kill can take down its
WebKitWebDriver child), but the group kill is registered only on the Node
`exit` event. Exit handlers do not run when the process dies to a default
signal disposition, and the detached group does not receive the terminal's
SIGINT. Trigger: Ctrl-C (or `kill`) on `npm run e2e` while a scenario is
running. The reviewer empirically confirmed Node skips `exit` handlers on
SIGINT death.

## Predicted observable failure
The orphaned tauri-driver/WebKitWebDriver/Vela group stays alive: a stale
test app window lingers, and port 4444 stays occupied so the next
`npm run e2e` exits at the already-listening guard.

## What
Interrupting the harness orphans the driver process tree because cleanup
is tied to an event that signals skip.

## Approach
Explicit `SIGHUP`/`SIGINT`/`SIGTERM` handlers in `run.mjs`: each active
scenario's `killTree` (the process-group SIGTERM) is registered in a
module-level `activeKills` set; the signal handler runs every registered
kill and exits with the conventional 128+signum code. The existing `exit`
listener stays for normal/throw paths; `activeKills` is pruned in the same
`finally` that already removed the `exit` listener, so the sets never leak
across scenarios.

## Files changed
- `tests/e2e/run.mjs` — signal handlers + `activeKills` registry (3 small
  edits around the existing killTree lifecycle)

## Guard proof
No JS unit runner exists in this repo (recorded gap). Manual red/green
check instead: start `npm run e2e -- --skip-build` in the background, send
SIGINT to the node process mid-scenario, then assert no
`tauri-driver`/`WebKitWebDriver`/debug `vela` processes remain and port
4444 accepts a new run. Red = orphans remain before the fix; green = clean
after.

Executed 2026-07-05 via a scripted proof (SIGINT fired the moment the app
process exists, survivors checked 2s later):
- Pre-fix: `tauri-driver`, `WebKitWebDriver`, and `vela` all survive —
  ORPHANS-REMAIN.
- Post-fix: no survivors — CLEAN; and a normal `npm run e2e -- --skip-build`
  stays green.

## Coder dispute (if any)
None — admitted as filed.

## Known gaps
SIGKILL can still orphan the group; not addressable from inside the
process and accepted.

## Reviewer comments
codex (codex-cli 0.142.5), manual-check mode, 2026-07-05 ~08:30 UTC.
Reviewed `25757ea79c99a71b61736bd70aa0d653a999bef1` against base
`f24ca117de2c03be998735cf2e03a31a85515a5f`. Verdict: **accepted**,
`guard_confirmed: true`. Comments: the fix closes the root failure
(signals now run the active scenario's group kill before exit); the
red/green proof is discriminating (same mid-scenario SIGINT: survivors
pre-fix, none post-fix, next run green).
