# eh-6: Playback scenario races the seeded source's first render

**Severity**: MEDIUM — a timing-dependent false-red undermines trust in the harness on slower startups
**Status**: Open
**Branch**: n/a — no-branches adaptation; fix lands as a single commit on `main`
**Commit**: (filled in after commit)

## Evidence
`tests/e2e/scenarios/playback.mjs:52-57` — the render wait accepts any
`.sidenav, h1, h2` (the pre-boot Welcome screen already has an `h2`), then
the `E2E Media` side item is looked up with a one-shot `driver.find`
(single WebDriver `/element` request). Trigger: WebDriver reaches the page
before the app's boot finishes loading the seeded local section. Filed by
codex in the e2e-2 batch pass.

## Predicted observable failure
`npm run e2e playback` intermittently fails with no-such-element for
`E2E Media` before exercising anything real — a flaky false-red.

## What
A one-shot element lookup races an async render it has no wait for.

## Approach
Wait specifically for the seeded source's sidebar button (polling
`button.sideitem` text for `E2E Media`) before the `find`/`click`.

## Files changed
- `tests/e2e/scenarios/playback.mjs` — targeted wait before the side-item
  lookup

## Guard proof
The red condition is a startup-timing race that cannot be forced
deterministically from outside the app (no hook slows boot). Statically,
the one-shot `/element` request fails whenever boot hasn't rendered the
section list; the fix replaces it with the same polling wait the rest of
the scenario already relies on, exercised green on every run. Manual-check
mode with an observational red, per the eh-4 precedent.

## Coder dispute (if any)
None — admitted as filed.

## Known gaps
None.

## Reviewer comments
(pending)
