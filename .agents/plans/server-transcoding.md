# Plan: server-side transcoding

## Status

**Draft v2, 2026-07-25. NOT yet approved for implementation.** Every product
choice in **Owner decisions** is now ruled and recorded in
`.agents/decisions.md`. What still blocks implementation is evidence, not
choices:

1. ~~Plex's decision endpoint and session lifecycle~~ — VERIFIED 2026-07-25
   against the owner's live server; see **Provider contracts**. `ping`/`stop` do
   not exist; teardown is `DELETE /transcode/sessions/<uuid>`.
2. ~~Plex's ladder tier values~~ — CONFIRMED 2026-07-25 from a live client; the
   table is in `.agents/decisions.md`. Tiers are resolution+bitrate pairs, and
   two share a label, so bitrate must always be displayed.
3. ~~Implementation slices~~ — WRITTEN 2026-07-25; see **Implementation
   slices**. Six slices, 1.0.12 through 1.0.17.
4. Emby's transcode contract remains unverified and stays best-effort per the
   2026-07-25 ruling.

Nothing now blocks implementation on evidence or on product choices. **The owner
has not said "implement".** Do not start code on the strength of the rulings
alone; this plan becomes active only when `.agents/state.md` names it as the
active implementation, the same gate the marker plan used.

The requirement is owner-stated (2026-07-25): Vela is direct-play only today and
must be able to ask the server to transcode. Two concrete drivers, both real for
the owner, not hypothetical:

- A 2010 iMac running Arch cannot decode much of the library. Its display has no
  HDR, so HDR passthrough is worthless there and transcoding is the primary
  playback path, not a fallback.
- Remote playback over a constrained link. Plex Relay in particular is
  bandwidth-capped, so a 4K remux cannot stream over it.

---

## Invalidated premise

`.agents/decisions.md` 2026-07-19 ("Duplicate-copy playback is policy-driven")
defines **Prefer Compatible** as choosing a compatible *copy* from among
duplicate versions. The owner keeps ONE copy per title, so that mode is inert
for this library and cannot be the answer to compatibility. Whether Prefer
Compatible is retired, redefined against transcoding, or left alone is an open
decision below. Do not silently repurpose it.

---

## Model (owner-settled)

Settled in owner-facing chat 2026-07-25:

1. **Quality is a target bitrate**, mirroring how Plex works — not resolution
   buckets and not a Vela-invented scale.
2. **The control is situational, not persistent per file and not a configured
   per-machine profile.** The same laptop is on a café link one day and 10GbE
   the next; the user switches the current setting when circumstances change and
   everything played uses it until they switch again.
3. **Selecting a transcode must be frictionless** — a control the user can reach
   quickly when they decide they want it, not a Settings excursion.
4. **An automatic decision is available but opt-in**, never the default
   behaviour.
5. **Never offer an option the server cannot deliver for that file.** The menu is
   derived per play from what the server reports, not from a hardcoded ladder.

Explicitly REJECTED by the owner, with reasons; do not re-propose:

- A per-machine quality profile as the trigger — too broad.
- Automatic client-capability detection as the default — brittle.
- Prompting on every play — annoying.
- Deciding from mpv's own playback health and caching the verdict per file —
  insufficient user control, and per-file state is wrong because the situation
  changes, not the file.
- A fixed Vela-authored bitrate ladder — diverges from what the server offers.

---

## Provider contracts (verified 2026-07-25 unless marked)

### Plex

- Transcode stream: `GET /{video|audio}/:/transcode/universal/start.{m3u8|mpd}`.
  Query parameters observed in `python-plexapi` `Playable.getStreamURL`
  (`plexapi/base.py`): `path` (the item key), `mediaIndex`, `partIndex`,
  `protocol` (`hls` default, `dash` selects `.mpd`), `fastSeek`, `copyts`,
  `offset`, `maxVideoBitrate` (floored at 64), `videoResolution` (must match
  `^\d+x\d+$`), `X-Plex-Platform`. The token is appended by the server-URL
  helper, not passed in the parameter map. `directPlay`, `directStream`,
  `session`, `subtitleSize` and `audioBoost` are NOT named in that helper and
  reach the URL only through its `**kwargs` passthrough.
