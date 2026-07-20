# Plan: duplicate-copy playback policy and title-level watched state

Status: **APPROVED FOR IMPLEMENTATION — owner settled the product behavior on
2026-07-19 and made one clean Claude Fable `openreview` the approval gate. The
gate passed over exact range `ad27cf0..13405dc`; implementation is authorized.**

## Plan review

`openreview claude` (`claude-fable-5` @ max, competitive) over
`ad27cf01ac9183ee8d2f5d713ea5fb535a895b93..13405dc16dca4c48808ace805674f0013161b53e`:
no material issue. The schema-valid one-shot verdict returned `clean` with the
exact base/head pair and an empty findings array on 2026-07-19.

## Implementation progress

### Slice 1 — complete

Implementation `c7ac901` adds the persisted policy/display preferences, exact
Player Settings choices and priority help, display diagnostics and independent
overrides, native macOS/Windows/Linux display adapters, mpv screen placement and
exact-session display observation, endpoint locality classification, and the
pure deterministic selector. Compatibility fix `cadbbb0` keeps the published
Wayland client/runtime API while vendoring the matching 0.31.10 scanner with
upstream's quick-xml 0.41 security/compatibility changes; its provenance is in
`src-tauri/vendor/wayland-scanner/VENDORED.md`.

Local canonical Node, npm audit/check/build, Rust MSRV/stable check, clippy,
unit, and cargo-audit gates passed. The final byte-identical Linux copy passed
native check/clippy/unit/frontend validation and a fresh-build real-app E2E run
(30/30). Windows native display compilation remains unverified locally because
the macOS cross-check reaches Tauri's resource build before Vela and this host
lacks `llvm-rc`; GitHub Windows CI remains the native proof when the owner later
authorizes a push.

Independent production mutations proved: unknown policy fails safe to Best;
Best ranks resolution before HDR; Compatible respects display fit; Fastest
respects host/LAN/internet; private addresses classify as LAN; HDR override is
independent; preferences round-trip; mpv uses `--screen-name`; display events
are complete and exact-session isolated; the UI choice text is exact; and the
secured Wayland scanner cannot be replaced by the API-incompatible upstream
git revision without the dependency guard failing. Every mutation was restored
to the committed bytes and the affected guard reran green.

### Slice 2 — complete

Implementation `7d9a00e` adds the provider-neutral version boundary, exact and
fresh Plex/Jellyfin/Emby resolution, direct-play eligibility plus shared
Best/Compatible/Fastest ranking on every play path, persistent Play Version
source choice, endpoint locality refresh, and merged show/season/episode
backings through navigation and TV continuation. Plex preserves every part of
the selected Media row. Jellyfin/Emby revalidate the exact MediaSourceId and
fresh PlaySessionId, and their stream token now travels only in mpv's owner-only
header include rather than the URL or argv.

The merged hierarchy fetches every page from every parent copy concurrently,
keeps provider ids authoritative, uses season/episode coordinates only inside
one canonical parent, refuses same-source or ambiguous false merges, and keeps
each backing's own parent path. A sequence can therefore cross a season using
all server copies even when the displayed face belongs to only one.

Local canonical Node/npm audit/check/build, Rust MSRV/stable check, clippy,
unit, and Cargo-audit gates pass (30 frontend guards, 198 Rust tests, zero known
vulnerabilities; 17 accepted Cargo warning-class notices). The byte-identical
Linux source passed exact npm/stable/clippy checks and a fresh-build real-app
E2E run (30/30). The mock stream rejects URL credentials and unauthenticated
media requests, proving the header reaches mpv's request while the URL remains
credential-free.

Independent mutations proved the direct-play tier, known-dimension compatible
fallback, hierarchy coordinate fallback and provider-id conflict rules,
same-source separation and per-backing parent paths, all-parent continuation,
stable exact Plex identity and complete split-part preservation, exact
Jellyfin/Emby MediaSource plus fresh session and token-free URL, migration of
backing hierarchy keys, the Play Version UI contract, and hierarchy arguments
through both browse surfaces. A self-review also caught a Windows-on-ARM-only
installer fallback corruption that host compilation could not see; a new
cross-platform source guard failed when the fallback was changed away from
`false`, then passed after exact restoration. Every mutation was restored and
its affected guard reran green.

## Goal

When the same logical video exists on more than one configured media server,
Vela chooses the source and media version according to one explicit persisted
policy, preserves a deliberate per-title override, and treats played/unplayed
state as belonging to the title rather than the copy that happened to play.

The feature applies to Plex, Jellyfin, and experimental Emby through the shared
source abstraction. It must not make ordinary library browsing perform detail
requests for every card; candidate probing belongs to the play boundary.

## Settled product behavior

### Settings

Settings -> Player exposes four radio-card choices:

