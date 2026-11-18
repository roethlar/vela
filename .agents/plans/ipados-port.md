# Plan: Vela on iPadOS (first of three Apple ports)

## Status

**DRAFT 2026-11-18. Not approved; no code is authorized.** Test specifics
written here before the code exists are not binding: tests are designed
with the code, in the slice that writes it (`.agents/decisions.md`
2026-11-18, "No tests are designed for code that has not been
written"). Review:
pending (`codereview codex gpt-6.2-astra xhigh`, owner's request of
2026-11-18). This is the first of three plans, in the owner's order:
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
  HERE"): every slice that lands repo code gets a `codereview codex`
  hunt. Every commit is pushed to `origin` and `github`
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
    FFmpeg demuxes into compressed `CMSampleBuffer`s whose format
    description carries the Dolby Vision configuration (`dvcC`/`dvvC`).
    They go to `AVSampleBufferDisplayLayer` and
    `AVSampleBufferAudioRenderer`, with one
    `AVSampleBufferRenderSynchronizer` as the clock for both. Only this
    path shows video Vela decodes itself and plays multichannel LPCM, so
    every codec plays locally (owner, 2026-11-18: no server transcode
    unless the user chooses one). Infuse is built the same way, by the
    libraries it links.
  - **P2 decides one thing** (owner, 2026-11-18: "add the player decision
    to the list of things the apple tv probe will decide"): whether Dolby
    Vision engages through the sample-buffer path, for each profile, on
    the Apple TV and the iPad Pro. P2 plays mixed combinations: Dolby
    Vision with TrueHD, software-decoded video with E-AC-3, and HDR10
    with DTS-HD MA.
  - **If a Dolby Vision profile does not engage that way**, P2 tries a
    throwaway remux of it to AVPlayer, to see whether that path engages
    it. A production remux path would then be designed for those
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
    are built for its own origin.
  - It has a connection timeout and a stall timeout that resets while
    bytes arrive, with no deadline on the whole response. It is cancelled
    when the play ends.
  - Each playlist's references are resolved against the URL it was
    finally served from, after redirects, and cleaned, before FFmpeg
    sees them.
  - A read returns exactly the bytes asked for: a `206` whose
    `Content-Range` matches, or, when the server sends the whole
    resource, the interval cut from it as it streams. Anything else is a
    read error.
  - FFmpeg's HLS demuxer runs with its HTTP persistence off, since it
    expects its own HTTP context when that is on.

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
- **Video:** compressed HEVC, H.264, AV1 (where the device has a
  hardware decoder) and VP9 go to Apple's pipeline as compressed sample
  buffers (A2). Only a Dolby Vision profile and device named in a
  reviewed amendment may take another path. Codecs Apple does not decode, such as MPEG-2, VC-1, H.264
  10-bit, and AV1 on devices without a hardware decoder:
  FFmpeg decodes them on the device, in software, into `CVPixelBuffer`s
  (dav1d for AV1). They are shown by the sample-buffer display layer on
  the same clock as the audio, whatever P2 decides for Dolby Vision.
  They never go to a server transcode unless the user picked one.
- **HDR and Dolby Vision:**
  - The engine tags colour primaries, transfer, matrix, mastering
    display and content light level.
  - For Dolby Vision it carries the configuration record and the RPU of
    each frame. Profile 7 (MEL or FEL) becomes 8.1 live through
    libdovi's conversion mode 2. Profile 10 is AV1, whose Dolby Vision
    metadata travels in metadata OBUs, so the engine carries those and an
    AV1 Dolby Vision format description (HLS names it `dav1`). Which
    subtype Apple's pipeline needs is found in P2.
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
  - Track switching happens without restarting the play.
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
- **Cropping:** off, manual or auto, rebuilt for the engine. The engine
  detects black bars on sampled frames.
  - On the software path, the frames are the decoded ones.
  - On the hardware sample-buffer path, the display layer gives no frame
    access. So the engine decodes, with a `VTDecompressionSession` of its
    own used only for detection, the frames at the presentation times it
    measures, starting from the random-access point before them. It
    measures the picture being shown, not the last keyframe, which in a
    long group of pictures can have different bars.
  - With the loopback, the frames come from `AVPlayerItemVideoOutput`.

  It crops by the layer's geometry, never by decoding the shown picture
  again. P2 covers frame access on the chosen renderer, and A9 tests a
  letterboxed, hardware-decoded clip on the minimum OS. It reports the same crop
  states `vela-crop.lua` reports today.
- **Health:** dropped frames and buffered seconds feed Automatic quality
  as `PlayerEvent`s.
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
    3. Coming back after that, the resume point is settled first: the
       newer of Vela's saved position and the server's current progress
       and watched state for the item. The player view shows the play
       paused there, and playing again starts a fresh session
       there.

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
    provides is enabled, and so is every external decoder library FFmpeg
    can use whose licence the licensing rule of 2026-11-12 permits: dav1d
    (AV1) and uavs3d (AVS3), both BSD and both pinned like the rest. A
    codec whose only decoder that rule excludes is listed in the Log,
    and is not claimed. swresample and swscale are built too. Any codec Apple's
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
  edited (repo-guidance: `src-tauri/gen/` is generated).
  - Device family: iPad only in this plan.
  - Bundle: the iOS app's bundle carries no mpv scripts, which are
    desktop resources (`tauri.conf.json` `bundle.resources`). A tracked
    `tauri.ios.conf.json` overlay leaves them out (decisions
    2026-11-12, licensing, and 2026-11-18, the App Store).
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
    the desktop).
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
    certificate is used.

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
16. Dolby Vision profiles 5, 8.1, 8.4, 7 (converted) and 10 (where the
    device decodes AV1) shown as Dolby Vision, each on the minimum OS.
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
`codereview codex` hunt. The Log
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
    uses.
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
- **A5, engine core (A-2).** Input, video through sample buffers, audio,
  events and commands, with XCTests on generated media.
- **A6, the plugin and `PlayerKind::Apple` (B-2, second half).** Rows 1–7
  and 12 in the simulator venue (B-5's harness work comes first).
- **A7, HDR and Dolby Vision (rows 15–16).** By eye on the owner's iPad,
  with the integrated engine in Vela's app. Apple TV is proven by
  `tvos-port.md` (T6, T7) with the integrated engine there. P2's
  throwaway build only chooses the architecture and counts for neither.
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

## Risks

- **P2 may find that neither path gives real Dolby Vision from MKV on
  the owner's Apple TVs.** That would contradict what shipping apps
  report (Infuse's Dolby Vision for profiles 5 and 8; AetherEngine's live
  7-to-8.1 conversion). Then the plan stops and the finding goes to the
  owner before any engine code lands.
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
