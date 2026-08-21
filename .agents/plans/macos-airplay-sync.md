# Plan: Prefer AVFoundation audio for macOS mpv playback

## Status

**Implementation completed for Vela 1.0.61 on 2026-08-21. Automated
verification and native DMG construction pass; packaged physical playback
acceptance is in progress.**

The diagnostics replaced the withdrawn AirPlay delay/route-monitor design with
one proven launch policy: on macOS, prefer mpv's AVFoundation audio output and
retain CoreAudio as initialization fallback. AVFoundation fixed the affected
live stream and followed system output changes in both directions without Vela
intervention.

The owner settled own-config precedence on 2026-08-20: enabling **Use my own
mpv config** disables Vela's AO preference. When
`mpv_use_own_config=true`, Vela omits its AO argument and the expert's
`mpv.conf` owns audio-output selection. Advanced mpv arguments remain the
explicit command-line override. All design decisions are settled;
implementation still requires a separate explicit go.

No Claude session or further plan review is authorized by this plan update.

## Goal and acceptance contract

Ordinary macOS Vela playback must launch mpv with AVFoundation as the preferred
audio output and CoreAudio as fallback. mpv and macOS must own live output-route
changes; Vela must not identify AirPlay, estimate endpoint latency, or modify
`audio-delay`.

Acceptance requires independently proved behavior:

1. A new default macOS play launches with
   `--ao=avfoundation,coreaudio` before user Advanced mpv arguments.
2. The affected network MKV/E-AC-3 5.1 stream starts physically synchronized on
   AirPlay; `current-ao` reports `avfoundation` without an IPC correction.
3. The same live play follows AirPlay -> built-in -> AirPlay automatically and
   remains physically synchronized.
4. A built-in-output launch remains physically synchronized.
5. Pause/resume and forward/backward seek remain responsive and synchronized on
   AVFoundation; queued samples do not leak across a seek.
6. Mute and volume remain responsive. Evidence must not rely on the two invalid
   manual timing attempts recorded below.
7. mpv Advanced arguments retain last-option-wins ownership: a later user
   `--ao=...` overrides Vela's default.
8. With own config enabled, Vela omits its AO preference and the user's
   `mpv.conf` owns audio-output selection.
9. No non-macOS launch argument, config schema, frontend control, IPC thread, or
   playback behavior changes.
10. If AVFoundation cannot initialize, mpv tries CoreAudio through the ordered
    AO list instead of failing playback outright.

## Established evidence

### Vela and mpv baseline

- Vela intentionally launches system mpv out of process from
  `src-tauri/src/playback.rs`; that product boundary remains settled.
- Unless the owner enables the own-config option, Vela passes `--no-config`.
  It appends one-line Advanced mpv arguments after Vela defaults, so mpv's
  last-option-wins behavior gives the user final command-line ownership.
- The affected host runs `/opt/homebrew/bin/mpv` 0.41.0. Its compiled automatic
  AO order begins CoreAudio then AVFoundation.
- Installed Vela 1.0.60 had `mpv_extra_args=null`,
  `mpv_use_own_config=false`, and `mpv_autocrop=null` during reproduction.

### Controlled playback evidence

- A credential-free 12-second 1280x720 60 fps stereo AAC flash/click fixture
  has SHA-256
  `f5e5b5641234a3876ad535bb41e8970f005730d194039d1cef40c7716262877e`.
- On AirPlay, Vela-equivalent standalone CoreAudio and AVFoundation runs both
  stayed physically synchronized after startup. CoreAudio swallowed the first
  click; AVFoundation swallowed roughly half of it. This disproved a general
  fixed AirPlay offset.
- An earlier mono fixture made CoreAudio fail channel-layout initialization
  with macOS `paramErr (-50)` and produced no audio. It was invalid for sync
  comparison but independently reproduced the condition behind Plezy's
  AVFoundation fallback.

### Affected real stream

- Installed Vela reproduced roughly two seconds of physically late audio on
  AirPlay while mpv reported `current-ao=coreaudio`, `audio-device=auto`,
  `audio-delay=0`, and `avsync=0.000006` seconds.
- The stream was network-backed MKV: H.264/VideoToolbox video at 23.976025 fps
  and 48 kHz E-AC-3 `5.1(side)` decoded/downmixed to 48 kHz stereo.
- An owner-approved live `ao=avfoundation` IPC write changed that exact process
  to AVFoundation without explicit reload, seek, or relaunch. mpv reported
  `avsync=0.000359` seconds and the owner confirmed physical sync became
  correct.
- Restarting the item created a fresh CoreAudio process and reproduced the
  offset. Reapplying AVFoundation restored sync, proving the effect was the AO
  selection rather than unrelated state.
- The second AVFoundation process followed AirPlay -> Mac mini Speakers ->
  AirPlay with `audio-device=auto`, remained on AVFoundation, and stayed
  physically synchronized. IPC reported `avsync=0.000011` and `0.000007`
  seconds after the respective transitions.

### mpv 0.41 control semantics

