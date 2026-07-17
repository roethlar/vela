# oled-1: True-black media-first theme

**Severity**: MEDIUM — the existing near-black theme still illuminates OLED
pixels across most of the window and does not let media art dominate a dark
room.
**Status**: Implementation and guard proof complete; primary Claude review
pending at Vela 0.1.57
**Branch**: `main` (owner-approved direct implementation)
**Base**: `d1fd8eed7071ed6dd18b4a8917aa428e031a82b5`
**Implementation**: `6029dbf1ce0551fb5552a1b37faba3ef4517dbfe`
**Version**: 0.1.57

## Evidence

Vela Dark uses `#0b0d10`, a radial background glow, and full-window film grain.
The owner requested a selectable OLED theme with a literal `#000000` background
and dimmer chrome while video cards and the Continue Watching carousel retain
their media brightness.

## Predicted observable failure

An OLED screen continues emitting light across supposedly black regions; a
global opacity or brightness treatment makes posters and the carousel recede
along with navigation; or the theme works only after Settings applies it and
flashes/falls back on the next app launch.

## What

Add an `OLED Black` theme whose canvas is truly black, whose non-media semantic
tokens are deliberately lower-luminance, and whose only scoped visual
exceptions remove ambient background treatment rather than altering media.

## Approach

`app.css` adds a complete OLED semantic palette, removes the body gradient, and
hides film grain only under `data-theme="oled"`. The Settings catalog and
pre-paint whitelist expose and persist the same id. Chrome dims through semantic
tokens; there is no root opacity/filter and OLED-scoped selectors cannot target
posters or the carousel. The existing real-app UI-foundation scenario asserts
computed canvas, token, grain, filter, and opacity values and captures the OLED
Home state.

## Files changed

- `src/app.css` — literal-black palette and ambient-treatment suppression.
- `src/lib/Settings.svelte`, `src/app.html` — picker, persistence, and
  pre-paint registration.
- `tests/ui-foundation.test.mjs` — synchronized catalog and exact OLED source
  contract.
- `tests/e2e/scenarios/uifoundation.mjs` — computed WebKit contract and
  screenshot.
- Six version surfaces — Vela 0.1.57.

## Guard proof

- Changing only `--bg` from `#000000` to `#010101` failed the literal-black
  assertion while the other five UI-foundation tests remained green.
- Restoring a radial body gradient failed the exact ambient-background
  assertion.
- Adding an OLED-scoped `.flowcard { opacity: 0.8; }` failed the no-media-
  override selector contract.
- Removing OLED separately from Settings and from `app.html` failed the picker
  and first-paint catalog assertions, respectively.
- In the real Linux app, forcing `.flowcard.center` to opacity `0.8 !important`
  failed the computed-style witness exactly at `centerOpacity: '0.8'` while the
  literal-black canvas, absent grain, tokens, and brightness remained correct.
  Restoring the committed stylesheet and rebuilding returned the scenario 1/1
  green.
- Restored local verification: pinned Node 26.5.0/npm 12.0.1, clean `npm ci`,
  zero npm vulnerabilities, 18/18 focused Node tests, zero Svelte diagnostics,
  production build, Rust 1.89 and stable checks, clippy with warnings denied,
  140/140 Rust tests, and zero Rust vulnerabilities.
- The normal real-app `uifoundation` run passed with a fresh binary. Its
  1280x800 OLED Home screenshot was inspected: black canvas and receded chrome
  are visually distinct, with the carousel remaining the focal plane.

## Coder dispute (if any)

None.

## Known gaps

The hermetic scenario's media item intentionally uses its title underlay rather
than network artwork. The exact source boundary plus real computed carousel
filter/opacity own the no-dimming requirement for actual images. No owner
playtest is required before code review.

## Reviewer comments

Pending Claude Fable 5 review of the exact pinned range with an independently
executed red/restored-green guard proof.
