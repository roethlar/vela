# Plan: Vela on tvOS (third of three Apple ports)

## Status

**DRAFT 2026-11-18. Not approved; no code is authorized.** Test specifics
written here before the code exists are not binding: tests are designed
with the code, in the slice that writes it (`.agents/decisions.md`
2026-11-18, "No tests are designed for code that has not been
written"). Review:
pending (`codereview codex gpt-6.2-astra xhigh`, owner's request of
2026-11-18).

This plan comes after `.agents/plans/ipados-port.md` and
`.agents/plans/ios-port.md` (`.agents/decisions.md` 2026-11-18). It
depends on the iPadOS plan's Part A:
- the player seam (A-1);
- the Apple engine (A-2);
- the media libraries (A-3);
- probe P2, which already runs the engine on the owner's Apple TVs.

## Owner rulings that bind this plan

- Everything listed in `ipados-port.md`, "Owner rulings that bind this
  plan", applies here unchanged: the Apple engine, no label, parity, the
  proper way, HDR, licensing, and the working rules.
- Apple TV shows real Dolby Vision, owner verbatim (2026-11-18): "tv will
  need dolby vision".
- The owner's test bed, verbatim (2026-11-18): "several apple tv 4ks all
  connected to LG OLEDs with hdr and dv support".

## Why tvOS is a second app and not a port

Apple ships no web view on tvOS. WKWebView and UIWebView are absent from
the SDK, and using them is private API, which App Review rejects. Tauri
does not target tvOS. So the Svelte page cannot run on Apple TV.
- **Its screens:** a native SwiftUI app.
- **What it shares with every other platform:** Vela's Rust core,
  through a Swift interface, and the Apple engine (A-2).

## Scope

When this plan is done, Vela installs on the owner's Apple TV 4Ks and
works as it does on the desktop:
- adding servers: Plex by PIN and QR code, Jellyfin and Emby;
- browsing, search, item and season pages, people, playlists, watched
  state, Continue Playing and settings;
- the player, with every parity row of `ipados-port.md`;
- the TV switching into Dolby Vision (profiles 5, 8.1 and 7 converted),
  HDR10 or HLG, at the content's frame rate.

Every user-visible feature of the Svelte page has a tvOS equivalent,
listed in the parity table below, unless the owner has approved its
absence.

## Non-goals (of this plan)

- Apple TV HD. It has no HDR, so nothing is written for it (the HDR rule
  in `.agents/repo-guidance.md`). The target is Apple TV 4K, every
  generation.
- tvOS-only additions with no desktop equivalent: the Top Shelf
  extension, Siri search, multiple user profiles, and screen-saver hooks.
  These are additions for a later plan.
- Any change to the desktop or iPad app beyond the core split, which
  must leave both behaving exactly as before.

## Owner inputs needed (one at a time, in this order)

- **V1. Each Apple TV's generation (a fact).** Shown in Settings,
  General, About, Model: A1842 is the first generation (2017), A2169 the
  second (2021), A2737 or A2843 the third (2022), or a later model.
  - Apple's current specifications list Dolby Vision profiles 5 and 8.1;
    older pages listed only 5. So the generation decides whether 8.1
    plays natively on that box (V-4).
  - Also the LG models, and the audio chain: TV speakers, eARC to a
    receiver, or a soundbar.
  - The ruling of O1 in `ipados-port.md` may already have answered
    this; ask only what is still unknown.
- **V2. The differences that need approval**, one at a time, before
  slice T8. See the list below.
- **V3. The distribution channel** is ruled once, as O2 of
  `ipados-port.md`. tvOS uses the same channel; TestFlight covers Apple
  TV.
- **V4. Where playlists live on tvOS (a decision).** Apple TV keeps only
  `UserDefaults` (and the Keychain) persistent, and terminates an app
  whose defaults reach 1 MB. The two choices:
  - **`UserDefaults`:** works with no account or network, but bounds
    playlists. That limit needs the owner's approval, and a save that
    would cross it is refused visibly, well before the 1 MB termination.
  - **The CloudKit private database:** no such bound, but it needs the
    owner's iCloud account signed in on each Apple TV. The plan must then
    also say what happens to edits already made while the account or the
    network is missing.

  T0 measures the owner's real documents to inform the choice.

## Differences that need the owner's approval (none approved yet)

These are the same four as `ipados-port.md` (external mpv and its
installer, the user's mpv options, window modes and screens, "Exit
Vela"), for the same reasons, plus none new. Settings that are portable
are rebuilt in SwiftUI, not dropped:
- skip policies, quality, Automatic and source policy;
- server certificates and Continue Playing;
- cropping;
- the player's volume and mute, as the app's own gain, like mpv's
  software volume.

## Assumptions (each proven or overturned by a probe before product code)

- **B1 (T0, then T1).** Vela's Rust core, once split from Tauri (T1),
  builds for `aarch64-apple-tvos` and
  `aarch64-apple-tvos-sim` and works there: tokio, reqwest with
  native-tls (Security.framework), and `if-addrs`. These targets have
  been Rust's tier 2 since 1.95 (2026-04-16; rust-lang/rust PR 152021),
  installed with rustup.
- **B2 (T0).** The Keychain on tvOS keeps items across app restarts,
  reboots and storage pressure. `UserDefaults` keeps data but is limited.
  Apple's tvOS guide says: "your app can only access 500 KB of persistent
  storage that is local to the device (using the NSUserDefaults class).
  Outside of this limited local storage, all other data must be purgeable
  by the operating system when space is low". The system warns at 512 kB
  and terminates the app at 1 MB.
- **B3 (P2 of the iPadOS plan, repeated on every generation).** The
  engine puts each of the owner's LG OLEDs into Dolby Vision mode, with
  display criteria built from a format description whose codec subtype
  is `dvh1` or `dvhe` (A3 of the iPadOS plan).
- **B4 (T0).** TrueHD, DTS and DTS-HD MA are decoded to multichannel
  LPCM on tvOS, as on the iPad. That is the baseline: lossless audio
  reaches the receiver intact, without Atmos or DTS:X objects.
  - A bitstream path is used only if tvOS has a real output-routing API
    for it. `AVAudioContentSource.passthrough` was reported as one, but
    Apple's Apple Positional Audio Codec specification ("Setting the
    Content Source type") describes it as a content classification given
    to an encoder, so it is not taken as that API.
  - A bitstream path is adopted only with receiver evidence that names
    the codec: the receiver shows TrueHD or DTS-HD MA, not PCM or Dolby
    MAT. The receiver's Atmos light alone is not evidence, because Apple
    notes that a continuous Dolby MAT connection can make older
    receivers show Atmos for other content.

## Design

### V-1. The core split (desktop and iPad code; no behaviour change)

Today the core is not separable from the Tauri app (explorer, 2026-11-18;
find by name):
- `source/` needs `playback` types.
- `playback.rs` needs `player::*` and `commands::PlaybackAdvance`.
- `config.rs` needs `player::PlayerKind` and
  `playback::PlaybackWindowState`.
- `selection.rs` needs `display::HdrState`.
- `config`, `connections`, `source` and `artwork` call
  `durable::ensure_commands_ready`.
- `durable.rs` keeps a static `APP_HANDLE` to emit `durable-state-fault`.
- `commands.rs` holds about 160 functions that are not commands and 61
  types: merging, the watched-state fan-out, the playlist cursor, runs
  and source choices.

The split:
- **A workspace crate `src-tauri/crates/vela-core`, with no Tauri
  dependency.** `src-tauri/Cargo.toml` becomes the workspace root, so the
  lockfile and the rule that Cargo runs from `src-tauri/` stay as they
  are. CI and the repo-guidance Verification commands change in the same
  commit:
  - `cargo test --workspace --locked` and `cargo clippy --workspace
    --all-targets`, because a plain `cargo test` from the Tauri package
    would build the core without running its tests;
  - `cargo audit` reads the workspace's one lockfile.

  An injected failing `vela-core` test proves CI runs those tests, and is
  then taken out.

  The crate
  holds the servers (`source/`, `plex_library.rs`, `plex_api.rs`), the
  stores (`storage.rs`, `durable.rs`, `config.rs`, `connections.rs`,
  `playlists.rs`, `recents.rs`), `selection.rs`, `automatic.rs` and
  `locality.rs`. It also takes the player-neutral trackers and seam of
  `ipados-port.md` A-1, and the non-command logic of `commands.rs`.
- **A `Host` trait the core calls for what differs per shell:** sending
  an event, the display's HDR capability, opening a URL, and the durable
  backend (V-3). Desktop-only types such as `PlaybackWindowState` stay in
  the Tauri crate. The core keeps an opaque per-shell extension where
  config must round-trip them, because the compatibility rails forbid
  dropping fields (`.agents/repo-guidance.md`).
- **The two playback workers started in `lib.rs` `.setup` move into the
  core**, and start and stop with it on every shell:
  - the Automatic quality step-down loop (`commands::apply_step_down`);
  - the playback sequence dispatcher, which admits a clean end, advances
    playlists, sends `continue-playing`, marks the item watched on the
    servers and sends `playback-ended`.

  A test drives a clean end of file and a step-down through the native
  entry points (`vela_core_start`, `vela_core_player_event`), and sees
  the playlist advance, the watched mark and the replacement play.
- **`src-tauri` keeps the Tauri commands, as thin wrappers with unchanged
  names, arguments and results** (the page does not change). It also
  keeps the mpv players, `display.rs`, the embedded hosts, and the
  `AppState` fields that are desktop or mpv only.

Proof:
- Every Rust and Node test passes.
- Both Linux end-to-end suites pass in full with the parity verdict
  PASS, the Windows suite in CI, and the iOS simulator venue.
- `scripts/check-other-platforms.sh` passes.
- Commands are classified in one table:
  - **shared commands**, wrappers over the core, each checked by a test
    that it reaches the core function it wraps (this guards the wiring,
    not just the units);
  - **shell operations**, which belong to Tauri or the desktop and are
    never wrapped for their own sake: `exit_vela`, `player_window_ready`,
    `player_set_rect`, `player_fullscreen` and the mpv commands;
  - **events**.

  Features that live only in the page, such as the theme
  (`Settings.svelte`), are listed apart. Each is checked through the
  interface that owns it.

### V-2. The core's Swift interface

One contract for both kinds of screens. The tvOS app calls the same
commands, by the same names, with the same JSON arguments and results as
the page:
- The core gets a small C ABI, written with cbindgen. Its calls are:
  - `vela_core_start` (the storage backend and an event callback);
  - `vela_core_call` (a command name and its JSON arguments, answered
    through a callback with JSON);
  - `vela_core_player_event` (the engine's `PlayerEvent`s);
  - `vela_core_stop`.
- **Plex artwork.** It reaches the page as a `vela-artwork:` marker that
  Tauri's custom protocol resolves (`artwork.rs`, registered in
  `lib.rs`). tvOS has no such protocol. So a further call,
  `vela_core_artwork` (a marker, answered with the image bytes), serves
  SwiftUI's image loader through the same bounded, authenticated
  resolver, and the credentials stay in Rust. A test renders a Plex
  poster from a cold cache against the mock Plex.
- **The other direction, core to engine.** A play can start in the core,
  not only from a button: a playlist advance, Continue Playing, or an
  Automatic quality step-down. So the host callback given to
  `vela_core_start` also receives:
  - `play`: a session id and a `PlayRequest`;
  - `command`: a session id and a `PlayerCommand`. Stopping is
    `PlayerCommand::Stop` through it, so a stop meant for an earlier
    session cannot end a later one.

  The Swift side answers each with success or failure. A command for a
  session that is no longer current is refused, as `control.rs` refuses
  one today. Tests: a play the core starts (a playlist advance, a
  step-down) reaches the engine, and a stale session's command is
  refused.
- **Swift `Codable` types for the DTOs**, with contract tests in both
  directions:
  - every DTO's Rust fixtures, with every optional field populated, are
    decoded in Swift, and each value is checked, not just the decode;
  - Swift-encoded arguments for every command go through the real
    dispatcher, covering optional fields, tagged results and errors.

  A field renamed on one side then fails instead of decoding as nil.
- **Why one JSON contract rather than a generated typed binding
  (UniFFI):** the page already speaks exactly this contract. Keeping it
  means a feature added to the core is available to both kinds of
  screens the same way, which helps parity.

### V-3. Storage on tvOS

Desktop and iPad keep the files of `durable.rs` unchanged. On tvOS the
core uses a second backend behind the same durable rules (fail closed,
never replace an unreadable document with defaults, explicit recovery):
- **Connection secrets** (tokens) go in the Keychain as immutable
  versions. A token that changes gets a new item with a new id, and each
  saved connections document names the ids it uses. The connection
  records without secrets go in `UserDefaults`. The save order:
  1. write any new Keychain items;
  2. write the connections document, which is the commit point;
  3. delete the items that no current or retained history document
     names, but only at the next launch. `UserDefaults` reaches disk
     asynchronously, so step 2's document may not be on disk when the
     app ends. At launch the backend reads back every current and
     history document, and deletes only the items none of them names.

  An interruption at any step leaves at most some unreferenced items,
  which are removed at the next start. So a rollback to any retained
  version finds its own secrets. Tests: an interruption at each step, a
  removal, and a token rotation followed by a rollback to each retained
  version, each through a relaunch.
- **Credentials inside other documents.** Item snapshots saved in recents
  (`config.json`, `recents.rs`) and in playlists (`playlists.rs`) carry
  artwork URLs. For Jellyfin and Emby those URLs include
  `api_key=<token>` (`source/jellyfin.rs`). The tvOS backend saves every
  such URL without its credential, as the source id with the path and the
  rest of the query. The core adds the credential back when it hands the
  URL out. A test saves every document and every history copy with a
  synthetic token, and checks the token appears in none of them.
- **`config.json` and `playlists.json`** are stored as the same JSON
  documents in `UserDefaults`, one key each. A key's write is atomic.
  Only one process exists, so there is no cross-process lock.
- **The valid-history copies** are kept as on the desktop: the three
  newest distinct valid versions of `config.json` and of
  `connections.json` (`.agents/decisions.md` 2026-07-23). Each is a
  version of its own in the backend, with the same dated recovery
  choices. A test restores each of the three, and checks that the
  damaged bytes are preserved.
- **Space.** Besides the Keychain, `UserDefaults` is the only persistent
  store on tvOS (Apple: 500 KB; a warning at 512 kB and termination at
  1 MB). tvOS must not get a limit the desktop does not have. So:
  - Playlists persist each entry's credential-free identity, its display
    context (title, episode label, source name) and its backing set
    (every server copy of a merged entry). The desktop keeps these so an
    unavailable entry stays identifiable and playback can pick another
    copy (`playlists.rs`). Only artwork and metadata that can be fetched
    again live in Caches. Recents keep Vela's own position, session and
    recency fields durably (`recents.rs`), because a server may hold an
    older position or none. Only their fetchable metadata may move to
    Caches.
  - Playlists still grow with every entry, so where they live durably is
    the owner's decision, V4. Until it is ruled, T3 does not land.
  - No save is refused for size unless the owner approves that limit. A
    store that is unexpectedly full fails closed, visibly, and is never
    truncated.
- **Artwork and library caches** go in Caches, which the system may
  purge; losing them costs only a reload.

### V-4. The engine on tvOS (additions to `apple/VelaEngine`)

- **Display criteria:** for each play,
  `AVDisplayManager.preferredDisplayCriteria` is set from the content's
  frame rate and from a format description that names Dolby Vision
  (`dvh1`/`dvhe` for HEVC; for profile 10, the AV1 Dolby Vision subtype P2
  identifies, on boxes that decode AV1), HDR10 or HLG. It is cleared when the play ends.
  - The owner's Apple TV settings must allow it ("Match Content: Dynamic
    Range" and "Frame Rate"). The owner's checklist says so.
  - The criteria are set only if the box's own format choice leaves that
    switch to the content.
- **Profile 8.1 on generations that take only profile 5.** Native 8.1
  is probed first, on every generation (V1).
  - Converting 8.1 to 5 is not a metadata change. Profile 5's base
    layer is encoded differently (IPT-PQ-c2), while 8.1's is
    HDR10-compatible, and dovi_tool's conversion modes convert metadata
    only. Relabelling, or converting only the RPU, can give a stream the
    box accepts with visibly wrong colours. The same goes for profile 7
    first converted to 8.1.
  - So a box that takes only profile 5 gets Dolby Vision from 8.1 only
    if a probe shows a real-time conversion of the picture and the
    metadata together, checked against a reference.
  - If no such conversion can be shown, that finding goes to the owner
    before the box is claimed to support those files.
- **Audio:** TrueHD, DTS and DTS-HD MA are decoded to LPCM, unless B4
  finds a real bitstream path. E-AC-3 with Atmos passes to Apple, which sends
  Dolby MAT.
- **The Siri Remote:** play/pause, scrubbing on the clickpad, and
  ±5 s and ±60 s jumps, as the desktop's arrows do (`playerControls.ts`;
  `tests/player-controls.test.mjs`). A test checks the commands that
  remote input sends. The skip button is a focusable control; the track lists, cropping and volume are in the
  player's menu. Now Playing information is kept up to date.

### V-5. The SwiftUI app (`apple/tvos/`)

- **The project:** generated from a tracked XcodeGen spec,
  `apple/tvos/project.yml`, with XcodeGen pinned by version and checksum,
  the tool Tauri itself uses for iOS. The `.xcodeproj` is generated and
  git-ignored. The app links `apple/VelaEngine` and an `.xcframework` of
  `vela-core` built for `tvos-arm64` and `tvos-arm64-simulator`.
- **Screens**, each a row of the parity table below. Navigation uses the
  focus engine: top-level tabs for Home, the libraries, Search,
  Playlists and Settings. Actions that are right-click menus on the page
  are long-press menus here.
- **Signing in:** Plex uses the existing PIN flow. `link_begin` already
  returns the code and a QR image of the `plex.tv/link` address, and
  `link_poll` waits for the account. When the account has several
  reachable servers, `link_poll` answers `chooseServer` and holds the
  credential in an expiring session; the source is saved only by
  `link_select_server`. So the app shows a focusable server chooser for
  that answer. T4 tests a two-server account (mock plex.tv) through
  choosing and saving. Jellyfin and Emby take an address and
  a user name and password through tvOS's keyboard, or a token, as
  `connect_jellyfin` and `connect_jellyfin_token` do today.
- **Durable-state faults and recovery:** the same choices the page
  offers: the status (`get_durable_state_status`), "Try again"
  (`retry_durable_state`, which resumes an interrupted recovery and
  reloads the registry), fresh recovery (`recover_invalid_file`) and the
  dated versions (`rollback_invalid_file`). T4 proves "Try again" on a
  store that was unavailable and comes back: the app reopens with its
  healthy data untouched.

### V-6. Keeping two kinds of screens in step

From the day this app lands, every user-visible feature must exist in
both the Svelte page and the tvOS app (the parity ruling). Two guards:
- the parity table below is the canonical list, and a feature is added
  to it in the commit that adds it to either side;
- a Node test (`tests/tvos-coverage.test.mjs`) checks that every shared
  command of V-1's table is called by the tvOS app's sources. Shell
  operations are exempt; each is either mapped to a tvOS equivalent or
  approved as absent. The test also checks that every page-only feature
  in the parity table has a tvOS row.

When this plan lands, the rule also goes into
`.agents/repo-guidance.md`.

## Parity table (screens and features; filled in by slice T4)

The first draft has one row per user-visible area of the page:
- Home: Continue Watching and the hubs.
- Library browsing: sort and direction, merged views and their
  overrides.
- Item page: versions, quality, play, play from the beginning, watched,
  add to playlist, info.
- Season page and episode rows.
- People.
- Search.
- Vela playlists: create, rename, delete, add, remove, reorder, play.
- Server playlists.
- Settings, tab by tab.
- Durable-state recovery.
- The player, with each control of the page's player view.

Slice T4 turns each area into rows, with the command or commands each
uses.

## Slices

Each slice is one or more commits, each with its tests and red-proofs,
each pushed to `origin` and `github` as it lands
(`.agents/push-policy.md`). Each slice that lands repo code gets a
`codereview codex` hunt.

- **T0, probes (throwaway; results in the Log).**
  - A throwaway crate with the core's pinned dependencies (tokio,
    reqwest with native-tls, `if-addrs`, serde and the rest of the core's
    list in `Cargo.lock`), built and run on the tvOS simulator and on an
    owner's Apple TV, with TLS to the mock Plex (B1). The real crate
    carries Tauri's runtime and build dependencies, which do not build for
    tvOS, so it cannot be the probe.
    The mock Plex (`tests/e2e/mockplex.mjs`) binds `127.0.0.1`,
    advertises that address, and its certificate names it. On an Apple
    TV that address is the TV itself. For this probe the mock binds the
    Mac's LAN address and advertises it, with a certificate for that name
    from a test CA installed on the Apple TV. The probe checks that the
    box accepts that certificate and refuses one from an untrusted CA.
  - Keychain and `UserDefaults` persistence across a restart, a reboot
    and a storage-pressure purge, if one can be caused (B2).
  - Whether tvOS has a bitstream output path at all, with receiver
    evidence that names the codec (B4).
  - The focus engine and remote events in a minimal SwiftUI player.
