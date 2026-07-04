# rev-2: Same-source duplicates collapse into one card (and crash the context menu)

**Severity**: MEDIUM — one version becomes unreachable from the All view, and opening the card's context menu throws (duplicate Svelte each-keys).
**Status**: Open
**Branch**: `fix/rev-2-same-source-collapse`
**Commit**: (pending)

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
(pending)

## Files changed
(pending)

## Guard proof
(pending)

## Coder dispute (if any)
None — the design intent (owner direction) was cross-source dedup; same-source
versions should stay separate cards as they were pre-batch.

## Known gaps
Stacked on rev-1's branch (shared file); merge order rev-1 → rev-5.

## Reviewer comments
(pending)
