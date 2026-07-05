# Plan: SMB/SSH playtest fixes (found on 0.1.21)

## Status

Draft — under `reviewloop codex` (plan-review mode). NOT approved for
implementation. Surface to owner only after the loop converges.

## Origin

2026-07-05 owner playtest of 0.1.21 against the real NAS: one SMB share and one
SSH folder added on the same host. Five bugs. Each was diagnosed this session by
parallel code readers; the two seek-hang mechanisms were additionally
adversarially verified line-by-line (verdict: upheld). This plan turns those
diagnoses into ordered, independently verifiable implementation slices.

It revises/extends prior plans rather than restating them:
`smb-share-root-autoadd.md` (bug 4), `smb-source-labeling.md` (bug 5), the merge
path from `library-all-view-rework.md` (bug 4), and the loopback proxy from
`smb-native-client.md` (bug 1).

## Owner UX ruling (2026-07-05, binding)

**No click may terminate in an error-like or dead-end message.** Clicking a
source shows that source's content; every button either acts or is not rendered.
This ruling reclassifies two things previously treated as "per-design":
- the empty per-source Home (was `library-all-view-rework.md` open-point-3:
  "local contributes no hubs") is now a bug — **bug 3**;
- the Remove button that calls a command which rejects local-family ids is the
  same violation class — folded into **bug 5**.

## Priority

- **P1 (functional/UX, break real use):** bug 1 (SMB seek), bug 2 (SSH seek),
  bug 3 (source-click dead-end), and the erroring-Remove half of bug 5.
- **P2 (quality/feature):** bug 4 (share-root classification — larger, unlocks
  metadata + merge), and the naming/rename half of bug 5.

---

## Bug 1 — mpv hangs on seek (SMB) — P1

### Root cause (adversarially verified)
The loopback proxy hardcodes `Connection: close` on every response
(`stream_proxy.rs:313`), so each mpv seek makes ffmpeg drop the stream and open a
**new TCP connection**. Every connection runs `serve_connection` → `connect_mount`
→ a brand-new `SmbConnection::connect` (`stream_proxy.rs:268-270`), which:
- creates a fresh libsmbclient context under the **process-wide**
  `ctx_lifecycle_lock` (`smb_client.rs:176-178`), and
- calls `list_dir("")` to re-enumerate the **share root** to "verify
  reachability" (`smb_client.rs:217-219`), then `stat` + `open`.

Meanwhile the previous stream's `SmbConnection::drop` grabs the **same** global
lock for a blocking `smbc_free_context(ctx,1)` network teardown
(`smb_client.rs:437-439`). So each seek serializes fresh-session setup behind the
old session's teardown on one lock, plus adds a share-root re-enumeration + stat +
open — each op bounded at `OP_TIMEOUT_MS=10s` (`smb_client.rs:163,185`). On a real
NAS (latency; per-client session caps) these stack into a felt freeze. Linear
playback is fine because it reuses one long-lived connection. The lifecycle lock
is NOT held across body streaming (`read_at` uses the per-connection ctx mutex),
so this is a setup-vs-teardown serialization + per-seek cost, not a body-stream
deadlock.

### Fix (ordered sub-slices, each its own commit)
1. **Drop the share-root `list_dir("")` from the stream-open hot path.** Verify
   reachability only at mount/add time, not per connection. Removes one full
   round-trip per seek. (`smb_client.rs:217-219` — separate a `verify()` from
   `connect()`/open.) Low risk.
