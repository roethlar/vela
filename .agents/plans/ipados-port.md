# Plan: Vela on iPadOS (first of three Apple ports)

## Status

**DRAFT 2026-11-18. Not approved; no product code is authorized.** Slice
S0, the probes P1 to P4, was approved by the owner on 2026-11-20
("approve the probes", `.agents/decisions.md`); the Log has their
results. Test specifics
written here before the code exists are not binding: tests are designed
with the code, in the slice that writes it (`.agents/decisions.md`
2026-11-18, "No tests are designed for code that has not been
written"). Review: five codex rounds on 2026-11-18
(`.agents/review/codereview-2026-11-18-apple-plans.md`); the amendment
of 2026-11-20, which follows the probes, has its own round
(`.agents/review/codereview-2026-11-20-apple-plans-amendment.md`).
Order of work (owner, 2026-11-20): the iPad and the iPhone are finished
first, and the Apple TV follows the week after; engine code for them
does not wait for P2's Apple TV half (`.agents/decisions.md`).
This is the first of three plans, in the owner's order:
this one, then `.agents/plans/ios-port.md` (iPhone), then
`.agents/plans/tvos-port.md` (Apple TV). Part A of this plan builds what
all three share. The other two plans point here instead of repeating it.

## Owner rulings that bind this plan

- Order and engine: `.agents/decisions.md` 2026-11-18. iPadOS first, then
  iOS, then tvOS. On all three, video plays through one engine built on
  Apple's own pipeline, never mpv ("Apple engine on all three"), because
  Apple TV must show real Dolby Vision ("tv will need dolby vision").
- The in-window player is the only player on these devices and carries no
  "experimental" label (D1 of `.agents/plans/embedded-player.md`, carried
  over by the same entry).
- Parity: `.agents/decisions.md` 2026-11-16. A feature that works on one
  platform and not another is a defect unless the owner has approved that
  difference explicitly, for that feature. The differences this port
  cannot avoid are listed under "Differences the owner approved"; all four
  were approved on 2026-11-18.
- Always do it the proper way: `.agents/decisions.md` 2026-11-17. A
  defect that can be fixed is fixed correctly. Removing a feature, a
  workaround or a shortcut is never offered instead.
- HDR is the reason Vela exists (`.agents/repo-guidance.md`, Mission
  Detail). No code path is written that cannot carry HDR. Where the iPad's
  screen cannot show HDR, Apple's pipeline handles the display; Vela
  writes no SDR-only path of its own.
- The licensing rule of 2026-11-12 binds every library shipped here:
  separate dynamic libraries, LGPL builds. It is settled; do not raise it.
- Working rules since 2026-11-12 (`.agents/state.md`, "HOW TO WORK
  HERE"): every slice that lands repo code gets a `codereview` hunt by
  the other model family from its author (`.agents/decisions.md`,
  2026-11-20). Every commit is pushed to `origin` and `github`
  (`.agents/push-policy.md`). Implementation details are settled by the
  agent and recorded. Only what changes Vela for its user goes to the
  owner.

## Scope

When this plan is done, Vela installs on the owner's iPad and works as it
does on the desktop:
- the same Svelte page, servers, browsing, search, playlists, watched
  state, Continue Playing and settings;
- videos play inside Vela's window through the Apple engine: HDR10 and
  HLG on HDR iPads, and real Dolby Vision for profiles 5, 8.1 and 8.4
  (the HLG-compatible profile iPhones record), for profile 7 converted
  live to 8.1, and for profile 10 (AV1) on devices with an AV1 decoder;
- every row of the parity list below passes.

The page renders correctly at every size an iPad window can take: full
screen in both orientations, Split View, Slide Over and the resizable
windows of iPadOS 26. Most of the compact layout iPhone needs is
therefore built here; `ios-port.md` adds what is specific to the phone.

## Non-goals (of this plan)

- iPhone as a device family (`ios-port.md`). Apple TV (`tvos-port.md`).
- Any change in how the desktop behaves. Part A changes desktop code
  (the player seam). Its proof is that the desktop's suites still pass
  with the parity verdict PASS.
- mpv or libmpv on any Apple mobile device (ruling of 2026-11-18).
- Features the desktop does not have either: picture in picture, AirPlay
  of video from the iPad, offline downloads, Stage Manager multiple
  windows. These are additions for a later plan, not parity gaps.
- Publishing on the App Store. The distribution channel is decision O2.

## Owner inputs needed (one at a time, in this order)

- **O1. Devices: ANSWERED 2026-11-18.** The owner has an iPad (A16)
  base model, an iPad Pro (M5), and several third-generation Apple TV
  4Ks (2022), each with an LG OLED with HDR and Dolby Vision. The facts
  and what each device covers are in `.agents/machines.md`, "Apple test
  devices".
  - The HDR and Dolby Vision checks on iPad run on the iPad Pro (M5),
    including profile 10, since its media engine decodes AV1.
  - The A16 iPad covers tone-mapped playback, the software AV1 path and
    the smaller size. Whether it takes part in any HDR check is decided
    by its measured EDR headroom (B-1).
  - Probe P2 runs on the iPad Pro and on one of the Apple TVs. They are
    all one generation.
  - All run the latest release, 27.x (owner, 2026-11-18). The TVs'
    audio chain is still unknown, and is asked when the slice needing it
    comes.
- **O2. Distribution channel: RULED 2026-11-18** (`.agents/decisions.md`,
  that date).
  - While Vela is being built: TestFlight internal testing for the builds
    the owner uses, which CI uploads, and development-signed builds from
    a Mac with Xcode for probes and debugging.
  - Later, the App Store, publicly. Possibly as a paid app with free
    desktop versions, a door that must not be closed.
  - The Apple apps' bundle identifier is `com.roethlar.Vela`. Owner,
    2026-11-18: "com.michael is not an appropriate prefix"; the owner's
    songr-mobile signs as `com.roethlar.Songr`. The owner confirms it and
    the App Store name before the App Store Connect record is created,
    since both are fixed once it exists. The desktop's
    `com.michael.vela` is a separate question.
- **O3. Xcode on `nagatha`: NOT NEEDED (corrected 2026-11-18).** Xcode
  27.2 is installed, with the iOS 27.2 and tvOS 27.2 SDKs and simulators,
  as is Xcode-beta 27.0. The earlier note that it was missing was the
  agent's error. `xcode-select` points to the Command Line Tools, so a
  bare `xcodebuild` fails; commands set
  `DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer`, which needs
  no `sudo`. An Apple Development certificate for the owner's team is in
  the keychain, so development signing needs only the devices registered
  and paired.
- **O5. AVS2: APPROVED as unsupported, 2026-11-18** (owner: "yeah").
  AVS2, a Chinese broadcast codec, has only a GPL decoder, which Vela's
  licensing rule keeps out, so it does not play in the Apple engine.
- **O4. The differences below: APPROVED 2026-11-18**, for iPad, iPhone
  and Apple TV.

## Differences the owner approved (2026-11-18)

Each is something the desktop has that cannot exist on iPad. The owner approved all four as absent on iPad, iPhone and Apple TV
(2026-11-18, `.agents/decisions.md`).

1. External mpv as a player, the "Videos play in" choice, mpv's path and
   the automatic mpv installer: iPadOS cannot start another program.
2. The user's own mpv options and "use my own mpv.conf" (parity row 13):
   they are mpv's options, and no mpv runs here.
3. "Player window" (the first-open window mode), the remembered screen,
   and fullscreen as a window state: an iPad app's window is not placed
   by the app.
4. "Exit Vela": iPadOS apps do not quit themselves (Apple's guidelines).

Everything else the desktop has is rebuilt for the engine, not dropped:
skip markers, black-bar cropping in all three modes, Automatic quality,
track choice, the server-certificate setting, and keyboard shortcuts on a
hardware keyboard.

## Assumptions (each proven or overturned by a probe before product code)

Where each stands after the probes of 2026-11-20 (the Log has the
evidence). Each line says what was shown and what was not:
- A1 holds, on the simulator and on the iPad Pro.
- A2, on the iPad Pro, by the route A-2 now describes (the engine
  decodes; the layer presents):
  - profiles 5 and 8.1 (CMv2.9 and CMv4.0): shown, by the owner's eye;
  - profile 7: files converted to 8.1 beforehand play as Dolby Vision.
    The live conversion A2 names is not built or shown, and the look of
    a converted FEL title is the open point the Log records;
  - profiles 8.4 and 10: not shown, no file;
  - a picture the app made, with the decoder's own RPU attached, is
    presented as Dolby Vision (a copy of a decoded profile 5 picture).
    A picture composed from two layers with a converted RPU is not
    shown; that is P5's question;
  - seeking, track switching and memory: measured on the Mac or the
    iPad as the Log says; exact seeking on the iPad itself was not run.
  - On the Apple TVs: nothing yet.
- A3 is not probed (Apple TV).
- A4 holds on the iPad's speakers for sound decoded to PCM and for AC-3
  and E-AC-3 passed on. What a surround route shows is not probed.
- A5 holds, simulator and device.
- A6 holds for scripts, finding, clicks, typing and screenshots, with
  adapters for pointer coordinates and keys; Escape has no route yet.

What P2 left open is decided in the slice that owns it, never taken as
settled: the live conversion and profiles 8.4 and 10 in A7 (each with the
check P2 would have made: the decoder's attachment, then the owner's
eye), the FEL composition in P5, the Apple TV in `tvos-port.md`.

- **A1 (P1).** A Tauri 2 iOS app shows the page in WKWebView, and a
  native layer can sit beneath a transparent web view, with touches on the
  page's controls still reaching the page. Tauri's generated Xcode project
  (`src-tauri/gen/apple`, git-ignored, never edited) can be configured
  from tracked files alone: `tauri.conf.json` `bundle.iOS`, a merged
  `Info.ios.plist`, and plugins.
- **A2 (P2).** The engine puts an LG OLED into Dolby Vision mode from an
  Apple TV, and plays Dolby Vision on an HDR iPad. It does this for
  profiles 5 and 8.1 from MKV, and for profile 7 converted to 8.1 live
  with libdovi, through the engine's renderer.
  - **The renderer is Apple's sample-buffer path, for everything.**
    FFmpeg demuxes. Pictures reach `AVSampleBufferDisplayLayer` and
    sound `AVSampleBufferAudioRenderer`, with one
    `AVSampleBufferRenderSynchronizer` as the clock for both. Only this
    path shows video Vela decodes itself and plays multichannel LPCM, so
    every codec plays locally (owner, 2026-11-18: no server transcode
    unless the user chooses one). Infuse is built the same way, by the
    libraries it links.
  - **As first written, the layer was to be given compressed samples**
    whose format description carries the Dolby Vision configuration
    (`dvcC`/`dvvC`). P2 found that route engages Dolby Vision and drops
    pictures on deeply reordered streams. The route that holds, and that
    A-2 describes since 2026-11-20: the same format description opens a
    decompression session of the engine's own, and the layer is given
    its decoded pictures, which carry the RPU.
  - **P2 decides one thing** (owner, 2026-11-18: "add the player decision
    to the list of things the apple tv probe will decide"): whether Dolby
    Vision engages through the sample-buffer path, for each profile, on
    the Apple TV and the iPad Pro. P2 plays mixed combinations: Dolby
    Vision with TrueHD, software-decoded video with E-AC-3, and HDR10
    with DTS-HD MA.
  - **If a Dolby Vision profile does not engage that way**, P2 tries a
    throwaway remux of it to AVPlayer, to see whether that path engages
    it. The result counts only if the remux is shown to carry the
    profile's sample entry, its `dvcC`/`dvvC` box and the per-frame
    RPUs; FFmpeg's MP4 writer can drop the box under its default
    settings. A production remux path would then be designed for those
    profiles and devices only, in an amendment reviewed before any code.
    Its known requirements are in the review findings ipd-38, ipd-39 and
    ipd-43 to ipd-46 (`.agents/review/codereview-2026-11-18-apple-plans.md`).
    It would not be designed before P2 shows it is needed.
- **A3 (P2).** The TV's Dolby Vision mode is triggered by
  `AVDisplayManager.preferredDisplayCriteria`, built with
  `AVDisplayCriteria(refreshRate:formatDescription:)` (tvOS 17), from a
  format description whose codec subtype is `dvh1` or `dvhe`. Reported
  elsewhere: a description from decoded pixel buffers switches the TV to
  HDR10 only (KSPlayer issue 633).
- **A4 (P2).** FFmpeg's LGPL decoders turn TrueHD, DTS and DTS-HD MA into
  multichannel LPCM that Apple's audio output accepts. Dolby Digital Plus
  with Atmos passes to Apple untouched.
- **A5 (P3).** Vela's Rust crate builds for `aarch64-apple-ios` and
  `aarch64-apple-ios-sim` with the changes of slice A3. Inside the iOS
  sandbox, tokio, reqwest with native-tls, `File::lock` and the durable
  stores work in the app container.
- **A6 (P4).** The end-to-end scenarios can drive the page inside Tauri's
  WKWebView on the iOS Simulator, through Appium's XCUITest driver and the
  harness's own WebDriver client (`tests/e2e/driver.mjs`).

## Part A — foundations shared by all three Apple ports

### A-1. The player seam in the Rust core (desktop code; no behaviour change)

Today the mpv players define what a play is (verified 2026-11-18; line
numbers drift, so find by name):
- `PlaySpec` (`src-tauri/src/playback.rs`) is built in
  `src-tauri/src/commands.rs` from `StreamResolution`
  (`src-tauri/src/source/mod.rs`). It becomes a `LaunchPlan`
  (`src-tauri/src/player/launch.rs`, `build_launch_for`) of mpv
  arguments, a header `include` file and a marker payload file.
- Every tracker reaches the player through mpv's JSON IPC socket, at a
  path made by `new_ipc_path` (`launch.rs`, under `private_runtime_dir`,
  a 0700 `/tmp/vela-<random>`). The trackers are:
  - the EOF watcher, which drives playlists, Continue Playing and
    watched-on-server through the dispatcher in `src-tauri/src/lib.rs`;
  - the position reader, which also confirms that a play started;
  - the Plex timeline and Jellyfin/Emby reports;
  - the health sampler, whose frame drops and cache feed Automatic
    quality;
  - the state reporter (`src-tauri/src/player/report.rs`), which emits
    `player-state` to the page;
  - the command channel (`src-tauri/src/player/control.rs`).
- Skip markers are armed by `vela-markers.lua` inside mpv, which
  publishes `user-data/vela-markers/active`. Cropping is done by
  `vela-crop.lua` (embedded) or `autocrop.lua` (external).
- Multi-part Plex items are joined into an mpv `edl://` URL
  (`src-tauri/src/plex_library.rs`).

The seam:
1. **`PlayRequest`, player-neutral.** It carries:
   - the parts, a list of URLs (replacing the `edl://` join, which
     becomes the mpv adapter's job), the HTTP headers and the title;
   - the start position and duration;
   - the markers with each kind's skip policy, the crop mode and the
     TLS-verify setting;
   - the one-off notice, and the Automatic quality step-down state.

   `build_launch_for` becomes the mpv adapter that turns a
   `PlayRequest` into today's `LaunchPlan`, byte for byte.

   The desktop's own launch inputs stay beside it, in a desktop launch
   context the mpv adapter takes with the `PlayRequest`: the inherited
   window state and the assigned screen (`commands.rs` passes them into
   the launch today, and `launch.rs` turns them into mpv options).
   Likewise, the position reader's window and display observations
   (fullscreen, maximized, display names and size, OSD dimensions) stay
   in the mpv adapter's own path, which keeps feeding the window-state
   memory. They are not `PlayerEvent`s. A test runs consecutive plays on
   different screens, including an embedded play that falls back to
   external, and checks each reopens where it should.
2. **`PlayerEvent`, player-neutral.**
   - Position (at least 4 Hz), duration, paused, volume, muted.
   - Audio and subtitle tracks (id, title, language, selected).
   - The active skip kind and the crop state.
   - Opened, start confirmed, and closed.
   - The end reason: eof, quit, stop or error. Only eof advances
     playlists and Continue Playing.
   - The final position.
   - Frame drops and buffered seconds.

   An mpv adapter produces these from the existing IPC observers. The
   trackers, the reports, the dispatcher and `player-state` consume
   `PlayerEvent`s instead of raw IPC.
3. **`PlayerCommand`, player-neutral.** Toggle pause; seek relative or
   absolute; set volume; toggle mute; set the audio track; set the
   subtitle track or off; skip the active marker; toggle crop; stop.
   `control::Channel` becomes a trait, with an mpv socket implementation
   and, later, the Apple bridge.
4. **The marker rule, written once for players without mpv.**
   `vela-markers.lua` keeps arming markers for mpv. For any other player,
   a Rust marker arbiter takes positions in and gives the active marker,
   autoskip seeks and the skip action out. Both implementations run one
   shared table of cases, so they cannot drift. The cases come from
   `tests/markers-script.test.mjs`, moved into a fixture both tests read.
5. **`PlayerKind::Apple` and `PlayerHandle::Apple`** exist only on Apple
   mobile targets. `select::choose` there has no fallback, because there
   is nothing to fall back to. The desktop's kinds are unchanged.

Proof: every existing Rust and Node test passes. Both Linux end-to-end
suites pass in full with the parity verdict PASS. The Windows suite
passes in CI. `scripts/check-other-platforms.sh` passes. Each adapter is
red-proved: break the mapping and a test fails for that reason.
`#![allow(dead_code)]` on the Apple-only pieces is removed in the commit
that wires them (repo-guidance, Guard discipline).

### A-2. The Apple engine (`apple/VelaEngine`, a Swift package)

One engine for iPadOS, iOS and tvOS. It lives in a Swift package so that
the iPad app (through a Tauri plugin, B-2) and the tvOS app (directly) use
the same code. It takes a `PlayRequest` and `PlayerCommand`s and reports
`PlayerEvent`s across a small bridge.

Inside:
- **Input:** FFmpeg's `libavformat` reads every part through HTTP(S),
  with the request's headers and the TLS-verify setting. The auth header
  never appears in a URL, a log or a loopback address (parity row 7).
  Multi-part items play as one timeline. Server transcodes arrive as HLS
  (the quality tiers of `plex_library.rs` and `source/jellyfin.rs`), so
  FFmpeg's HLS demuxer reads the master and child playlists and their
  MPEG-TS or fragmented-MP4 segments.
- **Every network request is the engine's own.** FFmpeg never opens a
  connection. Its built-in HTTP follows redirects and reuses connections
  without asking the engine. Instead, every playlist and segment is
  fetched in the core, by a playback HTTP client built for each play
  from the `PlayRequest`'s policy, and called across the bridge as a
  read of a byte range. It is not the provider clients, which ignore the
  player's certificate setting, follow redirects themselves and end a
  request after 15 s. FFmpeg reads those bytes through a custom
  I/O context.
  - The core's client is not Apple's URL Loading System, so App
    Transport Security does not govern it (P3 confirms).
  - It uses reqwest with native-tls. It verifies certificates unless the
    user's certificate setting for playback says otherwise, and it keeps
    no cookie store.
  - It follows no redirect by itself (`Policy::none`). Each `Location` is
    checked against the origin rule below, and the next request's headers
    are built for its own origin. A chain stops at 10 hops or a repeated
    URL, with a playback error that names no credential.
  - It has a connection timeout and a stall timeout that resets while
    bytes arrive, with no deadline on the whole response. It is cancelled
    when the play ends.
  - Each playlist's references are resolved against the URL it was
    finally served from, after redirects, and cleaned, before FFmpeg
    sees them.
  - A read returns up to the bytes asked for, with a count and an end of
    file flag. A `206` must start at the requested offset; when the server
    sends the whole resource instead, the interval is cut from it as it
    streams. A read that begins exactly at the end of the resource is
    the end of the file, not an error: a `416` whose `Content-Range`
    gives a total equal to the requested offset, or an offset equal to a
    total already known, returns no bytes and the end-of-file flag. An
    HLS byte range must come back whole, and "whole" is measured against
    what FFmpeg asked for in that read (its `offset` and `end_offset`):
    for consecutive byte ranges of one resource its HLS demuxer asks for
    several segments in one read. Each segment's own end is held as its
    bytes are used. Anything else is a read error.
  - One logical open is one representation. A playlist is fetched whole,
    resolved and cleaned, and served from that snapshot. Media read in
    several ranges is tied to one version by `If-Range`, with a
    validator HTTP allows there: a strong ETag, or a Last-Modified date
    that counts as strong (RFC 9110, 13.1.5). A weak ETag is never sent.
    A `200` in answer to a conditional range means the resource was
    replaced: the logical open restarts, and no byte of the new version
    reaches the demuxer that holds the old one.
    - With no usable validator, ranges cannot be tied to a version. The
      engine then keeps reading the response it has for as long as the
      demuxer reads forward, and on each new range checks what the
      server does say: the total length in `Content-Range` must be the
      first response's. A different total is a replaced resource, and
      the open restarts. A resource replaced by one of the same length,
      on a server that gives no validator, is not detected; the play's
      diagnostics record that the server gave none.
  - FFmpeg's HLS demuxer runs with its HTTP persistence off, since it
    expects its own HTTP context when that is on.
  - FFmpeg's protocol whitelist admits only the engine's own I/O, so no
    demuxer can open a connection itself (the SDP demuxer would open RTP
    directly). Streams that need another transport are not played.

  The client applies the policy to each request and to each redirect
  before it is followed:
  - auth headers go only to the connection's own origin (scheme, host
    and port), as Plex part URLs are already pinned to it
    (`plex_library.rs`);
  - no credential goes anywhere else;
  - the TLS-verify setting holds on every request.
- **Credential-free stream requests.** Today a Jellyfin transcode URL
  carries `api_key` in its query (`source/jellyfin.rs`,
  `transcode_url`), and the play command passes that URL on beside the
  auth header (`commands.rs`). For the Apple engine:
  - the core builds the transcode request without `api_key`, and the
    token travels in the header on the master, child-playlist and
    segment requests;
  - a playlist or segment URI the server wrote with a credential in its
    query (`api_key`, `X-Plex-Token`, `X-Emby-Token`) is cleaned: if it
    is same-origin and the credential is the active connection's, the
    parameter is removed, the rest of the query kept, and the request
    sent with the header. Any other credential-bearing URI is refused.
    No token ever leaves in a URL;
  - FFmpeg sees playlist URIs only after cleaning, and its log goes
    through a sink that drops any URL query and header, so a refusal or
    an error cannot write a credential to a log;
  - the desktop's recorded Jellyfin and Emby boundary
    (`.agents/decisions.md` 2026-07-23) is unchanged.
- **Video: the engine decodes, the layer presents** (amended 2026-11-20
  after P2; the Log has the measurements).
  - **Decoded pictures, in the order they are shown, are what the layer
    gets.** Compressed samples are never given to the layer. P2 did that
    first, and on 2160p streams with deep B-frame reordering the layer
    lost about one picture a group (41 of 255 on the iPad Pro), whatever
    decode times or reordering hints the samples carried. Decoded
    pictures in presentation order lost none.
  - **What VideoToolbox takes, it decodes, in a decompression session of
    the engine's own.** Which streams those are is asked of VideoToolbox
    for each stream, never read from a list: the engine builds the
    stream's format description and creates the session. If VideoToolbox
    answers that it does not support the format, or the first pictures
    fail to decode, the stream goes to software. An answer that the
    decoder is not available now (another app holds it) is not that: the
    engine tries again, and tells the user if it stays so. (P2: H.264 at
    10 bits, which this plan used to list as software-only, is decoded
    by Apple's decoder on the M5 and on the Mac; profile 7 Dolby Vision
    is refused at session creation.)
  - **Dolby Vision is presented only for pictures VideoToolbox decoded.**
    The layer applies Dolby Vision from an RPU attached to each picture
    (below), and VideoToolbox is what attaches it, under a key
    (`DolbyVisionRPUData`) that Apple's public headers do not declare.
    The engine never writes that attachment itself. Writing it would
    rest the picture on undocumented behaviour, and the Apple apps use
    public API only (`.agents/decisions.md`, 2026-11-18, Apple builds).
    P2 saw that writing it works today, and that the bytes are the
    stream's raw RPU; that is an observation in the Log, and nothing in
    the engine is built on it. Apple's documented per-frame metadata
    session makes metadata from a picture; it does not take a stream's.
    So a Dolby Vision stream that VideoToolbox will not decode is never
    presented as Dolby Vision from software:
    - where its base layer is a valid picture by itself (profiles 8.1
      and 8.4, and profile 10's HDR10- and HLG-compatible forms), FFmpeg
      decodes it and it plays as that HDR10 or HLG picture, and the
      play's state says it is not Dolby Vision;
    - where it is not (profile 5, and profile 10 with no compatible base
      layer), the play ends with an error that says this device will not
      decode this Dolby Vision stream. It is never shown without its
      metadata, which is the green and purple picture.
  - The format description says what an ISO sample entry would: the
    codec type, the configuration record from the container (`hvcC`,
    `avcC`, `av1C`, `vpcC`), the colour tags, the mastering display and
    the content light level. A stream that arrives as Annex B (MPEG-TS,
    which is also what a server's HLS transcode can carry) has its
    parameter sets collected into that record and its NAL units given
    length prefixes before the session sees it. P2 did not exercise
    Annex B; slice A5 proves it with a transport-stream clip and a real
    HLS transcode.
  - **A stream can change format while it plays** (an HLS discontinuity,
    new parameter sets in an Annex B stream). When the parameter sets
    change, the engine builds the new format description and asks the
    session whether it accepts it
    (`VTDecompressionSessionCanAcceptFormatDescription`). If it does not,
    the pictures already decoded are shown, and a new session, or the
    software decoder if VideoToolbox does not support the new format,
    takes over at the next random-access point. The clock runs on.
  - Decoded pictures wait in a reorder queue and leave it in presentation
    order. The queue's depth comes from the stream (HEVC's
    `sps_max_num_reorder_pics`, H.264's `max_num_reorder_frames`, or the
    decoded-picture-buffer size when the stream does not say), bounded at
    sixteen. The engine decodes ahead of the clock only within a budget
    of pictures and bytes it owns (the footprint bound below).
  - Pictures go to the layer as image sample buffers, only while the
    renderer reports it is ready for more.
  - **A track's display rotation is applied.** A recording made holding
    a phone upright is stored on its side with a rotation in the track
    (FFmpeg gives it as the stream's display matrix), and a decoded
    pixel buffer does not carry it. The engine turns the layer by it,
    and everything measured against the picture, cropping and bitmap
    subtitles among it, uses the turned picture's coordinates. Profile
    8.4 recordings from an iPhone are the case in this plan's scope.
  - What VideoToolbox does not take (MPEG-2, VC-1, MPEG-4 ASP, VP9 where
    it is refused, AV1 on devices without a hardware decoder): FFmpeg
    decodes on the device, in software, into `CVPixelBuffer`s (dav1d for
    AV1), and those take the same hand-over to the layer. They do not
    pass through the reorder queue: FFmpeg's decoders hold their delayed
    pictures themselves and give pictures out in the order they are
    shown, so a second queue would pay for the reordering twice, in
    delay and in memory. The reorder queue is for VideoToolbox's output,
    which comes in decode order. The budget counts the pictures a
    software decoder holds as that decoder's, and the short lead of
    pictures waiting for the layer as the engine's.
    They never go to a server transcode unless the user picked one.
  - A software-decoded picture carries the same description a hardware
    one would: primaries, transfer, matrix and range from the decoded
    frame, and from the container where the frame does not say; the
    pixel aspect ratio (a 704 x 480 picture is shown at 4:3); mastering
    display and content light level.
  - **Interlaced video is deinterlaced.** The owner's libraries hold
    interlaced MPEG-2 (broadcast recordings); H.264 at 1080i is the same
    case. Software-decoded fields go through FFmpeg's `bwdif` (LGPL; A-3
    builds libavfilter for it and asserts the licence). For streams
    VideoToolbox decodes, the session's field mode asks it to
    deinterlace. That property is optional for a decoder: the engine
    checks whether the session took it, and when it did not, the
    session's pictures go through `bwdif` like the software ones. A test
    plays an interlaced clip on each path and checks the picture rate
    and that no combing is left on a moving edge.
  - Only a Dolby Vision profile and device named in a reviewed amendment
    may be presented any other way (the AVPlayer remux that P2 would try
    on an Apple TV if a profile does not engage there).
- **HDR and Dolby Vision:**
  - The engine tags colour primaries, transfer, matrix, mastering
    display and content light level.
  - **Dolby Vision is carried by the decoder and applied by the layer**
    (P2, the iPad Pro and the Mac). The session's format description
    names the stream `dvh1` and carries its configuration box (`dvcC` for
    profiles up to 7, `dvvC` above). VideoToolbox then attaches each
    picture's RPU to the decoded picture (`DolbyVisionRPUData`), and the
    layer applies it when it presents the picture. Profile 5 is only
    recognised as `dvh1`: as `hvc1` its box is ignored and the picture is
    the unprocessed green and purple. Profile 8.1 is recognised either
    way; the engine names both `dvh1`.
  - **Profile 7 is refused by VideoToolbox as it comes** (P2: the session
    cannot be created, MEL or FEL). What the engine does depends on the
    enhancement layer, which the RPU tells apart:
    - **MEL** (the enhancement layer carries no picture): the engine
      converts each RPU to 8.1 live with libdovi's mode 2, drops the
      enhancement layer's NAL units, and names the stream 8.1. By the
      format nothing of the picture is lost. P2 showed the converted
      stream decodes with its RPU and plays; its look has not been
      checked against a reference.
    - **FEL** (the enhancement layer carries part of the picture): the
      picture the disc means is both layers together, and the conversion
      drops one. Composing them means decoding the enhancement layer
      beside the base layer and applying the RPU's mapping and residual.
      The composed picture is the engine's own, not VideoToolbox's, so
      how it reaches the screen is the open question, and there are
      three ways a FEL title can play:
      1. converted, as MEL is: Dolby Vision's mapping, the enhancement
         layer dropped;
      2. composed and presented as HDR10: the whole picture, by a
         documented path, with Apple's HDR10 mapping in place of the
         disc's dynamic metadata, and no Dolby Vision mode;
      3. composed and presented as Dolby Vision, by attaching a
         converted RPU to the composed picture. P2 saw the layer honour
         that on a picture the app made (the Mac by measurement, the
         iPad by the owner's eye). It is the undocumented attachment
         above, which the engine does not write.
      Composition is unproven on a device, so it has a probe of its own,
      P5 (Slices), which measures ways 2 and 3 beside way 1. **Which way
      Vela takes is the owner's choice, made with P5's results**: way 3
      is the disc's picture with its own mapping and bears on the App
      Store rule; way 2 keeps to public API and gives up the mapping.
    - Until the owner has chosen and it has landed, a FEL title plays by
      the MEL conversion, and the engine says so: the play's state
      carries "Dolby Vision, converted, enhancement layer not applied",
      so that nothing shown to the user or written in a test claims the
      disc's full picture.
  - Profile 10 is AV1, whose Dolby Vision metadata travels in metadata
    OBUs, so the engine carries those and an AV1 Dolby Vision format
    description (HLS names it `dav1`) to VideoToolbox. Profiles 10 and
    8.4 were not in P2: no file. Each is proved in A7 when a file exists.
    In software there is no Dolby Vision for it, as for any profile
    (above); FFmpeg's dav1d path would not supply a raw RPU in any case,
    only the parsed metadata.
  - Native 8.1 is used wherever the device takes it. P2 checks that per
    device and per Apple TV generation.
  - Converting 8.1 to 5 for a device that takes only 5 needs the picture
    converted as well as the metadata. Profile 5's base layer is
    IPT-PQ-c2, encoded differently from 8.1's HDR10-compatible one, and
    dovi_tool's conversion modes convert metadata only. So it is adopted
    only if a probe shows a real-time conversion of both, checked against
    a reference (`tvos-port.md`, V-4).
  - On tvOS the engine sets display criteria per play and clears them at
    the end (`tvos-port.md` owns that part).
- **Audio:**
  - AC-3 and E-AC-3 (Atmos included) pass to Apple, and so do AAC, ALAC
    and MP3, compressed, to the audio renderer.
  - TrueHD, DTS, DTS-HD MA, FLAC, Opus, Vorbis and PCM variants are
    decoded by FFmpeg into multichannel LPCM.
  - **Decoded sound is timed by counting samples.** The first buffer's
    time comes from the container; each later buffer starts where the
    last one ended. A container time is believed again only when it is
    far from the counted one (0.1 s in P2; the engine's value is set
    with the tests), and that is then a real discontinuity: the count
    starts again at the container's time, for the sound from there on.
    What is already queued before the discontinuity is still to be
    heard and stays queued; the renderer is flushed only by a seek, a
    change of track or its own request. P2 found why the count is
    needed: a TrueHD
    frame is 40 samples, Matroska stamps packets to the millisecond, the
    stamps repeat and overlap, and Apple's renderer, given a buffer for
    each, reports no error and plays nothing. Small frames are gathered
    into buffers of tens of milliseconds.
  - AC-3 and E-AC-3 go to the renderer one access unit to a sample
    buffer, and are timed by the same count. An E-AC-3 access unit is an
    independent substream's frame together with the dependent substream
    frames that belong to it (a 7.1 programme carries its extra channels
    that way). They cover the same stretch of time, so the clock moves
    once for the unit, by the samples its independent frame declares in
    its header (256, 512, 768 or 1536), however many frames the unit
    holds. A container that delivers the substreams' frames apart has
    them gathered into the unit first.
  - Decoded sound is described to Apple by a standard channel layout
    where the source layout has one (mono, stereo, 5.1 as
    `MPEG_5_1_A`, 7.1 as `MPEG_7_1_C`, the channels moved from FFmpeg's
    order into the layout's), and by a label for each channel otherwise.
    P2 heard both ways on the iPad's speakers, where left and right can
    be told and surround positions cannot; row 17's channel clips on a
    surround route settle the positions.
  - Track switching happens without restarting the play, and the new
    track is heard from where the picture is. P2's probe only showed the
    renderer can be flushed and refilled while the picture runs on: it
    continued from where the demuxer had read to, which leaves silence
    as long as what was queued. The engine reads the new track from the
    clock's position; how (a second reader, or a short cache of every
    audio track's packets) is settled in A8.
  - The engine reports the format and channel layout it hands to Apple,
    so tests can assert them (row 17).
- **Subtitles:**
  - Text formats (SRT, ASS/SSA, WebVTT, mov_text) are rendered by libass,
    which uses its CoreText font provider, into a layer above the video.
    libass consumes ASS events only, so FFmpeg's subtitle decoders
    (`subrip`, `webvtt`, `mov_text`, `ass`) decode each format into ASS
    dialogue events. Each libass track is first set up with the
    decoder's subtitle header (for a Matroska ASS track, its script
    resolution and named styles, from `CodecPrivate`), and the file's font
    attachments are loaded, so positioned and styled signs render as
    authored. The engine then hands the events to libass with their start
    and end times on the synchronizer's clock. A test shows a visible cue
    and then its removal for each listed format.
  - Bitmap formats (PGS, VobSub, DVB) come from FFmpeg's decoders and are
    drawn as images, scaled with the picture.
- **Seeking is exact.** Both renderers, the session and the queues are
  emptied; decoding starts at the keyframe before the target; the clock
  starts again at the target itself. The picture shown is the one whose
  time on screen contains the target, which is the last picture that
  starts at or before it, not the first that starts after: with a
  variable frame rate a picture can cover seconds, and the last picture
  of a file has none after it. Pictures that end before the target are
  decoded and not shown, and sound is cut at the target to the sample.
  P2's probe took the first picture at or after the target, on a clip of
  constant rate where that is within a frame; it measured 86 to 225 ms a
  seek at 2160p on the Mac, with up to 72 pictures decoded and thrown
  away.
- **Cropping:** off, manual or auto, rebuilt for the engine. The engine
  detects black bars on sampled pictures. Since the engine decodes, it
  has every picture it shows, hardware-decoded or software-decoded, and
  samples those; no second decoder is needed. VideoToolbox's own buffers
  can be in a losslessly compressed format (`&xv0`, `&xf0` in P2), so the
  detector reads a small converted copy of the pictures it samples, and
  A9 measures what that costs. With the conditional AVPlayer remux, the
  frames come from `AVPlayerItemVideoOutput`.

  It crops by the layer's geometry, never by decoding the shown picture
  again. A9 tests a letterboxed, hardware-decoded clip on the minimum OS.
  It reports the same crop states `vela-crop.lua` reports today.
- **Health:** dropped frames and buffered seconds feed Automatic quality
  as `PlayerEvent`s. The dropped frames that feed it are only the
  pictures the decoder, hardware or software, could not deliver in time
  for their turn: that is the signal the owner approved ("sustained
  decoder frame drops mean this machine cannot decode the file",
  `.agents/decisions.md` 2026-07-25). The renderer's own count (its video
  performance metrics) also includes pictures that missed their moment
  on screen for other reasons. It is kept as a diagnostic of the play and
  never reaches the step-down. The iOS 27 SDK reports a renderer's failures
  through the result of each enqueue and a stream of rendering events;
  the engine uses those, not the deprecated `error` property.
- **Footprint.** With the engine decoding, the decoded pictures waiting
  to be shown are Vela's memory. In Apple's 10-bit formats each
  component takes 16 bits, so a 2160p picture is 24.9 MB when not
  compressed (3840 x 2160 x 1.5 x 2 bytes, before row padding), and
  sixteen are about 398 MB. VideoToolbox's own buffers are often
  losslessly compressed and smaller; a software-decoded or copied
  picture is the full size. P2's 286 to 408 MB was the whole process's
  footprint with a fixed queue of sixteen. The engine's budget is the
  stream's reorder depth plus a small lead, in pictures and in bytes.
  A picture's real size is known only when it exists: VideoToolbox
  hands over its buffer in the decode's callback, with strides and a
  backing size the hardware chose. So admission works in two steps.
  Before a picture is sent to be decoded, copied or decoded in
  software, an upper bound is reserved for it: its dimensions rounded
  up to the alignment, at 16 bits a component for a 10-bit format, for
  the format asked for. When the buffer exists, the reservation is
  replaced by its real size (its planes' strides and its backing
  allocation). A decode is not started when its reservation would pass
  the budget. A5 sets the budget and measures it on the A16 iPad, and
  `ios-port.md` sets the phone's.
- **Lifecycle:**
  - The engine configures `AVAudioSession` for playback and handles
    interruptions.
  - Now Playing information and remote commands (play/pause, seek, skip)
    go through `MPNowPlayingInfoCenter` and `MPRemoteCommandCenter`.
  - Leaving the foreground follows one contract:
    1. The play pauses, and a background task starts. If the app comes
       back before finalization has begun, the same session continues.
       Once finalization has begun, a return never resumes that
       session; step 3 applies.
    2. UIKit may grant no background time, and sends no termination
       callback to a suspended app. So, while a play runs, its position
       and any server transcode still to be torn down are kept in a
       playback checkpoint of its own, never in `config.json`, whose
       three-version history ordinary playback would otherwise use up.
       What a teardown needs is written before the request that can
       start a transcode. The position is written every few seconds
       while playing, coalesced, and again at every lifecycle
       transition.
       Recents are updated at finalization.
       - Finalization starts as soon as the task's remaining time falls
         below what finalizing needs, not at expiry. It reports the final
         position (Plex timeline `stopped`, Jellyfin and Emby `Stopped`),
         tears down any server transcode as the desktop does
         (`playback.rs`), and keeps the play resumable.
       - A finalization that did not complete is finished when the app
         next comes to the foreground or launches, from the checkpoint.
       - Transcode teardown and progress are tracked apart. A teardown
         stays pending until the server confirms it, and is retried.
         A finalization finished late retries only that cleanup. Its
         position is never sent to the server later, because the
         servers' progress writes cannot be made conditional on nothing
         newer having happened. The position stays in Vela's recents.
       - A server's play session (Jellyfin and Emby `Started` with no
         `Stopped`) is tracked apart from encoder cleanup and from
         progress. It is closed later only by a request that writes no
         item progress, and only before this device starts another play.
         Jellyfin's `Stopped` clears the device's current session, so a
         close not yet sent when a new play starts is dropped. A close
         already sent whose answer has not come cannot be called back.
         The new play's `Started` waits until that request is answered
         or has failed; its own time limit bounds the wait. If it failed
         with no answer, the server may still act on it afterwards, so
         the new play does not rest on `Started` alone: its progress
         reports begin at once, and they name the item again. Whether a
         late `Stopped` for the old play leaves the new one current after
         the next report is shown against a real Jellyfin and a real Emby
         before the lifecycle slice builds on it; if it does not, this
         rule comes back for review. Where a
         provider has no such close, the server's own expiry closes it.
         The Log records, per provider, what that expiry does to the
         position (Jellyfin's can report the old one).
    3. Coming back after that, the player view shows the play paused at
       the resume point, which is settled on return and again when the
       user presses Play.
       - The checkpoint also records the server's position and watched
         state as they were when it was written. At Play, the server is
         read again, and what it reports now is compared with what the
         checkpoint recorded:
         - a different position or watched state: the server wins,
           because another device moved it;
         - a position then and none now: the server wins as well. That
           is a reset made elsewhere (the item cleared, or finished and
           marked watched), and the play starts where the server says,
           from the beginning if it says nothing. Vela's checkpoint never
           brings back progress the user cleared;
         - the same as recorded: Vela's position wins;
         - a provider or an item that gives no resume state at all, then
           and now, leaves nothing to compare, and only then does Vela's
           position win for lack of one.
         No clocks are compared.
       - A seek the user made in the paused view wins over both.
       - Playing starts a fresh session there.

    Proof: whether the app comes back, is suspended or is ended, the
    servers see one final report and the transcode torn down.

Tests: XCTest in the simulator (A7), with test media generated as in
A-3. They include an AAC direct play (the harness's standard clip,
`tests/e2e/helpers.mjs` `makeClips`) and a real move to a server's HLS
transcode when Automatic steps down, against the mock servers.

### A-3. The media libraries for Apple devices (`packaging/apple-media/`)

This follows the libmpv set's recipe (`packaging/libmpv/versions.txt`;
`fetch()` in `scripts/build-libmpv-macos.sh`):
- **Pins:** one line per component in `packaging/apple-media/versions.txt`,
  each with version, role, sha256, URL and mirrors.
  `tests/dependency-policy.test.mjs` is extended to that file.
- **Components:**
  - FFmpeg, the same version as the libmpv set's pin, LGPL:
    `--disable-gpl --disable-nonfree --disable-version3
    --disable-autodetect`. The script asserts the licence string and the
    expected `CONFIG_*` features afterwards, as the macOS script does.
    Every decoder, demuxer and parser FFmpeg's LGPL configuration
    provides is enabled. External decoder libraries are chosen for
    coverage, by codec and profile: every codec and profile that only an
    external library decodes, and whose library the licensing rule of
    2026-11-12 permits, has one. That is not every library: where FFmpeg
    offers two for one codec that cannot be built together, the one that
    covers more is built and the other is written down as its
    alternative. The inventory is made from the pinned FFmpeg release
    codec by codec, says for each library which codecs and profiles it
    covers, and each library is pinned like the rest. Known today: dav1d
    (AV1), uavs3d (AVS3, built with `COMPILE_10BIT=1` so 10-bit plays)
    and, for MPEG-5 EVC, the full decoder `libxevd`, not the
    baseline-only `libxevdb`: the pinned `configure` stops when both are
    enabled. If the inventory finds a profile the chosen library does
    not decode, that gap goes to the owner like any other. AVS2, whose only decoder (davs2) is GPL, is not supported, by
    the owner's approval (O5). Any other codec whose only decoder that
    rule excludes goes to the owner the same way. swresample and
    swscale are built too, and libavfilter for the deinterlacer (`bwdif`,
    LGPL in the pinned release, with no GPL dependency in its
    `configure`; the script asserts that as it asserts the rest). Any codec Apple's
    hardware does not take is then decoded in software, and the engine
    falls back to software whenever VideoToolbox refuses a stream.
    FFmpeg's HTTP and HTTPS protocols are compiled in, because its HLS
    demuxer checks a URL's protocol before it calls the custom I/O. Every
    read still goes through the core (A-2, transport).
  - libass, with FreeType, HarfBuzz and FriBidi, and CoreText on.
  - dav1d.
  - libdovi: the C interface of the `dolby_vision` crate, built with
    cargo-c.
- **Output:** dynamic `.xcframework`s for `ios-arm64`,
  `ios-arm64-simulator`, `tvos-arm64` and `tvos-arm64-simulator`, with a
  `MANIFEST` and the licence texts.
- P2 built the pinned FFmpeg for iOS, the iOS simulator, tvOS and macOS
  with the LGPL flags, statically and with far fewer parts than this
  section asks for; it says the release builds for those targets and
  nothing about this recipe.
- **Where it builds:** `scripts/build-apple-media.sh`, and a CI job
  `apple-media` on a macOS runner. It is cached by its pins and its
  toolchain record, like `libmpv-macos`, and the Xcode version is
  selected explicitly.

Test media: `tests/e2e/helpers.mjs` `makeClips` is extended to make HDR10,
HLG and Dolby Vision clips. The video is 10-bit HEVC from
`hevc_videotoolbox` on macOS. dovi_tool (pinned, test-only, never shipped)
generates and injects the RPU of profile 8.1 into that HDR10-compatible
video. Profile 5 cannot be made this way, because its base layer must
itself be encoded as IPT-PQ-c2. Its fixture is an independently validated
profile 5 sample, published, with its source and checksum pinned
(test-only). Test-only mkvtoolnix
(pinned) muxes the Dolby Vision mapping into MKV. Profile 7 test media
cannot be made this way, so P2 and the owner's checks use real files from
the owner's servers.

## Part B — the iPad app

### B-1. Rust on iOS (slice A3; the explorer's inventory, 2026-11-18)

What must change for the crate to build and behave in the sandbox. This
list is completed by P3, and the slice lands what P3 finds:
P3 ran on 2026-11-20 (the Log has the evidence) and adds these:
- `player_fullscreen` (`commands.rs`) calls `WebviewWindow::set_fullscreen`,
  which Tauri has on desktop only: the one compile error on iOS. On iOS
  the player view is the window, so the command answers the claim and
  asks the window nothing.
- `player/embedded/mod.rs` builds `macos_geometry` and `macos_display`
  for every test build (`any(target_os = "macos", test)`), and they use
  `session`, which is desktop-only. The test half of that gate is
  narrowed to the desktop systems.
- The iOS build needs swift-rs 1.0.9 or later under Xcode 27 (it comes
  with slice T0's Tauri update).
- With the rename repair below, the unit tests run in the simulator pass
  but for the nine that start an external player's process; those are
  gated to the desktop with the code they test.
- On a device the sandbox refuses `/tmp` and the temporary directory is
  the container's own, as this list assumed.
- Compile out on iOS what spawns programs or exists only for mpv:
  - the external player (`src-tauri/src/player/external.rs`);
  - mpv detection and installation (`check_mpv`, `install_mpv`,
    `set_mpv_path`) and `open_url`'s `open`/`xdg-open`/`rundll32`
    (`commands.rs`);
  - the IPC socket and header-include paths (`launch.rs`
    `private_runtime_dir`, which writes under `/tmp`, unwritable in the
    sandbox).

  The engine needs none of these. Plex's sign-in opens its link through
  Tauri's opener plugin.
- **Durable saves fail on iOS from the second write on.**
  `storage::rename_noreplace` has branches for Linux (`renameat2`), macOS
  (`renamex_np`) and Windows, and returns `Unsupported` everywhere else.
  It backs `install_json_new` and `write_private_new`, which
  `preserve_valid_history` (`durable.rs`) uses on every save over an
  existing `config.json` or `connections.json`. Both predicates change together:
  the macOS branch becomes `target_vendor = "apple"`, since libc declares
  `renamex_np` for every Apple target, and the fallback branch below it
  (`not(any(linux, macos, windows))`) excludes Apple targets. Widening
  only the first would leave the fallback also compiled for iOS, so the
  rename could succeed and then report `Unsupported` before syncing the
  parent directory. Red-proof: tests on the iOS target save twice, and
  try a rename onto an existing file, which must be refused.
- **Branches iOS takes by mistake.** iOS is `unix` but not `macos`, so
  every `all(unix, not(target_os = "macos"))` branch is a Linux branch
  that compiles there. Each is given its correct iOS meaning or compiled
  out:
  - the pkexec/apt mpv installer (`commands.rs`, around `check_mpv`);
  - `xdg-open` in `open_url`;
  - the Linux mpv paths in `external.rs`.
- **Settings stored under mpv's name.** `get_mpv_advanced` and
  `set_mpv_advanced` carry portable settings beside mpv's: the skip
  policies for intros, credits and commercials, the playback quality,
  and the cropping mode, whose getter and setter also live in the mpv
  commands. The portable ones move to commands of their own, so iOS
  keeps them.
- **Settings must still load.** The page loads Settings with one
  `Promise.all` that includes `check_mpv` (`Settings.svelte`). A command
  compiled out on iOS would reject that whole load before the servers or
  any portable setting appear. So, in the same slice that removes any
  command, the load becomes capability-aware: it asks only for what the
  backend says exists. A test on the iOS build opens Settings, sees the
  existing servers, and saves the crop mode.
- **The display's HDR capability.** `get_playback_preferences` and
  source selection (`selection.rs`, `display::HdrState`) learn it from
  the desktop's monitors through the window. On iOS it comes from the
  screen's EDR headroom (`UIScreen.potentialEDRHeadroom`), passed up by
  the plugin.
- **The platform name sent to servers.** `platform_name` (`lib.rs`)
  passes `std::env::consts::OS` through for unknown systems, so Plex
  would be told `ios`. It answers `iOS` (and later `tvOS`).
- Config location: `storage::config_dir_file` uses
  `ProjectDirs("com","vela","vela")`. On iOS, `directories` takes its
  macOS branch with `HOME` set to the app container, which gives
  `<container>/Library/Application Support/com.vela.vela`. That is the
  explorer's reading, not tested; P3 checks it. The desktop location does
  not move.
- The many gates written `cfg(any(target_os = "linux", target_os =
  "windows", target_os = "macos"))` leave iOS in the "other system"
  branches. For example, `select.rs` would say "video in Vela's window is
  only available on Linux". Each such branch gets an iOS answer.
- The settings that are mpv's are hidden on iOS only once each is ruled
  (O4). Until then they are not hidden.
- `Info.ios.plist`:
  - `NSLocalNetworkUsageDescription`, the same sentence as macOS.
  - App Transport Security: the narrowest exception covering what the web
    view loads over plain HTTP from servers (artwork), plus local
    networking. Rust's own HTTP client does not go through Apple's URL
    loading, so ATS does not apply to it.
  - No background modes beyond what A-2's lifecycle needs.

### B-2. The Tauri iOS app and the engine's plugin

- **The app:** `tauri ios init` makes `src-tauri/gen/apple` from tracked
  configuration. CI makes it on every build; it is never committed or
  edited (repo-guidance: `src-tauri/gen/` is generated). P1 (Log,
  2026-11-20) found what that takes on the iOS 27 SDK:
  - **The scene lifecycle.** An app built with that SDK that has not
    adopted it is ended at launch. Tauri's window library adopts it from
    Tauri 2.12 (tao 0.37) when `Info.plist` has a
    `UIApplicationSceneManifest`. So Tauri is updated first, for every
    platform, in a slice of its own (T0), and `Info.ios.plist` carries
    the manifest with `UIApplicationSupportsMultipleScenes` false.
  - `tauri.ios.conf.json` sets the identifier, `minimumSystemVersion`
    and `developmentTeam`, and `Info.ios.plist` is merged at each build.
    Those were shown to take effect. Its `bundle.resources` was not: see
    "Bundle", below.
  - iPad only is set in a tracked copy of Tauri's XcodeGen template
    (`bundle.iOS.template`, `TARGETED_DEVICE_FAMILY: "2"`). Tauri reads
    that path against the directory its CLI runs in, so every command
    that builds the app runs from the repo root, and a test of the build
    scripts holds that.
  - A build on a Mac whose `xcode-select` does not point at Xcode fails,
    because Tauri's CLI does not pass `DEVELOPER_DIR` on. CI selects
    Xcode itself; `docs/development.md` says what a local Mac needs.
  - Device family: iPad only in this plan.
  - Bundle: the iOS app's bundle carries no mpv scripts, which are
    desktop resources (`tauri.conf.json` `bundle.resources`). A tracked
    `tauri.ios.conf.json` overlay leaves them out (decisions
    2026-11-12, licensing, and 2026-11-18, the App Store).
    - The overlay says `"resources": []`. Tauri merges an object into
      the base configuration's object, so `"resources": {}` removes
      nothing: P1's overlay said that, and the probe's app carried
      `mpv-scripts/`, `autocrop.lua` and `LICENSE.GPL` among them. A
      list replaces the map. With the list, and a clean generated
      project, a build has none of them (tried in the simulator build
      after the review).
    - Tauri does not clear `gen/apple/assets`: what an earlier build
      copied there stays and is bundled again. CI generates the project
      afresh on every build; a local build that changes resources starts
      from a removed `gen/apple`.
    - Packaging does not trust the configuration. It looks inside the
      built app and fails if it finds `mpv-scripts`, a `.lua` file or
      `LICENSE.GPL`, in the simulator app of every push and in the
      archive of every release.
  - Minimum iPadOS: 27. The owner's devices run the latest release and
    are updated as each one ships (owner, 2026-11-18), and the owner is
    Vela's only user. So P2 and every check run on the 27.x the devices
    have, and no older release is supported or tested. When a new major
    ships, the floor moves with the devices.
  - Icons: `tauri icon` from a 1024 x 1024 source. The repo's largest
    is `src-tauri/icons/icon.png` at 512 x 512, so slice A4 finds or
    makes the source artwork.
  - If P1 shows a needed setting cannot be expressed through tracked
    files, the slice stops and reports it rather than editing the
    generated project.
- **The plugin:** `tauri-plugin-vela-player`, in-tree, with Rust and
  Swift (an `ios/` Swift package that depends on `apple/VelaEngine`).
  - It makes the web view transparent and puts the engine's layer beneath
    it, sized to the rectangle the page reports (`player_set_rect`, as on
    the desktop). P1 showed the layer under the page and real touches
    reaching the page over it, in the simulator and on the iPad Pro.
  - The page's coordinates are the web content's, and the web view's
    content can start below the status bar (32 pt in P1) while the web
    view itself fills the screen. The plugin converts the page's
    rectangle with the web view's own geometry (its content inset and
    zoom), never by assuming they agree. B-3's `viewport-fit=cover`
    makes the inset zero, and the conversion stays.
  - The view behind the page is white in Tauri's window. The plugin
    gives the picture's area a black backing, so no white shows around a
    letterboxed picture or before the first one.
  - The plugin's build leaves generated files in its directory
    (`.tauri/`, `permissions/autogenerated/`, `ios/Package.resolved`);
    the commit that adds the plugin ignores them.
  - `PlayerHandle::Apple` sends `PlayerCommand`s through the plugin and
    receives `PlayerEvent`s on a Tauri channel. From there the trackers
    and `player-state` work exactly as for mpv.
  - `player-opened` and `player-closed` keep their meaning, so the page's
    player view needs no player-specific branch.

### B-3. The page on iPad (the explorer's inventory, 2026-11-18)

Lowest effort first:
1. **Viewport and input:**
   - `viewport-fit=cover`, with safe-area insets on the header, the
     fixed footer and the player bar.
   - `dvh` in place of `100vh` for the page's own height. The on-screen
     keyboard is handled by the visual viewport, because iPad WebKit can
     shrink or pan the visual viewport while `dvh` layout keeps its size.
     Dialogs, Settings, the fixed footer and form actions are sized and
     scrolled by `window.visualViewport` (height and offset). A test opens
     the server form at compact width with the software keyboard up, and
     checks that the field and its action are both reachable.
   - Inputs at 16px or more, so iPadOS does not zoom, with fitting
     `autocapitalize`, `autocorrect` and `inputmode`. The server URL
     field becomes `type=url`.
   - Hover effects only under `@media (hover: hover)`.
   - `-webkit-touch-callout: none` on posters.
2. **The right-click menus.** Nine actions are reachable only through
   them: mark watched and unwatched, remove from Continue Watching, add
   to a playlist, play a version, play at a quality, info for a Continue
   Watching item, play from the beginning, and scan a library. Each
   becomes reachable by touch: long-press, and a visible "…" button on
   posters, cards and detail pages. The same menu shows as a sheet at
   compact width. Right-click keeps working with a pointer.
3. **The player's touch model.**
   - A tap toggles the bar instead of pausing.
   - Double-tap fullscreen is dropped, because the view is already the
     whole window.
   - Buttons are at least 44 pt.
   - Below about 900 px the bar takes a compact form: the two track lists
     and the volume move into one sheet opened from a single button.
     Nothing is removed, since Vela's own volume and mute are parity
     features. The hardware buttons still change the device's volume.
   - Seeking works by dragging the bar and by swiping horizontally on the
     picture.
   - A hardware keyboard keeps every desktop shortcut.
4. **The responsive shell.**
   - The sidebar becomes a drawer below about 700 pt.
   - The season view and the item page stack vertically at compact
     width.
   - Settings uses top tabs at compact width.
   - Truncated titles are readable without `title=` tooltips.
5. **Capability flags from the backend** (as `get_player` already gives
   `offered`) decide what the page shows. The page never sniffs the
   platform.

Every change keeps the desktop page the same at desktop sizes with a
mouse. That is proved by the desktop end-to-end suites, and by Node tests
of any new layout logic.

### B-4. Build, signing and CI

- **CI, on every push:** a macOS job builds the crate for
  `aarch64-apple-ios` and `aarch64-apple-ios-sim`, runs the engine's
  XCTests and builds the simulator app. The Xcode version is selected
  explicitly.
- **Type-check from Linux:** `scripts/check-other-platforms.sh` gains
  `aarch64-apple-ios`, with a fake compiler like its macOS one where
  needed.
- **Release workflow:** an iOS job builds a signed IPA for the channel O2
  rules. It only runs when its secrets are present, and partial secrets
  are an error, as for macOS. The signing follows the channel:
  - **TestFlight:** an Apple Distribution certificate and an App Store
    provisioning profile for the app's bundle id. These are either stored
    as secrets, or fetched by automatic provisioning with an App Store
    Connect API key whose role allows it. The same key, or one with
    upload rights, uploads the build.
  - **Development builds:** an Apple Development certificate and a
    development profile listing the owner's devices. No distribution
    certificate is used. The probes' builds ran on the iPad Pro with the
    wildcard development profile already on `nagatha`, which lists that
    iPad and neither the A16 iPad nor an iPhone; a profile that lists
    those needs the owner's Apple account in Xcode or an App Store
    Connect key (`.agents/machines.md`).

  Tauri's iOS signing requirements are the reference. The slice proves
  its channel end to end: a build from a fresh runner installs and starts
  on the owner's iPad.
- **Privacy manifests:** Apple requires the bundle that contains a
  binary to declare the required-reason APIs that binary calls, and App
  Store Connect rejects uploads, TestFlight's included, without them
  (review findings tvp-19, ipd-23).
  - The app's tracked `PrivacyInfo.xcprivacy` is merged into the
    generated project from tracked files (B-2's rule). It covers what
    the app binary calls, the Rust core included: `UserDefaults`, and
    file timestamps (`stat`, through `std::fs::metadata`).
  - Each dynamic framework of A-3 carries its own manifest for what it
    calls, written when the framework is built.
  - Packaging inventories the archive's executables and frameworks, and
    checks each one's manifest.
- **Version and tags:** the version is Vela's one version. Tags and
  publishing stay the owner's.
- **Repo guidance:** the iOS commands join "Verification" in
  `.agents/repo-guidance.md` when slice A4 lands.

### B-5. Verification venues

P4 (Log, 2026-11-20) ran the `smoke` scenario unchanged through Appium
with the harness's own client. What the Apple venue's driver adds to
that client, in slice A6:
- element clicks are real touches (`appium:nativeWebTap`); without it
  they are scripted clicks, which the desktop's are not;
- pointer actions take screen coordinates there, so `clickAt`,
  `movePointer` and `doubleClick` convert from the page's coordinates
  with the same geometry the plugin uses;
- keys with nothing focused go through XCUITest's hardware-key command,
  with a hardware keyboard connected to the simulator: letters, Space
  and the arrows arrived that way in P4. **Escape did not.** A6 finds
  how Escape is sent, or shows on a device with a keyboard that the
  page receives it, before any scenario that presses it is counted.

- **Unit tests:** Rust unit tests, Node tests and the engine's XCTests,
  with generated media, in the simulator.
- **End-to-end:** the scenarios run on the iOS Simulator through Appium's
  XCUITest driver (P4), against the harness's mock Plex and Jellyfin/Emby
  servers on the Mac host. That needs harness work, done in slice A6
  before any parity row is claimed for the Apple engine:
  - **Run sets per platform.** `parity-verdict.mjs` reads exactly two
    result files, requires both players and rejects mixed platforms. So
    the desktop verdict stays the external and embedded pair, unchanged,
    and an Apple run set gets its own verdict over the rows that apply to
    it.
  - **An Appium session** in `driver.mjs`, beside its `tauri:options`
    session, with an adapter for the web-view context.
  - **Apple playback observations** in place of the waits on mpv's socket
    (`helpers.mjs`). The scenarios learn a play's state from
    `player-state` and from the mock servers' reports.
  - **A first proof:** before A6 relies on the venue, it shows one
    complete play, seek, stop and server report.
- **By eye, on devices:** HDR and Dolby Vision cannot be judged in the
  simulator, so the owner checks them on the iPad, in the order the
  plan's "For the owner" section will give when A10 lands. The Dolby
  Vision rows use P2's reference clip and its stripped-metadata control,
  played through the integrated engine.

## Parity list for the Apple engine

Rows 1–12 are those of `.agents/plans/embedded-player.md`, "Behaviour
inventory", and each must pass with the Apple engine:

1. Play starts and resumes at the start position.
2. Plex timeline and Jellyfin/Emby check-ins, and the final stop.
3. The final position on quit, the recents stamp and `playback-ended`.
4. A clean end marks the item watched, in every Continue Playing mode.
5. Playlists and server playlists advance.
6. Source choice and source policy.
7. The auth header never appears in a URL, a log or a title.
8. Skip markers, as a button and as autoskip.
9. Cropping off, manual and auto.
10. The Automatic quality step-down.
11. The unrequested-play notice.
12. A new play stops the previous one (12a), and the app's end stops
    playback (12b). On iPad the app's end is the lifecycle contract of
    A-2: finalization when the background task runs out, or when iPadOS
    terminates the app.
14. A play ends at the end of the file rather than idling. With nothing
    queued and Continue Playing off, the player view closes at the end
    of file; it does not stay on the last frame. A test plays a natural
    end with nothing queued.

Rows 13a and 13b (the user's mpv options and own-config mode) are mpv's
own; they are difference 2, approved as absent on 2026-11-18.

New rows, for every Apple device:

15. HDR10 and HLG tagged and shown in HDR.
16. Dolby Vision profiles 5, 8.1, 8.4, 7 and 10 (where the device
    decodes AV1) shown as Dolby Vision, each on the minimum OS. Profile 7
    MEL is converted. For profile 7 FEL the row is open until the owner
    has chosen among A-2's three ways after P5; while it plays
    converted, the play's state says so.
17. Each audio format listed in A-2, decoded or passed as A-2 says. The
    ear cannot tell stereo, multichannel LPCM and Atmos apart, so the
    proof is threefold:
    - XCTests assert the format and channel layout the engine hands to
      Apple for each source format;
    - channel-identification clips (a voice per channel) are played on
      the device;
    - the actual route is recorded with what it showed: speaker,
      headphones, or an HDMI adapter to a receiver, whose display names
      the format, and Atmos on equipment that can show it.
18. Text and bitmap subtitles, chosen and turned off.
19. Multi-part items play as one timeline.
20. The TLS-verify setting is honoured.

## Slices

Each slice is one or more commits, each with its tests and red-proofs,
each pushed to `origin` and `github` as it lands
(`.agents/push-policy.md`). Each slice that lands repo code gets a
`codereview` hunt by the other model family from the one that wrote it
(`.agents/decisions.md`, 2026-11-20): codex for what Claude wrote, a
Claude reviewer for what codex wrote. The Log
records what landed.

- **S0, probes.** Throwaway code outside the repo, with results in the
  Log:
  - P1: the Tauri iOS shell, and a native layer under a transparent page.
  - P2: the engine's paths on an HDR iPad and on each of the owner's
    Apple TVs with its LG. It covers profiles 5, 8.1 (CMv2.9 and CMv4.0)
    and 7 (MEL and FEL, converted), HDR10, HLG and SDR; E-AC-3 Atmos,
    TrueHD and DTS-HD MA; seeking, track switching and memory. Profile
    8.4 comes from an iPhone recording, profile 10 plays on hardware that
    decodes AV1, and every route P2 tries is run on the oldest OS it
    would claim.
    - Each Apple TV run starts in SDR, with Match Content's Dynamic
      Range on, so a box already in Dolby Vision mode cannot pass for a
      switch. SDR and HDR10 clips are played as controls.
    - Metadata, not just the mode, is checked with a reference clip: one
      base layer with two RPUs chosen to look visibly different, plus
      the same base layer with its RPU stripped (dovi_tool, test-only).
      The owner confirms each looks as its metadata says, on the TV and
      on the iPad.
  - P3: the Rust crate built and run in the iOS Simulator, with TLS to
    the mock Plex, `File::lock` and the config location.
  - P4: Appium driving the page in the simulator.
  - Before P2, Probe P2's media plan names which of the owner's files it
    uses: "Probe P2's media plan", below, written 2026-11-20, with what
    the owner's test library lacks.
  - Run on 2026-11-20 (Log): P1, P3 and P4, and the parts of P2 that
    Assumptions lists as shown on the iPad Pro. P2 is not finished: what
    Assumptions lists as not shown stays open, and P2 on the Apple TVs
    waits for the owner (the order of work, Status).
- **T0, Tauri 2.12 on the desktop.** Tauri 2.12.7 with what it brings
  (tao 0.37, wry 0.57, swift-rs 1.0.9; 72 crates in `Cargo.lock` in P1's
  trial) and the npm packages its CLI requires at the same minor
  (`@tauri-apps/api`, `@tauri-apps/cli`, `@tauri-apps/plugin-dialog`).
  Nothing of iOS is in it.
  - **It raises Vela's declared Rust floor from 1.89 to 1.90**, in the
    same change. Tauri 2.12 declares 1.90 (`tauri`, `tauri-build` and
    `tauri-runtime-wry`; tao 0.37 and wry 0.57 declare 1.85), and Cargo
    refuses a dependency above the building compiler's version, so the
    1.89 check that CI runs cannot pass with it. There is no Tauri
    release with the scene lifecycle below that floor. So the manifest's
    `rust-version` and its comment, CI's floor job and its toolchain pin,
    the commands in `.agents/repo-guidance.md` (Verification) and the
    documents that name the floor move together, and
    `.agents/decisions.md` records the new number under the rule of
    2026-07-15, which stands: a declared floor, tested.
  - It lands by itself and is proved as any desktop change is: the whole
    verification set at the new floor and on stable, both Linux
    end-to-end suites with the parity verdict PASS, the Windows suite and
    the macOS jobs in CI, and its review.
  - P1's trial compiled Vela against 2.12.7 with no source change, on
    macOS and iOS, and the macOS unit tests gave what `main` gives. It
    built with Rust 1.99, which is why it did not meet the floor.
- **A1, the media library build.** Scripts, pins, CI job, licence texts,
  and the dependency-policy test.
- **A2, the player seam (A-1).** It lands in steps, each with the desktop
  suites green: `PlayRequest` and the mpv adapter; `PlayerEvent` and the
  trackers moved onto it; `PlayerCommand` and the channel trait; the
  marker arbiter and its shared fixture.
- **A3, Rust on iOS (B-1).** The crate builds for iOS in CI and in
  `check-other-platforms.sh`.
- **A4, the Tauri iOS target (B-2, first half).** The app runs in the
  simulator with the page and no playback; the icons; the CI job.
- **A5, engine core (A-2).** Input; video decoded by the engine and
  presented by the layer, the hardware and the software path, Annex B
  input and interlaced input included; sound, decoded and passed on,
  timed by sample count; seeking; events and commands; the footprint
  budget. With XCTests on generated media.
- **A6, the plugin and `PlayerKind::Apple` (B-2, second half).** Rows 1–7
  and 12 in the simulator venue (B-5's harness work comes first).
- **A7, HDR and Dolby Vision (rows 15–16).** By eye on the owner's iPad,
  with the integrated engine in Vela's app. Apple TV is proven by
  `tvos-port.md` (T6, T7) with the integrated engine there. P2's
  throwaway build only chooses the architecture and counts for neither.
  Profile 7 here is the live conversion (libdovi in the engine), MEL and
  FEL, with FEL's state saying what it is.
- **P5, a probe: a FEL picture from both layers.** Throwaway, results in
  the Log, before any of it is engine code. It must show, on the iPad
  Pro and the A16 iPad: the enhancement layer decoded beside the base
  layer at the film's rate; the two composed as the RPU says, at the
  depth the result needs; the composed picture agreeing with one
  composed offline from the same frames by an independent tool; and the
  composed picture on the screen both ways A-2 names, as HDR10 and with
  a converted RPU attached, beside the converted play. What each costs
  in memory and heat is measured. The results go to the owner, who
  chooses among A-2's three ways for FEL titles; the chosen way is then
  designed in an amendment, reviewed, and built after A7. If composition
  does not hold on a device, that goes to the owner too.
- **A8, audio, subtitles and multi-part items (rows 17–19).** Also
  TLS-verify (row 20).
- **A9, markers, cropping and Automatic quality (rows 8–11).** Row 11,
  the notice of an unrequested relaunch, is proved here with a real
  step-down. The mock server degrades the stream under control (the
  harness records today that its mocks cannot force the condition,
  `tests/e2e/parity.mjs`, so this control is built here), and the test
  checks the resumed position and that the notice shows exactly once.
  Also the page on iPad (B-3), after O4's rulings.
- **A10, lifecycle, signing and distribution (B-4, O2).** Then the docs
  (README platform list, `docs/`), the release notes and the owner's
  checklist.

## Probe P2's media plan (2026-11-20)

Source: the owner's Plex library "Test Video" on `altiera` (owner,
2026-11-20: "I added a library to plex called "Test Video" which contains
hdr test video found on github"). 139 files, read through Plex that day
with the sign-in Vela holds on `nagatha`; the list with every stream's
facts is `p2/test-video-library.json` in the probes' folder
(`.agents/machines.md`). Most are HEVC HDR10 clips of one to eight seconds
with no sound.

What P2 uses from it:

| For | File | Facts |
|-----|------|-------|
| SDR control, and software decoding | `2022-08-31,21.08.25.mkv` | H.264 High 10 (10-bit, which Apple's hardware does not take), 1080p, 1 s |
| HDR10 control | any of the 125 HDR10 clips; `La La Land.mkv` | HEVC Main 10, 2160p, 8 s |
| HDR10, long, for seeking and memory | `01. Black Clipping_1_HDR10.mp4`, `240-1000nits-pattern.mp4` | 300 s and 360 s, one AAC track each |
| HDR10 with lossless sound | `Aquaman_test.mkv` | 2160p, TrueHD 7.1 with Atmos, 11 s |
| HLG | `snow-fades.mp4`, `Grayscale BT.2100 HLG.mkv` | 2160p 20 s with AAC; 1080p 105 s |
| Dolby Vision 8.1 | `01freeguy.mkv`, `01interstellar.mkv`, `02themeg-dovi.mkv` | 1080p, level 3, 4 s, no sound |
| Dolby Vision 8.1 at 2160p | `contrast-recovery-pq-output-colornoise.mkv` | level 7, 4 s, with an ASS subtitle |
| Dolby Vision 5 | `dark.mkv` (`transition.mkv` is the same file) | 2160p, level 6, **one frame** |
| AV1 (software on the A16 iPad and the Apple TVs, hardware on the M5) | `waterfall.mkv` | 2160p at 59.94, HDR10, 6 s, Opus |
| VP9 | `hdr1500.mkv` | profile 2, 2160p, HDR10, under a second |
| Subtitles | `asuka.mkv` and `vindiesel.mkv` (ASS), `scenechange.mkv` (SRT) | |

Four entries are not media: `landing_dv.mkv`, `landing_hdr.mkv`,
`landing_sdr.mkv` and `tonemap_flicker.mkv` are 133-byte Git LFS pointer
files (the first names a file of 84 MB). Fetched with Git LFS they would
be real clips.

What P2 needs and the test library does not have, and where it comes
from instead. The owner, 2026-11-20: "scan plex for whatever you need for
testing". The owner's other Plex libraries were read that day (9,555
media entries; read-only, with the sign-in Vela holds on `nagatha`), and
short excerpts were cut from the files below by stream copy into
`p2/media2/` of the probes' folder. They are the owner's films and
episodes: they stay on the owner's machines and devices and are in no
repository.

| For | Excerpt | From | Facts |
|-----|---------|------|-------|
| Profile 7, MEL, with TrueHD Atmos | `p7-fifth-element.mkv` | The Fifth Element (1997) | 2160p, 12.6 s, CMv2.9; TrueHD 7.1 with Atmos, AC-3 5.1, PGS |
| Profile 5 longer than a frame, with E-AC-3 Atmos | `p5-shantaram.mkv` | Shantaram, "Dead Man Walking" (2022) | 3840x1606, 25 s; E-AC-3 5.1 with Atmos; SRT |
| Profile 8.1 with DTS-HD MA; several tracks | `p81-el-camino.mkv` | El Camino (2019) | 2160p, 25 s; DTS-HD MA 5.1 and two AC-3 stereo tracks; PGS and SRT |
| HDR10 with DTS-HD MA | `hdr10-dtshdma-witch-hunter.mkv` | The Last Witch Hunter (2015) | 3840x1596, 15 s; DTS-HD MA 7.1 |
| DTS:X | `hdr10-dtsx-ex-machina.mkv` | Ex Machina (2014) | 2160p, 10 s; DTS-HD MA 7.1 with DTS:X |
| Software-decoded video with E-AC-3 | `vc1-eac3-last-samurai.mkv` | The Last Samurai (2003) | VC-1 1080p, 20 s; E-AC-3 5.1 and 2.0; PGS |
| MPEG-2, interlaced | `mpeg2-480i-snl.mkv` | Saturday Night Live (1981) | 704x480 at 29.97 interlaced, 20 s; AC-3 2.0; EIA-608 captions |

Also cut: `p7-fel-mockingjay2.mkv` (The Hunger Games: Mockingjay, Part 2,
2015: profile 7 with a full enhancement layer, by dovi_tool's reading;
2160p, 13 s, TrueHD 7.1 with Atmos), which with the Fifth Element excerpt
(renamed `p7-mel-fifth-element.mkv`) gives both kinds of profile 7. Each
has a copy converted to profile 8.1 on the Mac with dovi_tool's mode 2,
its enhancement layer discarded (`p7to81-fel-mockingjay2.mkv`,
`p7to81-mel-fifth-element.mkv`): the same conversion libdovi makes live.

Three entries the scan chose could not be read: Plex answered 404 for
Mortal Engines (2018) and timed out on Oldboy (2003) and RED 2 (2013).
Told to the owner.

Still without a file: profile 8.4 (an iPhone's recording) and profile 10
(the libraries hold no AV1).

Found in passing: the libraries hold 199 MPEG-2 entries, interlaced
broadcast recordings among them, 187 VC-1 and 595 MPEG-4 ASP. A-2 does
not mention deinterlacing; the engine's software path needs it for these.

Read with dovi_tool 2.3.4 on 2026-11-20: the three 1080p 8.1 clips carry
CMv4.0 metadata, and `contrast-recovery-pq-output-colornoise.mkv` and the
profile 5 frame carry CMv2.9. So both versions are in the library.

Made by the probe on 2026-11-20, as S0 says, from `02themeg-dovi.mkv`'s
base layer (`p2/dv/` in the probes' folder has the recipes): `ref-low.mkv`
(an RPU saying the scene peaks at 100 nits), `ref-high.mkv` (one saying
10,000 nits, with full saturation trims) and `ref-stripped.mkv` (no RPU,
no Dolby Vision box).

## Risks

- **P2 may find that neither path gives real Dolby Vision from MKV on
  the owner's Apple TVs.** That would contradict what shipping apps
  report (Infuse's Dolby Vision for profiles 5 and 8; AetherEngine's live
  7-to-8.1 conversion). By the owner's order of 2026-11-20 the iPad's
  and iPhone's engine is built before that half of P2 runs. So a
  finding there no longer stops engine code from landing: it goes to the
  owner, and the engine is changed for the Apple TV as far as it needs.
- **The attachment that carries the RPU has no name in Apple's public
  headers.** `DolbyVisionRPUData` is the key VideoToolbox was seen to
  use. The engine only passes on pictures VideoToolbox decoded, with
  whatever VideoToolbox attached, and never writes the key, so Dolby
  Vision in Vela rests on public API: a documented codec type, a
  decompression session and the display layer. The one thing that would
  need the key written is way 3 for FEL titles, which is the owner's to
  allow or not (A-2).
- **A FEL picture from both layers may not be possible in real time**
  on one of the devices (P5). Until it is shown, FEL plays converted and
  says so.
- **The engine now owns the decoded pictures**, so its memory is Vela's
  to bound; the iPhone has the least of it (`ios-port.md`).
- **A single Svelte page for desktop and iPad can regress the desktop.**
  Every page slice runs both desktop suites.
- **The engine is the largest new code in the repo.** The rule "the
  newest fix is the most dangerous code" (repo-guidance) applies to every
  engine commit, and so does red-proofing each claimed behaviour
  separately.
- **The simulator cannot show HDR, Dolby Vision or real audio formats.**
  Those rows rest on by-eye checks. They are written as "not verified by
  any automated test" until the owner has done them.

## Verification before claiming completion

- The repo-guidance Verification set, for both sides of the app.
- Both Linux end-to-end suites in full, with the parity verdict PASS.
- The Windows suite in CI.
- The iOS CI job, and the simulator venue with every parity row the
  simulator can judge.
- The owner's by-eye checks for rows 15–17, recorded with what was seen.

## Log

- 2026-11-18: drafted after the owner's rulings of that day. Inventories
  were made by four read-only explorers: the player seam, the desktop-only
  Rust code, the page's touch and size assumptions, and the build, CI and
  test infrastructure. Their findings are folded into A-1, B-1, B-3 and
  B-4.

- 2026-11-20: **probes P1, P3 and P4 ran on `nagatha`** (owner's go of
  that day: "approve the probes"). Xcode 27.2, the iOS 27.2 simulator, a
  simulator made for the probes ("Vela Probe iPad Pro", iPad Pro 13-inch
  M5), a throwaway clone of `main` at `d32c5ea`. Code, logs and
  screenshots: `.agents/machines.md`, "Apple probes on nagatha". P2 has
  not run: it needs the owner's devices and files. None of A1, A5 or A6
  was overturned. What each probe found:

  **P3 (A5: the Rust crate on iOS). Holds, with these changes:**
  - `swift-rs` 1.0.7, which `Cargo.lock` pins, cannot build Tauri's Swift
    package for iOS under Xcode 27: SwiftPM adds the host's `-sdk` and
    `-target` after the ones swift-rs passes. swift-rs 1.0.9 has the fix.
  - With that, the crate type-checks for `aarch64-apple-ios` and
    `aarch64-apple-ios-sim` after one change: `player_fullscreen`
    (`commands.rs`) calls `WebviewWindow::set_fullscreen`, which Tauri has
    on desktop only.
  - The test build needs one more: `player/embedded/mod.rs` builds
    `macos_geometry` and `macos_display` under `any(target_os = "macos",
    test)`, and they use `session`, which iOS does not build.
  - The compiler confirms B-1's `rename_noreplace` defect by itself: on
    iOS it warns that the code after the `Unsupported` return is
    unreachable.
  - The unit tests ran inside the simulator (`cargo test --target
    aarch64-apple-ios-sim --lib`, with `xcrun simctl spawn` as the
    runner): 595 tests. As `main` is, 556 passed and 38 failed: 27 on the
    rename (every durable save over an existing file), 11 that start a
    stand-in mpv or `sleep`. With B-1's rename fix (`target_vendor =
    "apple"` on both predicates), 585 passed and 9 failed, all 9 tests of
    the external player that start a process. tokio, reqwest and the
    durable stores' tests are among the 585.
  - Inside the app, in the simulator (a routine in the throwaway build,
    results in `logs/` there):
    - the config directory is `<container>/Library/Application
      Support/com.vela.vela`, as the explorer read it;
    - three durable saves through `config::update` succeeded, with the
      version history beside `config.json`, and the value read back;
    - `File::lock` holds: a second handle is refused while the first has
      it, and gets it after release;
    - `rename_noreplace` moves onto a fresh name and refuses an existing
      file, whose bytes stay;
    - TLS: a server whose CA the simulator trusts answers with
      verification on; a server with an unknown CA is refused with
      verification on and answers with it off;
    - plain HTTP from reqwest reaches an IP address, a `.local` name and a
      full domain name. Apple's `URLSession`, asked for the same URLs from
      the same app, refuses the full domain name with App Transport
      Security's error -1022. So ATS does not govern the core's client.
  - Not proven by the simulator: the sandbox. `/tmp` was writable there,
    which a device will not allow. The same routine runs on the iPad when
    it is paired (with P2).
  - Seen on screen: the page shows "mpv is required" with Linux's
    install text, so iOS does take the Linux branches B-1 lists.

  **P1 (A1: the Tauri iOS shell and a native layer under the page).
  Holds, with these changes:**
  - **An app built with the iOS 27 SDK must adopt the scene lifecycle or
    it does not launch.** Vela on Tauri 2.11.5 (tao 0.35.3) was killed at
    launch on iOS 27.2 inside UIKit's check for it. tao adopts scenes for
    a single-window app from 0.37, when `Info.plist` has a
    `UIApplicationSceneManifest`. That needs Tauri 2.12.7 (tao 0.37.3,
    wry 0.57.3; 72 crates in `Cargo.lock` move), and the same minor of
    the npm packages, which the CLI enforces: `@tauri-apps/api` 2.12.5,
    `@tauri-apps/cli` 2.12.6, `@tauri-apps/plugin-dialog` 2.8.4. Vela's
    Rust compiled against 2.12.7 with no further change, on iOS and on
    macOS, where the unit tests give the same result as `main`.
    **The update reaches every desktop platform, so it lands by itself
    first, proved by the desktop suites, before any iOS work.**
  - With it, and `Info.ios.plist` carrying the scene manifest
    (`UIApplicationSupportsMultipleScenes` false), Vela's page shows in
    WKWebView on the iPad simulator and the Rust core answers it.
  - Tracked files alone do configure the generated project:
    - `tauri.ios.conf.json`: the identifier, `minimumSystemVersion`,
      `developmentTeam`, and `bundle.resources` emptied, so the app
      carries no mpv scripts;
    - `Info.ios.plist`: merged at each build (the scene manifest,
      `NSLocalNetworkUsageDescription`);
    - iPad only needs `bundle.iOS.template`, a tracked copy of Tauri's
      XcodeGen template with `TARGETED_DEVICE_FAMILY: "2"`. Its path is
      read against the directory the CLI runs in, not the config's;
    - `src-tauri/gen/apple` was deleted and made again from those files,
      and the build came out the same.
  - An in-tree plugin with Swift builds and links from a path
    dependency. It leaves generated files in its own directory
    (`.tauri/`, `permissions/autogenerated/`, `ios/Package.resolved`),
    which the real plugin's commit must ignore.
  - **The layer.** An `AVSampleBufferDisplayLayer` in a view put beneath
    the web view (`insertSubview(_:belowSubview:)` on tao's view) shows
    through the page wherever the page is transparent, with the page's
    text and controls drawn over it. Screenshots taken at different
    moments show its moving bar at different places.
  - **Touches.** Real touches (XCUITest, trusted events) on a button
    over the layer, and on Vela's own "Add a source" over the layer,
    reached the page: the counter counted, and Settings opened.
  - The page's coordinates are not the view's. The web view fills the
    screen, and its content starts below the status bar (32 pt on this
    iPad), so a rectangle the page reports must be moved by that inset,
    unless the page covers the safe area (`viewport-fit=cover`, B-3),
    which makes the inset zero. `player_set_rect` for iOS is written and
    tested with that in mind.
  - The web view is already not opaque at launch (Vela asks Tauri for a
    transparent window), and tao's view behind it is white. The plugin
    gives the area behind the page a black backing.
  - The simulator reports an EDR headroom of 1.0. HDR is a device matter,
    as the Risks say.
  - Local builds: Tauri's CLI does not pass `DEVELOPER_DIR` on to
    `xcodebuild`, and its `ios init` wants CocoaPods (`.agents/machines.md`
    has how `nagatha` is set up).

  **P4 (A6: the scenarios through Appium). Holds, with adapters:**
  - Appium 4.0.0 with its XCUITest driver 13.4.0 attaches to Tauri's
    WKWebView as a web context.
  - The harness's own client (`tests/e2e/driver.mjs`), given a session
    opened with Appium's capabilities, ran **the `smoke` scenario
    unchanged, and it passed.** `execute/sync` waits for a promise, so
    `invoke` calls work as they do on the desktop.
  - What slice A6's harness work must add:
    - `appium:nativeWebTap`, or an element click is a scripted click
      (`isTrusted` false). With it, the click is a real touch;
    - pointer actions take screen coordinates, so `clickAt`,
      `movePointer` and `doubleClick` need the same inset as the layer;
    - typing into a field works as it is;
    - W3C key actions arrive only while a text field has focus. XCUITest's
      `mobile: keys`, with a hardware keyboard connected to the
      simulator, delivers letters, Space and the arrows with nothing
      focused, as trusted events. **Escape did not arrive that way** in
      any of three runs. Whether a real keyboard's Escape reaches the page on
      an iPad is not known; A6 finds a route or the owner's keyboard
      settles it.
  - Not tried: a play. There is no engine yet.

  **Found beside the probes:** on `main`, on macOS, two unit tests fail
  (`player::launch::tests::the_minimal_embedded_launch_is_exactly_the_recorded_one`
  and `the_full_embedded_launch_is_exactly_the_recorded_one`): they
  compare with launch arguments recorded on Linux, and macOS adds its
  own. CI runs the unit tests on Linux only. Recorded in
  `.agents/state.md`, Blockers.

- 2026-11-20, later: **P3's and P1's device half, on the owner's iPad Pro
  13-inch (M5), iPadOS 27.2**, with the same throwaway build, signed for
  development with the wildcard profile already on `nagatha`
  (`.agents/machines.md`). The owner unlocked the iPad and started it.
  - **The sandbox holds what the simulator could not show.** `/tmp` is
    refused ("Operation not permitted"), so nothing of the desktop's
    `private_runtime_dir` can be used on iOS, as B-1 says. The temporary
    directory is the container's own `tmp/`.
  - The config directory is `<container>/Library/Application
    Support/com.vela.vela` there too. Three durable saves, `File::lock`
    and the no-replace rename behaved as in the simulator.
  - TLS with verification on: the owner's own Plex server answered over
    its `plex.direct` address, so the device's trust store is used; the
    test server with an unknown CA was refused, and answered with
    verification off.
  - Plain HTTP from reqwest reached the Mac by address, by `.local` name
    and by a full domain name. `URLSession`, from the same app, refused
    the full domain name with ATS's -1022. ATS does not govern the core's
    client on a device either.
  - The requests to the local network succeeded, so the Local Network
    permission was granted on the iPad (`NSLocalNetworkUsageDescription`
    is in the build's `Info.ios.plist`).
  - **EDR:** `potentialEDRHeadroom` is 16 and `currentEDRHeadroom` 1.2 on
    this screen. B-1's HDR capability reads from a real number here.
  - The window was 1600 x 1200 points (landscape, with the iPad set to
    show more space), not the 1376 x 1032 of the default setting. The
    page must not assume one iPad size.
  - The display layer rendered on the device (its renderer's status is
    "rendering", about 90 frames in three seconds, no error). **Seen by
    the owner on the iPad** (2026-11-20, asked "Do you see the colour
    bars with the bar moving?" and "Does tapping the button count up?":
    "1 yes 2 yes"): the pattern shows under the transparent page and
    moves, and a finger on the page's button over it counts. A1 holds on
    the device.
  - The owner, seeing the build: "it's showing the install mpv banner at
    the top which obviously needs to be removed from the ios versions".
    That is B-1's mpv detection, compiled out in slice A3, and one of the
    four differences approved on 2026-11-18.

- 2026-11-20, P2 begun: **the throwaway player, and what it showed on the
  Mac** (`nagatha`, macOS 27, its own XDR screen). macOS is not one of
  P2's venues; these results say the approach works in Apple's pipeline
  of this OS generation, and nothing yet about the iPad or the Apple TV.
  - The pinned FFmpeg (9.0.2, checksum matched) builds for iOS, the iOS
    simulator, tvOS and macOS with `--disable-gpl --disable-nonfree
    --disable-version3 --disable-autodetect`, and reports "LGPL version
    2.1 or later". The probe's build is static and bare, not A-3's recipe.
  - The player (`p2/player/` in the probes' folder): FFmpeg demuxes; the
    compressed video goes to `AVSampleBufferDisplayLayer` as sample
    buffers whose format description carries `hvcC` and the Dolby Vision
    box (`dvcC` for profiles up to 7, `dvvC` above); FFmpeg decodes the
    sound to PCM for `AVSampleBufferAudioRenderer`; one
    `AVSampleBufferRenderSynchronizer` is the clock.
  - **What VideoToolbox does with each way of naming the stream** (first
    picture decoded by a decompression session, its attachments read):
    - profile 5, codec type `dvh1` with `dvcC`: the picture comes out
      tagged PQ with a `DolbyVisionRPUData` attachment (267 bytes);
    - profile 5, codec type `hvc1`, with or without the box: no RPU, and
      colours "guessed by VideoToolbox" as BT.709. **Profile 5 needs
      `dvh1`.**
    - profile 8.1, `dvh1` with `dvvC`, and also `hvc1` with `dvvC`: the
      RPU attachment (156 bytes) and `DolbyCompatibilityID` 1;
    - profile 8.1, `hvc1` without the box: HDR10 tags only, no RPU.
  - **Played through the layer:** `01freeguy.mkv` (8.1): 100 pictures,
    none dropped. `dark.mkv` (profile 5, one frame), captured from the
    window: natural colours as `dvh1`, and the green and purple of an
    unprocessed IPT picture with Dolby Vision switched off
    (`shots/p2-mac-p5-dvh1.png`, `shots/p2-mac-p5-off.png`). So the layer
    applies the metadata, on the Mac. `snow-fades.mp4` (HLG, AAC): picture
    and sound on the one clock; 11 of 219 pictures dropped at 2160p, all
    near the start, not looked into.
  - The iOS 27 SDK deprecates the renderers' `error` property for the
    result `enqueue` returns and a stream of rendering events; A-2's
    engine is written to the new ones.
  - Built for the iPad (signed like the other probe build) and installed
    on the iPad Pro. Not run there yet: the iPad was locked.

- 2026-11-20, P2 on the owner's iPad Pro 13-inch (M5), iPadOS 27.2 (the
  throwaway player, opened by the owner; its log copied from the app's
  container to `logs/p2-ipad-log-1.jsonl` in the probes' folder):
  - **Dolby Vision engages through the sample-buffer path on the iPad.**
    `dark.mkv` (profile 5, from MKV) as `dvh1` with `dvcC`, then with
    Dolby Vision switched off: the owner, asked whether the first looks
    natural and the second green and purple, "yes, DV seems to work".
    The decoder agrees with the Mac's, frame for frame: profile 5 as
    `dvh1` comes out PQ with a 267-byte RPU attachment and as `hvc1`
    without one; profile 8.1 carries its RPU as `dvh1` and as `hvc1` with
    `dvvC` (156 and 173 bytes on the two clips), and none without the
    box. All on the hardware decoder.
  - The 8.1 clips at 1080p and 2160p rendered. The frame counts of that
    run say nothing about smoothness: the probe's looping replayed old
    timestamps (repaired since).
  - HDR10 (`Aquaman_test.mkv`), HLG (`snow-fades.mp4`) and AV1 at 2160p
    and 59.94 (`waterfall.mkv`: hardware decoder, 2 of 356 pictures
    dropped) rendered. While they showed, the screen's
    `currentEDRHeadroom` was 3.2 of a potential 16 (it was 1.2 under the
    page of the first probe build).
  - **A4, first result: sound was missing.** TrueHD 7.1 decoded by FFmpeg
    to 8-channel float PCM and given to `AVSampleBufferAudioRenderer` with
    a label for each channel: the renderer reported "rendering" and no
    error, 316,840 frames went in, and the owner heard nothing ("no audio
    on E"). The iPad's route is its speakers, 2 channels. Three ways of
    describing the same sound are on the iPad for the owner's ears
    (Apple's 7.1 layout tag with the channels rearranged for it, the
    labels again, a stereo downmix); not answered yet.
  - **10-bit H.264 (High 10) is decoded by Apple's own decoder**, on the
    Mac and on the iPad (`2022-08-31,21.08.25.mkv`: a decompression
    session reports a hardware decoder and a 10-bit picture, and the Mac's
    window shows the test pattern). A-2 names "H.264 10-bit" among the
    codecs Apple does not decode; that is wrong for this OS and these
    chips. What goes to software is decided by asking VideoToolbox, as
    A-2 already says for refusals, never by a list.
  - FFmpeg's software path into the same layer works (the same clip
    through `h264` and pixel buffers, on the Mac). In window captures on
    the Mac its greys came out a little darker than the Apple path's;
    the cause was not found, and the engine's software path has to match
    the hardware path's tagging exactly.

- 2026-11-20, P2 continued (the iPad Pro and the Mac):
  - **The silent TrueHD was the timestamps, and it is repaired in the
    probe.** A TrueHD frame is 40 samples (0.83 ms), and Matroska stamps
    packets to the millisecond, so the stamps repeat and overlap (0, 38,
    38, 39, ... 43, 43). The probe gave Apple's audio renderer 1,200 tiny
    buffers a second with those stamps: it reported "rendering", no
    error, and played "a couple of ms of static at load" (the owner),
    whichever way the channels were described, a stereo downmix included.
    The decoded sound was right all along: the probe's PCM is identical,
    sample for sample, to FFmpeg's own decode of the file. Repaired by
    counting time in samples from the first stamp, in buffers of 50 ms,
    and believing a container stamp again only when it is more than 0.1 s
    off. The owner, with that build on the iPad's speakers (a 2-channel
    route with spatial audio on): "audio worked". **For A-2: decoded
    sound is timed by sample count; container stamps only anchor it.**
    Which channel description the engine should use was not separated:
    all three variants got the same repair, and the owner paused sound
    tests before saying which he heard.
  - The owner's other answers on the iPad: 10-bit H.264 shows the same
    sharp pattern through Apple's decoder and through FFmpeg's ("they
    look basically the same but I haven't looked too closely"); the VP9
    clip through FFmpeg's decoder shows its frame (a dim room; the
    agent's caption called it bright, wrongly); Dolby Vision 8.1 at 2160p,
    looping, "seems smooth".
  - **Compressed samples given straight to the layer lose pictures on
    streams with deep B-frame pyramids at 2160p.** `Aquaman_test.mkv`
    (HEVC, eight B-frames between anchors): 25 to 34 of 255 pictures
    dropped on the Mac, 41 of 240 on the iPad, steadily, about one a
    group. `asuka.mkv` (three B-frames): 7 to 12 of 242. `snow-fades.mp4`:
    19 to 24 of 289. Clips without that reordering lose none
    (`contrast-recovery...`, `waterfall.mkv` in AV1, the 1080p clips).
    Tried on the Mac and of no effect: the container's decode times, no
    decode times, decode times made by the probe sixteen frames early,
    the attachment that says later samples may show earlier, a remux to
    MP4, no sound.
  - **With a VideoToolbox session of the engine's own, none are lost.**
    The probe decodes each sample in a `VTDecompressionSession`, sorts
    the decoded pictures by presentation time in a queue of sixteen, and
    gives the layer decoded pictures in the order they are shown: 0
    dropped on all three clips (Mac).
  - **Dolby Vision engages on that path too** (Mac): the session's
    pictures carry the `DolbyVisionRPUData` attachment, and the layer
    applies it: `dark.mkv` (profile 5) shows natural colours
    (`shots/p2-mac-p5-vt-dvh1.png`); the 8.1 clip plays 100 pictures,
    none dropped.
  - **What P2 says about A-2's design, so far:** the layer stays, and the
    engine does the decoding. Hardware codecs go through a decompression
    session of the engine's own and reach the layer as decoded pictures
    in display order, as software-decoded ones already do. The engine
    then also has every decoded picture in hand, which is what cropping's
    detector needs without a second session. A-2's "compressed sample
    buffers to the layer" is amended, and the amendment reviewed, before
    engine code. Not yet shown on the iPad or an Apple TV: the build with
    this path is on the iPad, silent, and has not run (locked).

- 2026-11-20, P2, the reference clip: one picture of `ref-low.mkv`,
  `ref-high.mkv` and `ref-stripped.mkv` (the media plan says what they
  are), each through the engine's own decompression session into the
  layer on the Mac, captured from the window: mean luma 94.8, 52.3 and
  88.1 of 255. The same base picture is drawn three ways, so the layer
  uses what each RPU says, not just the fact of Dolby Vision. The three
  are in the iPad build for the owner's eyes. The player also builds for
  tvOS (unsigned; no Apple TV is paired and no tvOS profile exists yet).

- 2026-11-20, P2, **the engine's own decompression session, on the iPad
  Pro** (`logs/p2-ipad-log-3.jsonl`; every clip silent):
  - `Aquaman_test.mkv` given to the layer compressed: 41, 42 and 42 of 255
    pictures dropped in three passes. Decoded by the player's own session
    and given in display order: 0 of 255. The owner, asked whether the
    second is smoother: "y".
  - **Dolby Vision engages on that path on the iPad.** Profile 5 through
    the session (decoded as `&xf0`), then with Dolby Vision off (`pf20`):
    natural, then green and purple; the owner: "y". Dolby Vision 8.1 at
    2160p through the session: 0 dropped.
  - The reference clip: the owner sees the two metadata sets differ on
    the iPad, though not as on the Mac: "G is not darker than F, but it's
    less vibrant". (On the Mac's screen the second set was much darker.)
    So what each RPU says changes the picture on the iPad too. Whether
    the stripped copy looks different from both was not said.
  - With this, A2 holds for the iPad Pro for profiles 5 and 8.1, by the
    route "FFmpeg demuxes, the engine's VideoToolbox session decodes, the
    sample-buffer layer presents". Still to do for P2 on the iPad:
    profile 7 converted (no file yet), 8.4 and 10 (no file), seeking,
    track switching, memory, E-AC-3 with Atmos and DTS-HD MA (no file),
    and which channel description sound needs. The Apple TV half has not
    begun.

- 2026-11-20, P2, seeking, on the Mac (`240-1000nits-pattern.mp4`, HEVC
  2160p, six minutes, silent, the engine's own session): six seeks, two
  seconds apart, to 30, 200, 10, 340, 95.5 and 180 s. Each showed its
  first picture within one frame after the target, 86 to 225 ms after it
  was asked, having decoded and thrown away the 16 to 72 pictures between
  the keyframe before the target and the target. No picture dropped; the
  process's footprint peaked at 22.6 MB. Seeking to the exact picture
  falls out of the engine doing the decoding: with compressed samples
  given to the layer, the pictures before the target cannot be held back.
  The same test and a one-minute memory run are in the iPad build
  (silent); not run there yet (locked). Track switching has no file: the
  library has none with two audio tracks.

- 2026-11-20, P2, profile 7 and the owner's own films (the media plan
  says what each excerpt is; the Mac, silent, the engine's own session):
  - **Apple's decoder refuses profile 7 as it comes.** A decompression
    session for a profile 7 stream, MEL or FEL, named `dvh1` or `hvc1`
    with its `dvcC` box, cannot be created (-12910). Without the box the
    base layer decodes as plain HDR10, no RPU.
  - **Converted to 8.1 it is Dolby Vision.** The copies converted with
    dovi_tool's mode 2 (libdovi's conversion, done beforehand here)
    decode as `dvh1` with `dvvC` and carry the RPU attachment (163 bytes
    on the FEL clip). Both play, 100 pictures, none dropped. So A-2's
    live conversion of profile 7 is the route, and the device question it
    rested on is answered for this decoder; by eye on the iPad it is on
    the owner's list. The live conversion itself (libdovi inside the
    engine) is not built in the probe.
  - A real profile 5 scene (`p5-shantaram.mkv`): `dvh1`, RPU of 316 bytes,
    plays, none dropped.
  - 2160p films through the session, 4 s of each, none dropped: Dolby
    Vision 8.1 (`p81-el-camino.mkv`), HDR10 (`hdr10-dtshdma-witch-hunter.mkv`,
    `hdr10-dtsx-ex-machina.mkv`).
  - FFmpeg's software decoders into the layer: VC-1 at 1080p
    (`vc1-eac3-last-samurai.mkv`) and MPEG-2 at 480i (`mpeg2-480i-snl.mkv`),
    none dropped. The probe neither deinterlaces nor applies the pixel
    aspect ratio; the engine must do both.
  - Sound, built and not yet heard: AC-3 and E-AC-3 handed to Apple's
    audio renderer undecoded (one sample buffer a frame; the renderer
    takes them and reports "rendering" on the Mac); spoken channel checks
    in 7.1 (FLAC) and 5.1 (E-AC-3, AC-3, DTS), made with the Mac's voice,
    each voice proven to sit in the channel it names; a change of audio
    track in the middle of a play. All of it is in the list on the iPad,
    for the owner's ears.

- 2026-11-20, P2 on the iPad Pro, the owner's films and the sound list
  (`logs/p2-ipad-log-4.jsonl`; the engine's own session for every clip;
  every clip: no picture dropped, the audio renderer "rendering", no
  jump in the sound's timing). The owner's answers, in his words:
  - Profile 5, a real scene, its E-AC-3 with Atmos handed to Apple
    undecoded: "looks and sounds correct".
  - Profile 7 FEL converted to 8.1, against the same pictures as plain
    HDR10 (silent on purpose): "c is much brighter and has no sound". So
    the two differ on the iPad, and the HDR10 rendering is the brighter.
    **Which of the two is right is not known.** On the Mac's screen one
    frame of each measured alike (mean luma 18.4 as Dolby Vision, 18.2
    as HDR10; the MEL clip 59.9 and 51.9; the 8.1 film 75.7 and 78.4).
    A profile 7 title whose enhancement layer carries real picture can
    come out wrong once that layer is discarded, and the iPad's smaller
    headroom (2.2 to 2.4 during these clips) also maps the two
    differently. It needs a reference: the same scene in a player known
    to be right, on the same iPad, by the owner's eye. Asked of the
    owner.
  - A change of audio track in the middle of a play (`p81-el-camino.mkv`,
    DTS-HD MA to AC-3 at 7.9 s and back at 15.8 s): "I noticed a track
    change in the breaking bad scene". 743 pictures, none dropped. The
    probe's switch leaves a gap as long as what was queued of the old
    track; the engine re-reads from the current position instead.
  - The spoken channel checks, 7.1 and 5.1, each way of describing the
    channels, and E-AC-3, AC-3 (undecoded) and DTS (decoded): "yes
    positioning seems correct but hard to tell with an ipad not connected
    to a surround system". Left and right are settled; the surround
    positions wait for the Apple TV's soundbar. So A4 holds on the iPad
    for TrueHD, DTS-HD MA, DTS:X (as DTS-HD MA), DTS, FLAC and AAC
    decoded to PCM, and for AC-3 and E-AC-3 passed undecoded; Atmos as
    Atmos cannot be told on these speakers.
  - **Memory.** The player's footprint peaked at 286 to 408 MB on the
    2160p clips, 45 to 49 MB on 1080p in software, 25 MB on the 480i
    clip. Most of the 2160p figure is the probe's fixed queue of sixteen
    decoded pictures. The engine sizes that queue from the stream's own
    reordering depth, and A-2 gets a footprint bound for it (the iPhone
    plan already asks for one).
  - The screen's `currentEDRHeadroom` followed the content: 2.2 to 2.4
    during the HDR and Dolby Vision clips, 1.0 during the SDR ones.
  - **Where P2 stands for the iPad Pro:** Dolby Vision engages for
    profiles 5 and 8.1 (CMv2.9 and CMv4.0) and for profile 7 converted
    (decoder and playback; its look is the open point above); HDR10, HLG
    and SDR play; the sound formats above play; seeking is exact (Mac;
    the iPad's run of it was not made). Not done: profiles 8.4 and 10 (no
    file), the live profile 7 conversion (libdovi in the player), and
    everything on an Apple TV.

- 2026-11-20, P2, **profile 7 with a full enhancement layer is not right
  once that layer is thrown away.**
  - The owner compared with Infuse on the same iPad: "mockingjay in Infuse
    looks like the brighter clip, C". C is the probe's plain HDR10
    rendering of the disc's base layer; B, the 8.1 conversion, is the
    darker one. So the probe's HDR10 path matches the player the owner
    trusts, and the conversion of this FEL title does not.
  - What the disc says (dovi_tool): its RPU is for a 4000-nit master
    (`0.0050/4000 nits`, trims for a 1000-nit target), where the MEL disc
    (The Fifth Element) is a 1000-nit master. The conversion keeps that
    metadata and discards the layer that, with the base layer, makes the
    picture it describes. ASSUMPTION, fitting both facts and not proven:
    the display mapping then treats a base layer that stands by itself as
    HDR10 as if it were the brighter composite, and darkens it.
  - **So A-2's "profile 7 (MEL or FEL) becomes 8.1 through libdovi's
    conversion" holds for MEL at most.** For FEL the picture the disc
    means needs both layers put together.
  - **Whether the layer would show such a picture: yes, on the Mac.** The
    probe copied each decoded profile 5 picture into a pixel buffer of
    its own making (plain 10-bit, `xf20`), with the decoder's attachments,
    and gave the layer that: the window measures exactly as with the
    decoder's own buffer (mean Y 25.66, U 126.92, V 128.50). The same copy
    without the `DolbyVisionRPUData` attachment measures as the
    unprocessed picture (33.89, 122.16, 120.85). So the layer's Dolby
    Vision is driven by that attachment and works on pictures the app
    makes. A picture the engine composes from a base and an enhancement
    layer, with an RPU attached, is therefore something the layer can
    present. The same two clips are on the iPad for the owner's eyes.
  - Not built or shown: decoding the enhancement layer beside the base
    layer and putting the two together (the residual and its scaling are
    in the RPU; open implementations exist for offline use), at 2160p in
    real time, and at what bit depth the result is handed over. That is a
    probe of its own before A-2 promises it.
  - MEL converted (The Fifth Element) has not been compared with a
    reference by eye.

- 2026-11-20, P2 on the iPad, two answers from the owner:
  - **The layer applies Dolby Vision to pictures the app makes, on the
    iPad too.** Profile 5 (`p5-shantaram.mkv`) with each decoded picture
    copied into the player's own pixel buffer and the RPU attached, then
    without the RPU: natural, then green and purple; the owner: "1 yes".
  - The MEL comparison with Infuse could not be made: "I cannot tell.
    there's no way to pause or rewind in your test app so a comparison
    has to go by visual memory and that is unreliable." The probe app now
    has what that needs: pause, a step of one picture either way, a
    second either way, restart, previous and next clip, each clip held at
    its end, a clock with the film's own time (roughly), and it runs in
    Split View beside another player. The controls were driven by a
    script on the Mac first (pause holds the picture; steps move one
    picture; seeks land where asked; nothing dropped). The first step
    after a pause can move two pictures, since the pause lands between
    two; later steps are single.

- 2026-11-20, **correction to the entry on profile 7 FEL above.** That
  entry reads the owner's comparison ("mockingjay in Infuse looks like
  the brighter clip, C") as showing that the 8.1 conversion "does not"
  match "the player the owner trusts", and the agent told the owner the
  conversion "is the one that's wrong". The comparison does not show
  that. It shows that Infuse's picture of that disc on the iPad looks
  like the probe's plain HDR10 rendering. Whether Infuse plays that disc
  in Dolby Vision at all on an iPad is not known here; if it falls back
  to HDR10 for full-layer discs, its picture says nothing about how the
  conversion should look. What stands without the comparison:
  - the conversion's picture is darker than the plain HDR10 one on the
    iPad (the owner), and the two measured alike on the Mac;
  - by the format, a FEL disc's intended picture is the base and the
    enhancement layer together, and the conversion drops one of them, so
    the conversion cannot be that picture exactly. How far off it is on a
    given title is not measured;
  - for MEL the enhancement layer carries no picture, so the conversion
    loses nothing by the format. That is not confirmed by eye, and the
    comparison asked of the owner for it (The Fifth Element against
    Infuse) had the same weakness and was withdrawn. The owner, of that
    request: "I don't understand what I'm supposed to be judging here."
  A-2's direction is unchanged by this: MEL converts; FEL is rebuilt from
  both layers, after a probe of its own.

- 2026-11-20, **correction to P1's entry above** (review finding iph-22).
  That entry says the overlay emptied `bundle.resources` "so the app
  carries no mpv scripts". It did not: the overlay was `"resources": {}`,
  which Tauri merges into the base map, and the probe's app carried the
  desktop's mpv scripts and `LICENSE.GPL`. The agent had listed the
  bundle's top folder and not looked inside `assets`. The probe app was a
  throwaway on the owner's own iPad and simulator and went nowhere else.
  With `"resources": []` and the generated `assets` folder emptied, a
  fresh simulator build has no Lua and no GPL file. B-2 has the rule and
  the check that follow.
