# Plan: UI embellishments for v1.0.0 (graphical elements, animations, polish)

## Status
**SLICE 3 AUTHORIZED 2026-07-16; CURRENT-TREE PLAN REFRESH AND PRIMARY CLAUDE
OPEN REVIEW ACTIVE.** Slice 1 is complete at Vela 0.1.53 and Slice 2 at Vela
0.1.55. The owner's "go on Slice 3" activates only the subtle motion and
designed-empty-state pass below. Code remains gated until the refreshed Slice 3
contract receives a clean unprimed Claude `openreview`. The plan was queued
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

`Icon.svelte` defines 13 typed inline SVG icons, all used. Slice 1 removed the
semantic-color, duplicate-primitive, and raw-glyph drift; Slice 2 gave every
media image a fixed failure underlay and source-safe reveal. The play queue and
its drawer were deleted by playlist Slice 1; no UI slice may target or
resurrect them.

The remaining Slice 3 rough spots, refreshed against Vela 0.1.55:

- item and season detail surfaces plus their mount-time crumb bars appear
  without an entrance cue; Settings already owns the correct fade/pop and must
  not be changed;
- grid and Home hub cards already share one bounded poster stagger, but cast
  cards do not; the reduced-motion blanket zeros duration but not animation or
  transition delay, so staggered cards can remain in their backwards-filled
  hidden pose for up to 308ms;
- cover-flow transforms already use `--ease`, but the cards lack a compositor
  hint and the centered image has no visual ground; the QR hover retains the
  only non-deliberate bare `ease` timing;
- press feedback exists only on primary buttons, episode rows have an abrupt
  tint-only hover, and the grid watched badge appears without the existing
  `vela-pop` cue;
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
  and WebView2 installations. It also adds `--danger-solid`,
  `--danger-solid-hover`, and `--on-danger` for high-emphasis destructive
  actions. Existing `--danger-text` remains the general danger foreground;
  `--danger-bg` and `--danger-border` remain the tinted secondary/status
  treatment. No new brand color or theme-specific exception.
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

## Slice 2 design calibration (2026-07-16)

- **Signature and hierarchy:** artwork remains the visual subject. Loading
  polish may soften its arrival but must add no competing ornament, blur,
  scaling, gradient veil, or new color.
- **Motion:** one 180ms opacity transition using `--ease`; no delay and no
  transition of layout, transform, filter, or fallback content. Images remain
  non-blocking and clickable throughout. The global reduced-motion rule makes
  the transition effectively immediate without changing final visibility.
- **Loading surface:** existing fixed dimensions and aspect ratios remain the
  layout contract. Data-loading skeletons remain unchanged; they do not live
  for each image request. A frame-local no-art or film-icon underlay is what
  stays visible while an individual image loads and what remains after failure.
- **Functional-image exception:** the Plex authorization QR is not media art.
  It gets asynchronous decoding for consistency but no fade or fallback layer;
  its white quiet zone and meaningful alternative text remain unchanged.
- **Accessibility:** media art is decorative wherever the surrounding button,
  title, or cast metadata already names the item. Underlay text/icons are
  `aria-hidden`; the QR keeps its meaningful alternative text. Opacity never
  removes an element's layout box or changes the control's accessible name.
- **Self-critique:** a shimmer per frame or generated blurred placeholder would
  be more conspicuous but would either invent motion around every poster or add
  a cache pipeline. The restrained load/fallback layer directly fixes the pop
  and blank-frame defects while keeping the art dominant.

## Slice 3 design calibration (2026-07-16)

- **Subject / audience / job:** Vela remains a desktop cinema-library browser
  for people choosing from their own media servers. Motion must clarify a
  surface change or a direct manipulation; empty copy must point toward an
  existing way forward.
- **Color and type:** retain the complete semantic theme catalog and Geist
  Variable. This slice adds no palette, font, gradient veil, or decorative icon
  family. Media absence uses Vela's existing film-strip vocabulary; curation
  absence uses the existing playlist icon.