Upstream `audio/out/ao_avfoundation.m` at tag `v0.41.0` establishes:

- `AOCONTROL_SET_MUTE` calls `AVSampleBufferAudioRenderer.setMuted` directly;
- `AOCONTROL_SET_VOLUME` calls `AVSampleBufferAudioRenderer.setVolume`
  directly;
- pause changes `AVSampleBufferRenderSynchronizer` rate directly;
- stop cancels media-data requests, flushes the renderer, and stops the
  synchronizer.

The observed 96,000-sample AO queue therefore does not itself establish a
two-second delay for normal mute/volume controls. Seek still needs packaged
acceptance because it exercises mpv's higher-level stop/reset path.

### Invalid manual control tests

- The first volume test had no external timing cue, so the owner could not
  judge response latency.
- The second attempted to pair volume changes with mpv `show-text`. IPC
  returned success, but no OSD text appeared in the playback window.
- Neither attempt is evidence that volume response passed or failed. Do not
  cite them as acceptance proof.

### Plezy comparison

- Installed Plezy 2.16.0 embeds mpv 0.41.0 through MPVKit 1.0.23 and explicitly
  configures `ao=coreaudio,avfoundation`.
- Plezy contains no macOS AirPlay detector, CoreAudio route listener, polling
  loop, or automatic AirPlay delay policy.
- Plezy's actual runtime AO was not recoverable non-invasively. It is no longer
  load-bearing because Vela's own live A/B isolated AVFoundation as the working
  path.

Machine-specific values and full diagnostic chronology live in
`.agents/machines.md`.

## Design

### 1. Make base mpv command construction testable

Extract the existing command initialization and deterministic/default argument
block from `play` into a small helper in `src-tauri/src/playback.rs`. The helper
takes the resolved mpv executable and loaded `AppConfig`, returns a
`std::process::Command`, and includes:

- own-config/`--no-config` handling;
- existing video, HDR, hardware-decoding, and platform GPU defaults;
- the new macOS AO preference when Vela owns defaults;
- Advanced mpv arguments appended last.

Keep authentication include, autocrop script, IPC endpoint, resume position,
title, `--`, and media URL construction in the existing `play` flow. This keeps
credential-bearing and per-play resources outside unit-testable base argv and
avoids a broad playback refactor.

The helper must preserve every existing argument and ordering contract except
for the intentional macOS AO insertion. The real `play` path must construct its
command only through this helper, so the testable path is the shipped path.

### 2. Add the macOS AVFoundation-first default

Under `#[cfg(target_os = "macos")]`, append exactly:

`--ao=avfoundation,coreaudio`

Append it with Vela defaults before iterating `mpv_extra_args`. The order means
an Advanced entry such as `--ao=coreaudio` remains authoritative without
special parsing. Do not add `--audio-delay`, `--audio-device`,
`--ao-avfoundation-max-lookahead`, or any CoreAudio framework/crate dependency.

The ordered AO list gives mpv the fallback. Vela must not inspect
`current-ao`, retry a backend itself, or alter behavior after launch.

### 3. Preserve own-config ownership

When `mpv_use_own_config=true`, omit Vela's AO argument along with
`--no-config`. An expert's `mpv.conf` owns AO selection. They can opt into the
proven path with `ao=avfoundation,coreaudio` in that file or through Advanced
arguments. This rule was owner-approved on 2026-08-20.

### 4. Document the resulting contract

Update `docs/usage.md` where Advanced mpv/own-config behavior is explained:

- macOS Vela defaults to AVFoundation with CoreAudio fallback because that path
  keeps AirPlay and local output transitions synchronized;
- Vela does not apply an AirPlay delay;
- Advanced `--ao` arguments override the default;
- own-config precedence follows the owner ruling.

Add a concise macOS playback fix entry to `RELEASE_NOTES.md`. Do not expose a
new setting or describe the discarded `--audio-delay=-2.0` workaround as a
product solution.

## Delivery slice

One code/docs commit is sufficient because the behavior is one launch-policy
change and its guards:

1. write failing command-construction guards;
2. prove each guard fails for the intended missing/reversed behavior;
3. extract the helper without changing current argv;
4. add the macOS AO default behind the settled own-config rule;
5. update usage and release notes;
6. run automated and packaged macOS acceptance;
7. record evidence in this plan and `.agents/state.md` and commit the complete
   slice.

Do not add dormant configuration, frontend state, a route module, IPC monitor,
or target dependency.

## Automated guards and red proof

Unit-test the helper through `Command::get_args` without spawning mpv:

1. default macOS config contains exactly one
   `--ao=avfoundation,coreaudio`;
2. it appears before every Advanced argument;
3. a later Advanced `--ao=coreaudio` is retained and is the last effective AO;
4. own-config true omits Vela's AO preference;
5. own-config false still contains `--no-config`, while true does not;
6. all existing video/HDR/GPU defaults remain present;
7. non-macOS helper builds append no Vela AO;
8. no managed `audio-delay` or `audio-device` argument appears.

Required red proof:

