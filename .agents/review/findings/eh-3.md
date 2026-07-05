# eh-3: Driver requests are unbounded — any stall becomes an opaque 5-minute hang

**Severity**: MEDIUM — a driver/webkit stall turns a ~2s scenario into a silent 300s wait that ends in an error naming no culprit
**Status**: In progress
**Branch**: n/a — no-branches adaptation; fix lands as a single commit on `main`
**Commit**: (filled in after commit)

## Evidence
`tests/e2e/driver.mjs` `#cmd` — every request uses bare `fetch` with no
`AbortSignal`, so the only bound is undici's default 300s headers timeout.
Filed by the coder from live observation (2026-07-05): a WebDriver session
was created driver-side (`/status` → "A session already exists") but the
response never reached the harness; the run sat wedged for 300s and then
failed with `HeadersTimeoutError`, which names no method, path, or step.

## Predicted observable failure
Any lost response or driver stall makes a scenario hang ~300s instead of
failing in seconds; with several scenarios the hangs compound; the
eventual error is unattributable, so diagnosis requires live process
archaeology instead of reading the failure.

## What
The WebDriver client has no per-request deadline and no way to show which
call is in flight.

## Approach
`AbortSignal.timeout` on every `#cmd` request (30s default; 60s for
`newSession`, which legitimately covers app launch), timeout errors that
name the method and path, and a `VELA_E2E_DEBUG=1` mode that logs each
request with millisecond timing to stderr.

## Files changed
- `tests/e2e/driver.mjs` — deadlines + debug logging in `#cmd`

## Guard proof
No JS unit runner exists in this repo (recorded gap). Manual red/green
check instead: against a listener that accepts but never responds, a
driver call must fail in ~its deadline naming the call, not in 300s with
`HeadersTimeoutError`. Red = 300s opaque; green = fast and named.

Executed 2026-07-05: red observed live repeatedly (300s
`HeadersTimeoutError`, no call named). Green: against a silent listener,
`exec` failed after 30004ms with
`POST /session/x/execute/sync → no response within 30000ms`.

## Coder dispute (if any)
None — coder-filed.

## Known gaps
The underlying cause of the observed lost `newSession` response (works in
isolation, wedges in full runs, same machine-state day) is still under
diagnosis; this finding bounds and names such failures, it does not claim
to remove their cause.

## Reviewer comments
(pending)