- **T1, the core split (V-1).** In steps, each with every suite green.
  Its exit condition includes the extracted `vela-core` building for
  `aarch64-apple-tvos` and `aarch64-apple-tvos-sim` in CI.
- **T2, the Swift interface and its contract tests (V-2).**
- **T3, the tvOS storage backend (V-3)**, with the budget test and
  red-proofs for refusal and recovery.
- **T4, the app's shell:** signing in, sources, status, durable recovery,
  and the full parity table.
- **T5, the browsing screens.**
- **T6, the player:** the engine, the remote, display criteria (V-4) and
  rows 1–12 of the parity list.
- **T7, the audio path B4 chose**, and the tvOS by-eye rows 15–17 on
  every generation the owner has.
- **T8, settings and the remaining rows**, after V2's rulings.
- **T9, CI, signing and distribution.**
  - CI: a macOS job builds `vela-core` for tvOS, runs the Swift tests and
    the UI tests in the tvOS simulator against the mock servers, and
    builds the app.
  - The release workflow gets the TestFlight job if O2 is ruled that way.
  - Privacy manifests, as in `ipados-port.md` B-4. The tvOS target's
    tracked manifest covers the app binary and the Rust core, with
    `UserDefaults` (app-only, reason CA92.1) and file timestamps (`stat`,
    through `std::fs::metadata`). Each shipped framework carries its own
    manifest, and packaging checks each one in the archive.
  - Docs, the release notes, the owner's checklist, and the
    repo-guidance rule of V-6.

