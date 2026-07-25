# Plan: Intro, credit & commercial marker skipping via mpv OSD (v2)

## Status

**ACTIVE v2, revision 7 — 2026-07-25.** Supersedes the removed
`.agents/plans/skip-credits-intros.md` (v1). Incorporates both 2026-07-22 plan
reviews. Self-contained for a cold implementer.

The owner activated this plan for implementation on 2026-07-25 ("yes, activate
the marker plan"). All three activation conditions are satisfied:

1. Every product choice in **Owner decisions** below is settled in owner-facing
   chat, one decision at a time, and the ruling is recorded here and in
   `.agents/decisions.md` (complete 2026-07-23).
2. The separately approved app-wide config-integrity/recovery prerequisite
   (`.agents/plans/config-integrity-recovery.md`) is implemented, reviewed,
   verified, committed, and guard-proven through its Slice 3 closeout
   (2026-07-24, version 1.0.4, `21ecbe8` production and `8b550d6` closeout).
3. `.agents/state.md` names this plan as the active implementation.

Revision 7 changes only the status above and rebases the slice version sequence
onto the actual 1.0.4 prerequisite base; no product behavior in this plan
changed at activation.

v1 claimed "Owner-approved — implementing" without a matching `state.md` or
`decisions.md` entry; do not treat v1 status as authority over this file.

---

## Goal

When a supported server publishes intro, credits, or commercial time ranges,
Vela offers skip during external-mpv playback:

- **Button** (owner-approved default for all missing marker settings): native
  mpv ASS/OSD prompt inside the video window ("Skip Intro" / "Skip Credits" /
  "Skip Commercials") activated by clicking it or, while it is displayed,
  pressing Space.
- **Auto-skip**: seek to marker end with a brief OSD toast.
- **Off**: no script injection for that kind.

HTML/webview overlays cannot sit on the external mpv window (decision
2026-05-23: external mpv for HDR). Skipping is implemented by a **Vela-authored**
bundled Lua script, following the same resource/injection pattern as
`autocrop.lua` / `vela-autocrop.lua`.

---

## Non-goals (v1 of the feature)

- Webview or Tauri overlays on the video frame.
- Mid-title "Jump to credits" when playback is *outside* a credits marker.
- Editing, creating, or writing markers back to the server.
- Preview, recap, or other unapproved marker kinds.
- Replacing marker data inside an already-running mpv process. Each launch uses
  one immutable marker snapshot; later server changes apply on the next launch.
- Changing resume, progress check-in, or watched-threshold policy.
- Embedding or forking stock mpv scripts for this feature.
- Blocking play when markers are missing, the script is missing, or parse fails.

---

## Why (constraints)

- Playback is an external `mpv` process (`src-tauri/src/playback.rs`); position
  is observed over JSON IPC; the webview does not own the video surface.
- Marker data lives on the media server, not in Vela's recents/config.
- Continue Playing and playlist advance typically **re-invoke** `playback::play`
  with a new process (not in-process playlist next). Per-launch marker injection
  is therefore the complete path. The owner rejected live IPC marker refresh on
  2026-07-23.

---

## Architecture (end-to-end)

```
play command
  → selected source resolves stream + best-effort markers
      Plex: markers ride the existing selected-detail response
      Jellyfin: MediaSegments request runs alongside mandatory resolve work
      Emby: empty until upstream publishes an equivalent marker-range API
  → resolve bundled vela-markers.lua // same Resource resolver as autocrop
  → if policy off for all kinds OR no usable markers OR script missing:
        play as today (no script)
  → else:
        write private markers payload file (process-private runtime dir)
        launch mpv with --script=… + policy script-opts + child-only payload env
  → Lua: read payload, observe time-pos, OSD / seek per policy
```

**Degrade, never refuse play.** Missing script, unreadable payload, empty
markers, or a provider marker-fetch error must not fail `play`. Log at
warn/info; launch without the feature. Mirror autocrop: a missing `--script=`
path would make mpv refuse to start, so existence-check before injecting.

---

## Data model (`src-tauri/src/source/mod.rs`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MarkerKind {
    Intro,
    Credits,
    Commercial,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MediaMarker {
    pub kind: MarkerKind,
    /// Inclusive start of the skippable range, milliseconds from media start.
    pub start_ms: u64,
    /// Seek target / range end, milliseconds. Must be > start_ms after normalize.
    pub end_ms: u64,
}
```

No `Unknown` string variant — drop unrecognized server types during parse.
Commercial is modeled because both Plex and Jellyfin publish commercial ranges;
Preview and Recap are not silently treated as ads.

### Selected-resolution result

```rust
pub struct StreamResolution {
    // existing fields ...
    /// Best-effort intro/credits/commercial ranges for this exact selected item.
    /// Provider marker failure is normalized to empty before construction.
    pub markers: Vec<MediaMarker>,
}
```

Do **not** add a separate `MediaSource::markers()` call. The selected Plex
resolve already fetches `/library/metadata/{id}`; a third request on every play
would add avoidable latency and duplicate rediscovery behavior. Each provider
collects markers while building `StreamResolution`; marker failure alone never
fails stream resolution. Add an `include_markers: bool` argument to
`resolve_stream` / `resolve_stream_version`; the play command passes `true`
only when at least one resolved policy is not `off`. Constructors that cannot
supply markers use `[]`.

### Normalize rules (shared helper, unit-tested)

After parse, drop a marker if any of:

- `end_ms <= start_ms`
- range longer than `MAX_MARKER_MS = 30 * 60 * 1000` (30 minutes) — guards
  garbage data
- duplicate exact `(kind, start_ms, end_ms)` triples

Sort by `start_ms` ascending. Overlapping same-kind ranges: keep both; runtime
picks the first range that contains `time-pos` (see Lua rules).

---

## Server parsing

### Plex (`src-tauri/src/source/plex.rs` + `plex_library.rs`)

- **Endpoint:** change `PlexLibrary::get_item_detail` to accept
  `include_markers: bool`. Item-detail UI and version enumeration pass `false`;
  the selected `resolve_stream_version` passes the play command's value. When
  true, extend that existing selected-detail request to
  `GET /library/metadata/{ratingKey}?includeMarkers=1`, preserving its current
  auth and `Accept: application/xml` headers. Use reqwest query construction,
  not string concatenation. `resolve_stream_version` returns the parsed markers
  on `StreamResolution`; do not issue a focused third fetch.
- **DTO:** add `#[serde(rename = "Marker", default)] markers: Vec<PlexMarker>`
  to `PlexDetail`. `PlexMarker` captures XML attributes `type`,
  `startTimeOffset`, and `endTimeOffset` as strings/options and parses each
  marker at mapping time. A malformed offset therefore drops that marker
  instead of making the mandatory detail response unreadable. A credits
  marker's optional `final` attribute does not change its kind.
- **Map:**
  - `type` / `type` field `intro` → `MarkerKind::Intro`
  - `credits` → `MarkerKind::Credits`
  - `commercial` → `MarkerKind::Commercial`
  - anything else → skip
  - `startTimeOffset` → `start_ms`, `endTimeOffset` → `end_ms` (Plex units are
    milliseconds; assert in fixtures)
- **Failure:** retain the current `ensure_ready` / one rediscovery retry for the
  mandatory selected-detail request. Marker absence is empty. There is no
  independent marker *parse* error to propagate, because the fields ride the
  mandatory response — but adding the query parameter does create a request
  failure mode the plain request lacks, so a failed marker-bearing request
  retries once without the parameter rather than failing the play (finding
  `mk-2`, 2026-07-25; an earlier revision of this bullet wrongly claimed there
  was no independent marker error at all).
- **Tests:** XML fixture with intro + credits + commercial + unknown type +
  inverted range; empty markers; and an HTTP mock that fails unless
  `includeMarkers=1` is present on the existing metadata request. A synthetic
  commercial record guards Vela's parser; the owner has no commercial-marked
  Plex item. Plex's official support documentation confirms that current
  servers detect, mark, and expose commercial ranges to supported players.
  Server-generated XML captured on Plex's official forum confirms the existing
  marker shape and exact `type="commercial"` value:
  `https://support.plex.tv/articles/115003944134-removing-commercials/` and
  `https://forums.plex.tv/t/commercial-ad-skipping-on-recordings-not-working/762904`.

### Jellyfin / Emby (`src-tauri/src/source/jellyfin.rs` only)

There is **no** `emby.rs`. Emby is `Flavor::Emby` on the same client, but the
Jellyfin MediaSegments contract must not be assumed to exist on Emby.

- **Jellyfin endpoint:** authenticated `GET /MediaSegments/{itemId}` with
  repeated `includeSegmentTypes=Intro`, `includeSegmentTypes=Outro`, and
  `includeSegmentTypes=Commercial` query parameters.
  The response is the normal Jellyfin query envelope with `Items[]`; each item
  has `Type`, `StartTicks`, and `EndTicks`. This is a range API — do not derive
  ranges from `Chapters[]` or chapter names. Use a dedicated segment-response
  DTO rather than widening the existing `BaseItem` envelope. Contract evidence:
  Jellyfin's `MediaSegmentsController.cs`, `MediaSegmentDto.cs`, and
  `MediaSegmentType.cs` in the upstream `jellyfin/jellyfin` repository.
- **Map:** `Type: Intro` → Intro; `Type: Outro` → Credits;
  `Type: Commercial` → Commercial; unknown segment types are ignored. Convert
  both tick fields with the existing 100ns-ticks helper (`ticks / 10_000`),
  then run shared normalization. Jellyfin's current official SDK enumerates
  Commercial, Intro, and Outro, and the server controller owns this endpoint:
  `https://typescript-sdk.jellyfin.org/variables/generated-client.MediaSegmentType.html`
  and
  `https://github.com/jellyfin/jellyfin/blob/master/Jellyfin.Api/Controllers/MediaSegmentsController.cs`.
- **Concurrency:** for `Flavor::Jellyfin`, start the segment request alongside
  the mandatory selected resolve work (`tokio::join!` or an equivalent
  non-detached future). Mandatory item/playback-info failures retain today's
  error behavior; segment failure logs a credential-free warning and yields
  `[]`.
- **Compatibility:** a missing/unsupported MediaSegments endpoint (including
  404) yields `[]`; playback remains supported on such Jellyfin servers.
- **Emby:** `Flavor::Emby` makes no MediaSegments request and returns `[]`.
  Emby's current official static OpenAPI (`https://swagger.emby.media/openapi.json`,
  checked 2026-07-22) publishes no MediaSegments route or schema. Add Emby
  marker support only when its upstream API publishes an equivalent range
  contract; do not reuse Jellyfin's route by ancestry or guesswork.