2. **Don't hold `ctx_lifecycle_lock` across the blocking `smbc_free_context`.**
   Restructure `Drop` (`smb_client.rs:437-439`) so teardown does not serialize a
   concurrent new-session setup — e.g. take the ctx out under the lock, release,
   then free; or move frees to a dedicated reaper. Preserve libsmbclient's
   per-context single-thread invariant. Medium risk (lock discipline; this is the
   repo's "no shared lock across blocking network work" earned practice).
3. **Add a write timeout on the proxy socket** in `serve_target`
   (`stream_proxy.rs:292-337` — today only `read_request` has a 10s read timeout).
   A client that stops reading without closing can otherwise block `write_all`
   forever and pin a thread / hold an SMB handle open. Low risk.
4. **Reuse the SMB session per playback token** so a seek does not rebuild a full
   session. The proxy registers a target by token (`smb_vfs.rs:268-272` →
   `stream_proxy::register_smb`); cache the live `SmbConnection` (and, where
   feasible, the open read handle, seeking via `smbc_lseek`) keyed by that token,
   reused across HTTP connections for the token and torn down when the token is
   unregistered / playback ends. A new Range request for a token **supersedes**
   the prior in-flight stream for that token (abort it) so the two never contend
   on one context's mutex. Highest complexity — the real fix; lands last so the
   cheap wins ship first.

Keep-alive (dropping unconditional `Connection: close`) is **evaluated, not
assumed**: ffmpeg often opens a new connection per seek regardless, so the
per-token session cache (4) is the load-bearing fix; keep-alive is an optional
add-on only if it demonstrably reduces reconnects.

### Verification (hermetic, no NAS)
Extend the existing `Target::Mem` proxy test seam (`stream_proxy.rs:36,277-285`)
with a fake backend whose `open` incurs an artificial delay and whose teardown
blocks:
- **Concurrent-seek serialization:** first GET (200) still streaming, then a
  second GET `Range: bytes=X-` (the seek); assert the second's first byte arrives
  within a bound. Fails today (serializes on `ctx_lifecycle_lock`); passes after
  slice 2/4.
- **No readdir on open:** assert the stream-open path performs no `list_dir` —
  fails today (slice 1 fixes it).
- **Write timeout:** a client that stops reading mid-body; assert `serve_target`
  returns within a bound (slice 3).
- **Owner playtest** on the NAS confirms the felt freeze is gone.

### Risk
Highest of the five. Touches the SMB concurrency core. Must not violate
libsmbclient's per-context single-threading or the repo's lock-across-blocking
invariant. Each sub-slice guarded independently before the next.

---

## Bug 2 — mpv hangs on seek (SSH) — P1

### Root cause
Distinct from bug 1 — **SSH does not use the proxy.** sshfs items hand mpv the
**raw mount path** (`local.rs:857-881`, `native_remote=false`,
`resolve_stream_url` default `None` at `vfs.rs:34-36`), so on an SSH seek Vela
runs no data-path code; mpv reads the FUSE mount directly. The mount is built with
`reconnect`, `ServerAlive*`, `BatchMode`, `follow_symlinks` but **no channel or
cache tuning** (`sshfs.rs:62-90`). The leading mechanism (owner-reproducible, same
NAS): sshfs's single default SFTP channel + kernel readahead means a seek's read
queues behind the outstanding sequential-readahead backlog on that one channel
(head-of-line blocking) until it drains. Vela's lever is exactly these mount
options — so this is Vela-fixable, not "environmental."

### Fix
Add sshfs mount options in `sshfs.rs` to remove the single-channel bottleneck
and/or the readahead backlog. Candidates, to confirm empirically against the
owner's host (fast repro): multiple SFTP channels (`-o max_conns=N`, checking the
installed sshfs supports it and its interaction with `reconnect`), and/or cache /
readahead tuning. Land the confirmed option set; document why.

### Verification
Primarily owner playtest (reproduces in seconds). Optional hermetic path: a
loopback `sshd` + sftp + sshfs mount in a test to compare seek latency with vs
without the options — flagged optional because it needs sshfs + sshd in CI. State
clearly if the hermetic test is deferred to owner confirmation.

### Risk
Low code risk (mount-arg change). The real risk is picking the wrong option set;
mitigated by the owner's fast reproduction loop and option-compatibility checks.

---

## Bug 3 — Clicking a source dead-ends on empty Home — P1 (UX ruling)

### Root cause
`selectSource()` forces `mode="home"` (`+page.svelte:202`); local-family sources
contribute no hubs (`local.rs:723`) and a fresh mount has no recents, so the
home-scope render hits the dead-end branch "Nothing on your home screen yet — pick
a library from the sidebar" (`+page.svelte:1074-1078`) — even though that source's
library sections are already listed in the sidebar's Library group
(`+page.svelte:1027-1031`, the `activeSource !== null` branch).

### Fix
Clicking a source routes to its **content**, not the empty hub-home. When a
specific source is selected (`id !== null`), navigate into its library: auto-open
its first/only section (browse mode) — or, if it has multiple sections, a
per-source landing that shows content. Keep home/hero for the aggregate ("All")
scope where hubs/recents exist. Belt-and-suspenders per the UX ruling: never
render the "Nothing on your home screen yet" dead-end when browsable sections
exist; show them instead.

### Verification
E2E scenario (fits the harness; reuse `seedLocalMedia`/`openLibraryGrid`): add a
local/SMB-class source, click its sidebar entry, assert it lands on content (a
grid or its sections) and that the dead-end text is absent. Guard-proven
red/green.

### Risk
Low (frontend nav). Must not regress server-source per-source Home, which has
real hubs and should still land there.

---

## Bug 4 — Share/mount root shows bare metadata-less cards, starves the merged view — P2

### Root cause
The share-root auto-add (`commands.rs:514-521`, `f05919e`) registers the **entire
share as one flat, kind-auto folder**. The local walker/classifier
(`local.rs:700-714,955-976` `detect_kind`; `local.rs:311-360` `walk_items_level`)
is built for single-purpose library dirs; a NAS root of category folders
(Movies/, TV/…) is mis-classified into one flat section that renders the top-level
directories as bare cards with no parsed title/year and no poster. Enrichment
(`metadata.rs`) keyed to those paths finds nothing, and a completed online lookup
only writes cache with no UI event, so posters never appear without re-entering
the view. The consolidated "All" view **already includes SMB/SSH**
(`commands.rs:1922-2005` `get_type_listing`; `dedup_across_sources` by title+exact
year since mounts carry no provider ids; `kind_rank` local>smb/ssh>plex>jf/emby) —
but it is fed those year-less, title-mangled junk items, which can never dedup
against server copies. The merge plumbing needs no change; the root classification
starves it.

