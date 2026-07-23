# Plan: Intro & credit marker skipping via mpv OSD (v2)

## Status

**Draft v2 — 2026-07-22.** Supersedes `.agents/plans/skip-credits-intros.md`
(v1). Incorporates the 2026-07-22 plan review. Self-contained for a cold
implementer.

Not active implementation until:

1. This plan (or an explicit owner go naming it) is reflected in
   `.agents/state.md` **Now/Next** and Active Sources, and
2. Any open owner decisions in **Owner decisions** below are settled or the
   stated defaults are accepted as binding.

v1 claimed "Owner-approved — implementing" without a matching `state.md` or
`decisions.md` entry; do not treat v1 status as authority over this file.

---

## Goal

When Plex or Jellyfin/Emby metadata includes intro or credits time ranges,
Vela offers skip during external-mpv playback:

- **Button** (product default): native mpv ASS/OSD prompt inside the video
  window ("Skip Intro" / "Skip Credits") with a keyboard confirm.
- **Auto-skip**: seek to marker end with a brief OSD toast.
- **Off**: no script injection for that kind.

HTML/webview overlays cannot sit on the external mpv window (decision
2026-05-23: external mpv for HDR). Skipping is implemented by a **Vela-authored**
bundled Lua script, following the same resource/injection pattern as
`autocrop.lua` / `vela-autocrop.lua`.

---

## Non-goals (v1 of the feature)

- Webview or Tauri overlays on the video frame.
- Mouse-click hit-testing on the ASS "button" (stretch only; keyboard is v1).
- Mid-title "Jump to credits" when playback is *outside* a credits marker.
- Editing, creating, or writing markers back to the server.
- Commercial / ad markers (ignored if present; not modeled).
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
  is therefore the primary path; live IPC marker refresh is optional insurance,
  not required for correct multi-item play.

---

## Architecture (end-to-end)

```
play command
  → MediaSource::markers(item_key)   // empty Vec / Err → continue without skip
  → resolve bundled vela-markers.lua // same Resource resolver as autocrop
  → if policy off for all kinds OR no usable markers OR script missing:
        play as today (no script)
  → else:
        write private markers payload file (process-private runtime dir)
        launch mpv with --script=… + script-opts (policies + payload path)
  → Lua: read payload, observe time-pos, OSD / seek per policy
```

**Degrade, never refuse play.** Missing script, unreadable payload, empty
markers, or `markers()` error must not fail `play`. Log at warn/info; launch
without the feature. Mirror autocrop: a missing `--script=` path would make mpv
refuse to start, so existence-check before injecting.

---

## Data model (`src-tauri/src/source/mod.rs`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MarkerKind {
    Intro,
    Credits,
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

No `Commercial`, no `Unknown` string variant in v1 — drop unrecognized server
types during parse. Keeps the Lua payload and policies small.

### Trait

```rust
/// Intro/credits time ranges for the item, if the backend exposes them.
/// Default: empty. Errors and empty lists both mean "no skip UI" — callers
/// must not fail playback on Err.
async fn markers(&self, _item_key: &str) -> Result<Vec<MediaMarker>, String> {
    Ok(Vec::new())
}
```

Callers: treat `Err` like empty (log once). Prefer logging the error string
without item URLs or tokens.

### Normalize rules (shared helper, unit-tested)

After parse, drop a marker if any of:

- `end_ms <= start_ms`
- range longer than a hard sanity cap (e.g. 30 minutes) — guards garbage data
- duplicate exact `(kind, start_ms, end_ms)` triples

Sort by `start_ms` ascending. Overlapping same-kind ranges: keep both; runtime
picks the first range that contains `time-pos` (see Lua rules).

---

## Server parsing

### Plex (`src-tauri/src/source/plex.rs` + `plex_library` as needed)

- **Endpoint:** same family as detail — `GET /library/metadata/{ratingKey}` with
  marker inclusion. Use `includeMarkers=1` (and keep existing Accept headers
  the client already uses). Do **not** require a full `item_detail` DTO mapping;
  a focused markers fetch or an extension of the existing metadata GET is fine
  as long as unit tests pin the query.
