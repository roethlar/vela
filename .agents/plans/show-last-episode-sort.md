# Plan: TV "Date Last Episode Added" sort (owner ask, 2026-07-10)

## Status
**LANDED 2026-07-10 on the owner's explicit "go" (`9cd3323`, 0.1.44) —
awaiting owner playtest.** Both slices (show-only "Last episode added"
sort; per-library sort persistence) verified before landing: unit tests
guard-proven red→green, `sortpersist` restart E2E red→green on the VM,
full suite 13/13, local CI green, Plex key live-verified. The r1
governance question (the item's recorded "add that to the queue, but
don't code" vs the later "continue") was resolved exactly as the review
demanded: the staged work was held uncommitted until the owner's
explicit landing go. Owner report (sorting playtest, 2026-07-10):
sorting works, but the date-added sort on SHOW libraries uses the
series' own addedAt, so a show whose newest episode just arrived
doesn't surface.

## Diagnosis (code-confirmed 2026-07-10)
The owner's "it seems" reading is correct by construction: Vela's sort
keys are passed to Plex verbatim (`plex.rs items` →
`get_section_content_with_type_alpha_sorted(sort_ref)`), and
`addedAt:desc` on a show section sorts by the SERIES `addedAt` (when
the show was first added), not by its newest episode. Jellyfin maps
`addedAt` → `DateCreated` (`jellyfin.rs map_sort:594-611`) — same
series-level semantics. No existing key exposes leaf-added recency.

Server-side support exists on both backends for exactly this:
- **Plex:** show sections accept `sort=episode.addedAt:desc` — the key
  behind Plex Web's own "Last Episode Date Added" sort. **VERIFIED
  2026-07-10 against the owner's live server** (read-only probe, token
  local-only per the token-handling stance): `addedAt:desc` returns the
  series-added order, while `episode.addedAt:desc` surfaces shows whose
  SERIES addedAt is months old but whose newest episode just arrived
  (e.g. a weekly show jumped to the top) — exactly the reported gap.
  An unknown key would degrade to Plex's default order, non-fatal.
- **Jellyfin:** `SortBy=DateLastContentAdded` (series-level "newest
  content added"). Emby (same client code path): **ASSUMPTION** it
  accepts the same name — Vela is Plex-first and JF/Emby listing polish
  is the same deferred class as `item_detail`; a server that ignores
  the SortBy returns its default order, non-fatal.

## Design
One new Vela sort key, SHOW-scoped end to end: `episodeAddedAt:desc`,
label **"Last episode added"** (placed after "Recently added" in the
dropdown).

1. **Backend whitelist:** `commands.rs ALLOWED_SORTS` +=
   `"episodeAddedAt:desc"`. The merged All-view whitelist
   (`get_type_listing`, `commands.rs:1029-1038`) is NOT extended: the
   DTO carries no per-item "newest episode" field to merge-sort on, so
   the key stays per-source, exactly like `rating:desc` (the existing
   comment already documents that class). The frontend never offers it
   there (below).
2. **Plex mapping:** translate the Vela key to Plex's `episode.addedAt`
   at the `plex.rs items()` boundary (small pure fn beside the call:
   `episodeAddedAt:desc` → `episode.addedAt:desc`, everything else
   passes through) — Vela keys happen to be Plex-native today; this is
   the first divergence, kept in one visible spot.
3. **Jellyfin mapping:** `map_sort` gains
   `"episodeAddedAt" => "DateLastContentAdded"`.
4. **Frontend:** the `SORTS` entry carries `showOnly: true`; the sort
   `<select>` filters it out unless the active SECTION is a show
   section (`active?.sectionType === "show"`); the merged type view
   keeps its existing `TYPE_SORTS` filter (key not added). `select()`
   gains the symmetric guard `selectType()` already has: entering a
   non-show section with a show-only sort selected resets to
   `titleSort:asc` — today `select()` never resets `sort`, so the
   show-only key would otherwise leak into a movie-section request.

## Slice 2 — per-library sort persistence (owner ask 2026-07-10, "sort
should stick per library", superseding this plan's original
no-persistence non-goal)
1. Config: new `section_sorts: BTreeMap<String, String>` (namespaced
   section key → Vela sort key), `#[serde(default)]` so old configs
   load; entries for removed sections linger harmlessly. Round-trip
   unit test.
