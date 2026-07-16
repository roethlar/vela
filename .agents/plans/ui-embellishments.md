# Plan: UI embellishments for v1.0.0 (graphical elements, animations, polish)

## Status
**SLICE 1 AUTHORIZED 2026-07-16 — owner go: "Work on the UI stuff."
Implement and review only the theme-correct visual-language foundation in
Slice 1. Slices 2 and 3 retain separate go gates.** The plan was queued
2026-07-10 at the bottom of the functional work; that preceding work is now
clear enough for the owner to activate UI polish. Owner rulings 2026-07-10:
slice 4 (macOS vibrancy) is OUT — "app is linux/wayland first, so
carving out a macos specific style is probably not worth it unless it's
trivial", and it is not trivial (private API + transparent window +
per-theme translucent tokens); motion personality is SUBTLE, binding:
"it should never get in the way." First item of the v1.0.0 release
track (owner, 2026-07-10): "1. UI embellishments … 2. polish docs …
3. graphics & screenshots for socials — 2 & 3 gated on 1 and anything
else that needs finishing first." This plan covers item 1 only; the
v1.0.0 ordering lives in `.agents/state.md ## Next`.

## Ground truth (frontend inventory, refreshed 2026-07-16)

Eight Svelte files + one global stylesheet; no CSS framework, component
library, or animation dependency. Svelte 5 runes are used throughout. Theming
maps semantic CSS variables across 10 dark/light themes, pre-paint applied from
localStorage in `app.html`. One font (Geist Variable). Motion is six shared
`@keyframes` in `app.css` (shimmer/fade/pop/slide×2/rise) plus per-component CSS
transitions; the hero cover-flow is a pure-CSS 3D transform driven by inline
styles. `prefers-reduced-motion` is a blanket CSS kill-switch; any new motion
must stay CSS-expressed or CSS-variable-driven so it remains covered.

`Icon.svelte` currently defines 11 inline SVG icons; only `search` is unused.
Raw UI glyphs remain in item/season rating metadata and Settings status/warning
copy. The play queue and its drawer were deleted by playlist Slice 1; no UI
slice may target or resurrect them.

Known rough spots, expressed by defect class so queued-plan line numbers cannot
rot again:

- posters pop in with no fade and some failed images vanish instead of using a
  styled placeholder;
- component styles contain hardcoded semantic accent and danger colors that
  bypass the theme catalog, including search/playlist focus and playlist/error
  states added after this plan was drafted;
- `.heroarrow` and QR presentation retain literals that must be classified as
  semantic UI color or deliberate media/quiet-zone contrast;
- play overlay/button, progress, chip, no-art, person-link, and primary-button
  styles are re-declared across components; progress alone has 6px, 5px, 4px,
  and 3px variants;
- empty states are plain text except the Welcome screen.

## Direction (and what we deliberately do NOT add)
Build on the existing hand-rolled system: Svelte 5 built-ins
(`svelte/transition`, `svelte/animate`) plus the app's own keyframe
language and tokens. **No animation framework.** Svelte Motion
(Framer-compatible, gestures/FLIP/springs) and sveltekit-view-transition
were researched and declined: this app has ~6 surfaces and one
navigation overlay; a framework buys API surface we don't need, adds a
dependency to a repo with exactly one UI dep, and its JS-driven motion
escapes the CSS reduced-motion kill-switch. The View Transitions API
was likewise declined for now: Vela's "pages" are same-document state
swaps, and support/behavior across WebKitGTK (Linux) vs WKWebView
(macOS) vs WebView2 (Windows) is uneven — a compat matrix we don't need
when scoped Svelte transitions do the same job predictably.

The one dependency-shaped candidate is desktop chrome, not web UI:
`tauri-apps/window-vibrancy` (frosted translucency behind the
sidebar/header). macOS-only in practice (Linux defers to the
compositor; Windows acrylic/mica is its own look), requires
`macos-private-api` + transparent window + token rework for translucent
surfaces. Real native-feel payoff, real cost — owner decision below.

## Slice 1 design calibration (2026-07-16)

- **Subject / audience / job:** Vela is a desktop cinema-library browser for
  people who care about their own server, artwork, and HDR playback. The UI's
  single job is to make choosing and starting a title feel immediate.
- **Color:** retain every existing theme's semantic palette. Slice 1 adds
  explicit `--accent-tint` and `--accent-glow` values to every theme rather than
  depending on `color-mix`; the standing Linux WebKitGTK 2.52.3 supports that
  function, but explicit tokens keep behavior deterministic on older WKWebView
  and WebView2 installations. No new brand color or theme-specific exception.
- **Type:** retain Geist Variable for display, body, and utility roles. This
  foundation slice fixes consistency, not typography; introducing a decorative
  face would compete with poster artwork and violate the approved non-goals.