- **Layout:** preserve the sidebar, cover-flow, rails, grids, detail split,
  playlist controls, and responsive breakpoints. One shared empty-state
  component owns icon/heading/hint presentation while each surface continues
  to own positioning and all existing controls.
- **Motion signature:** a quiet "projector settle": detail surfaces rise and
  fade once when mounted, their crumb bar arrives from above, and the centered
  Continue Watching card gains one low, diffuse ground shadow. This shadow is
  the slice's single aesthetic risk and reinforces Vela's existing cinematic
  stage rather than decorating unrelated surfaces.
- **Restraint:** do not add a second animation to Home hub cards—the shared
  poster already gives both grids and hubs the bounded entrance stagger. Only
  the previously static cast cards join that language. Settings keeps its
  existing fade/pop. Watched pop applies only to the grid badge, not every
  watched chip or every pre-watched episode in a list.
- **Self-critique:** animating every rail, inventing search/person artwork, or
  adding calls to action to every empty state would look more conspicuous but
  would make a media browser feel templated and busy. Reusing two existing
  icons, one shared keyframe language, and the controls already on screen keeps
  the artwork and cover-flow as Vela's identity.

## Slices (each independently shippable: commit + primary Claude code review +
independent Agy/Gemini 3.1 Pro second review + version bump; ordered so later
slices layer on earlier ones)

### Slice 1 — Foundation: theme-correct states, one visual language
No new visuals; makes every later slice land evenly across all themes.
1. Sweep every component `<style>` hex/rgba literal against the theme catalog.
   Replace any value that shadows an existing accent, danger, warning, text,
   surface, border, or shadow meaning with semantic tokens. Add explicit
   `--accent-tint`/`--accent-glow` values to all 10 themes and use them for every
   focus/current-state accent. Deliberate survivors are media-art black/white
   scrims and controls, the QR white quiet zone, theme-preview swatches, and the
   grain data URI; record any other survivor before landing. Use the existing
   `--danger-text` for failure copy and text-only destructive actions; combine
   it with `--danger-bg`/`--danger-border` for tinted failure or secondary
   destructive controls such as Settings Disconnect/Remove/Clear. Define
   `--danger-solid`, `--danger-solid-hover`, and `--on-danger` in all 10 themes
   only for the high-emphasis solid destructive action, such as a confirmed
   playlist deletion. Preserve this existing hierarchy; do not restyle every
   destructive control as solid and do not allowlist a destructive literal.
   Tokenize hero-arrow and QR shadows where a semantic shadow token applies.
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
1. Add one DOM-preserving Svelte action in `src/lib/imageReveal.ts`. It accepts
   the resolved source URL, removes its loaded class on initialization and a
   source change, adds it only after a successful `load` with nonzero natural
   width, removes it on `error`, and performs a queued
   `complete && naturalWidth > 0` check so memory/disk-cached images whose load
   event preceded attachment cannot remain transparent. Clean up both event
   listeners. Keep the actual `<img>` in each owning component; a wrapper
   component would break the existing scoped selectors.
2. Add one global `.image-reveal` primitive in `app.css`: opacity zero by
   default, opacity one in the loaded state, and only a 180ms opacity transition
   using `--ease`. Apply it and the action to all nine media-art `<img>`
   templates: grid/rail, Continue Watching, both playlist views, cast headshot,
   detail backdrop/poster, episode-row still, and selected-episode still. Pass
   the resolved URL to the action so in-place enrichment or selection changes
   reset the old loaded state. Preserve the current eager/lazy choices: this
   slice changes decoding and presentation, not fetch priority.
3. Put the appropriate fixed-frame fallback underneath each media image rather
   than hiding a broken element or retaining component-wide failure flags.
   Grid/rail, Continue Watching, main detail poster, episode-row thumbnail, and
   selected-episode still use the shared title-bearing `.noart`; cast headshots
   and the two 3rem playlist thumbnails use the existing film icon; the
   decorative detail backdrop falls back to its existing themed surface with
   no text. Make the image an absolute cover layer within the already bounded
   frame. Remove `failedPosters`, `posterFailed`, `stillFailed`, and every
   inline `display:none` / `visibility:hidden` image-error handler. This also
   prevents one failed URL from poisoning another surface or an enriched
   replacement URL; navigation can retry an image after a server recovers.