- **Nodes:** `Marker` children (XML or JSON, matching whatever the Plex client
  already parses for metadata).
- **Map:**
  - `type` / `type` field `intro` → `MarkerKind::Intro`
  - `credits` or `final_credits` → `MarkerKind::Credits`
  - anything else → skip
  - `startTimeOffset` → `start_ms`, `endTimeOffset` → `end_ms` (Plex units are
    milliseconds; assert in fixtures)
- **Tests:** fixture XML/JSON with intro + credits + unknown type + inverted
  range; empty markers; missing include.

### Jellyfin / Emby (`src-tauri/src/source/jellyfin.rs` only)

There is **no** `emby.rs`. Emby is `Flavor::Emby` on the same client.

- **Endpoint:** prefer data already available from a single-item GET if chapters
  are present; otherwise `GET /Items/{id}` / user-scoped item with chapter
  fields, or `GET /Items/{id}/Chapters` when needed. Implement the path that
  returns `Chapters[]` with `MarkerType` / start/end positions on current
  Jellyfin; Emby may return sparser or differently named fields — map what is
  present, degrade to empty when absent.
- **Map:**
  - `MarkerType` (or equivalent) `Intro` → Intro, `Credits` / `FinalCredits` → Credits
  - chapter `StartPositionTicks` → ms via `/ 10_000` (JF ticks = 100ns)
  - end: next chapter start, explicit end field if present, or skip if end cannot
    be determined (do not invent end = duration unless a fixture proves the
    server omits end and clients are expected to use duration)
- **Tests:** fixture chapter JSON for Intro+Credits; ticks conversion; Emby
  empty/missing MarkerType → empty list, not error.

---

## Config (`src-tauri/src/config.rs`)

Follow the **tolerant string + command-layer normalize** pattern used by
`mpv_autocrop`, `continue_playing`, and `playback_source_policy` so a
hand-edited or future value cannot make the credential-bearing config
unreadable.

```rust
// On AppConfig — NOT a hard serde enum:
/// Intro skip policy: "off" | "button" | "autoskip". Missing/unknown → product
/// default applied in the command layer (see normalize_skip_policy).
pub skip_intros: Option<String>,
/// Credits skip policy: same closed set as skip_intros.
pub skip_credits: Option<String>,
```

**Product defaults (binding unless owner overrides):**

| Field | Default when missing/unknown |
|-------|------------------------------|
| `skip_intros` | `button` |
| `skip_credits` | `button` |

Normalize helper (same spirit as `normalize_autocrop`):

- accept only lowercase `off`, `button`, `autoskip` after trim
- anything else → product default for that field (or `off` if the autocrop-style
  "unknown fails closed to safest" is preferred — **v2 binds unknown → product
  default `button`**, matching "missing means default product behavior"; document
  in Settings that unknown stored values reset to Button on next save if the UI
  rewrites them)

Round-trip tests: old configs without these fields load; save preserves other
fields; legacy inert local/SMB/SSH fields still untouched.

### Settings UI

- **Location:** Settings → **Player** tab (beside black-bar cropping / mpv
  advanced), in `src/lib/Settings.svelte`.
- Two selects: "Skip intros", "Skip credits" — options Off / Button / Auto-skip.
- Wire via extending `get_mpv_advanced` / `set_mpv_advanced` **or** a small
  dedicated pair `get_skip_policies` / `set_skip_policies`. Prefer extending the
  existing mpv-advanced command if the payload stays cohesive; otherwise a
  dedicated command is fine. Do not invent a third config write path that skips
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

Launch args must use the same prefix, e.g.:

- `--script-opts-append=vela-markers-payload=<path>`
- `--script-opts-append=vela-markers-intro-policy=button`
- `--script-opts-append=vela-markers-credits-policy=button`

Do **not** put the full marker JSON on the command line.

### Payload file

- Written under the existing process-private runtime directory pattern used for
  IPC sockets / auth includes (`playback.rs` private runtime dir on Unix;
  appropriate per-user temp on Windows).