- remove only the macOS AO append and observe guard 1 fail;
- reverse the AO order and observe guard 1 fail;
- move Advanced arguments before defaults and observe guard 3 fail;
- invert the settled own-config branch and observe guard 4 fail.

Restore each mutation and rerun the focused test before broader verification.

## Verification

### Automated

Run the backend verification entry points from `.agents/repo-guidance.md`:

- `cargo +1.89.0 check --locked` from `src-tauri/`;
- `cargo +stable check --locked` from `src-tauri/`;
- `cargo +stable clippy --all-targets --locked -- -D warnings` from
  `src-tauri/`;
- `cargo +stable test --locked` from `src-tauri/`;
- `cargo audit --file Cargo.lock` from `src-tauri/`;
- `git diff --check`.

Run the focused command-construction tests during development and retain the
red/restored-green transcript in this plan. Because documentation and shipped
bundle behavior also change, build the frontend and a local macOS app through
the repo's normal entry point before packaged acceptance.

### Packaged macOS acceptance

Use a newly built Vela app; a live IPC override is not acceptance for launch
policy.

1. With AirPlay already selected, start the affected network MKV/E-AC-3 5.1
   item. Confirm `current-ao=avfoundation` and physical sync from launch.
2. Change AirPlay -> built-in -> AirPlay during the same play. Confirm sound
   follows and remains synchronized without Vela intervention.
3. Launch the same item initially on built-in output and confirm sync.
4. Pause for several seconds, resume, seek forward, and seek backward. Confirm
   prompt response and physical resynchronization after each action.
5. Exercise mute and volume using normal mpv controls. Source proves direct
   renderer controls; acceptance confirms no user-visible regression. Do not
   reuse the invalid no-cue/hidden-OSD procedures.
6. Play the local stereo AAC flash/click fixture and check startup audio is not
   materially worse than the diagnosed half-click preroll.
7. Set a temporary Advanced `--ao=coreaudio`, launch once, and prove the expert
   override wins; remove it immediately afterward.
8. Exercise own-config mode according to the settled rule and restore the
   owner's original setting.

Record only sanitized AO/format/timing evidence. Never retain media-server
URLs, titles, headers, tokens, or full playback logs.

## Risks and rollback

- **AVFoundation unavailable or fails initialization:** ordered AO fallback
  preserves CoreAudio playback, though that fallback can retain the diagnosed
  AirPlay offset.
- **Compressed passthrough/expert audio setups:** Vela's reproduced default
  decoded E-AC-3 to PCM. Experts requiring CoreAudio-exclusive/passthrough can
  override `--ao` in Advanced arguments; document this without changing
  `audio-spdif` policy.
- **Startup preroll:** AVFoundation swallowed roughly half of the first 50 ms
  diagnostic click. Packaged acceptance must check realistic startup content.
- **Control behavior:** direct renderer volume/mute and synchronizer pause are
  source-backed, but seek remains a required real-play acceptance check.

Rollback is one argument-policy removal plus its documentation. No persisted
schema or migration is introduced.

## Implementation evidence

- `src-tauri/src/playback.rs` now routes the shipped `play` path through the
  testable base-command helper. On macOS with Vela-owned defaults it appends
  exactly `--ao=avfoundation,coreaudio` before Advanced arguments; own-config
  mode omits the AO preference. No route detector, delay, device pin, setting,
  IPC monitor, or dependency was introduced.
- Focused guards proved red for each intended regression before restoration:
  missing AO preference, reversed `coreaudio,avfoundation` order, Vela's AO
  appended after an Advanced override, and Vela's AO applied in own-config
  mode. The restored focused tests pass.
- Vela was bumped once from 1.0.60 to 1.0.61 through `scripts/bump.sh` as
  required by the repository's code-change version policy.
- Final automated verification on 1.0.61 passed: pinned Node/npm assertion,
  npm audit, 71 frontend policy tests plus Svelte diagnostics, frontend build,
  Rust 1.89 and stable checks, stable clippy with warnings denied, all 365 Rust
  tests, and `git diff --check`.
- RustSec audit initially found newly published `RUSTSEC-2026-0258` in locked
  `h2` 0.4.15. The separate prerequisite commit `3e1d0c8` updates only the
  locked package to patched 0.4.16 and release documentation. The rerun has no
  vulnerabilities and retains the repository's 17 existing informational
  warnings.
- `scripts/build.sh --native` produced and launched
  `dist/Vela_1.0.61_aarch64.dmg`. Physical AirPlay sync, route transition, and
  normal pause/seek/mute/volume observations remain pending on that running
  package; do not infer them from automated command-construction evidence.

## Prior review disposition

The two retained Claude verdicts in
`.agents/review/openreview-2026-08-20-airplay-plan.md` reviewed the withdrawn
route-detection/constant-delay design, not this AVFoundation-first plan. Their
records and provenance remain intact as historical constraints if a future
change reintroduces compensation. They are not an active decision queue.

The owner's durable review-process constraint remains: never start more than
one Claude session without explicit approval; never impose a low timeout,
terminate a Claude process, or discard a verdict without explicit approval.