4. Add `decoding="async"` to all ten literal runtime `<img>` templates,
   including the QR. The QR is the explicit sole exception from the action,
   loaded class, cover layer, and fallback behavior.
5. DECLINED for v1.0.0 (recorded option for later): ThumbHash/BlurHash
   placeholders. Media servers don't supply hashes, so Vela would have
   to decode + hash each poster once and cache it (new disk-cache
   infra) — real work for a subtle win over fade-in; revisit if poster
   loads still feel harsh after this slice.

#### Slice 2 guard and real-app contract

1. Add a focused Node contract wired into `npm run check`. Import the action
   directly and prove an ordinary load reveals, an error/source update hides,
   an already-complete cached image reveals, a later cached source reveals
   after the queued check, and destroy detaches listeners. Its source half
   inventories every literal runtime `<img>`: all ten require async decoding,
   exactly the nine media images require the shared action/class/cover layer,
   the QR must remain the sole no-fade exception, each media frame must retain
   its specified underlay, and the old inline hide handlers/failure states are
   forbidden.
2. Extend the existing Jellyfin mock serializer with opt-in Primary, Backdrop,
   and series-primary image tags and add the production URL shapes under
   `/Items/{id}/Images/...`. The mock image controller records arrival and
   response separately, can hold a named path until explicit release, can
   return a named 404, and releases all pending responses before close. Serve a
   tiny deterministic valid image with explicit content type/length and no
   timers; a server-held response provides the positive unloaded-state witness.
3. Add a focused Linux `imagepolish` real-app scenario. On a visible first card,
   wait for the held request, assert fixed geometry, incomplete image, opacity
   zero, and the configured opacity-only transition; release it, then
   condition-wait for nonzero natural size, loaded state, and opacity one. Open
   detail and repeat for a separately held backdrop. A distinct 404 poster must
   leave its title-bearing detail fallback visible. Drill a mock show into its
   season: a successful episode image must reveal in both row and panel, while
   a failed episode must leave title-bearing `.noart` in both places, never a
   visibly broken/hidden blank. Assert the mock response witnesses before every
   failure claim.
4. The scenario captures the held and loaded states in Vela Dark, then settled
   success/failure states in One Light, restoring Dark in `finally`. A second
   focused run uses the existing throwaway GTK reduced-motion preference,
   first proves the WebKit media query is active, then repeats the held/release
   path and requires every image transition duration to be at most 0.01ms.
   Assertions wait on server/DOM predicates, never animation time or a sampled
   mid-transition frame.
5. Jellyfin/Emby intentionally lack rich cast detail, so hermetic E2E cannot
   render Plex headshots. The complete source contract plus Svelte compilation
   owns that surface; do not add a production IPC test hook or make live Plex a
   gate merely to feed test metadata.

Red-prove these Slice 2 guard families independently after the implementation
commit, restoring from the committed head after each injection: successful
load/loaded-class behavior; cached and changed-source reset behavior; complete
async-decoding/media-surface inventory; failure-underlay taxonomy (including an
episode thumbnail); and reduced-motion suppression. Rerun each focused guard
green before the canonical frontend and full Linux suites.

### Slice 3 — Motion and empty-state pass (subtle, in Vela's existing language)

1. **CSS-only surface choreography.** Reuse `vela-rise` for a 200ms entrance
   on the `ItemDetail` and `SeasonDetail` roots and `vela-slide-down` for a
   160ms mount-time entrance on each page-level `.crumbs` branch. Do not key or
   replay the crumb animation for an in-place crumb-array update. Keep the
   Settings scrim/panel's existing `vela-fade`/`vela-pop`; do not add a second
   mechanism or import a Svelte transition. Strengthen the global
   `prefers-reduced-motion` block to zero animation and transition delays as
   well as durations, because backwards-filled staggered cards otherwise stay
   hidden during their original delay.