- Owner-only permissions where the OS allows.
- Contents: compact JSON, e.g.
  `{"markers":[{"kind":"intro","start_ms":0,"end_ms":90000},...]}`
- Path only on argv. Overwrite per launch; best-effort delete on mpv exit (same
  discipline as other per-launch temps — document if cleanup is best-effort only).
- Size: markers lists are small; still avoid argv for the body (quoting, length,
  consistency with auth-include lesson).

### Runtime behavior

1. On load: read options; open payload path; parse JSON; on any failure, set
   loaded marker false-equivalent and no-op (do not crash mpv).
2. Set `user-data/vela-markers/loaded` = true when script is active with a
   successful parse (for E2E), even if markers array is empty.
3. Observe `time-pos` (or equivalent).
4. Determine active marker: first range where
   `start_ms/1000 <= t < end_ms/1000` and the kind's policy is not `off`.
5. **button:** show ASS/OSD bottom-right, e.g. `Skip Intro (S)` / `Skip Credits (S)`.
   While the prompt is shown, force-bind **`s`** to seek to `end_ms/1000`
   (absolute). **Unbind when leaving the range** so stock screenshot `s` works
   outside skip windows. Prefer `seek … absolute+exact` if available on the
   mpv version floor Vela already assumes; otherwise absolute and document
   keyframe skew as accepted.
6. **autoskip:** once per range entry, seek to end; brief OSD "Skipped Intro" /
   "Skipped Credits"; do not loop-seek if still inside due to keyframe snap
   (mark range as consumed for this load).
7. Leaving range: clear OSD; clear force binding.
8. Resume into a range: treat as inside → show button or autoskip immediately.
9. User seeks back into a previously skipped range: **show button again** /
   allow re-autoskip once per re-entry (simple; no permanent dismiss).
10. If `end` is past duration: seek to end-of-file / last frame safely (mpv
    clamps); do not error.

### Keyboard (v1)

- Confirm key while prompt visible: **`s`** (force-bound only in-range).
- No mouse hit-test in v1.
- Document in Settings help text: "While the skip prompt is visible, press S."

---

## Playback integration

### Resolve path

Same as autocrop in `commands.rs`: `AppHandle` resource resolve

`mpv-scripts/vela-markers.lua` → `PlaySpec` field(s), e.g. `markers_script: Option<String>`.

### When to fetch markers

At play-prep for the item being launched (alongside stream resolve), on the
async side before `spawn_blocking`/`play`:

- `source.markers(&raw_key).await` — on `Err`, log and use `[]`.
- If both normalized policies are `off`, skip fetch (optional optimization).
- If markers empty after normalize, do not inject script.

### Arg construction

Pure helper (unit-tested, mirror `autocrop_args`):

```text
markers_args(script, intro_policy, credits_policy, payload_path) -> Vec<String>
```

- No script path → `[]`
- Both policies `off` → `[]` (even if script present)
- Else: `--script=…`, policy opts, payload path opt

`play` existence-checks script path; if missing, log and inject nothing.

### Playlist / continue

No live marker IPC required for v1: each auto-advance re-enters the play
command and rebuilds `PlaySpec`. Optional later: `script-message vela-markers-set`
if a single mpv process ever plays multiple URLs without respawn.

### IPC progress path

Do not couple skip seeks to Vela progress trackers beyond existing time-pos
observation. A skip is a normal seek; server check-in continues as today.

---

## Phased implementation slices

Each slice: one focused commit; run verification appropriate to the touch set;
red-proof any new behavioral guard (temporarily break production code, confirm
test fails for the right reason, restore).

### Slice 1 — Model + server parsing

- `MediaMarker` / `MarkerKind` + `MediaSource::markers` default.
- Plex + Jellyfin/Emby implementations with fixture unit tests.
- Shared normalize helper + tests.
- No UI, no mpv wiring yet.
- **Verify:** `cargo +stable test --locked`, `cargo +stable clippy … -D warnings`
  from `src-tauri/`.

### Slice 2 — Lua script + provenance

- Add `vela-markers.lua`.
- Update `PROVENANCE.md` (Vela MIT entry).
- Manual or scripted mpv smoke optional; unit-level not applicable to Lua beyond
  later E2E load marker.
