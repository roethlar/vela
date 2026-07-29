# Plan: Independent library sort direction

## Status

**OWNER-CONFIRMED 2026-07-28 after landing on the owner's explicit `go`
(`c0d1412`, 1.0.59).**

Owner requirement, recorded 2026-07-28: library sorting needs an
ascending/descending option.

The owner settled the control contract as a direction-neutral sort-field
dropdown plus an adjacent boxed up/down arrow that toggles direction. Up means
ascending, down means descending, and changing either dimension preserves the
other. The implementation, five independent guard mutations, canonical local
verification, and focused/full Linux real-app coverage are complete. No product
decision or implementation gate remains open. The owner playtested the landed
control, confirmed it working on 2026-07-28, and authorized publication of the
1.0.59 GitHub release.

## Goal

Make every currently offered library sort reversible without changing the
existing request token, persistence schema, source routing, merged-listing
snapshot identity, or default ordering.

Acceptance requires all of the following:

1. Every sort field available in a source library can be requested in both
   ascending and descending order.
2. The merged `All` view offers both directions for every field it can already
   sort locally; source-only fields remain absent there.
3. The direction is visible as a boxed up/down arrow and remains
   keyboard-operable and text-labelled for assistive technology rather than
   encoded only in a field label.
4. Changing the field preserves the selected direction; changing the direction
   preserves the field.
5. A library with no saved preference still opens at `titleSort:asc`.
6. Existing saved values remain valid without migration. A newly selected
   field/direction pair persists as the same `<field>:<direction>` token and is
   restored on the first listing request after restart.
7. The merged `All` view remains session-only, matching current behavior.
8. Missing primary values sort last in both directions in merged listings.
9. Invalid fields, directions, and field/view combinations remain fail-closed.
10. The existing show-only and merged-view field restrictions cannot leak when
    moving between libraries or views.

## Confirmed baseline

- `src/routes/+page.svelte` owns one `sort` token. `SORTS` currently couples
  each field to one direction and one direction-bearing label.
- The frontend includes `sort` in `ListingRequest`; direction changes therefore
  already participate in stale-request rejection and merged snapshot identity.
- Source-library preferences persist the complete token through
  `set_section_sort` into `AppConfig.section_sorts`. No config shape change is
  required.
- `src-tauri/src/config.rs::ALLOWED_SECTION_SORTS` and
  `src-tauri/src/commands.rs::validate_sort` are the closed input boundary.
- Jellyfin/Emby `map_sort` already splits the token and translates either
  suffix to `Ascending` or `Descending`.
- Plex accepts its native field token verbatim. The show-only
  `episodeAddedAt` field is the exception and currently translates only
  `episodeAddedAt:desc` to `episode.addedAt:desc`.
- `get_type_listing` separately restricts merged sorts, and
  `merge_sort_page` currently implements only the existing fixed directions.
- `tests/e2e/scenarios/sortpersist.mjs` guards the full frontend → Tauri →
  Jellyfin query → config → restart path for one descending field.
- The landed baseline and original verification history live in
  `.agents/plans/library-sorting.md` and
  `.agents/plans/show-last-episode-sort.md`.

## Approved product contract

### Controls

- Replace the direction-bearing field options with a direction-neutral field
  selector carrying `aria-label="Sort by"`.
- Add an adjacent square button that displays `↑` for ascending and `↓` for
  descending. Activating it toggles the current direction without changing the
  selected field.
- Give the arrow button a dynamic accessible name and tooltip that state both
  the current direction and the activation result, for example
  `Sort direction: ascending; activate for descending`.
- Put both controls in one `.sort-controls` group. The group, not an individual
  selector, owns the existing auto margin in the breadcrumb row so wrapping
  remains coherent at narrow widths.
- Keep the visible button arrow-only as the owner specified. Use text in its
  accessible name and tooltip; do not make arrow orientation the only semantic
  signal.

### Fields and availability

| Token field | Visible label | Source library | Merged `All` | Restriction |
|---|---|---:|---:|---|
| `titleSort` | Title | yes | yes | none |
| `year` | Year | yes | yes | none |
| `addedAt` | Date added | yes | yes | none |
| `episodeAddedAt` | Last episode added | yes | no | show libraries only |
| `originallyAvailableAt` | Release date | yes | yes | none |
| `rating` | Rating | yes | no | source-side only; DTO has no rating |
| `lastViewedAt` | Last played | yes | yes | none |

Both suffixes are valid for every row wherever that field is available.

### State transitions

- Keep `<field>:<direction>` as the one canonical in-memory and wire token.
- Derive the displayed field and arrow-button state from that token.
- On field change, compose the new field with the current direction, persist if
  a source library is active, then reload.