2. **One entrance language and one grounded stage.** Preserve and guard the
   current poster snippet's `Math.min(i, 14) * 22ms` delay: the same snippet
   already animates browse grids and Home hub cards, so no rail animation is
   added. Add the same bounded delay plus `vela-rise` only to cast cards. Add
   `will-change: transform` to the at-most-nine rendered flowcards and one
   pointerless, low-z `.flow::after` radial ellipse using the existing shadow
   token beneath the centered card. Keep the existing reactive inline 3D
   transforms and `--ease` timings. Replace the QR hover's bare `ease` with
   `var(--ease)`; linear shimmer and spinner animations remain deliberate
   exceptions.
3. **One shared empty-state primitive.** Add `src/lib/EmptyState.svelte` with
   one decorative typed `Icon`, one `h2`, one muted hint, and an optional
   polite-status role. It owns only typography, spacing, and presentation;
   parent surfaces own centering/in-flow placement and retain every existing
   Back, crumb, sort, create, rename, delete, search, sidebar, and Settings
   control. Migrate the Welcome state to this primitive so it is the actual
   shared reference rather than a parallel component-local style.
4. **Exact full-surface empty taxonomy.** Use the film icon for media absence
   and the playlist icon for curation absence, with these headings and hints:
   - Home with available library navigation: **No titles on Home yet** / Choose
     a library from the sidebar to start browsing.
   - Authenticated Home with no section or type navigation: **No libraries
     found** / Check the connected server, then use Refresh libraries.
   - Settled browse/container: **No titles in this view** / Go back, refresh
     libraries, or choose another library.
   - Search: **No matches for “{searchTerm}”** / Check the spelling or try a
     broader search. This result alone uses the component's polite-status role.
   - Person browse: **No titles found for {personView.name}** / Go back to keep
     browsing.
   - Vela playlist index: **No playlists yet** / Name one above, then add
     titles from their context menus. Render it only when the list load did not
     fail; a failed request must not simultaneously claim an authoritative
     empty result.
   - Empty Vela playlist: **This playlist is empty** / Use a title's context
     menu to add it here.
   - Empty server playlist: **This server playlist is empty** / Add videos on
     {playlist.sourceName}, then reopen it here.
   - Successfully loaded zero-episode season: **No episodes in this season** /
     Go back and choose another season. If episodes exist but selection is
     absent, use **Choose an episode** / Select one from the list to see details
     and playback options.
   Keep loaders (`aria-busy` and the episode skeleton), list failures, the
   add-to-playlist menu's compact absence line, server-playlist sidebar
   metadata, and Settings' inline no-server message outside the full empty
   primitive. Split the season panel's current `!selected` conflation so neither
   loading nor `listError` renders invitational empty copy. Correct the nearby
   Home comment that still cites removed local/SMB/SSH sources while touching
   that branch; do not change its routing behavior.
5. **Composable micro-interactions.** Give every enabled button the same 1px
   press feedback through the individual `translate` property, which composes
   with poster, flowcard, hero-arrow, and QR `transform` geometry; remove the
   duplicate primary-only transform implementation. Add a 2px horizontal
   `translate` plus short `background`, `border-color`, and `translate`
   transition to the already tinted episode-row hover. Apply `vela-pop` only to
   `.watchedbadge`, not to static watched chips or episode marks. The reduced-
   motion block must suppress the new individual-translate motion as well as
   every new duration and delay.

#### Slice 3 guard and real-app contract

1. Add `tests/ui-motion.test.mjs` to `npm run check` with four source/CSS
   contracts: CSS-only surface/crumb/Settings motion plus the complete reduced-
   motion kill-switch; existing hub and new cast bounded stagger plus
   cover-flow grounding/easing; composable press, episode hover, and grid-badge
   pop; and the exact shared empty-state structure/inventory, including failure
   and loading exclusions. The contract may name deliberate linear shimmer and
   spinner timing, but no other bare easing exception.
