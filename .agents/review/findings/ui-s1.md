# ui-s1: Theme-correct visual-language foundation

**Severity**: MEDIUM — non-default themes could retain Vela Dark focus and
danger colors while the same controls visibly changed weight, spacing, icon
style, or progress thickness between surfaces.
**Status**: In progress — primary Claude review pending
**Branch**: `main` (approved Slice 1 implementation)
**Commit**: `fe46850fe9d117c3745d5067a61e29b1ee058f27` plus focused integration fixes
through `c1c4db4dee0eb196a7ccd480e2945e826ee23d07`

## Evidence

At base `d96eb46`, component styles still owned a literal Vela Dark focus
glow, hardcoded playlist failure/destructive colors, four progress heights,
and duplicated play controls, placeholders, chips, person links, and primary
buttons. Item/season ratings and Settings statuses also used raw text glyphs,
while `Icon.svelte` accepted arbitrary names and retained a dead branch. The
authorized goal and design boundaries are recorded in
`.agents/plans/ui-embellishments.md` Slice 1.

## Predicted observable failure

Switching themes could leave focus and destructive states colored for Vela
Dark. The same playback state could render a 3px, 4px, 5px, or 6px progress
track depending on the surface; shared buttons and placeholders could drift by
component cascade; migrated icons could regress to platform-dependent glyphs or
silently disappear. Reduced-motion users must not inherit active transitions
from the consolidated primitives.

## What

Make all semantic focus, selected, failure, and destructive states theme-owned;
give six recurring visual primitives one global owner; and make all remaining
UI icons typed inline SVGs with readable text retained for assistive
technology.

## Approach

Every theme defines explicit accent tint/glow and solid-danger tokens. Global
CSS owns play controls, the 4px gradient progress track, no-art placeholder,
chip, person link, and primary-button states; components retain only layout or
interaction context and exclude primary buttons from generic shorthand rules.
`Icon.svelte` now has a closed literal-name union with star, heart, and alert
geometry, and raw UI glyph call sites use the shared SVG component. A source
contract and Linux real-app scenario guard ownership, themes, focus,
accessibility, compiled icons, normal/reduced motion, and cross-surface computed
styles.

## Files changed

- `src/app.css` — all-theme tokens and global visual primitives.
- `src/lib/Icon.svelte`, `ItemDetail.svelte`, `SeasonDetail.svelte`,
  `Settings.svelte`, `PlaylistsView.svelte`, `ServerPlaylistView.svelte`, and
  `src/routes/+page.svelte` — typed icon use, semantic tokens, and component
  cascade narrowing.
- `tests/ui-foundation.test.mjs`, `package.json` — fail-closed source contract
  in the canonical frontend check.
- `tests/e2e/scenarios/uifoundation.mjs`, `playlistedit.mjs` — real WebKit
  computed-style/theme/reduced-motion coverage and the SVG-chevron interaction
  selector.

## Guard proof

- Restoring the literal gold search glow failed the semantic-color contract at
  `src/routes/+page.svelte`; restoring the committed token returned it green.
- Restoring `.flowcard .progress { height: 6px; }` failed global primitive
  ownership; restoring the committed 4px owner returned it green.
- Replacing the audience SVG with `♥` failed the typed/dead/raw icon contract;
  restoring the committed heart icon returned it green.
- The first Linux run exposed a scoped `button { font: inherit }` override that
  stripped global primary weight. Extending the contract to cover generic font
  shorthand failed on that exact rule; narrowing the component cascade returned
  it green.
- The reduced-motion E2E expectation failed under normal GTK settings, then
  passed with only the scenario's throwaway GTK config disabling animations;
  the machine setting remained unchanged.
- Restored verification: exact Node/npm assertion, clean `npm ci`, zero npm
  vulnerabilities, canonical frontend check/build, focused normal and reduced
  Linux real-app scenarios, and the final full Linux suite 25/25. Dark Home and
  One Light Settings/detail screenshots were inspected at the final head.

## Coder dispute (if any)

None.

## Known gaps

The owner is unavailable to playtest this track, by explicit ruling. Jellyfin
intentionally lacks rich item detail, and Tauri freezes its invoke bridge, so
the real-app scenario proves compiled Settings icons and sparse detail
primitives while the source contract owns star/heart and readable-label
coverage. No production-only test hook was added.

## Reviewer comments

Pending primary Claude Fable 5 review, followed by an independent Grok second
review.