### Fix
Make share/mount roots **category-aware**: when a root folder is kind-auto,
classify each immediate subdirectory individually (reuse `detect_kind` /
`looks_like_show` per subdir) and expose each qualifying subfolder as its own
movie/show section; fall back to today's flat walk only when the root itself
already looks like a single library. Items then parse title/year at the correct
level, gain metadata, and flow into the already-built merged view. Apply the same
to SSH mount roots (`mount_ssh` also permits kind-auto). Secondary: fire a UI
refresh event when a background metadata lookup lands so posters appear without
re-entry.

### Verification
Rust unit tests on the classifier with fixture trees (category-root vs
single-library-root vs mixed) — hermetic, local fs, no NAS. E2E for the
merged-dedup path if a nested local fixture makes it practical.

### Risk
Medium. Changes listing/classification semantics; must not regress existing
single-purpose folders. Interacts with the listing cache. Larger than the P1
slices — sequenced after them.

---

## Bug 5 — Connected tab: triplicated rows, a Remove button that errors, URL-as-name — P1 (erroring button) + P2 (rest)

### Root cause
One SMB share renders as **three rows** in the Connected tab: the leaked
registered `smb-<id>` source (`Settings.svelte:643` filters only `kind !== "local"`,
so `smb`/`ssh` kinds — added 2026-07-04 — leak through), the mount record
(`:661-666`), and the auto-added root folder subrow (`:667-673`). All carry the
same name → looks tripled. SSH shows two (`:643` leak + `:675-683`). Removal is not
a shared-id accident but three projections of one entity: the **source row's**
Remove calls `remove_source`, which **rejects local-family ids and errors**
(`commands.rs:207-213`) — a dead-end click; the mount row's Remove fully cascades;
the folder subrow leaves a zombie zero-folder share (`local.rs:127`). Naming: no
add form captures a name, so the URL-shaped default (`server/share`
`commands.rs:439`; `host:remote_path` `commands.rs:950`) becomes the permanent
sidebar/source label (`+page.svelte:1037`), with no rename path. `mount_smb`/`mount_ssh`
already accept an optional `name` the UI never sends.

### Fix
- **P1 (dead-end + triplication):** exclude the whole local family
  (`["local","smb","ssh"]`, matching backend `LOCAL_FAMILY_KINDS`) from the
  registered-sources loop (`Settings.svelte:643`). This drops the leaked row
  (SMB 3→2, SSH 2→1) **and** removes the erroring Remove button in one change.
  Settle remove-last-folder semantics (refuse, or cascade to unmount) so no zombie
  share survives.
- **P2 (naming):** add an optional Name field to both add forms → pass through the
  existing `name` param (no backend schema change); improve the no-name default
  (bare share / last path segment, disambiguated on collision); add a rename
  command + affordance for existing mounts (propagating to the root-folder name
  copies at `commands.rs:517,981` or section labels stay stale).

### Verification
Frontend: Connected tab renders one row per mount (+ its real folder rows), no
erroring button. Rust unit tests for rename and remove-last-folder semantics.
E2E optional.

### Risk
Low. Split into a Connected-tab slice (P1) and a naming slice (P2) so the quick UX
relief ships without waiting on rename.

---

## Slice order & commits

1. Bug 1 sub-slices 1→3 (cheap SMB wins: drop readdir, lock discipline, write
   timeout), then sub-slice 4 (per-token session reuse).
2. Bug 3 (source-click routing) — small, removes a P1 dead-end.
3. Bug 5 P1 (Connected-tab filter + remove-last-folder) — small, removes a P1
   dead-end + the triplication.
4. Bug 2 (SSH mount options) — needs owner option-confirmation loop.
5. Bug 4 (share-root category-aware classification) — larger.
6. Bug 5 P2 (naming + rename).

One finding ↔ one commit; each slice guard-proven (test red→green, or owner
playtest where automation can't reach) before the next. Bump version per landed
code slice (routine).

## Open decisions (owner)
- **Local-family Home rails:** bug 3 makes clicking a source show content;
  separately, should local/SMB/SSH also contribute a "Recently added" hub to the
  aggregate Home? That reverses `library-all-view-rework.md` open-point-3 and is a
  product call, not required by the UX ruling.
- **Bug 2 hermetic test:** build a loopback sshd+sshfs test, or accept
  owner-playtest confirmation for the SSH mount-option fix?
- **Keep-alive (bug 1):** pursue HTTP keep-alive in addition to the per-token
  session cache, or is the cache sufficient?

## Non-goals
- No change to the delegated-mpv playback model (HDR passthrough via mpv's own
  window stands).
- No change to the merged-view dedup/ranking algorithm (bug 4 feeds it correct
  input; the algorithm itself is unchanged).
- macOS/Windows SMB stays OS-mounted; these fixes are Linux-native-path scoped
  except the frontend (bug 3) and Connected tab (bug 5), which are cross-platform.

## Review log
- (pending) `reviewloop codex` on this plan.