2. Add one hermetic Linux `uimotion` scenario. Seed populated movie/show
   navigation, a zero-item library, a zero-episode season, an empty server
   playlist, and a second connected source with no views. Use mock state plus
   an ordinary refresh to make Continue Watching appear only after the initial
   Home empty assertions. Exercise the reachable Home/no-library, browse,
   no-match search, Vela playlist root/new empty detail, server playlist, and
   zero-episode states. Person and rich cast remain source-contract-only because
   Jellyfin/Emby do not expose Plex's rich person detail in this harness; do not
   add a production test hook.
3. In the normal run, assert computed animation names, 100–300ms durations,
   `--ease` timing, bounded card delays, flowcard `will-change`, the grounded
   pseudo-element, and parsed individual `translate` support. Wait only on
   server/DOM predicates, never animation sleeps or sampled mid-animation
   frames. Capture dark Home/detail and One Light empty-state screenshots and
   inspect affected keyboard focus and existing responsive rules.
4. Run the same focused scenario with the existing throwaway GTK
   reduced-motion preference. First prove the WebKit media query is active,
   then require every rendered Slice 3 animation/transition duration and delay
   to be at most 0.01ms and the new individual translate to be suppressed.
5. Red-prove each claimed family after the implementation commit, restoring
   from the committed head after every injection: detail entrance, season
   entrance, mount-time crumbs, preserved Settings pop, reduced-motion delay,
   existing hub cap, cast animation/cap, flowcard hint, ground shadow, easing,
   global press composition, episode hover, watched-badge pop, and each empty-
   state category/loading-failure exclusion. Separately inject an unsuppressed
   new animation and transition into the Linux reduced-motion run. Rerun each
   focused guard green, then the canonical frontend checks and complete Linux
   real-app suite.

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
  verifies theme persistence/selection and computed focus styles, proves the
  compiled Settings status/warning SVGs, and compares the shared progress,
  primary-button, play-control, and no-art primitives across real Home,
  playlist, Settings, grid, and sparse Jellyfin detail surfaces. The source
  contract owns the star/heart rating-icon and readable-label proof: Jellyfin
  intentionally has no rich detail implementation, and Tauri 2 exposes
  `__TAURI_INTERNALS__.invoke` as non-writable and non-configurable, so feeding
  fake rich metadata would require a production test hook. Slice 1 does not add
  one merely to satisfy the harness.

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

**r2 — recorded 2026-07-16 — Claude Code 2.1.211 / `claude-fable-5` —
base `306d66a007d59db9881eba6adbd3485de9ffc8e7`, head
`b2dea9276d9dac40a760bfc60cb3e28ec8128073`; verdict `findings`.**

The unprimed revised-plan review returned one LOW finding, ADMITTED: existing
danger tokens describe subtle failure banners, while the playlist Delete action
needs a solid destructive fill. The previous plan simultaneously required
tokenization and prohibited an honest literal survivor, leaving no correct
implementation. Addressed by defining explicit per-theme solid/hover/on-danger
tokens and keeping the banner trio semantically separate. A fresh Claude plan
`openreview` is required on this revised head before code begins.

**r3 — recorded 2026-07-16 — Claude Code 2.1.211 / `claude-fable-5` —
base `b2dea9276d9dac40a760bfc60cb3e28ec8128073`, head
`ec686439badb3ead9bac74df9d6d8eaf5aba0201`; verdict `findings`.**

The unprimed revised-plan review returned two LOW findings, both ADMITTED. The
r2 taxonomy omitted text-only destructive actions, and it contradicted the
existing tinted Settings Remove/Disconnect controls. Addressed by making
`--danger-text` the general danger foreground, retaining background/border for
tinted failure and secondary-destructive treatments, and reserving the new
solid trio for high-emphasis confirmed destruction. This preserves the current
visual hierarchy while removing every semantic literal. A fresh Claude plan
`openreview` is required on this revised head before code begins.

