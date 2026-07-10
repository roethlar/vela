# Plan: UI embellishments for v1.0.0 (graphical elements, animations, polish)

## Status
**QUEUED 2026-07-10 — decisions resolved, parked at the BOTTOM of the
queue on owner order ("queue first, v1 polish goes to the bottom");
per-slice go still required when picked up.** Owner rulings 2026-07-10:
slice 4 (macOS vibrancy) is OUT — "app is linux/wayland first, so
carving out a macos specific style is probably not worth it unless it's
trivial", and it is not trivial (private API + transparent window +
per-theme translucent tokens); motion personality is SUBTLE, binding:
"it should never get in the way." First item of the v1.0.0 release
track (owner, 2026-07-10): "1. UI embellishments … 2. polish docs …
3. graphics & screenshots for socials — 2 & 3 gated on 1 and anything
else that needs finishing first." This plan covers item 1 only; the
v1.0.0 ordering lives in `.agents/state.md ## Next`.

## Ground truth (frontend inventory, 2026-07-10)
Five Svelte files + one global stylesheet; no CSS framework, no
component library, no animation dependency. Svelte 5 runes throughout.
Theming is fully tokenized (~30 semantic CSS variables × 10 dark/light
themes, pre-paint applied from localStorage in `app.html`). One font
(Geist Variable). Motion today is six shared `@keyframes` in `app.css`
(shimmer/fade/pop/slide×2/rise) + per-component CSS transitions; the
hero cover-flow is a pure-CSS 3D transform driven by inline styles.
`prefers-reduced-motion` is a blanket CSS kill-switch (`app.css:372`) —
any new motion must stay CSS-expressed (or CSS-variable-driven) so the
blanket rule keeps covering it. Icons: one inline-SVG `Icon.svelte`
(10 Lucide-style icons, 2 unused), with raw emoji glyphs (★ ♥ ✓ ✗ ⚠)
still used in detail/settings surfaces.

Known rough spots (inventory findings the slices below target):
posters pop in with no fade; failed thumbs vanish (`visibility:hidden`)
instead of showing the grid's styled placeholder; a few interactive
states hardcode the DARK theme's accent rgba (search glow
`+page.svelte:1596`, drawer row `:2294`) and break under the other 9
themes; `.heroarrow`/QR hardcode literal colors; play-overlay/progress
bar/chip styles are re-declared per component with drifted values
(5px/4px/3px progress heights); empty states are plain text except the
Welcome screen.

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

## Slices (each independently shippable: commit + reviewloop + version
bump + owner playtest; ordered so later slices layer on earlier ones)

### Slice 1 — Foundation: theme-correct states, one visual language
No new visuals; makes every later slice land evenly across all themes.
1. Tokenize the hardcoded accent states with
   `color-mix(in srgb, var(--accent) N%, transparent)` (or new
   `--accent-glow`/`--accent-tint` tokens): search focus glow, drawer
   current-row tint; replace `.heroarrow`/QR literals with tokens where
   a token exists (QR stays white by design — quiet-zone contrast).
2. De-duplicate the drifted shared styles into `app.css` utility
   classes (or one shared block): play overlay, progress bar (one
   height), chips, no-art placeholder, person link, primary button.
3. Icon unification: add `star`, `heart`, `alert` (and whatever ✓/✗
   need) to `Icon.svelte`; replace the emoji glyphs in
   ItemDetail/SeasonDetail/Settings; delete or wire the 2 dead icons.
   Keep the `·` middot separators (typography, not iconography).

### Slice 2 — Image loading polish (the single most visible fix)
1. Poster/backdrop/headshot fade-in: `opacity 0→1` CSS transition
   driven by the img `load` event (class flip), `decoding="async"`
   everywhere; skeleton shimmer already exists and now hands off
   smoothly instead of popping.
2. Unified failed-image treatment: detail/season/queue thumbs get the
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
   search, queue, episode panel) the Welcome treatment — an `Icon` +
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
- `npm run check` + `npm run build`; full CI set when `src-tauri`
  changes (slice 4 only).
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
- Owner playtest per slice — visual polish is owner-judged; screenshots
  in the PR-style summary for before/after.

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