2. Backend: `set_section_sort(section_key, sort)` command (whitelist-
   validated, key length-capped); `get_sections` stamps each
   `SectionDto.sort` from config, fail-closed against `ALLOWED_SORTS`
   (a stale/hand-edited value degrades to the default, never errors).
   Sources construct `sort: None` — they know nothing of persistence.
3. Frontend: `select()` sets the sort deterministically on entry — the
   persisted value when valid for the section's type (the show-only
   guard folds in here), else Title (A–Z); `changeSort()` writes the
   choice to config best-effort and mirrors it onto the in-memory
   section so re-entry within the session agrees. The merged type view
   stays session-only (not a library).
4. E2E `sortpersist.mjs` (restart machinery from `curation.mjs`):
   change a library's sort → assert the mapped SortBy reaches the mock
   AND `section_sorts` lands in config.json → restart the app → reopen
   the library → the FIRST listing request must already carry the
   persisted SortBy (a regression sends the default SortName) and the
   select must show the choice. Red→green proven per repo rule.
   **Red→green COMPLETE 2026-07-10 (after the VM returned): RED against
   the slice-1-only tree at exactly the persistence discriminator
   ("timed out waiting for the section_sorts entry in config.json" —
   the re-sort itself passed, proving the red is persistence, not
   plumbing); GREEN with slice 2; full suite 13/13.**

## Non-goals
- No merged All-view support (no DTO field; per-source only, like
  rating).
- No episode-level data fetching or client-side recomputation — the
  server owns the semantics.
- No new sort for movie/video sections.
- ~~No persistence of the selected sort across sessions~~ SUPERSEDED by
  slice 2 (owner ask, 2026-07-10). The merged type view remains
  session-only.
- No per-type or global sort persistence (per LIBRARY only, as asked).
- No pruning of `section_sorts` entries for removed sections.

## Verification
- Unit tests (guard-proven red→green): the Plex key translation
  (new pure fn: the one divergent key + passthrough for every other
  ALLOWED_SORTS entry) and `map_sort`'s new arm (JF `SortBy` name);
  slice 2: `section_sorts` config round-trip + defaults-empty.
- `npm run check` + `npm run build`; from `src-tauri/`: `cargo check
  --locked`, `cargo clippy --all-targets --locked -- -D warnings`,
  `cargo test --locked`.
- Full E2E suite on the VM for regression. NO new scenario: the mock
  serves movies only — asserting the new ordering would need
  series/episode support in the mock, and the ordering semantics live
  server-side anyway. Recorded automation gap (same class as the
  item-detail flows): the owner playtest is the behavioral check.
- Owner playtest (0.1.44, real Plex): open a show library → sort
  "Last episode added" → a show whose newest episode arrived recently
  surfaces at the top (the exact case from the report); movie sections
  don't offer the option; switching from a show section sorted this way
  to a movie section lands on that movie library's own remembered sort
  (or Title A–Z); restart the app → each library reopens on its
  remembered sort.

## Review log
Plan-review loop (playbook `reviewloop`, reviewer `codex exec --json
--sandbox read-only` 0.144.1, mac host).

**r1 — 2026-07-10 — verdict `reopened`, 1 finding (governance, not
technical), ADMITTED.** Base `80bb883`, head `c8e5203`,
`guard_confirmed:false`. Finding: the plan treated the owner's
"continue with anything else you can do" as implementation authority
over an item whose recorded instruction was "add that to the queue,
but don't code" — and the specific boundary outranks the generic
continuation (AGENTS.md specific-over-generic). Disposition: the coder
believes the "continue" WAS specific (a direct reply to this sort
being named next in the queue), but a recorded do-not-code line is the
owner's to lift, not the coder's to interpret away — the staged
implementation stays uncommitted and the decision is routed to the
owner. No technical findings were raised against the design itself.