- On direction change, compose the current field with the new direction,
  persist if a source library is active, then reload.
- On source-library entry, accept a saved token only when both suffix and field
  are valid and the field is allowed for that section type; otherwise use
  `titleSort:asc`.
- On merged-view entry, preserve the current complete token only when its field
  is merged-sortable; otherwise use `titleSort:asc`.
- A direction change creates a different `ListingRequest.sort` and
  `MergedSnapshot.sort`; existing generation/snapshot checks must continue to
  reject continuation data from the old direction.

### Ordering

- Title ordering compares case-folded display titles in the requested
  direction.
- Year and release date continue to share year-granularity data in merged
  listings.
- Date added and last played compare their existing optional millisecond
  fields.
- For every optional primary field, `None` sorts after every `Some` in both
  directions.
- Equal primary values use case-folded title ascending as the deterministic
  secondary order in both directions. This preserves the existing descending
  behavior and avoids reversing tie groups merely because the primary direction
  changed.

## Implementation slice

Land this as one cohesive code slice and one commit. Do not split frontend,
backend allowlisting, merged comparison, and the end-to-end guard into
separately shippable states.

### 1. Frontend field/direction model

In `src/routes/+page.svelte`:

1. Replace `SORTS` with field metadata containing the base key, neutral label,
   show-only flag, and merged-view availability.
2. Add a closed `SortDirection` type and helpers that parse and compose only
   known `<field>:<asc|desc>` tokens. Do not use an unchecked string suffix as
   a UI capability decision.
3. Replace exact-token `TYPE_SORTS` checks with field-based merged capability
   checks.
4. Keep `sort` as the canonical request/persistence token. Render the field
   dropdown and arrow button from parsed field/direction values and route both
   change handlers through one function that sets the complete token, performs
   the existing best-effort per-library persistence, and reloads.
5. Update `select` and `selectType` to validate the complete token under the
   section/view restrictions before adopting it.
6. Replace the single `.sort` styling rule with a `.sort-controls` group, field
   selector styling, and a square direction-button rule. Preserve the current
   breadcrumb layout, focus visibility, theme tokens, and narrow-width
   wrapping.

### 2. Closed backend token contract

In `src-tauri/src/config.rs` and `src-tauri/src/commands.rs`:

1. Expand `ALLOWED_SECTION_SORTS` to the explicit Cartesian set of the seven
   existing fields and `asc`/`desc`. Keep it closed; do not accept arbitrary
   `<text>:<direction>` values.
2. Preserve `set_section_sort` and `section_sorts` schema/API shapes.
3. Replace the merged-view exact-token match with a helper that accepts both
   directions for only `titleSort`, `year`, `addedAt`,
   `originallyAvailableAt`, and `lastViewedAt`.
4. Keep rejection strings and default `titleSort:asc` behavior stable unless a
   test proves a wording change is required.

In `src-tauri/src/source/plex.rs`:

5. Translate both `episodeAddedAt:asc` and `episodeAddedAt:desc` to the
   corresponding `episode.addedAt:<direction>` Plex token.
6. Continue passing every other allowed token through byte-for-byte.

`src-tauri/src/source/jellyfin.rs::map_sort` needs no production change unless
implementation evidence disproves its existing suffix translation.

### 3. Direction-aware merged ordering

In `src-tauri/src/commands.rs`:

1. Refactor `merge_sort_page` to branch on the validated field and direction
   rather than duplicating ten unrelated exact-token arms.
2. Use an explicit optional-value comparator so missing values stay last for
   both directions; ordinary `Option` ascending comparison is not sufficient
   because it puts `None` first.
3. Preserve title-ascending tie breaks for year/release-date, date-added, and
   last-played comparisons.
4. Preserve pagination after sorting and the existing immutable snapshot
   behavior.

### 4. Guards

Rust unit coverage:

- Expand command/config validation tests so every allowed field accepts both
  suffixes, representative ascending values survive config validation and
  round-trip, and unknown fields/directions still fail.
- Expand the Plex translation test to prove both leaf-added directions map to
  `episode.addedAt:<direction>` while all other new tokens pass through.
- Add merged-sort tests for title, year/release date, date added, and last
  played in both directions.
- For each optional merged field, include missing data and prove it remains
  last in ascending and descending order.
- Pin the title-ascending tie break independently from primary direction.

Extend `tests/e2e/scenarios/sortpersist.mjs`:

- Address the field dropdown and direction button by their accessibility
  labels, not presentation classes.
- Prove a field change preserves the current direction.
- Prove the visible arrow and accessible name agree with the current direction
  before and after activation.