- **Tests:** query-envelope fixture with Intro + Outro + Commercial + unknown +
  invalid ranges; tick conversion; an HTTP mock pinning the exact endpoint and
  repeated filter parameters; endpoint failure → successful stream resolution
  with empty markers; Emby → empty markers and zero MediaSegments requests.

---

## Config (`src-tauri/src/config.rs`)

Use one closed serde enum. The owner explicitly rejected the existing
**tolerant string + command-layer normalize** pattern for invalid settings on
2026-07-22: Vela must not guess what an unrecognized value meant or initialize
runtime behavior from a default config after a load/validation failure.

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkipPolicy {
    Off,
    Button,
    Autoskip,
}

// Missing remains distinguishable from invalid for old-config compatibility.
pub skip_intros: Option<SkipPolicy>,
pub skip_credits: Option<SkipPolicy>,
pub skip_commercials: Option<SkipPolicy>,
```

**Product defaults:** owner-approved 2026-07-22 for intro and credits and
2026-07-23 for commercials. A missing marker-policy field is valid and means
`button`. A present value outside the closed enum is invalid config and must
fail deserialization; it is not equivalent to a missing value.

| Field | Default when missing |
|-------|----------------------|
| `skip_intros` | `button` |
| `skip_credits` | `button` |
| `skip_commercials` | `button` |

The play command resolves `None` from the documented per-kind defaults before
source resolution and copies only the enum into `PlaySpec`; every missing marker
policy resolves to `SkipPolicy::Button`. `playback::play` does not read config
or interpret strings. There is no `normalize_skip_policy` string helper.

This feature depends on separate, approved app-wide config-integrity/recovery
work. That prerequisite must remove runtime fallbacks from config load errors,
surface a blocking notice that the settings file may be damaged or tampered
with, and recommend creating a new config. Recovery occurs only after explicit
user confirmation: preserve the invalid file byte-for-byte in a unique private
backup, then atomically replace `config.json` with fresh defaults. A backup or
replacement failure leaves the original config authoritative and reports the
failure without logging config contents. Missing fields with documented
defaults and the explicitly tolerated legacy local/SMB/SSH fields remain valid.
Do not implement this global refactor opportunistically inside a marker slice.

Round-trip tests: old configs without the marker fields load and map each kind
to its approved default; an unknown marker value rejects the whole config; save
preserves other fields; legacy inert local/SMB/SSH fields remain untouched. The
prerequisite recovery plan owns its broader validation, notification,
exact-backup, atomicity, permissions, and failure-path guards.

### Settings UI

- **Location:** Settings → **Player** tab (beside black-bar cropping / mpv
  advanced), in `src/lib/Settings.svelte`.
- Three selects: "Skip intros", "Skip credits", "Skip commercials" — options
  Off / Button / Auto-skip.
- Extend the existing `MpvAdvanced` DTO and `get_mpv_advanced` /
  `set_mpv_advanced` commands with all three fields. Setter parameters are
  optional for compatibility with an older frontend; when present they
  deserialize as the closed enum and are stored through the existing
  `config::update` path. Do not add a second config write path or bypass
  `CONFIG_LOCK` / atomic save.

---

## Bundled Lua (`src-tauri/resources/mpv-scripts/vela-markers.lua`)

### Provenance

- **Vela-authored, MIT** (repo license). Not upstream mpv code.
- Update existing `PROVENANCE.md` to list `vela-markers.lua` as Vela-owned.
- Do **not** add a new GPL LICENSE for this file. Keep `LICENSE.GPL` only for
  stock `autocrop.lua`.

### Script options namespace

mpv derives option prefixes from the script name; dashes vs underscores bite
(see `vela-autocrop.lua` header). Use an **explicit** identifier:

```lua
require "mp.options".read_options(options, "vela-markers")
```

Launch args must use the same prefix:

- `--script-opts-append=vela-markers-intro-policy=button`
- `--script-opts-append=vela-markers-credits-policy=button`
- `--script-opts-append=vela-markers-commercial-policy=button`

Do **not** put either the payload path or full marker JSON in mpv's comma-split
script-option list. Set the path only on the child process with
`Command::env("VELA_MARKERS_PAYLOAD", path)`; Lua reads it with
`os.getenv("VELA_MARKERS_PAYLOAD")`. This avoids cross-platform option-list
escaping for Windows/user paths. Policy values are a closed ASCII set and stay
in script options so Vela can append them after user-supplied mpv arguments.

### Payload file

- Written by a non-fatal `try_write_marker_payload` helper under the existing
  process-private runtime directory pattern used for IPC sockets / auth
  includes (`playback.rs` private runtime dir on Unix; per-user temp on
  Windows).
- Use a unique `vela-markers-{pid}-{counter}.json` name, exclusive creation, and
  owner-only permissions where the OS allows. Before creating, best-effort
  prune older files with that exact Vela-owned prefix from this process's
  private directory; never scan or delete outside it.
- Contents: compact JSON, e.g.
  `{"markers":[{"kind":"intro","start_ms":0,"end_ms":90000},...]}`
- The child-only environment carries the path. Lua reads the complete file,
  then immediately calls `os.remove(path)` whether parsing succeeds or fails.
  Rust removes it if `cmd.spawn()` fails. A crash before either cleanup can
  leave only owner-private, non-credential marker timing data; the next launch's
  prefix-prune removes it.
- Size: markers lists are small; still avoid argv for the body (quoting, length,
  consistency with auth-include lesson).

Any directory, create, serialize, write, or cleanup error logs a
credential-free warning and returns `None`; marker setup must not use `?` on the
`play()` error path. Only add `--script` and marker policy args, and only set the
environment variable, after script existence and payload creation both succeed.

### Runtime behavior

1. On load: read options and `VELA_MARKERS_PAYLOAD`; read the complete payload;
   best-effort unlink it; parse with `mp.utils.parse_json`. On any failure, set
   loaded/active properties to false-equivalent and no-op (do not crash mpv).
2. Set `user-data/vela-markers/loaded` = true when script is active with a
   successful parse. Architecture prevents injection for an empty normalized
   marker list, so E2E must supply a real marker. Also publish
   `user-data/vela-markers/active` as `intro`, `credits`, `commercial`, or empty
   for a deterministic prompt-path assertion.
3. Observe `time-pos`.
4. Determine active marker: first range where
   `start_ms/1000 <= t < end_ms/1000` and the kind's policy is not `off`.
5. **button:** use `mp.create_osd_overlay("ass-events")` to render an interactive
   on-screen button `[ Skip Intro ]` / `[ Skip Credits ]` /
   `[ Skip Commercials ]` with a `(Space)` keyboard hint in the bottom-right
   corner of the video window.
   While shown:
   - Observe `osd-dimensions` and recompute the visible button rectangle whenever
     the mpv window/OSD size changes; publish the same coordinates through
     `user-data/vela-markers/button-bounds` for deterministic E2E inspection.
   - Register a Vela-owned mouse input section and constrain it with
     `mp.set_mouse_area` to that exact rectangle; bind `MBTN_LEFT` inside the
     area only, so clicks elsewhere retain normal mpv behavior.
   - Force-bind **`SPACE`** under a Vela-owned binding name only while the button
     is displayed. Normal mpv Space-to-pause behavior resumes immediately when
     the button clears.
   - Mouse click and Space call the same `activate_skip` function. It marks the
     entry consumed, clears the overlay/mouse area/bindings, then executes
     `seek <end> absolute+exact`, so a clamped landing still inside the range
     cannot reactivate the button immediately.
6. **autoskip:** once per range entry, mark the entry consumed, seek with
   `absolute+exact`, and show a brief ordinary OSD toast. Do not loop-seek if
   mpv still reports a position inside the range.
7. Leaving range: clear the ASS overlay, active user-data property, force
   bindings, and current-entry latch.
8. Resume into a range: treat as inside → show button or autoskip immediately.
9. After at least one observed position outside that marker, seeking back into
   it is a new entry: show the button / allow one auto-skip again. Seeking
   within a consumed marker without first leaving does not re-arm it.
10. If `end` is past duration: seek to end-of-file / last frame safely (mpv
    clamps); do not error.

### Mouse & Keyboard Interaction (v1)

- **Primary:** left-clicking the visible `[ Skip Intro ]` / `[ Skip Credits ]` /
  `[ Skip Commercials ]` button executes the skip; its hit area must match the
  rendered rectangle.
- **Secondary:** pressing **Space** while the button is visible executes the
  same action. Space pauses normally whenever no skip button is visible.
- Document in Settings help text: "Click the on-screen skip button or press
  Space while it is visible."

---

## Playback integration

### Resolve path

Same as autocrop in `commands.rs`: `AppHandle` resolves
`mpv-scripts/vela-markers.lua`. `PlaySpec` gains exactly
`markers_script: Option<String>`, `markers: Vec<MediaMarker>`,
`intro_policy: SkipPolicy`, `credits_policy: SkipPolicy`, and
`commercial_policy: SkipPolicy`; all values are already resolved and
policy-filtered before `playback::play`.

### When to fetch markers

At play-prep for the item being launched, on the async side before
`spawn_blocking` / `playback::play`:

- Resolve all three policies before selected stream resolution.
- Pass `include_markers = true` when any of the three policies is not Off into
  the selected source's `resolve_stream` / `resolve_stream_version` call.
- Plex includes markers in its existing selected-detail response; Jellyfin
  starts its best-effort MediaSegments future alongside mandatory resolution;
  Emby and unsupported constructors return `[]`.
- Copy only markers whose kind is enabled into `PlaySpec`. If that normalized,
  policy-filtered list is empty, do not write a payload or inject the script.

### Arg construction

Pure helper (unit-tested, mirror `autocrop_args`):

```text
markers_args(script, intro_policy, credits_policy, commercial_policy, has_payload) -> Vec<String>
```

- No script path → `[]`
- All three policies Off → `[]` (even if script present)
- No successfully written payload → `[]`
- Else: `--script=…` plus the three closed policy options. The payload path is
  supplied separately through the child environment.

`play` existence-checks the script, then best-effort writes the payload. Missing
script or payload failure logs and injects nothing. Marker arguments are
appended after user mpv options so user configuration cannot replace Vela's
payload policies; IPC/title/auth invariants remain reasserted afterward as
today.

### Playlist / continue

No live marker IPC: each auto-advance re-enters the play command and rebuilds
`PlaySpec`. The Lua script has no `script-message vela-markers-set` path. If a
future architecture plays multiple URLs in one mpv process, marker replacement
requires a new owner decision and plan rather than dormant insurance now.

### IPC progress path

Do not couple skip seeks to Vela progress trackers beyond existing time-pos
observation. A skip is a normal seek; server check-in continues as today.

---

## Phased implementation slices

Each slice: one focused commit; run verification appropriate to the touch set;
red-proof any new behavioral guard (temporarily break production code, confirm
test fails for the right reason, restore from the committed pre-injection
state). Run `scripts/bump.sh` in every slice that changes shipped Rust,
frontend, or Lua behavior, per the active version decision. From the activation
base of 1.0.4 (the config-integrity prerequisite closeout) the four code slices
below land as 1.0.5 through 1.0.8; if the base moves again before a slice
starts, use the next patch each time rather than these numbers.
Because the bump script updates both JavaScript and Rust/bundle version
surfaces, finish every bumped slice with the full canonical dual-side CI command
set below even when its focused feature work touches only one side. Targeted
tests and red proofs run before that full set; real-app E2E becomes mandatory in
the product-flip slice where the feature is launchable.

### Slice 1 — Model + selected provider resolution

- `MediaMarker` / `MarkerKind`, `StreamResolution.markers`, and the
  `include_markers` resolve argument.
- Plex existing-detail marker inclusion + Jellyfin MediaSegments / Emby no-op,
  with exact HTTP and fixture tests.
- Shared normalize helper + tests.
- No UI, no mpv wiring yet.
- Run `scripts/bump.sh` (1.0.4 → 1.0.5 on the activation base).
- **Focused verify before the full set:** MSRV/stable check, clippy, tests, and
  Cargo audit from `src-tauri/`; red-prove the query/schema and
  marker-error-degrades guards separately.

**Slice 1 evidence (2026-07-25, version 1.0.5, commit `c7aa963`).**
`MarkerKind`, `MediaMarker`, `MAX_MARKER_MS`, and the shared
`normalize_markers` helper live in `src-tauri/src/source/mod.rs`;
`StreamResolution` carries `markers`, and `include_markers` is a required
argument on both `resolve_stream` and `resolve_stream_version`. Plex adds
`includeMarkers=1` to its existing selected-detail request through reqwest query
construction and maps `<Marker>` children at mapping time, so an unknown kind or
malformed offset drops that marker rather than the mandatory response; the
item-detail and version-enumeration call sites pass `false`. Jellyfin issues
`GET /MediaSegments/{id}` with the three repeated `includeSegmentTypes` filters
concurrently with the mandatory item fetch (`tokio::join!`), maps Outro to
Credits, converts 100ns ticks, and returns `Vec<MediaMarker>` with no error
channel — a marker failure structurally cannot fail a resolve. Emby issues no
request at all. The play command passes `include_markers = false` until the
Slice 3 config boundary and the Slice 4 product flip exist.

Canonical local verification passed at this commit: exact Node 26.5.0/npm 12.0.1
toolchain, `npm ci`, npm audit (0 vulnerabilities), `npm run check` (301 files,
0 errors), `npm run build`, `cargo +1.89.0 check --locked`, `cargo +stable
check/clippy --all-targets -D warnings`, 269 Rust tests (259 before this slice),
and `cargo audit` at its existing allowed-warning baseline.

Post-commit guard pass: fifteen regressions were injected one at a time and each
guard failed for its own reason, with every restore taken from the committed
state and verified clean — max-length bound removed; zero-length range accepted;
dedup ignoring kind; sort removed; Plex unknown kind accepted; Plex offsets read
as seconds; Plex malformed offset defaulting to 0; markers always requested;
markers never requested; segments route changed; commercial filter dropped;
Outro mapped to Intro; ticks not converted; marker failure escaping instead of
degrading; and the Emby flavor gate removed. The five whose assertions carry
custom messages were re-run individually to confirm each compiled and panicked
on its intended assertion rather than on a build error. No guard was vacuous.

**Slice 1 review and repair (2026-07-25).** `codex` at the literal slug
`gpt-5.6-sol` / xhigh (owner-named inline, session-only) reviewed exact range
`e7ea7dc..c7aa963` over MCP in a read-only sandbox and returned a schema-valid
verdict with both pins echoed: two MEDIUM findings, both admitted, none
declined. Both concerned best-effort marker I/O sitting on the playback critical
path — neither disputed the marker model or the parsing.

`mk-1` (`be32bde`, 1.0.6): the Jellyfin marker lookup inherited the general
15-second request timeout and the mandatory playback-info call did not start
until it finished, so a stalled marker endpoint delayed mpv launch. The lookup
now has its own 4-second bound, and both resolve paths join markers against the
full mandatory async block instead of the item fetch alone.

`mk-2` (`2971672`, 1.0.7): a Plex server that errored on
`/library/metadata/{id}?includeMarkers=1` while still answering the plain
request lost playback rather than markers. `PlexLibrary::get_item_detail` now
retries once without the parameter on failure, which covers every caller and
leaves the rediscovery structure untouched. This corrects the Plex **Failure**
bullet above, which had claimed no independent marker error existed.

Both fixes are independently red-proven from the committed state — removing
mk-1's bound made the stalled lookup take 16.16s and fail its assertion,
confirming the reviewer's mechanism exactly; disabling mk-2's fallback made the
detail fetch fail with the 500. The full canonical dual-side set passed at
1.0.7 with 271 Rust tests. No follow-up review was dispatched, so no clean
verdict is claimed for the repaired code.

### Slice 2 — Lua script + provenance

- Add `vela-markers.lua`.
- Update `PROVENANCE.md` (Vela MIT entry).
- Implement explicit overlay/binding/entry-latch behavior and child-env payload
  read/unlink. Full behavior is guarded when launch wiring lands.
- Run `scripts/bump.sh` (1.0.5 → 1.0.6 on the activation base).
- **Focused verify before the full set:** file present under resources;
  `tauri.conf.json` already maps
  `resources/mpv-scripts/` (no manual resource-map change); run mpv's script
  load against a minimal valid payload if practical and record when deferred to
  slice 4's real-app E2E.

**Slice 2 evidence (2026-07-25, version 1.0.8, commit `42ab254`).**
`src-tauri/resources/mpv-scripts/vela-markers.lua` is added and
`PROVENANCE.md` records it as Vela-authored MIT with no upstream ancestor,
stating that `LICENSE.GPL` covers stock `autocrop.lua` only. `tauri.conf.json`
already maps the whole `resources/mpv-scripts/` directory, so the new file
bundles with no resource-map change — confirmed by reading the existing mapping,
not assumed.

The script reads its payload path from `VELA_MARKERS_PAYLOAD` on the child
environment, never from the comma-split script-opts list; policies use the
explicit `vela-markers` identifier with dashed keys. Policy defaults are `off`
rather than the product's Button default: Vela always passes all three
explicitly, so the defaults are unreachable in a real launch, and `off` is the
only value that cannot make the player seek when it was never told to. An
unrecognized policy is treated the same way — the settings layer has already
rejected anything invalid before launch, so the player is the wrong place to
guess.

The button's hitbox is computed first and the box drawn to it, because ASS
exposes no text metrics and a control whose clickable area disagrees with its
pixels is worse than none. OSD space is window pixels, so the published
`button-bounds` rectangle is directly what the Slice 4 E2E clicks. `draw_button`
returns whether it actually reached the screen and the caller latches `armed`
only on success, so a tick arriving before the video output has published its
dimensions retries on the next tick instead of latching an invisible button.

Verified against real mpv 0.41.0 on the dev machine, since no automated harness
covers Lua: `luac -p` parses clean; loading the script with a two-marker payload
publishes `user-data/vela-markers/loaded = true` and the script removes the
payload file itself; a missing payload and an unparseable payload both yield
`loaded = false` with no mpv diagnostics, and the unparseable payload is still
removed — proving the unlink happens regardless of parse outcome. The full
canonical dual-side set passed at 1.0.8 with 271 Rust tests.

NOT verified here and deferred to Slice 4's real-app E2E, exactly as this slice
specifies: button rendering, the mouse hitbox, the temporary Space binding, seek
behavior, and the entry latch. The headless `--vo=null` venue publishes no OSD
dimensions, so no button can be drawn to assert against; `active` stayed empty
for that reason. No guard in the repo's automated suite covers this file yet.

### Slice 3 — Config + command boundary (no visible control yet)

- **Precondition (SATISFIED 2026-07-24):** the separately planned app-wide
  config-integrity/recovery prerequisite is implemented, reviewed, verified, and
  committed — `.agents/plans/config-integrity-recovery.md`, version 1.0.4.
- `skip_intros` / `skip_credits` / `skip_commercials` on `AppConfig`.
- Closed `SkipPolicy`; map missing fields to their approved per-kind defaults;
  extend `MpvAdvanced` get/set through the existing locked atomic config path.
  Do not expose the controls in Settings yet, so no shipped UI offers a setting
  that playback ignores.
- Serde round-trip / invalid-value rejection tests.
- Run `scripts/bump.sh` (1.0.6 → 1.0.7 on the activation base).
- **Focused verify before the full set:** Rust checks/tests/audit; red-prove
  missing-field defaulting and unknown-value rejection plus legacy-field
  round-trip preservation.

**Slice 3 evidence (2026-07-25, version 1.0.9, commit `f62345d`).**
`SkipPolicy` is a closed serde enum in `config.rs` with `SkipPolicy::MISSING`
(Button) as the single place the approved missing-field default is applied, and
`skip_intros` / `skip_credits` / `skip_commercials` are `Option<SkipPolicy>` on
`AppConfig` so absence stays distinguishable from an invalid value. `AppConfig`
already carries `deny_unknown_fields`, and the closed enum makes an
out-of-enum value fail deserialization, so no entry in `validate` was needed —
rejection happens before validation runs.

`MpvAdvanced` echoes all three already resolved, so the UI has a concrete value
to bind; `set_mpv_advanced` takes them as optional parameters for an older
frontend and writes through the existing `config::update` path under
`CONFIG_LOCK`. No second write path. No Settings control renders them yet, so
nothing shipped offers a setting playback ignores. Each policy is stored
explicitly rather than collapsed to `None` for the default, so a future change
to the product default cannot silently move a user who deliberately chose
today's.

Full canonical dual-side set passed at 1.0.9 with 275 Rust tests (271 before).
The three claimed behaviors were red-proven separately from the committed state,
each compiling and each restored clean: resolving the missing default to `Off`
failed the default guard; a tolerant deserializer for `skip_intros` failed the
unknown-value guard; and `skip_serializing` on `smb_mounts` failed the legacy
rollback guard (by index-out-of-bounds on the stripped vector rather than a
message, but for its own reason).

### Slice 4 — Atomic product flip: launch + Settings + E2E + docs

- Pass the resolved policy intent into selected resolution; policy-filter the
  returned markers; add `PlaySpec` policy/marker/script fields.
- Resolve `vela-markers.lua`; non-fatally create and clean the private payload;
  inject policy args + child environment through pure/testable helpers.
- Add all three Settings → Player controls in the same commit that makes them
  work.
- Extend the controlled Jellyfin mock's real route and add the behavioral E2E
  legs below. Update README Player notes with policies, clickable-button
  behavior, and the temporary in-range Space binding.
- Guarantees: play succeeds with missing script, empty markers, marker endpoint
  failure, payload write failure, or payload parse failure. An invalid policy
  never reaches play because config loading fails closed. Unit-test
  `markers_args` and payload-write failure matrices.
- Run `scripts/bump.sh` (1.0.7 → 1.0.8 on the activation base).
- **Verify:** full canonical dual-side set plus targeted and full Linux E2E.
  Red-prove every behavior claimed by the E2E separately.

**Slice 4 status (2026-07-25): production landed, BEHAVIOURAL VERIFICATION NOT
DONE.** The flip is committed as `5dd3e35` at version 1.0.10: `PlaySpec` carries
`markers_script`, `markers`, and the three resolved policies; `commands.rs`
resolves policies before stream resolution, passes `include_markers` only when a
policy is enabled, filters returned markers to enabled kinds, and resolves
`vela-markers.lua` through the same resolver as autocrop; `playback::play`
existence-checks the script, best-effort writes an owner-only payload, appends
the marker args after user options, and sets `VELA_MARKERS_PAYLOAD` on the child
only; a spawn failure or shutdown removes the payload. All three Settings →
Player controls and the README Player notes landed in the same commit, and
`mockjf.mjs` serves the real `/MediaSegments/{id}` route, recording a contract
violation when a required `includeSegmentTypes` filter is missing.

Verified: full canonical dual-side set at 1.0.10 with 278 Rust tests (275
before), covering the `markers_args` injection-polarity matrix and owner-only
payload creation.

NOT verified, and NOT to be treated as working: the five behavioural E2E legs in
**Behavioral E2E acceptance** above are neither written nor run. No skip button
has ever been rendered, clicked, or activated by Space; no auto-skip seek has
been observed; the commercial path is unexercised end to end. The macOS dev host
cannot run the suite (`.agents/machines.md`), and the Linux venue's clone was 16
commits behind at `95312fc` with local modifications, so it needs a checksum-
verified `scp` sync and a debug rebuild before any leg can run. Until those legs
pass, this slice is incomplete and the feature is unproven in a real player.

### Behavioral E2E acceptance

Use the existing generated 30-second Jellyfin clip and real mpv IPC harness;
extend `mockjf.mjs` to serve the exact MediaSegments query envelope. Do not add
a test-only production payload override.

1. **Auto-skip:** return an Intro range whose end is far enough ahead that
   normal playback cannot reach it before the assertion deadline. Seed
   `autoskip`, require the script load marker, and assert `time-pos` crosses the
   range end within that deadline.
2. **Button mouse activation:** seed `button`, require
   `user-data/vela-markers/active == "intro"`, and publish the rendered hitbox
   through `user-data/vela-markers/button-bounds`. On the Linux Xvfb venue,
   target the mpv window and inject a real pointer click at the hitbox center
   with `xdotool`; assert `time-pos` crosses the range end and the active
   property clears. Add `xdotool` to the E2E prerequisite documentation.
3. **Button Space activation:** relaunch the same controlled marker in Button
   mode, send `SPACE` through mpv IPC, assert the same seek/clear behavior, then
   prove a later Space outside any marker toggles pause normally.
4. **Injection polarity:** provider HTTP tests prove `include_markers = false`
   makes no Jellyfin MediaSegments request; launch unit tests prove all-Off,
   empty markers, absent script, and payload-write failure inject nothing.
5. **Commercial path:** the controlled Jellyfin response supplies a Commercial
   range on the generated clip; assert the configured policy uses the same real
   clickable-button and autoskip paths as intro/credits. No owner library item
   is required.

For red proofs, independently break the endpoint/schema mapping, commercial
kind mapping, auto-seek, mouse hitbox/click handler, Space binding/restoration,
and fail-open payload path; each claimed guard must fail for its own reason.
After automation is green, owner playtest a real Plex and Jellyfin title with
available intro/credits markers in Button and Auto-skip modes. Commercial
acceptance is automated because the owner has no commercial-marked item. Emby
is explicitly out of this playtest until its upstream contract exists.

---

## Verification plan (canonical)

Use `.agents/repo-guidance.md` **Verification** for the full dual-side set when
both frontend and Rust change. Minimum relevant set:

| Step | Command | Where |
|------|---------|--------|
| Toolchain | `node scripts/check-js-toolchain.mjs` | repo root |
| JS install | `npm ci` | repo root (when lock/deps or clean CI parity) |
| npm audit | `npm audit` | repo root |
| Frontend | `npm run check` | repo root |
| Frontend build | `npm run build` | repo root |
| MSRV | `cargo +1.89.0 check --locked` | `src-tauri/` |
| Stable check | `cargo +stable check --locked` | `src-tauri/` |
| Clippy | `cargo +stable clippy --all-targets --locked -- -D warnings` | `src-tauri/` |
| Tests | `cargo +stable test --locked` | `src-tauri/` |
| Rust audit | `cargo audit --file Cargo.lock` | `src-tauri/` |
| E2E | `npm run e2e` (or targeted scenario) | repo root; Linux venue per `.agents/machines.md` |

Guard discipline: every new behavioral claim above is red-proofed when
introduced, including provider query/schema, missing script, payload failure,
invalid-policy config rejection, mouse hit-testing/seek, temporary Space
activation/restoration, and auto-seek.

---

## Owner decisions

All rows are settled as of 2026-07-23. Any future product-choice row must be
asked in owner-facing chat and recorded here and in `.agents/decisions.md`;
recommended values are never implementation authority on their own.

| Topic | Recommended ruling | Alternatives | Status |
|-------|--------------------|--------------|--------|
| Default policy (intros & credits) | `button` | `off` or `autoskip` | **APPROVED — owner, 2026-07-22** |
| Confirm key while prompt shown | `SPACE`, force-bound only while displayed | different key; no keyboard activation | **APPROVED — owner, 2026-07-22** |
| Mouse click on OSD | required primary interaction with exact hit-testing | non-clickable notice | **APPROVED — owner, 2026-07-22** |
| Unknown config string | reject the whole config; notify and offer explicit backup-then-fresh-config recovery | normalize to `button` or `off` | **APPROVED — owner, 2026-07-22** |
| Commercial markers | support as a separate policy wherever the upstream server publishes ranges; synthetic tests are sufficient | ignore / unmodeled | **APPROVED — owner, 2026-07-22** |
| Default commercial policy | `button`, matching intro/credits while requiring confirmation | `off` or `autoskip` | **APPROVED — owner, 2026-07-23** |
| Live IPC marker updates | none; marker snapshot is fixed per mpv launch | add insurance now | **APPROVED — owner, 2026-07-23** |

Present overrides to the owner as single plain-English asks if any default is
contested; do not batch.

---

## Files likely touched

| Area | Paths |
|------|--------|
| Resolution / DTO | `src-tauri/src/source/mod.rs` |
| Plex | `src-tauri/src/source/plex.rs`, `src-tauri/src/plex_library.rs` |
| JF/Emby | `src-tauri/src/source/jellyfin.rs` |
| Config | `src-tauri/src/config.rs` |
| Play | `src-tauri/src/playback.rs`, `src-tauri/src/commands.rs` |
| Resources | `src-tauri/resources/mpv-scripts/vela-markers.lua`, `PROVENANCE.md` |
| UI | `src/lib/Settings.svelte` |
| E2E | `tests/e2e/mockjf.mjs`, marker scenario, mpv IPC helper, `tests/e2e/README.md` |
| User docs | `README.md` Player/HDR notes |
| Version | `scripts/bump.sh` (it owns the canonical version-surface set) |
| Durable state | this plan, `.agents/decisions.md`, `.agents/state.md` |

---

## Relationship to v1

| v1 | v2 |
|----|----|
| Hard `SkipPolicy` enum on `AppConfig` | Restored by owner ruling; missing is valid Button, unknown rejects config |
| CLI JSON in script-opts | Private payload file + child-only path environment |
| Mouse click in scope | Restored as the required primary interaction |
| Permanent `s` binding | Replaced by temporary in-button `SPACE`; pause restored outside |
| `emby.rs` | `jellyfin.rs` + `Flavor` |
| Commercial / Unknown kinds | Commercial restored by owner ruling; unknown kinds still dropped |
| Fail-closed launch wording | Degrade; never block play |
| Partial verification list | Full repo verification entry point |
| "Implementing" without state.md | Draft until state/decisions catch up |
| New GPL LICENSE for script | Vela MIT + PROVENANCE update only |

---

## Review log

- **2026-07-22 — v1 review:** architecture OK; config pattern, launch polarity,
  click/keybinding, argv JSON, Plex/JF endpoint specificity, Emby path, state
  drift, verification gaps. Closed by writing this v2.
- **2026-07-22 — v2 review:** found the wrong Jellyfin chapters contract,
  load-only E2E, under-specified fail-open payload lifecycle, avoidable Plex
  fetch, unresolved cold-implementer choices, and missing version/docs work.
  Closed in revision 2 with the exact MediaSegments contract, selected-resolve
  marker flow, child-env payload lifecycle, behavioral real-mpv E2E, settled
  implementation mechanics, and explicit version/docs slices. At that revision,
  product choices remained pending owner rulings.
- **2026-07-22 — owner ruling 1:** missing `skip_intros` and `skip_credits`
  values default to Button. The confirm key and unknown-string behavior were
  separate decisions later settled below.
- **2026-07-22 — owner ruling 2:** Button means a genuinely clickable mpv
  control, not a notice. Left-click is primary; Space activates the same skip
  action only while the button is displayed and resumes normal pause behavior
  afterward. This closes both the mouse and confirmation-key rows.
- **2026-07-22 — owner ruling 3:** an unknown marker policy invalidates the
  settings file; it never normalizes to Button or Off. The app-wide recovery
  contract is a user-facing damaged/tampered notice and an explicit
  backup-then-fresh-config action. This exposes conflicting tolerant/fallback
  behavior in current code, so a separate approved config-integrity/recovery
  plan is now a prerequisite rather than hidden scope in this feature.
- **2026-07-22 — owner ruling 4:** support commercial ranges wherever an
  upstream server publishes them; the owner's library does not need suitable
  test content. Current official evidence supports Plex and Jellyfin. Emby's
  published OpenAPI has no equivalent route, so Emby remains empty rather than
  guessed. Deterministic synthetic provider responses and generated-video E2E
  own commercial coverage.
- **2026-07-23 — owner ruling 5:** a missing commercial policy means Button,
  matching intro and credits. Commercial detection never auto-skips merely
  because an older config lacks the new field; the viewer must click the
  on-screen control or press Space while it is visible.
- **2026-07-23 — owner ruling 6:** marker data is fixed for one mpv launch.
  Vela does not push server-side marker additions or changes into the active
  player. Every later title launch, including automatic continuation, resolves
  a fresh snapshot through the normal play path.