1. **Prefer Best** (default for missing/unknown config values). Rank resolution
   first, HDR within the same resolution tier, then bitrate. Thus 4K HDR > 4K
   SDR > 1080p HDR > 1080p SDR; higher resolutions extend the same rule.
2. **Prefer Compatible.** Use the playback display's actual pixel dimensions
   and current HDR/EDR/Advanced Color state. Prefer a version at or below the
   display resolution and matching its HDR state. If none fits, degrade to the
   nearest playable version rather than fail playback.
3. **Prefer Fastest Source.** Rank the server endpoint as same machine > local
   network > internet, then use Prefer Best within the winning locality tier.
4. **Ask Every Time.** A normal manual play of a title with multiple reachable
   source copies asks which source to use. The choice is not persisted.

An inline help box states those exact orders and explains that the existing
card menu's **Play Version** action is the persistent per-title override for
the three automatic modes. In Ask Every Time, choosing a Play Version row is
the answer for that play only and does not write an override. Ask mode ignores
pre-existing per-title overrides.

The Player page also shows the detected playback-display resolution and HDR
state. An Advanced fallback permits independent resolution and HDR overrides;
Auto remains the default. Overrides exist for unavailable or incorrect native
detection, not as the normal setup path.

### Compatible display identity

- A manual play targets the monitor containing Vela's main window at the moment
  Play is invoked. Vela passes the matching mpv screen name where the windowing
  backend supports placement.
- Existing mpv IPC observation is extended to retain `display-names`,
  `display-width`, and `display-height` for the exact playback session.
  Automatic playlist/Continue Playing successors use that observed mpv output,
  so a sequence stays compatible with the display it is actually using.
- macOS queries the matching `NSScreen` EDR values; Windows queries Advanced
  Color for the matching display through DisplayConfig; Wayland queries the
  matching `wl_output` through `color-management-v1`. X11 is SDR. A compositor
  or platform that cannot answer yields `unknown`, never a fabricated HDR
  capability; Compatible treats unknown HDR as SDR-safe unless the user set an
  override.
- On Wayland the compositor owns final window placement. If the first mpv
  window lands on a different output than Vela, its observed output becomes
  authoritative for automatic successors and the Settings diagnostic changes
  to that output. The manual override remains available for the first play.

### Ask mode and sequences

- A standalone duplicate play asks every time.
- A Vela playlist or TV continuation creates a playback-run affinity. The
  first logical item in that run with multiple reachable source copies asks;
  the chosen source is reused only for that run.
- If the chosen source lacks or cannot resolve a later logical item, one
  reachable alternative is used directly; multiple alternatives ask again and
  replace the run affinity. Closing/replacing playback, starting another item
  manually, or exhausting the run clears the affinity. Nothing is persisted.
- A server-owned playlist remains owned by its server. If that server is
  offline, its playlist is unavailable and Vela neither reroutes cached entries
  through another server nor pretends it can advance or update it.

### Watched state

- Played/unplayed is title-level. A manual Mark watched or Mark unwatched on a
  merged item independently updates every currently configured backing.
- A natural clean completion independently marks every backing played. Partial
  playback/resume position remains source/version-specific and is reported only
  to the stream that played.
- Fan-out is concurrent and best-effort per server. Successful mutations are
  never undone because another server is offline. If at least one server
  succeeds, the command returns a structured partial-success result and the UI
  names the sources that could not be updated. If none succeeds, Vela restores
  its local curation mutation and reports failure.
- There is no offline mutation queue. An unavailable server cannot play or
  update state; a later authoritative refresh may therefore show that backing's
  older state until the user performs another edit while it is reachable.

## Current evidence and conflicts to remove

- `src-tauri/src/commands.rs::rank_backings` currently applies a Plex-first
  kind ladder and stable registry order. It has no policy setting or quality,
  locality, or display inputs.
- `BackingRef` carries only source id + rating key. Rich quality metadata lives
  behind provider detail/playback endpoints, which is why selection must happen
  at play time rather than during merged pagination.
- Plex's `get_part_url_for_rating_key` currently ranks HDR before resolution;
  Jellyfin/Emby's `select_media_source` ranks directness/locality/HDR before
  resolution. Both conflict with the settled Prefer Best ordering and must use
  the shared selector.
- `set_watched` routes one namespaced key. Clean-EOF completion likewise carries
  one item/watch key. The current merged-view E2E deliberately proves the old
  single-backing behavior and must be replaced, not retained.
- Merged shows currently drill through one metadata-rich backing. Episodes do
  not retain cross-source alternatives, so a sequence cannot honor source
  affinity or fail over at a missing episode without a merged-hierarchy path.
- Tauri's monitor abstraction supplies name/geometry/scale but not HDR state;
  native platform adapters are required behind one internal interface.

## Technical design

### 1. Persisted policy and display profile

