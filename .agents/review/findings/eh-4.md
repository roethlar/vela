# eh-4: Screenshots hang nondeterministically on the live Wayland desktop

**Severity**: HIGH — the harness's core deliverable (unattended runs with screenshots) fails whenever the owner is using the machine, which is exactly when unattended runs matter
**Status**: In progress
**Branch**: n/a — no-branches adaptation; fix lands as a single commit on `main`
**Commit**: (filled in after commit)

## Evidence
`tests/e2e/run.mjs` (pre-fix) launched the app on the owner's live Wayland
session. Wayland compositors stop issuing frame callbacks to
occluded/unfocused windows, and WebKit's snapshot path waits on a frame:
`GET /screenshot` then never returns. Observed live 2026-07-05 with
`VELA_E2E_DEBUG=1`: `POST /session` 245ms, every exec in single-digit ms,
`GET /screenshot` timing out at 30s in three consecutive runs while the
owner worked in another window — and the same code passing whenever the
test window opened on top (early-morning green runs).
`WEBKIT_DISABLE_COMPOSITING_MODE=1` does not avoid it (tested, 3/3 fail).

## Predicted observable failure
Any run started while the owner's focus is elsewhere fails its first
screenshot (formerly a 300s opaque hang per eh-3; a 30s named timeout
after it), so the harness only worked when watched — defeating its
purpose. Test windows also pop over the owner's work.

## What
Screenshot rendering depends on desktop window visibility, making runs
nondeterministic on a live session.

## Approach
Default-headless: `run.mjs` starts a private `Xvfb` display (`:97`,
overridable via `VELA_E2E_DISPLAY`) and points scenario processes at it
with `GDK_BACKEND=x11`; on X11-without-compositor, rendering never depends
on visibility. `VELA_E2E_HEADED=1` opts back into the real desktop for
watching a run live. Xvfb lifecycle is tied to the run (exit handler +
signal kills).

## Files changed
- `tests/e2e/run.mjs` — managed Xvfb + display env for scenario spawns
- `tests/e2e/README.md` — Xvfb requirement, HEADED/DISPLAY knobs

## Guard proof
No JS unit runner exists in this repo (recorded gap). Manual red/green:
red = 3/3 consecutive headed runs failing `GET /screenshot` at 30s while
the desktop was in use (transcribed above); green = 3 consecutive
default-headless runs passing with ~50ms screenshots. The red condition
depends on the owner's live focus, so it is observational rather than
scripted; the green condition is scripted and repeatable.

## Coder dispute (if any)
None — coder-filed.

## Known gaps
Headed runs remain focus-dependent by nature (documented in the README).
mpv playback scenarios (future slices) will need the same display env
passed through to mpv, or `--vo=null` per the plan.

## Reviewer comments
(pending)
