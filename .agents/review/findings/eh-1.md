# eh-1: Detached driver process group is orphaned on SIGINT/SIGTERM

**Severity**: MEDIUM — an interrupted run leaves a live app window + drivers and blocks every subsequent run on the occupied port
**Status**: Open
**Branch**: n/a — no-branches adaptation; fix lands as a single commit on `main`
**Commit**: (filled in after commit)

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
(to fill in with the fix)

## Files changed
(to fill in)

## Guard proof
No JS unit runner exists in this repo (recorded gap). Manual red/green
check instead: start `npm run e2e -- --skip-build` in the background, send
SIGINT to the node process mid-scenario, then assert no
`tauri-driver`/`WebKitWebDriver`/debug `vela` processes remain and port
4444 accepts a new run. Red = orphans remain before the fix; green = clean
after.

## Coder dispute (if any)
None — admitted as filed.

## Known gaps
SIGKILL can still orphan the group; not addressable from inside the
process and accepted.

## Reviewer comments
(pending)