**r4 — 2026-07-16T17:15:35Z — Claude Code 2.1.211 /
`claude-fable-5` — base `ec686439badb3ead9bac74df9d6d8eaf5aba0201`,
head `a5e818c1d67f9b3c74614770e17625575449616a`; verdict `clean`.**

The unprimed pass returned the exact SHAs, `verdict: clean`, and no findings.
Plan review is converged. The owner's Slice 1 go is active; implementation may
begin without reopening Slices 2–3.

**Slice 2 r1 — 2026-07-16T22:30:32Z — Claude Code 2.1.211 /
`claude-fable-5` — base `90ed3c84fe477eda4deb23c769331c4cf67b8d66`,
head `a8c31f82ee1218fe512225caae16989fe0bea3e6`; verdict `clean`.**

The unprimed goal-first review of the refreshed image-polish contract returned
the exact SHAs, `verdict: clean`, and no findings. A preceding envelope was
discarded fail-closed because the orchestrator had supplied an incorrectly
expanded head coordinate; the one allowed schema retry used the actual full
SHA and is the recorded result. Slice 2 plan review is converged and the
owner's go is active; implementation may begin. Slice 3 remains gated.

## Slice 1 implementation record

Implementation landed at `fe46850`, followed by focused cascade and test
integration repairs through `c1c4db4`; `0ce3629` versioned the independently
shippable slice as 0.1.53. The token, shared-primitive/progress, icon, and
generic-button-cascade guard families were each proven red for the intended
reason and restored green. Canonical frontend verification and a locked Rust
1.89 compile passed at 0.1.53. Fresh-build Linux evidence includes normal and
reduced-motion focused runs, the playlist selector integration, and a final
25/25 real-app suite. Final dark Home and One Light Settings/detail screenshots
were inspected; keyboard focus, shared computed styles, and reduced-motion
behavior were covered without an owner playtest.

Primary Claude Code 2.1.211 (`claude-fable-5`) and independent Grok 0.2.101
(`grok-4.5`) both accepted exact reviewed head `969f06a` against base
`d96eb464`, each with `guard_confirmed:true`, independent red/restored-green
proof, and no material comments. The fail-closed record is
`.agents/review/findings/ui-s1.md`. At Slice 1 close, Slices 2–3 were still
unauthorized; Slice 2 was activated separately on 2026-07-16 as recorded in the
current status, while Slice 3 remains gated.

## Slice 2 implementation record

Implementation landed at `830cabd`; the first real-app run exposed and fixed a
nested season-card selector assumption at `c22a07e`; `49c0dc9` versioned the
independently shippable slice as 0.1.55. One source-aware Svelte action now
owns successful, failed, cached, changed-source, stale-work, and cleanup
behavior. Nine media-art images reveal over fixed title/icon/surface underlays;
the Plex QR remains the sole no-fade exception while all ten runtime images use
asynchronous decoding.

The ordinary-load, changed-source/cache, image inventory, failure-underlay,
opacity/duration, and reduced-motion guard families were independently proven
red for the intended reasons and restored green. Canonical frontend checks and
the complete MSRV/stable/clippy/test/audit Rust set passed at 0.1.55 with zero
known vulnerabilities. Fresh-build focused normal and reduced-motion Linux
runs passed; the reduced-motion assertion was proven non-vacuous with an
injected 180ms exception; the final real-app suite passed 26/26. Six exact-head
dark/light held, loaded, and failed-state screenshots were inspected without an
owner playtest.

Primary Claude Code 2.1.211 (`claude-fable-5`) and the owner-authorized
independent Agy 1.1.3 (`Gemini 3.1 Pro (High)`) both accepted exact reviewed
head `0ccb269` against base `e98220e` with `guard_confirmed:true`, independent
red/restored-green proof, and no material comments. Grok's failed attempts were
discarded fail-closed and remain in the record rather than counting as review.
Exact evidence: `.agents/review/findings/ui-s2.md`. Slice 3 remains
unauthorized.

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