- **Layout:** sidebar, cover-flow hero, rails, grids, and overlays remain
  structurally unchanged.
- **Signature:** the dimensional Continue Watching cover-flow and the user's
  media art remain Vela's memorable element. Shared controls and line icons
  become quieter and more coherent around it.
- **Self-critique:** a new palette, font pairing, or decorative surface would
  make this slice look more visibly "designed" but would be generic polish and
  scope expansion. The subject-specific choice is restraint: let cinema art and
  the cover-flow carry personality while removing theme and glyph drift.

## Slices (each independently shippable: commit + primary Claude code review +
independent Grok second review + version bump; ordered so later slices layer
on earlier ones)

### Slice 1 — Foundation: theme-correct states, one visual language
No new visuals; makes every later slice land evenly across all themes.
1. Sweep every component `<style>` hex/rgba literal against the theme catalog.
   Replace any value that shadows an existing accent, danger, warning, text,
   surface, border, or shadow meaning with semantic tokens. Add explicit
   `--accent-tint`/`--accent-glow` values to all 10 themes and use them for every
   focus/current-state accent. Deliberate survivors are media-art black/white
   scrims and controls, the QR white quiet zone, theme-preview swatches, and the
   grain data URI; record any other survivor before landing. Use the existing
   `--danger-*` tokens for playlist and status failures. Tokenize hero-arrow and
   QR shadows where a semantic shadow token applies.
2. Make `app.css` the single global owner of six visual primitives: play
   overlay/button, a 4px gradient progress bar, styled no-art placeholder,
   chip, person link, and primary button including hover/disabled/press states.
   Components keep only layout/context selectors and narrow generic button
   rules so they cannot override `.primary`. Preserve existing class names and
   E2E selectors.
3. Icon unification: add typed `star`, `heart`, and `alert` geometries; reuse
   existing `check`, `close`, and `chevron` for ✓, ✗, and the submenu arrow.
   Replace raw glyphs in ItemDetail, SeasonDetail, Settings, and the add-to-
   playlist menu; delete the sole unused `search` branch; make the icon name a
   literal union so an unknown name cannot silently render an empty SVG. Keep
   middot separators and prose arrows. Preserve accessible Rating, Audience
   rating, and Watched text when decorative SVGs replace readable glyphs.

### Slice 2 — Image loading polish (the single most visible fix)
1. Poster/backdrop/headshot fade-in: `opacity 0→1` CSS transition
   driven by the img `load` event (class flip), `decoding="async"`
   everywhere; skeleton shimmer already exists and now hands off
   smoothly instead of popping.
2. Unified failed-image treatment: detail/season thumbnails get the
   grid's styled text placeholder instead of vanishing.
3. DECLINED for v1.0.0 (recorded option for later): ThumbHash/BlurHash
   placeholders. Media servers don't supply hashes, so Vela would have
   to decode + hash each poster once and cache it (new disk-cache
   infra) — real work for a subtle win over fade-in; revisit if poster
   loads still feel harsh after this slice.

### Slice 3 — Motion pass (subtle, in the app's existing language)
1. Surface transitions: detail/season overlay enters with a short
   fade+rise (Svelte `transition:` or a `vela-*` keyframe — pick ONE
   mechanism and note that Svelte-injected animations are still killed
   by the reduced-motion blanket, verified at implementation), crumb
   bar slides, Settings modal keeps its pop.
