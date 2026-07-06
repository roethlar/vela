# Plan: Stop the local family re-walking over the network every open (DRAFT — likely DEFER)

## Status
**DRAFT / proposed 2026-07-06.** Owner observed SMB metadata "reloading every time"
and asked to plan a fix, but flagged it **may not be worth it** — they mostly use
Plex, and this is SMB/SSH-only. Recorded so the diagnosis isn't lost.
**Recommendation: DEFER** unless SMB/SSH use grows; if any part is done, do only
the small TTL gate (slice 1). No code written.

## Diagnosis (confirmed against code + the live cache on disk)
Metadata **is** cached and survives restarts — both `~/.config/vela/
listing_cache.json` (enriched item listings, 1.2 MB on disk) and
`metadata_cache.json` (online lookups, 298 entries, keyed by stable SMB
share-relative paths). A cache hit serves enriched items instantly
(`source/local.rs:753-757`).

What makes it *feel* uncached on SMB:
1. **Stale-while-revalidate re-walks on every browse, with no TTL.** Every level
   view fires `spawn_revalidate` (`local.rs:435,753-757`), which re-lists the dir
   and re-runs `enrich` on every item, then repaints via `listings-updated` if
   anything changed. Cheap on local disk; on SMB it's a **full network re-walk +
   per-item sidecar probe every open**.
2. **Sidecar probe is never cached** (`metadata.rs:169-182`): `read_nfo` +
   `local_artwork` do VFS `stat`/`open` per item every browse — network round-trips
   over SMB even when no sidecar exists.
3. **Online results trickle in after the walk.** The first walk stores
   filename-floor items (async iTunes lookup hasn't returned); a later browse
   re-walks, now finds the online result in `metadata_cache`, produces a *different*
   item, updates the listing cache, repaints. That "pop-in" is the visible reload.
   On the owner's share, 291/298 online lookups returned empty ("no match", also
   cached), 7 have posters — so most items sit at the filename floor and the churn
   is mostly the re-walk cost, not new data.
4. **Poster image bytes are never byte-cached** — the cache stores the poster URL
   (`https://…mzstatic…` or a `velasmb:` ref); the webview re-downloads each cover
   every session, over the network for SMB sidecar art.

## Proposed slices (do the minimum; each its own commit + reviewloop codex)
1. **TTL / native-remote gate on `spawn_revalidate`** (the one worth doing).
   Don't re-walk a level that was validated < N minutes ago; for `native_remote`
   (SMB) sources, only revalidate on an explicit refresh or first view per session,
   not on every browse. Guard-proven: a unit test that a second browse within the
   TTL does not enqueue a revalidation.
2. (Optional) **Skip the sidecar network probe when the online cache already holds
   a definitive entry** for the key, or cache sidecar results keyed by path+mtime
   so a re-browse doesn't re-`stat` over SMB.
3. (Optional) **Byte-cache poster art** for SMB/SSH (and online) so covers don't
   re-download each launch — a small on-disk image cache keyed by URL.

## Proportionality / "is this worth it?"
- **Slice 1 is small and removes the felt "reloading every open"** for SMB. If the
  owner wants any fix, this is it.
- Slices 2-3 are diminishing returns for an SMB/SSH-only path the owner uses
  least. **Recommend not doing them** unless SMB/SSH becomes primary.
- This overlaps the queued **Bug 4** (share-root classification / metadata unlock).
  If Bug 4 is also deferred (owner is Plex-first), this can wait with it.

## Non-goals
- No change to the online-lookup or sidecar *resolution* logic (it's correct;
  the issue is *when/how often* it re-runs over the network).
- No product shift toward local-first.

## Verification
- Unit test the TTL gate (deterministic, no network): a second `items()` within
  the window does not enqueue revalidation; after the window it does.