- **Verify:** file present under resources; `tauri.conf.json` already maps
  `resources/mpv-scripts/` (no conf change if directory already bundled).

### Slice 3 — Config + Settings UI

- `skip_intros` / `skip_credits` on `AppConfig`.
- Normalize + get/set command(s) + Settings → Player dropdowns.
- Serde round-trip / unknown-value tests.
- **Verify:** Rust tests + `npm run check` (+ `npm run build` if UI types change).

### Slice 4 — Launch wiring

- Fetch markers at play; write payload file; inject args via pure helper.
- PlaySpec + resolve_resource for `vela-markers.lua`.
- Guarantees: play succeeds with missing script, empty markers, bad policy
  strings.
- Unit tests for `markers_args` matrix (off/off, button+markers, no script, …).
- **Verify:** full Rust test/clippy; dual-side if commands/UI already landed.

### Slice 5 — Verification & E2E

- E2E: assert `user-data/vela-markers/loaded` (or IPC-equivalent) when policies
  enable injection and a synthetic/fixture path can supply markers **or** when
  script is injected with empty markers payload if that still sets loaded.
  Prefer: force a test payload in the harness when possible.
- Do **not** claim E2E "skipped real Netflix-style intro" without a controlled
  media fixture; launch + load marker is the automation bar.
- Owner playtest: real Plex/JF title with markers, both policies.

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

Guard discipline: any test that claims "missing script does not break play" or
"unknown policy normalizes" must be red-proofed once when introduced.

---

## Owner decisions

Defaults below are **binding for implementation** unless the owner overrides in
chat and the override is recorded here / in `decisions.md`.

| Topic | Binding default | Alternatives |
|-------|-----------------|--------------|
| Default policy (intros & credits) | `button` | `off` or `autoskip` |
| Confirm key while prompt shown | `s`, force-bound only in-range | different key; never rebind `s` |
| Mouse click on OSD | **out of v1** | stretch after keyboard ships |
| Unknown config string | normalize to product default `button` | fail closed to `off` |
| Commercial markers | ignore / unmodeled | separate policy later |
| Live IPC marker updates | not required (respawn per item) | add if process reuse appears |

Present overrides to the owner as single plain-English asks if any default is
contested; do not batch.

---

## Files likely touched

| Area | Paths |
|------|--------|
| Trait / DTO | `src-tauri/src/source/mod.rs` |
| Plex | `src-tauri/src/source/plex.rs`, possibly `plex_library.rs` / `plex_api.rs` |
| JF/Emby | `src-tauri/src/source/jellyfin.rs` |
| Config | `src-tauri/src/config.rs` |
| Play | `src-tauri/src/playback.rs`, `src-tauri/src/commands.rs` |
| Resources | `src-tauri/resources/mpv-scripts/vela-markers.lua`, `PROVENANCE.md` |
| UI | `src/lib/Settings.svelte`, possibly `src/lib/types.ts` |
| E2E | `tests/e2e/…` (scenario + harness property read) |
| Bundle | `src-tauri/tauri.conf.json` only if resource map changes (unlikely) |

---

## Relationship to v1

| v1 | v2 |
|----|----|
| Hard `SkipPolicy` enum on `AppConfig` | `Option<String>` + normalize |
| CLI JSON in script-opts | Private payload file + path in script-opts |
| Mouse click in scope | Keyboard-only v1 |
| Permanent `s` binding | In-range force-bind only |
| `emby.rs` | `jellyfin.rs` + `Flavor` |
| Commercial / Unknown kinds | Dropped |
| Fail-closed launch wording | Degrade; never block play |
| Partial verification list | Full repo verification entry point |
| "Implementing" without state.md | Draft until state/decisions catch up |
| New GPL LICENSE for script | Vela MIT + PROVENANCE update only |

---

## Review log

- **2026-07-22 — v1 review:** architecture OK; config pattern, launch polarity,
  click/keybinding, argv JSON, Plex/JF endpoint specificity, Emby path, state
  drift, verification gaps. Closed by writing this v2.
- **v2 plan review:** not yet run.