Add a closed Rust enum `PlaybackSourcePolicy` with serialized values `best`,
`compatible`, `fastest`, and `ask`. `AppConfig.playback_source_policy` is
optional for backward compatibility; command-layer normalization maps missing
or unknown values to `best`. Add optional display override fields whose closed
values also normalize fail-safe. Expose typed get/set commands and matching
TypeScript unions.

Create `display.rs` with a shared `DisplayProfile`:

```text
name, width_px, height_px, hdr = enabled|disabled|unknown,
evidence = native|mpv-observed|manual-override
```

The module resolves the main Tauri window's current monitor, maps an observed
mpv display name back to the native output, and performs target-specific HDR
queries without blocking an async worker. Platform dependencies are
target-specific. An unavailable protocol/API is a normal `unknown` result;
permission, FFI, and malformed-data errors never crash playback.

Extend `WindowStateObservation`/`PlaybackWindowSession` with display name and
pixel dimensions parsed only from owned mpv property-change events. Preserve
the existing exact-session isolation used for fullscreen/maximized state.

### 2. Candidate contract and deterministic selector

Add a provider-neutral `PlaybackVersion` returned by a new async
`MediaSource::playback_versions(raw_item_key)` method:

```text
source id/name, namespaced item key, opaque provider version id,
width, height, HDR, bitrate, direct-play rank, endpoint URL
```

No token, stream URL, or provider session id is serialized to the frontend or
logged. A companion `resolve_stream_version(..., version_id)` resolves the
exact selected version; the old `resolve_stream` becomes the single-version
default/fallback implementation for sources that do not enumerate versions.

- Plex parses every `<Media>` id/quality/bitrate and resolves the selected
  media's complete Part list (including split-file EDL behavior).
- Jellyfin/Emby maps every PlaybackInfo `MediaSource`, then revalidates the
  chosen MediaSourceId when resolving so the progress target receives the
  correct fresh PlaySessionId.
- Candidate collection clones registry sources under the lock, releases it,
  and probes backing sources concurrently with bounded per-source network
  timeouts. One offline source cannot delay or erase healthy candidates.

Direct-play/direct-stream support is an eligibility tier because Vela does not
own a general transcode profile. Within the highest available playable tier,
the pure selector applies:

- Best: `(height, width, hdr, bitrate)`, descending.
- Compatible: prefer candidates no larger than the target when any exist;
  otherwise the smallest larger candidate. Prefer an HDR-state match when one
  exists, then highest fitting resolution and bitrate.
- Fastest: endpoint locality first, then the Best key.
- Ask: group candidates by source for the UI; after the user chooses a source,
  select its Best candidate.

Final ties use stable source id then opaque version id, so results never depend
on request completion or registry insertion order.

Endpoint locality is recomputed at play time from DNS plus current interface
addresses: loopback or an address assigned to this host = same machine;
private/link-local address or a provider-verified local connection = LAN;
otherwise internet. Unknown resolves conservatively as internet. DNS/interface
enumeration runs off the async worker and is covered by pure address-class tests.

### 3. Merged hierarchy and selection boundary

Keep merged library pagination lean. When a merged show/season is opened, fetch
children from all parent backings concurrently and deduplicate them into merged
children:

- provider ids are authoritative when present;
- seasons may fall back to season index within the already-canonical show;
- episodes may fall back to `(season index, episode index)` within that show;
- an ambiguous or missing identity stays as separate entries rather than
  risking a false merge.

Extend backing hierarchy data only as needed for navigation/continuation, and
preserve it in recents and Vela playlist snapshots through existing serde
round-trips. The detail surface still uses the metadata-richest backing; play
selection is independent.

Move final source/version selection into the shared backend play boundary so
cards, details, recents, Vela playlists, server playlists, and automatic
continuations cannot bypass it. Automatic-mode per-title overrides filter to
the explicit source before ranking; an unavailable explicit source fails with
that source named instead of silently violating the override.

### 4. Ask handshake and run affinity

Change play commands to return a tagged result:

```text
started(session id) | superseded | source-choice-required(request id, choices)
```

The first call probes candidates. Ask mode returns source-level choices when
needed without launching mpv. A bounded, expiring in-memory request stores only
the immutable item/run coordinates and candidate identities. The follow-up
command consumes the exact request id plus selected source, revalidates that
source, and launches or reports that it became unavailable. Credentials never
cross the IPC boundary.

Add one accessible modal listing source name, locality, and its selected Best
version's resolution/HDR label. Escape cancels without altering playback. A
Play Version menu choice uses the same explicit-choice path.

Attach source affinity to the existing exact playlist/continuation run rather
than a global variable. Automatic continuation that needs a new choice stores
one pending continuation and emits an id-only Tauri event; the modal's response
must match that run/session before launch. Manual play invalidates pending work
and clears the old affinity, preserving the existing stale-session guards.