- **Decision endpoint — VERIFIED against the owner's live server 2026-07-25.**
  `GET /video/:/transcode/universal/decision` with the same parameter set as
  `start` returns 200 and a `MediaContainer` carrying
  `generalDecisionCode`/`generalDecisionText`,
  `directPlayDecisionCode`/`directPlayDecisionText`, and
  `transcodeDecisionCode`/`transcodeDecisionText` (observed: general 1001
  "Direct play not available; Conversion OK", directPlay 3000 "Direct play is
  disabled" when the request sets `directPlay=0`). Its `Metadata[0].Media[0]`
  describes the stream the server WOULD produce — `videoResolution`, `bitrate`,
  `protocol`, `container`, plus codec and dimension fields. This is how Vela
  learns whether a file can be direct-played and what a given tier would
  actually yield, without starting anything.
- **Session lifecycle — VERIFIED, and the common lore is wrong for this server.**
  `/video/:/transcode/universal/ping` and `.../stop` **do not exist — both 404.**
  The real shape:
  1. The CLIENT generates the session id and passes it as `session=<uuid>`.
  2. `GET /video/:/transcode/universal/start.m3u8?...&session=<uuid>` returns
     200 with an `#EXTM3U` playlist and creates the session.
  3. `GET /transcode/sessions` lists active sessions, keyed by that same uuid.
  4. **`DELETE /transcode/sessions/<uuid>` returns 204 and tears it down.**
  Observed session count across the probe: 0 → 1 → 0. Vela MUST issue that
  DELETE when playback ends or fails, or it orphans transcodes on the server.
  No keep-alive ping was found; segment fetches appear to be what keeps a
  session live, so an abandoned session's expiry behaviour is still unknown and
  is the reason the explicit DELETE is mandatory rather than optional.
- **Quality ladder — CONFIRMED client-side.** Plex does not publish it through
  the API; the values were read off a live client and are recorded in
  `.agents/decisions.md`, along with the resolution-only filtering rule.

### Jellyfin / Emby

- Transcode stream: `GET /Videos/{itemId}/master.m3u8` (variant:
  `/Videos/{itemId}/main.m3u8`), per `Jellyfin.Api/Controllers/
  DynamicHlsController.cs`. `mediaSourceId` is REQUIRED on the master route.
  Client-supplied constraints are explicit query parameters: `videoBitRate`,
  `audioBitRate`, `maxWidth`, `maxHeight`, `maxFramerate`, `videoCodec`,
  `audioCodec`, `segmentContainer`, `maxVideoBitDepth`, `requireAvc`,
  `maxAudioChannels`, plus `playSessionId`, `deviceId`, `startTimeTicks`,
  `segmentLength`, `minSegments`, and the stream-copy switches
  (`enableAutoStreamCopy`, `allowVideoStreamCopy`, `allowAudioStreamCopy`, all
  defaulting true).
- `deviceProfileId` is present but marked obsolete and is never read into the
  request DTO. **Do not build a DLNA-style DeviceProfile negotiation for the
  HLS route**; capabilities travel as explicit parameters.
- `POST|GET /Items/{itemId}/PlaybackInfo` reports per-media-source capability.
  Vela already calls this (`JellyfinClient::playback_info_response`) and already
  parses `SupportsDirectPlay`, `SupportsDirectStream`, and `Bitrate` into
  `MediaSourceInfo`. It does NOT parse `SupportsTranscoding`, `TranscodingUrl`,
  `TranscodingSubProtocol`, or `TranscodeReasons` — that is the first gap.
- Emby shares the ancestry but its transcode contract was NOT verified. Do not
  assume Jellyfin's routes work there; check its published OpenAPI first, the
  same discipline the marker plan applied to MediaSegments.

### Vela today

- Plex plays the direct part URL (`PlexLibrary::part_url_for_media`); split-file
  versions become an `edl://` join of parts.
- Jellyfin/Emby play `/Videos/{id}/stream?static=true` — direct stream, no
  transcode.
- Neither path can currently produce a transcode URL, and no code anywhere
  requests one (`grep -i transcod` over `src-tauri/src` matches only artwork
  transcoding and one comment).
- Remote Plex already works: `server_candidate_priority` ranks `plex.direct`
  local, `plex.direct` remote, other local, other remote, then Relay last and
  only when relay is allowed; HTTPS is required. Jellyfin/Emby take whatever
  base URL the user supplies and have no discovery or relay, so remote requires
  the user's own reachability (VPN or TLS reverse proxy).

---

## Deriving the menu (owner rule 5)

Per play, before offering anything:

1. Ask the server what it can do with this exact media source — direct play,
   direct stream, transcode — and read the source resolution and bitrate.
2. Offer **Original** only when direct play/stream is actually available, and
   label it with the source bitrate and resolution the way Plex does.
3. Offer transcode steps only when the server reports transcoding is available
   AND permitted for the account, filtered **by resolution**: a tier appears
   when its resolution is at or below the source's.
4. When the server can only direct-play, Original is the sole entry; when it can
   only transcode, Original is absent. The menu never contains an entry that
   would fail.

**Do not add a bitrate filter.** Confirmed against three live Plex samples
(`.agents/decisions.md`): a 10 Mbps 1080p source still offers the 20 Mbps and
12 Mbps tiers, and a 1.5 Mbps 384p source drops the 1.5 Mbps 480p tier despite
the identical bitrate. Resolution is the only axis Plex filters on. An earlier
revision of this plan said "only steps strictly below the source bitrate" — that
was inferred from one 4K sample and is wrong.

Every entry must show its bitrate, since two tiers share the label
"Convert to 1080p HD" and are otherwise indistinguishable.

---

## Interactions to resolve before implementation

- **HDR.** Transcoding tone-maps to SDR, forfeiting the reason playback is
  external mpv at all (`.agents/decisions.md` 2026-05-23, reaffirmed
  2026-07-14). On the iMac that cost is zero. On an HDR display it is the whole
  product. Any automatic path must not silently destroy HDR.
- **Chapters.** A transcode drops container chapters, which mpv otherwise reads
  and navigates for free on a direct-played file.
- **Markers.** Intro/credits/commercial ranges are server-side and time-based,
  so they remain valid under transcode. `.agents/plans/skip-credits-intros-v2.md`
  needs no change, but the E2E should not assume a direct-play URL.
- **Progress/resume.** Both backends already check in by item/session; a
  transcode adds a session id to keep alive, and Plex's session lifecycle is the
  unverified part above.
- **Split-file Plex media — RULED 2026-07-25, finding `tr-9`.** Vela joins Parts
  as `edl://` for direct play, while a transcode URL addresses ONE part index, so
  converting a split-file version would end the film at the first part boundary.
  Vela **refuses to convert** such a version: `PlexLibrary::conversion_possible`
  is false for anything other than exactly one part, `transcode_url` returns
  `None` so a truncating URL cannot be constructed at all, `playback_options`
  reports no transcoding so the menu never offers it, and a Settings-level
  quality request degrades to Original with a log naming the reason. **Real
  multi-part transcoding is DEFERRED** and remains unimplemented; it would need
  per-part sessions stitched back into an EDL, which is not in any slice below.

---

## Owner decisions

All settled 2026-07-25 and recorded in `.agents/decisions.md`; the entries there
are canonical and this list points at them.

1. **Per-title override — RULED.** A one-off context-menu choice that applies to
   the play it starts and saves nothing. Quality nests under version: two or more
   versions give `Play Version >` (servers, each expanding to that server's
   deliverable qualities); a single version gives `Play at Quality >` directly.
   The two labels never appear together, and the item is absent when the only
   version cannot be transcoded.
2. **Automatic — RULED.** An opt-in value of the quality setting, never the
   default. A play starts at Original; sustained decoder frame drops or a
   repeatedly starving demuxer cache step it down the ladder and resume at the
   current position. Nothing is remembered between plays. Thresholds, the
   observation window, and how many steps a single play may take are
   implementation detail for this plan to specify, not further owner decisions.
3. **Step values — RULED.** Plex's own ladder, the same list for every source
   kind, filtered per file to steps below the source bitrate and only when that
   server can transcode it. **The exact tier values are still unconfirmed and
   must be read off a current Plex client before implementation** — Plex does not
   publish them through its API.
4. **Placement — RULED.** Settings > Player. Being a normal setting it persists
   across restarts, which also settles the former open question 5.
5. *(folded into 4)*
6. **Prefer Compatible — RULED.** Unchanged behaviour; it is inert rather than
   broken for single-copy libraries. The UI must make the two controls
   non-competing: the duplicate-copy mode chooses WHICH COPY plays, the quality
   setting chooses HOW it is delivered, and label and help text say so.
7. ~~**Emby scope**~~ — RULED 2026-07-25: Emby transcoding is implemented
   best-effort and labelled limited, consistent with Emby's existing
   experimental status (`.agents/decisions.md` 2026-07-15). The owner has no
   Emby server, so it cannot be exercised; the UI and README say so and invite
   users to report issues rather than implying verified support. Do not block
   the feature on Emby, and do not claim Emby works.

---

## Implementation slices

Base version at planning time is 1.0.11, so these land 1.0.12 through 1.0.17;
if the base moves first, use the next patch each time rather than these numbers.
Every slice runs the full canonical dual-side set because `scripts/bump.sh`
touches both version surfaces, and every slice red-proves each behaviour it
claims SEPARATELY, from a committed state, per `.agents/repo-guidance.md`
(Guard discipline).

### Slice 1 — capability model and the tier ladder (no UI, no playback change)

- Add a provider-neutral `PlaybackOptions` to `src-tauri/src/source/mod.rs`:
  `can_direct_play: bool`, `can_transcode: bool`, `source_width/height`,
  `source_bitrate_kbps`, and `tiers: Vec<QualityTier>`.
- `QualityTier` is a closed set carrying label, bitrate and resolution, seeded
  from the confirmed Plex ladder in `.agents/decisions.md`. Ship the table as
  data, not scattered constants.
- Implement the filter: a tier is offered when its resolution is **at or below**
  the source's. **No bitrate filter** — see the plan section above.
- Plex: call `/video/:/transcode/universal/decision` and read
  `generalDecisionCode` plus `Metadata[0].Media[0]`.
- Jellyfin/Emby: extend `MediaSourceInfo` with `SupportsTranscoding`,
  `TranscodingUrl`, `TranscodeReasons` — currently unparsed.
- Nothing calls this from the play path yet.
- **Guards, proven separately:** the three live samples become fixtures — 2160p
  offers all nine convert tiers, 1080p/10 Mbps offers all nine INCLUDING the 20
  and 12 Mbps entries, 384p/1.5 Mbps offers only 328p and specifically drops the
  equal-bitrate 480p tier. Plus decision-response parsing, and Jellyfin
  capability parsing.

**Slice 1 evidence (2026-07-25, version 1.0.12, commit `499ab0b`).**
`QualityTier`, `QUALITY_TIERS` (the nine confirmed rungs), `tiers_for_source`,
and `PlaybackOptions` live in `src-tauri/src/source/mod.rs`. Jellyfin's
`MediaSourceInfo` now parses `SupportsTranscoding`, `TranscodingUrl` and
`TranscodeReasons`, and `playback_options()` maps them; transcoding is offered
only when the server says so, falling back to the presence of a server-built
transcode URL and otherwise assuming NO. `PlexLibrary::transcode_decision` calls
the verified decision endpoint and `DecisionContainer::conversion_ok` fails
closed on an unreadable answer.

Two facts were established by probing the live server during this slice and are
encoded in the code's comments: `videoResolution` is a bounding BOX, not the
output size — a 2.35:1 source asked for `1920x1080` returned `1920x1038`, and
`1280x720` at 2000 kbps returned `720x388` because bitrate bound first — and all
nine tier parameter pairs are accepted with `generalDecisionCode` 1001. The
480p/328p box widths (`848x480`, `584x328`) are Vela's choice, accepted by the
server; Plex's own client may send different widths, which is immaterial because
the server refits to the source aspect regardless.

Nothing calls any of this yet, by design, so the play path is byte-for-byte
unchanged. That required scoped `#[allow(dead_code)]` on exactly the new items,
each commented `TEMPORARY, remove in slice 3` — **slice 3 must remove them**; do
not widen them to a module or crate allow.

Canonical dual-side set passed at 1.0.12 with 285 Rust tests (279 before). Seven
regressions were injected separately from the committed state and each guard
failed for its own reason, every injection compiling and every restore verified
clean: a reintroduced bitrate filter; a resolution filter loosened past the
384p boundary; duplicated tier ids; tiers offered without transcode support; a
missing transcode flag assumed true; Jellyfin's bits-per-second left
unconverted; and an unreadable decision treated as permission.

### Slice 2 — the quality setting (config + Settings, still inert)

- `playback_quality` on `AppConfig` as a closed enum: `Original`, a tier
  identifier, or `Automatic`. **Missing means `Original`**, preserving today's
  direct-play behaviour and HDR passthrough for every existing install.
- Extend the existing `MpvAdvanced` get/set pair rather than adding a command.
- Settings > Player control, with help text that distinguishes it from the
  duplicate-copy mode per the 2026-07-25 ruling: that mode chooses WHICH COPY,
  this one chooses HOW IT IS DELIVERED. Neither may imply it governs the other.
- **CORRECTED (finding `tr-1`, 2026-07-25): the control does not SHIP in this
  slice.** As first written this slice put a working-looking selector in front
  of a play path that ignored it, which is exactly what
  `.agents/plans/skip-credits-intros-v2.md` slice 3 forbids — "no shipped UI
  offers a setting playback ignores". The control is authored here but withheld
  behind `QUALITY_CONTROL_READY` in `Settings.svelte`; slice 3 flips that flag
  and deletes the guard. Apply the same rule to any future slice: a control
  ships in the slice that makes it work, never before.
- Every entry displays its bitrate — two ladder tiers share a label.
- No playback behaviour changes in this slice.
- **Guards, proven separately:** missing field resolves to `Original`; an
  unknown stored value invalidates the document; round-trip of every value;
  legacy rollback fields untouched.

**Slice 2 evidence (2026-07-25, version 1.0.13, commit `9f87475`).**
`playback_quality: Option<String>` on `AppConfig`, following the repo's existing
validated-string pattern rather than a new enum, so it reuses
`validate_optional_closed`. Its valid set is BUILT FROM `QUALITY_TIERS` plus
`original` and `automatic`, so a tier that ever leaves the ladder cannot linger
in a config as a value nothing can honour. `config::playback_quality()` is the
single place the missing-field default is applied, and it treats an empty string
as absent. `set_mpv_advanced` rejects an unknown value rather than writing one
the loader would later refuse.

`MpvAdvanced` carries the resolved value plus the whole ladder, so Settings
renders labels and bitrates without duplicating the table in TypeScript. The
control shows the bitrate on every entry — mandatory, since two tiers share a
label.

Both halves of the 2026-07-25 Prefer Compatible ruling are implemented: the
duplicate-source section now opens with "This chooses **which copy** plays, not
how it is delivered", notes it does nothing for single-copy libraries, and
Prefer Compatible's summary reads "Pick the copy that best matches…"; the
quality control's help says it governs how the copy is **delivered**, names the
HDR and chapter cost, and says it is situational rather than per-title.

Nothing reads the setting at play time yet — playback is unchanged.

Canonical dual-side set passed at 1.0.13 with 287 Rust tests (285 before). Five
regressions were injected separately from the committed state, each compiling,
each restored clean: the missing-field default changed to a tier; an empty
string accepted as a value; the closed validation removed; the valid set
hand-listed instead of derived from the ladder; and the legacy rollback fields
stripped on save.

### Slice 3 — transcoded playback and session teardown

- Build the transcode URL per provider: Plex
  `/video/:/transcode/universal/start.m3u8` with a **client-generated session
  uuid**, `maxVideoBitrate`, `videoResolution`, `protocol=hls`, `offset`,
  `copyts`, `fastSeek`; Jellyfin `Videos/{id}/master.m3u8` with `videoBitRate`,
  `maxWidth`/`maxHeight`, the required `mediaSourceId`, and `playSessionId`.
- `StreamResolution` carries the quality actually used, so the layer above knows
  whether this play is transcoded.
- **Remove `QUALITY_CONTROL_READY`** from `Settings.svelte` (flag, the `{#if}`
  guard, and the now-redundant assertions in `tests/transcoding-ui.test.mjs`) —
  this is the slice that earns the control (finding `tr-1`).
- **Remove every `#[allow(dead_code)]` marked `TEMPORARY`** across
  `source/mod.rs`, `plex_library.rs` and `source/jellyfin.rs`.
- **Teardown is mandatory:** `DELETE /transcode/sessions/<uuid>` when the child
  exits, when launch fails, and on shutdown. Model it on `HeaderInclude`'s Drop
  guard in `playback.rs`, which already solves exactly this lifetime problem.
  There is no keep-alive ping; an abandoned session's expiry is unknown, which
  is why this cannot be best-effort.
- The Settings value now takes effect.
- **Guards, proven separately:** URL construction per provider; a unique session
  id per launch; teardown fires on normal exit; teardown fires on spawn failure.

### Slice 4 — the per-title one-off menu

- Context menu per the 2026-07-25 ruling: `Play Version >` with servers
  expanding to qualities when there are two or more versions, `Play at Quality >`
  listing qualities directly when there is one, never both, and absent entirely
  when the only version cannot be transcoded.
- Resolve options **lazily when the submenu opens**, not when the context menu
  opens — the Plex decision call is a network round trip per version and must
  not be paid on every right-click.
- The choice applies to the play it starts and persists nothing.
- **Guards, proven separately:** single-version menu shape; multi-version
  nesting; no persistence after a one-off play; no decision request until the
  submenu is opened.

### Slice 5 — Automatic

- Only active when the setting is `Automatic`. The play starts at `Original`.
- Watch mpv over the existing IPC connection for the two signals ruled on
  2026-07-25: sustained decoder frame drops (`decoder-frame-drop-count`) and a
  repeatedly starving demuxer cache (`demuxer-cache-duration`, `cache-speed`).
  Specify the window and thresholds here when implementing; they are
  implementation detail, not owner decisions.
- On either signal, step down one tier and resume at the current position.
- **Persist nothing.** The next play starts at `Original` again.
- **Guards, proven separately:** a drop-storm triggers a step down; a starving
  cache triggers a step down; a healthy play triggers neither; nothing is
  written to config afterwards.

### Slice 6 — Emby labelling and documentation

- Emby transcoding is implemented best-effort and labelled limited in the UI and
  README, inviting issue reports, per the 2026-07-25 ruling. Do not claim it
  works; no verdict may assert Emby support without a real server behind it.
- README Player notes gain the quality setting, the one-off menu, and the plain
  statement that transcoding forfeits HDR and drops container chapters.

## Verification

Canonical commands: `.agents/repo-guidance.md`. Additionally, before any code:
confirm the Plex decision/ping/stop endpoints and the Plex ladder against a live
server, and confirm Emby's transcode contract against its published OpenAPI.
Behaviour that renders on the video surface cannot be tested on the Linux E2E
venue (`.agents/machines.md`); transcoded playback itself is assertable there
because it needs no OSD.
