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

1. **UI driving — `tauri-driver` + WebdriverIO.** Playwright cannot attach
   to a Tauri window; Tauri's supported Linux route is `tauri-driver`
   (cargo-installed) fronting WebKitWebDriver (ships with webkit2gtk on
   Arch). Tests run the debug binary, drive the real webview (sidebar nav,
   Settings, folder pickers, context menus, All-view scrolling), and take
   screenshots into an artifacts dir for owner flip-through.
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

## Verification of the harness itself

The harness is code: each slice lands with the standard repo verification,
plus a deliberate red test (drive a scenario with a known-broken assertion)
to prove failures actually fail before trusting green runs.