2. Extend the grid's staggered `vela-rise` entrance to hub rails and
   the cast rail (bounded stagger like the grid's 14-card cap).
3. Hero cover-flow depth: `will-change: transform` on flowcards, a
   subtle ground shadow/reflection under the center card, and easing
   consistency (`--ease` token everywhere).
4. Designed empty states: give the plain-text empties (home, browse,
   search, playlists, server playlists, episode panel) the Welcome treatment — an `Icon` +
   one-line heading + muted hint, consistent spacing.
5. Micro-interactions sweep: consistent press states (`translateY(1px)`
   exists on some buttons — apply the pattern), hover affordance on
   episode rows, watched-badge pop-in (`vela-pop`).

### Slice 4 — CUT (owner ruling 2026-07-10)
macOS vibrancy via `window-vibrancy` — cut: the app is Linux/Wayland
first and the implementation is not trivial (`macos-private-api`,
transparent window, per-theme translucent surface tokens). Kept here as
the record of what was evaluated; revisit only on a new owner ask.

## Non-goals
- No animation/UI framework dependency (declined above, with reasons).
- No View Transitions API in v1.0.0.
- No theme redesign, no new font, no spacing-scale refactor (`--space-*`
  tokens would churn every file for invisible gain; new/touched code
  simply stops adding NEW magic values).
- No ThumbHash pipeline in v1.0.0 (recorded as a revisit option).
- No embedded video, no layout restructuring — this is polish, not IA.

## Verification (every slice)
- Run the canonical frontend verification set in
  `.agents/repo-guidance.md` (Verification). Run the full cross-language set if
  a slice changes `src-tauri`; do not restate the owned command enumeration
  here.
- Full E2E suite on the Linux VM — animations are exactly the kind of
  change that breaks driver waits (new transitions delay
  visibility/clickability); a suite pass is the no-regression gate.
  New motion must not require new E2E waits — if a scenario starts
  flaking, the animation is too slow or blocks interactivity.
- Reduced-motion check per slice: toggle `prefers-reduced-motion` and
  confirm new motion is inert (the `app.css` blanket rule covers CSS
  animation/transition; anything Svelte-injected gets verified
  explicitly).
- Theme sweep per slice: spot-check dark + one light theme (tokens, not
  per-theme CSS, should make the rest follow).
- The owner is not available to playtest this track (explicit 2026-07-16).
  Owner playtest is not a slice gate. Capture before/after screenshots in the
  default dark theme and one light theme; inspect keyboard focus, the reduced-
  motion state, and affected responsive layouts locally; include that evidence
  with the automated verification and review record.

Slice 1 adds two focused guards:

- a Node source-contract test, included by `npm run check`, that rejects
  hardcoded semantic accent/danger literals, raw migrated glyphs, duplicate
  component ownership of the six primitives, undefined icon names, and dead
  icon definitions;
- a Linux real-app `uifoundation` scenario that switches Dark ↔ One Light,
  verifies theme persistence/selection and computed focus styles, renders
  deterministic rating metadata to prove SVG icon replacement, and compares
  the shared progress/primary/no-art primitives across their real surfaces.

Red-prove the token, primitive-ownership/progress, and icon guard families
separately after the implementation commit, restore from the committed head,
and rerun each focused guard green before the full suite.

## Plan open review

**r1 — recorded 2026-07-16 — Claude Code 2.1.211 / `claude-fable-5` —
base `3e7b97ab16a9caf30cf3c9798ad415e0dabbfe45`, head
`306d66a007d59db9881eba6adbd3485de9ffc8e7`; verdict `findings`.**

The unprimed plan review returned three schema-valid findings; intake ADMITTED
all three because each carried exact evidence, an observable failure, and
justified severity:

1. MEDIUM — the July 10 inventory predated playlists and would send a cold
   implementer toward a deleted drawer while missing new playlist accent/danger
   literals. Addressed by refreshing current inventory, expressing the token
   work as a complete defect-class sweep, and recording deliberate literals.
2. LOW — the plan copied only part of the canonical verification list.
   Addressed by replacing it with the repo-guidance pointer and keeping only
   slice-specific visual/E2E gates here.
3. LOW — `color-mix` was an undecided compatibility assumption with no existing
   runtime proof. The Linux-first engine is WebKitGTK 2.52.3, but Slice 1 now
   deterministically chooses explicit per-theme tint/glow tokens so older
   cross-platform webviews do not rely on the function.

The revisions also incorporate a separate current-tree audit: one dead icon,
the deleted queue, exact shared-style drift, playlist semantic-color drift, and
a focused source plus real-app guard. A fresh Claude plan `openreview` is
required on the revised head before code begins.

## Decisions (resolved by owner 2026-07-10)
1. **Vibrancy: OUT** — Linux/Wayland-first app; macOS-specific styling
   only if trivial, and it is not. Slice 4 cut.
2. **Motion personality: SUBTLE, binding** — "it should never get in
   the way." Durations 100–300ms, small distances, existing `--ease`,
   nothing that delays interactivity or requires new E2E waits.
3. **Poster placeholder: fade-in only** (proposed default stands;
   ThumbHash pipeline stays a recorded later option).

## Research trail (2026-07-10)
Svelte 5 built-ins and ecosystem surveyed: svelte/transition + easing
(docs), Svelte Motion (motion.svelte.page — capable, declined as
overkill/reduced-motion-hostile), @formkit/auto-animate + @neodrag
(Svelte 5 attachments — no current drag/list-reorder need),
sveltekit-view-transition + WebKit support caveats, ThumbHash vs
BlurHash (thumbhash wins on fidelity/aspect/alpha; both need client
decode + a hash source we'd have to build), tauri-apps/window-vibrancy
(v2-compatible; macOS needs `macos-private-api` + transparent window;
Linux compositor-dependent). A Svelte 5 `animate:flip` regression is on
record upstream (sveltejs/svelte#13591) — avoid `animate:` directives;
nothing in the slices needs FLIP.
