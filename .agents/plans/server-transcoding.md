# Plan: server-side transcoding

## Status

**Draft v2, 2026-07-25. NOT yet approved for implementation.** Every product
choice in **Owner decisions** is now ruled and recorded in
`.agents/decisions.md`. What still blocks implementation is evidence, not
choices:

1. Plex's decision endpoint and transcode-session lifecycle (ping/stop) are
   unverified — confirm against a live server.
2. Plex's ladder tier values are unconfirmed — read them off a current client.
3. The plan still needs its implementation slices written, with the verification
   and guard proof each one owes.

The owner has not said "implement". Do not start code on the strength of the
rulings alone.

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
- **UNVERIFIED:** the decision endpoint
  (`/video/:/transcode/universal/decision`) and the session lifecycle
  (`.../ping`, `.../stop`). These are widely used by real clients but were not
  confirmed this session — the community OpenAPI path 404'd and Plex's support
  article returned 403. **Verify against a live server before implementing.**
- **UNVERIFIED:** Plex's quality ladder appears to be a client-side list rather
  than something the server publishes; nothing in the API code read this session
  exposed one, and `maxVideoBitrate`/`videoResolution` are free parameters.
  Confirm against a Plex client before relying on it.

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
   direct stream, transcode — and read the source bitrate.
2. Offer **Original** only when direct play/stream is actually available.
3. Offer transcode steps only when the server reports transcoding is available
   AND permitted for the account, and only steps strictly below the source
   bitrate. A 3 Mbps file offers no 20 Mbps entry.
4. When the server can only direct-play, Original is the sole entry; when it can
   only transcode, Original is absent. The menu never contains an entry that
   would fail.

Open: whether the step values below the source bitrate are Plex's client ladder,
a Vela list, or something derived — see Owner decisions.

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
- **Split-file Plex media.** Vela currently joins Parts as `edl://`. A transcode
  URL is per media/part index; decide what happens for multi-part versions.

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

## Verification

Canonical commands: `.agents/repo-guidance.md`. Additionally, before any code:
confirm the Plex decision/ping/stop endpoints and the Plex ladder against a live
server, and confirm Emby's transcode contract against its published OpenAPI.
Behaviour that renders on the video surface cannot be tested on the Linux E2E
venue (`.agents/machines.md`); transcoded playback itself is assertable there
because it needs no OSD.
