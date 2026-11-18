# Plan: Vela on tvOS (third of three Apple ports)

## Status

**DRAFT 2026-11-18. Not approved; no code is authorized.** Review:
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
    older pages listed only 5. So the generation decides whether the
    engine converts 8.1 to 5 on that box.
  - Also the LG models, and the audio chain: TV speakers, eARC to a
    receiver, or a soundbar.
  - The ruling of O1 in `ipados-port.md` may already have answered
    this; ask only what is still unknown.
- **V2. The differences that need approval**, one at a time, before
  slice T8. See the list below.
- **V3. The distribution channel** is ruled once, as O2 of
  `ipados-port.md`. tvOS uses the same channel; TestFlight covers Apple
  TV.

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

- **B1 (T0).** Vela's Rust core builds for `aarch64-apple-tvos` and
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
- **B4 (T0).** On tvOS 26 and later, `AVAudioContentSource.passthrough`
  (AVFAudio) sends TrueHD and DTS-HD MA to a receiver as a bitstream. It
  is documented, but whether it does this is not confirmed anywhere
  (2026-11-18). If it does, it is the engine's path for those formats on
  tvOS, so lossless audio keeps Atmos and DTS:X. If not, they are decoded
  to multichannel LPCM, as on the iPad.

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
- **A workspace crate `crates/vela-core`, with no Tauri dependency.** It
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
- **`src-tauri` keeps the Tauri commands, as thin wrappers with unchanged
  names, arguments and results** (the page does not change). It also
  keeps the mpv players, `display.rs`, the embedded hosts, and the
  `AppState` fields that are desktop or mpv only.

Proof:
- Every Rust and Node test passes.
- Both Linux end-to-end suites pass in full with the parity verdict
  PASS, the Windows suite in CI, and the iOS simulator venue.
- `scripts/check-other-platforms.sh` passes.
- A test lists every registered command and checks that it reaches the
  core function it wraps. This guards the wiring, not just the units.

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
- **Swift `Codable` types for the DTOs.** A contract test serializes
  every DTO's Rust fixtures to JSON, and the Swift tests decode them, so
  a field changed on one side fails the other.
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
     names.

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
- **The valid-history copies** (up to three per store on the desktop)
  become one copy per document on tvOS, to fit the budget.
- **The budget:** all documents plus their copies must stay under
  400 KB, measured on save. A save that would exceed it is refused with a
  visible error, never truncated. The slice measures the owner's real
  desktop documents to show the budget is enough.
- **Artwork and library caches** go in Caches, which the system may
  purge; losing them costs only a reload.

### V-4. The engine on tvOS (additions to `apple/VelaEngine`)

- **Display criteria:** for each play,
  `AVDisplayManager.preferredDisplayCriteria` is set from the content's
  frame rate and from a format description that names Dolby Vision
  (`dvh1`/`dvhe`), HDR10 or HLG. It is cleared when the play ends.
  - The owner's Apple TV settings must allow it ("Match Content: Dynamic
    Range" and "Frame Rate"). The owner's checklist says so.
  - The criteria are set only if the box's own format choice leaves that
    switch to the content.
- **Profile 8.1 on generations that take only profile 5:** converted to
  5 by the engine, if P2 or T0 shows it is needed (V1).
- **Audio:** B4 decides between bitstreaming TrueHD and DTS-HD MA and
  decoding them to LPCM. E-AC-3 with Atmos passes to Apple, which sends
  Dolby MAT.
- **The Siri Remote:** play/pause, scrubbing on the clickpad, and
  ±10 s and ±60 s jumps (matching the desktop's arrows). The skip button
  is a focusable control; the track lists, cropping and volume are in the
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
  `link_poll` waits for the server. Jellyfin and Emby take an address and
  a user name and password through tvOS's keyboard, or a token, as
  `connect_jellyfin` and `connect_jellyfin_token` do today.
- **Durable-state faults and recovery:** the same choices the page
  offers (`get_durable_state_status`, `recover_invalid_file`,
  `rollback_invalid_file`).

### V-6. Keeping two kinds of screens in step

From the day this app lands, every user-visible feature must exist in
both the Svelte page and the tvOS app (the parity ruling). Two guards:
- the parity table below is the canonical list, and a feature is added
  to it in the commit that adds it to either side;
- a Node test (`tests/tvos-coverage.test.mjs`) checks that every
  portable command registered in `src-tauri/src/lib.rs` is called by the
  tvOS app's sources, or is listed in the table as desktop-only with the
  owner's approval.

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

Each slice is one or more commits with tests and red-proofs, a
`codereview codex` hunt and a push.

- **T0, probes (throwaway; results in the Log).**
  - The Rust core, as it stands, built for the tvOS simulator and an
    owner's Apple TV, with TLS to the mock Plex (B1).
  - Keychain and `UserDefaults` persistence across a restart, a reboot
    and a storage-pressure purge, if one can be caused (B2).
  - The passthrough API on the owner's receiver (B4).
  - The focus engine and remote events in a minimal SwiftUI player.
- **T1, the core split (V-1).** In steps, each with every suite green.
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
  - Docs, the release notes, the owner's checklist, and the
    repo-guidance rule of V-6.

## Risks

- **Two kinds of screens can drift.** V-6's guards catch a missing
  command, not a missing behaviour. Each T slice's review checks the
  behaviour against the page's.
- **The `UserDefaults` budget.** A library with very many playlists
  could outgrow 400 KB. The refusal is visible and fails closed. Whether
  that ever happens to the owner is measured in T3.
- **Display criteria depend on the owner's Apple TV settings.** The
  owner's checklist names them, and the player shows when the box
  refused a switch, if the API reports it.

## Verification before claiming completion

- The repo-guidance Verification set.
- Every desktop and iPad suite, after the core split.
- The tvOS CI job, and its UI tests against the mock servers.
- The owner's by-eye checks on every Apple TV generation: the LG's Dolby
  Vision and HDR indicators, the frame rate, and the receiver's audio
  format. Each is recorded with what was seen.

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
  - passthrough: Apple's AVFAudio documentation, as reported, unconfirmed.