- Prove both `SortBy` and `SortOrder` reach the Jellyfin mock for descending and
  ascending requests.
- Prove the exact ascending token reaches `section_sorts`.
- Restart the app, reopen the library, and prove the first listing request,
  field dropdown, and direction button already reflect the persisted ascending
  token.
- Retain screenshot evidence with both controls visible.

### 5. Version and closeout

- Run `scripts/bump.sh` exactly once for the code slice; let the script choose
  the next patch version from the then-current source.
- After all guards and verification pass, update this status to landed with the
  exact commit/version and verification evidence.
- Rotate the completed item out of `.agents/state.md`; record any genuine
  remaining risk there rather than leaving implementation prose active.
- Do not publish a release, push a remote, or start a reviewer workflow without
  the separately required owner instruction.

## Verification

Focused development gates:

1. Run the affected Rust command/config/source tests after each backend change.
2. Run `npm run check` and `npm run build` after the frontend refactor.
3. Run `npm run e2e -- sortpersist` on the canonical Linux E2E venue recorded
   in `.agents/machines.md`.

Independent guard proof before landing:

1. Remove one newly allowed ascending token; prove the validation guard fails,
   restore it, and prove green.
2. Remove the Plex `episodeAddedAt:asc` translation; prove its focused guard
   fails for the mapping mismatch, restore it, and prove green.
3. Make merged ascending ordering use descending comparison; prove its focused
   ordering guard fails, restore it, and prove green.
4. Let an ascending optional comparison use ordinary `Option` order; prove the
   missing-last guard fails, restore it, and prove green.
5. Make the frontend arrow handler retain/emit `desc`; prove `sortpersist`
   fails at the request, persistence, or arrow-state assertion, restore it, and
   prove green.

After restoration, run the repo's complete cross-stack verification from
`.agents/repo-guidance.md`: exact JS toolchain check, clean install, npm audit,
frontend check/build, Rust MSRV and stable checks, clippy, Rust tests, Cargo
audit, and the full Linux E2E suite. The working tree must be clean after the
single landing commit.

## Closeout evidence

- The cohesive product/test/version slice landed in `c0d1412` at 1.0.59.
  `scripts/bump.sh` ran exactly once, advancing 1.0.58 → 1.0.59.
- Focused frontend check/build and affected Rust command/config/Plex/merged-sort
  tests passed. The Linux `sortpersist` scenario passed after a fresh Tauri
  build and again after committed-state restoration.
- All five prescribed mutations failed for their intended discriminator and
  passed after restoration:
  1. removing `episodeAddedAt:asc` from the closed allowlist was caught as that
     exact unsupported token;
  2. removing its Plex translation produced the exact
     `episodeAddedAt:asc`/`episode.addedAt:asc` mismatch;
  3. reversing title ascending produced the exact reversed title order;
  4. using ordinary ascending `Option` ordering put `undated` first and failed
     the missing-last assertion;
  5. forcing the arrow handler to stay descending timed out only while waiting
     for the second toggle's new ascending Jellyfin request.
- Canonical local verification passed with Node 26.5.0/npm 12.0.1, a clean
  install, zero npm vulnerabilities, frontend check/build, Rust 1.89 and
  rolling-stable checks, clippy with warnings denied, all 362 Rust tests, and
  Cargo audit with only the already-recorded 17 allowed
  unmaintained/unsoundness notices.
- The restored Linux full suite completed 36/39. The changed `sortpersist`
  scenario passed and retained both screenshots
  (`sortpersist-01-field-and-direction.png` and
  `sortpersist-02-persisted-after-restart.png`). The three failures were the
  explicitly out-of-scope, pre-existing harness races: `continueon`
  (delayed PlaybackInfo ordering), `playverbs` (player-action element timing),
  and `refresh` (refresh-owned listing settle timeout). No sorting failure
  occurred.
- The screenshot evidence was visually checked: both runs show the neutral
  `Year` field selector and adjacent boxed `↑`. The VM's temporary source
  overlay was restored, no app/driver/mpv/temp-config residue remained, and the
  VM returned to its prior stopped state.

## Out of scope

- New sort fields, filters, grouping, search ordering, detail/season ordering,
  or Home/Continue Watching ordering.
- Persisting merged `All` view preferences.
- Changing source-server sort semantics beyond direction.
- Adding rating to `ItemDto` or enabling rating/last-episode sorts in merged
  views.
- Config schema migration or cleanup of valid existing sort tokens.
- Repairing unrelated continuation/mpv or `refresh` E2E harness flakes.
- Publishing, packaging, release notes, or remote pushes.