## Risks

- **Two kinds of screens can drift.** V-6's guards catch a missing
  command, not a missing behaviour. Each T slice's review checks the
  behaviour against the page's.
- **The `UserDefaults` space.** If T0 shows that playlist references
  could outgrow it, playlists need the CloudKit private database. That
  depends on the owner's iCloud account, and the owner is told before it
  lands (V-3).
- **Display criteria depend on the owner's Apple TV settings.** The
  owner's checklist names them, and the player shows when the box
  refused a switch, if the API reports it.

## Verification before claiming completion

- The repo-guidance Verification set.
- Every desktop and iPad suite, after the core split.
- The tvOS CI job, and its UI tests against the mock servers.
- The owner's by-eye checks on every Apple TV generation, each recorded
  with what was seen:
  - the LG's Dolby Vision and HDR indicators, and the frame rate. The
    Apple TV starts in SDR, with Match Content's Dynamic Range on, so a
    box left in Dolby Vision mode cannot pass for a switch. SDR and HDR10
    clips are played as controls.
  - that the Dolby Vision metadata reaches the picture, not only that
    the mode switched. The reference is a clip made of the same base
    layer with two different RPUs, chosen so their scenes look visibly
    different (dovi_tool, test-only), plus a control with the RPU
    stripped. The owner confirms each looks as its metadata says.
  - the receiver's audio format.

  These run with the integrated engine in Vela's app (T6, T7). Probe
  P2's throwaway build is not counted as that evidence.

## Log

- 2026-11-18: drafted after the owner's rulings of that day. Facts about
  the code came from read-only explorers' inventories of the backend's
  Tauri coupling, its sign-in flows and its stores. The tvOS facts are
  sourced:
  - storage: Apple's App Programming Guide for tvOS, as quoted by Apple's
    forums;
  - display criteria: KSPlayer issue 633 and Anemone PR 21;
  - the Rust targets: rust-lang/rust PR 152021 and the 1.95.0 release
    notes;
  - passthrough: Apple's AVFAudio documentation, as reported. Per review
    finding tvp-13, Apple's Apple Positional Audio Codec specification
    describes it as an encoder's content type.
