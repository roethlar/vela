# rev-2: Same-source duplicates collapse into one card (and crash the context menu)

**Severity**: MEDIUM — one version becomes unreachable from the All view, and opening the card's context menu throws (duplicate Svelte each-keys).
**Status**: In progress
**Branch**: `fix/rev-2-same-source-collapse` (stacked on rev-1)
**Commit**: (filled after commit)

## Evidence
`src-tauri/src/commands.rs` `dedup_across_sources`: grouping ignores whether
the colliding item comes from the same source, so two files in one source
(`Dune (2021) 1080p.mkv`, `Dune (2021) 4K.mkv`) merge into one entry with two
backings that share a `sourceId`. `src/routes/+page.svelte` context menu:
`{#each mi.backing! as b (b.sourceId)}` — keyed by sourceId, which duplicate
same-source backings violate.

## Predicted observable failure
The All view shows one card where the source has two versions; right-clicking
it crashes the keyed each ("duplicate key" runtime error), and the second
version cannot be played from the All view at all.

## What
Cross-source dedup was the intent (one title backed by several sources);
same-source version collapse was an accident of the grouping key, and the
menu's key choice assumed backings are unique per source.

## Approach
Dedup becomes a cross-source merge only: the group-hit lookup in
`dedup_across_sources` now rejects a group that already holds a backing from
the item's own source (`.filter(...is_none_or(...all(source_id differs)))`),
so a same-source collision starts its own card — same-source versions remain
individually reachable, exactly the pre-batch behavior, while cross-source
copies still merge. The context-menu each-key changes from `b.sourceId` to
`b.sourceId + ":" + b.ratingKey` so no backing-list shape can ever collide
keys. Two earlier tests' expectations updated to the decided semantics
(same-source duplicates stay separate).

## Files changed
- `src-tauri/src/commands.rs` — same-source group-hit filter; new guard
  test; two test expectations updated.
- `src/routes/+page.svelte` — collision-proof menu key.

## Guard proof
- `commands::merge_tests::dedup_keeps_same_source_versions_as_separate_cards`
  — two same-source versions plus one cross-source copy → exactly two cards,
  cross-source still merged, the other version its own card. Removing the
  same-source filter makes it FAIL (verified); restoring makes it PASS
  (verified).

## Coder dispute (if any)
None — the design intent (owner direction) was cross-source dedup; same-source
versions should stay separate cards as they were pre-batch.

## Known gaps
Stacked on rev-1's branch (shared file); merge order rev-1 → rev-5.

## Reviewer comments
(pending)
