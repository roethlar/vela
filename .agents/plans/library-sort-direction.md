# Plan: Independent library sort direction

## Status

**DRAFT Revision 1 — awaiting one owner decision; no implementation go.**

Owner requirement, recorded 2026-07-28: library sorting needs an
ascending/descending option.

Revision 1 proposes two adjacent, explicitly labelled controls in a library
root: a direction-neutral `Sort by` field selector and an
`Ascending`/`Descending` direction selector. Changing either control preserves
the other dimension. Approval of that product contract is the only outstanding
decision; implementation remains blocked until the owner rules on it.

## Goal

Make every currently offered library sort reversible without changing the
existing request token, persistence schema, source routing, merged-listing
snapshot identity, or default ordering.

Acceptance requires all of the following:

1. Every sort field available in a source library can be requested in both
   ascending and descending order.
2. The merged `All` view offers both directions for every field it can already
   sort locally; source-only fields remain absent there.
3. The direction is visible and keyboard-operable as a labelled control rather
   than encoded only in a field label.
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

## Product contract proposed by Revision 1

### Controls

- Replace the direction-bearing field options with a direction-neutral field
  selector carrying `aria-label="Sort by"`.
- Add an adjacent direction selector carrying `aria-label="Sort direction"`
  with exactly `Ascending` (`asc`) and `Descending` (`desc`).
- Put both controls in one `.sort-controls` group. The group, not an individual
  selector, owns the existing auto margin in the breadcrumb row so wrapping
  remains coherent at narrow widths.
- Do not add a new icon or an icon-only toggle. The two direction names must be
  visible without hover and usable by keyboard and assistive technology.

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
- Derive the two displayed control values from that token.
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
4. Keep `sort` as the canonical request/persistence token. Render the controls
   from parsed field/direction values and route both change handlers through one
   function that sets the complete token, performs the existing best-effort
   per-library persistence, and reloads.
5. Update `select` and `selectType` to validate the complete token under the
   section/view restrictions before adopting it.
6. Replace the single `.sort` styling rule with a `.sort-controls` group and
   shared selector styling. Preserve the current breadcrumb layout, focus
   visibility, theme tokens, and narrow-width wrapping.

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

- Address controls by their accessibility labels, not presentation classes.
- Prove a field change preserves the current direction.
- Prove both `SortBy` and `SortOrder` reach the Jellyfin mock for descending and
  ascending requests.
- Prove the exact ascending token reaches `section_sorts`.
- Restart the app, reopen the library, and prove the first listing request and
  both visible controls already reflect the persisted ascending token.
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
5. Make the frontend direction handler retain/emit `desc`; prove `sortpersist`
   fails at the request or persistence assertion, restore it, and prove green.

After restoration, run the repo's complete cross-stack verification from
`.agents/repo-guidance.md`: exact JS toolchain check, clean install, npm audit,
frontend check/build, Rust MSRV and stable checks, clippy, Rust tests, Cargo
audit, and the full Linux E2E suite. The working tree must be clean after the
single landing commit.

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
