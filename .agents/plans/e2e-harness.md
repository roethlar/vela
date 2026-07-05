# E2E automation harness: drive the real app so playtests don't need the owner

Status: APPROVED-IN-PRINCIPLE 2026-07-04 via the owner-delegation decision
in `.agents/decisions.md` ("building an automated end-to-end test harness
... the harness design still lands as a written plan"). This document is
that design. Implementation may start without a further approval
round-trip; anything that deviates materially from this plan needs a plan
update first.

## Why

The owner is the throughput bottleneck for playtesting and explicitly asked
not to be (2026-07-04). Most items on the standing playtest list are
mechanically checkable: navigation, source add/browse flows, playback
start/seek/resume via mpv, recents/hero behavior, watch-state refresh.
Only visual judgments (HDR passthrough, artwork look, cover-flow feel)
inherently need the owner's eyes; screenshots reduce even those to a
quick flip-through.

## Architecture (three legs, one npm entry point)

1. **UI driving — `tauri-driver` + a vendored WebKitWebDriver.** Playwright
   cannot attach to a Tauri window; Tauri's supported Linux route is
   `tauri-driver` (cargo-installed) fronting WebKitWebDriver. Tests run the
   debug binary, drive the real webview (sidebar nav, Settings, folder
   pickers, context menus, All-view scrolling), and take screenshots into
   an artifacts dir for owner flip-through. See the 2026-07-05 deviation
   below for how WebKitWebDriver is obtained and which protocol client is
   used.
2. **Playback probing — mpv IPC.** mpv runs with `--input-ipc-server`
   (Vela already uses the socket for progress). The harness triggers a
   play through the UI, then talks to the socket directly: assert the
   stream URL shape (loopback proxy for SMB; token-free for Plex), seek,
   wait, quit, then assert the recents entry got position-stamped and the
   hero re-ordered. `--vo=null` keeps runs displayless where rendering
   doesn't matter.
3. **Live-backend smoke — existing env-gated tests, extended.** The SMB
   live probe (`VELA_SMB_LIVE=server/share cargo test --lib live_probe`)
   is the pattern: env-gated Rust tests that hit real backends (owner's
   NAS, Plex server) and skip silently when the env var is absent, so CI
   and other machines are unaffected.

A single `npm run e2e` orchestrates: build debug app → start tauri-driver
→ run the WDIO suite → collect screenshots + a pass/fail summary.

## Credentials & config hygiene

- Real credentials (SMB, server addresses) enter ONLY via env vars at run
  time (`VELA_E2E_SMB=server/share`, `VELA_E2E_SMB_USER`, `..._PASS`);
  never committed, never logged, never echoed in assertions or reports —
  consistent with the 2026-05-23 token/credential stance.
- The harness must not run against the owner's live `~/.config/vela`
  config. It points the app at a throwaway config dir (env override or
  `XDG_CONFIG_HOME`), seeding it per scenario. Mutating-server scenarios
  (mark-watched sync, continue-watching removal) run only when their env
  gate names the server, and only against owner-designated test items.

## Scenario backlog (initial, from the standing playtest list)

- Add SMB share natively (Linux) → source appears with auto-added root
  folder → browse depth → play → seek → quit → resume position recorded.
- Cover-flow hero: short local play appears centered after mpv exit;
  mark-watched drops it; remove-from-continue hides it, survives restart,
  replay restores it (also closes the pending Plex server-side check).
- Watch-state refresh without restart after a Plex play (>60s).
- Merged All view: scroll depth, "N sources" cards, "Play from" override
  persisting.
- Screenshot set: home (hero + rails), library grid, settings panels.

## Non-goals

- No CI wiring in the first slice (local runs on the owner's machine
  first; CI needs a display/compositor story and has no NAS access).
- No visual-diff assertions (screenshots are for human flip-through).
- No macOS/Windows automation (tauri-driver is Linux/Windows; macOS has no
  WKWebView driver — macOS smoke stays manual/parked).
- The harness never replaces the unit-test + guard-proof discipline.

## Deviation 2026-07-05: driver sourcing and protocol client (owner-approved)

The original plan assumed WebKitWebDriver "ships with webkit2gtk on Arch".
Verified false: Arch/CachyOS, Fedora, and openSUSE all build webkit2gtk
without the driver binary, and Debian tops out at 2.50.6 — no distro ships
a driver matching the installed webkit2gtk 2.52.4. AUR has nothing. The
library itself has automation-session support compiled in (the
`webkit_automation_session_*` symbols are exported), so only the small
frontend binary is missing.

Owner-chosen route (2026-07-05, over rebuilding webkit or an in-app
automation channel): **vendor Debian's WebKitWebDriver 2.50.6** plus its
ICU 72 libraries (soname-versioned, so they never shadow the system ICU in
child processes), fetched by `tests/e2e/fetch-driver.sh` into the
gitignored `tests/e2e/vendor/` with pinned URLs and sha256s. A live probe
against the debug Vela build validated the 2.50.6↔2.52.4 version skew:
session creation, script execution, element find, click (opened Settings),
and pixel-true screenshots all work. Known risk: the driver↔browser wire
protocol is WebKit-internal; a future webkit2gtk update may break it. If
that happens, the recorded fallback is an in-app automation channel
(debug-only, env-gated loopback endpoint in the Rust backend: JS eval
bridge + in-process WebKit snapshot), which trades the standard WebDriver
protocol for zero external dependencies.

Second deviation: **no WebdriverIO.** The harness speaks the WebDriver
protocol directly over Node's built-in `fetch` (`tests/e2e/driver.mjs`,
~8 endpoints). WDIO would add a very large dev-dependency tree to a repo
with deliberately few dependencies, for protocol plumbing this thin.

## Extension 2026-07-05: hermetic mock-server leg

The mutating-server scenarios were originally gated on the owner's live
servers. A fourth leg removes that dependency for the flows that only need
protocol-correct responses: the harness can start a **mock Jellyfin server**
on a loopback port inside the runner process (`tests/e2e/mockjf.mjs`),
seed a `sources` entry pointing at it (`build_source` restores it at boot
with no interactive auth), and assert both sides of a mutation — the HTTP
request the app sent and the UI state after its refetch. Stateful, minimal
endpoint surface (Views/Items/Resume/Latest/PlayedItems/PlaybackInfo),
requests recorded for assertions. Live env-gated smoke against real
backends remains the plan for transport/auth/version coverage; the mock
leg covers Vela's own logic hermetically. Scenarios get an optional
`cleanup` hook (runner-invoked in `finally`) to stop such servers.

## Verification of the harness itself

The harness is code: each slice lands with the standard repo verification,
plus a deliberate red test (drive a scenario with a known-broken assertion)
to prove failures actually fail before trusting green runs.