Server-playlist cursors never invoke cross-source rerouting: a failed owner
fetch/resolve ends the run as unavailable.

### 5. Title-level watched fan-out

Replace the one-key watch command input with the item/backing identity set.
Deduplicate `(source_id, raw_key)`, route every source before awaiting, and run
`mark_played` calls independently without holding registry/config locks.

Retain the curate-before-network rule. The undo token covers only Vela's local
recents/tombstone mutation and is applied only when zero backing mutations
succeed. Return source-safe names/counts, never URLs or errors that may contain
credentials. A partial result refreshes the merged surface and displays a
non-destructive warning.

Carry the immutable backing identity set in `PlaybackCompletion`. On exact
natural EOF, the dispatcher performs the same best-effort played fan-out before
its authoritative Home refresh and before successor repaint. Quit, error,
replaced, stale-session, and partial playback paths do not fan out played state.

## Implementation slices

Each slice is one coherent commit on a feature branch. New guards are proven by
reverting the production behavior, observing the intended failure, restoring
the committed bytes, and rerunning green.

1. **Policy/config/UI + display/locality foundations.** Persist and render the
   four modes and inline help, add display diagnostics/advanced overrides,
   native adapters, mpv display observation, locality classification, and pure
   ranking tests. No listing-path detail fan-out.
2. **Provider candidates + automatic modes + merged hierarchy.** Add the
   version contract, exact Plex/Jellyfin/Emby resolution, shared Best/
   Compatible/Fastest selection at every play boundary, persistent Play Version
   behavior, and merged show/season/episode backings.
3. **Ask Every Time.** Add the bounded handshake, accessible modal, explicit
   menu-choice behavior, exact-run source affinity, missing-copy re-prompt, and
   server-playlist no-reroute rule.
4. **Watched-state fan-out.** Fan out manual watched/unwatched and clean-EOF
   played mutations with partial-success reporting and zero-success rollback;
   retain source-specific resume/progress.
5. **Integration, documentation, and version.** Extend the two-server TLS E2E
   fixture with distinct media versions/localities and merged hierarchy. Prove
   all four policies, override precedence, Ask session lifetime/re-prompt,
   offline server playlist behavior, and all-backing watched fan-out. Update
   README/ISSUES, bump once, run the canonical suite plus fresh Linux real-app
   E2E, then run the required Claude `codereview` over the complete code range.

## Guard matrix

- Config: missing/unknown policy -> Best; every mode and display override
  survives restart; stale source overrides are pruned only by existing source
  removal behavior.
- Selector: 4K SDR beats 1080p HDR in Best; HDR breaks a same-resolution tie;
  bitrate breaks the remaining tie; Compatible skips 4K for a 1080p target and
  prefers SDR when HDR is disabled; Fastest enforces host > LAN > internet;
  final ties are deterministic.
- Display: platform adapters map the intended output; unsupported Wayland color
  management yields unknown; mpv display events cannot cross session ownership;
  manual override wins without altering native detection evidence.
- Providers: Plex selects the exact Media id and preserves all split parts;
  Jellyfin/Emby select the exact MediaSourceId and tracking session; tokens stay
  out of DTOs, argv, logs, and titles.
- Hierarchy: same episode coordinates merge only inside one canonical show;
  ambiguous coordinates do not merge; the affinity source is retained across
  season boundaries and missing episodes re-enter selection.
- Ask: standalone duplicates prompt every time; per-title overrides are ignored;
  a Play Version click does not persist; one run reuses a choice; stale/cancelled
  request ids cannot launch; manual replacement clears pending continuation.
- Watch: manual watched and unwatched reach every backing; one offline backing
  does not roll back successful peers; zero success restores local curation;
  clean EOF fans out once; quit/error/stale completion fans out zero times;
  resume/check-ins remain selected-source-only.
- Server playlists: unavailable discovery remains visible as unavailable;
  owner loss during playback stops the sequence without cross-source reroute or
  playlist writes.

## Verification

Run the canonical commands from `.agents/repo-guidance.md`. The feature spans
frontend, backend, config, playback, provider networking, and platform code, so
the full Node/Rust/build/audit set is mandatory. Run fresh-build Linux E2E,
targeted macOS/Windows compile checks where available, and the affected native
package builds. Native HDR detection requires manual smoke evidence on each
available platform; unavailable platforms remain explicitly unverified rather
than inferred from compilation.

## Accepted edges and non-goals

- No persistent offline watched-state queue and no later background replay.
- No cross-source resume-position synchronization.
- No codec/GPU benchmark or transcoding policy; provider-declared direct
  capability is only an eligibility constraint.
- No embedded player and no change to mpv's ownership of video/HDR output.
- No mutation of server-owned playlists.
- Emby remains experimental until real-server coverage exists.
