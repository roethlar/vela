# Plan: Diagnose macOS AirPlay A/V synchronization

## Status

**Revised 2026-08-20 after the owner approved a backend-first plan correction.
The owner approved diagnostic execution on 2026-08-20; it is in progress.
Implementation is not approved.**

The earlier design assumed Vela had to detect AirPlay and impose a nominal
2,000 ms video delay. Direct inspection of Plezy 2.16.0 falsified the premise
that a working mpv client necessarily does that: Plezy contains no macOS
AirPlay detector or automatic delay policy. The listener, polling, delay
setting, and route-transition design from the earlier draft are withdrawn.
They must not be implemented from this document.

The next owner gate is a controlled runtime experiment identifying which
audio output actually gives correct physical presentation timing. A later
implementation plan must be based on that evidence and separately approved.

## Problem and goal

On the affected macOS host, Vela-spawned mpv presents video ahead of audio
when the system output is an AirPlay/HomePod speaker. Infuse and Plezy do not
show the same defect. The goal of this plan is to isolate the material
difference before selecting a fix.

Any eventual product fix must:

1. work automatically as the system output changes, without a per-device
   manual setting;
2. keep Vela's settled external-mpv playback boundary;
3. preserve ordinary built-in, USB, HDMI/DisplayPort, Bluetooth, aggregate,
   and virtual output behavior;
4. preserve advanced mpv argument precedence and the opt-in own-config path;
5. avoid a guessed constant delay unless controlled physical measurement
   proves no backend can own the timing correctly.

## Established evidence

### Vela path

- Vela launches the system mpv executable out of process from
  `src-tauri/src/playback.rs`.
- Unless the owner enables the own-config option, Vela passes `--no-config`.
- Vela sets video/HDR options but does not select `--ao`; advanced user
  arguments are appended after Vela defaults and therefore retain their
  documented override precedence.
- The affected host's `/opt/homebrew/bin/mpv` is mpv 0.41.0. Its compiled
  automatic audio-output order is CoreAudio, AVFoundation, null,
  CoreAudio-exclusive, then PCM.
- A direct AirPlay probe selected `ao=coreaudio`. mpv reported about 11.6 ms
  of device latency, which does not represent the observed wireless
  presentation delay.

### Plezy comparison

- The installed Plezy is 2.16.0 build 142, matching source tag `2.16.0`.
- Plezy embeds mpv 0.41.0 through MPVKit 1.0.23 and explicitly sets
  `ao=coreaudio,avfoundation` before `mpv_initialize`.
- Plezy has no macOS AirPlay transport detector, CoreAudio route listener, or
  polling loop that controls synchronization, and no automatic AirPlay delay.
  Its persisted audio-sync offset defaults to zero and is applied only when
  the user has selected a nonzero value.
- Plezy added the AVFoundation fallback because macOS 27 can reject
  `ao_coreaudio` channel-layout initialization with `paramErr (-50)`. Static
  source therefore cannot establish whether a particular play used
  CoreAudio or fell through to AVFoundation.
- Retained macOS unified logs do not expose Plezy's selected mpv AO. The
  actual working AO remains unverified.
- Plezy's MPVKit fork contains custom AVFoundation timing and presentation
  patches, while its CoreAudio implementation is upstream mpv 0.41.0 apart
  from build-guard changes. Those patches are a possible difference only if
  Plezy actually selects AVFoundation or its embedded presentation path is
  otherwise material.

### Limits on inference

- Infuse is closed source. Its correct behavior is comparative evidence, not
  evidence for a specific internal implementation.
- Plezy and Vela sharing an mpv version and AO priority does not prove they
  use the same AO at runtime.
- mpv's internal `avsync` can be zero while a wireless endpoint presents
  audio later than the latency exposed to mpv. Acceptance must include a
  physical flash/click observation, not only mpv properties.
- System default output is not necessarily mpv's actual output if an own
  config or explicit argument pins a device.

## Hypotheses to distinguish

1. **Audio-output hypothesis:** standalone CoreAudio omits AirPlay's endpoint
   latency, while AVFoundation owns a presentation clock that keeps physical
   audio and video synchronized. Plezy works because CoreAudio fails on
   macOS 27 and its AVFoundation fallback opens.
2. **Embedded-path hypothesis:** both apps select CoreAudio, but Plezy's
   embedded MPVKit/video presentation path behaves differently from mpv's
   standalone macOS window.
3. **Launch-context hypothesis:** the AO and renderer can work, but a Vela
   environment, effective argument, startup order, or route-change sequence
   changes the result.
4. **Backend-gap hypothesis:** neither upstream standalone backend accounts
   for the endpoint delay. A route-aware correction may be necessary, but
   only after the first three hypotheses are eliminated.

## Diagnostic acceptance contract

The diagnostic slice is complete only when its evidence record contains:

1. one local, credential-free flash/click fixture used for every comparison;
2. the same physical Mac, display, HomePod/AirPlay target, volume conditions,
   and route-selected-before-launch sequence for the controlled A/B runs;
3. forced CoreAudio and forced AVFoundation results using Vela-equivalent
   standalone mpv arguments, on both built-in output and AirPlay;
4. `current-ao`, `audio-device`, `audio-delay`, and available mpv A/V clock
   properties captured without media-server URLs or credentials;
5. physical sync measured from the fixture, with observation uncertainty
   recorded; mpv's internal clock is supporting evidence only;
6. Plezy's actual `current-ao` for one synchronized AirPlay play, or a clear
   record that it could not be obtained non-invasively;
7. one mid-play output-switch observation only for each launch path already
   shown to be synchronized, recording whether mpv follows the new default;
8. a filled result matrix and an evidence-backed disposition for all four
   hypotheses;
9. a proposed minimal production design, its regressions and verification
   surface, followed by a separate owner approval before any product edit.

## Diagnostic procedure

### Stage 1 — establish a controlled fixture and baseline

- Generate a short local file under an owner-private temporary directory.
  It must contain repeated, frame-identifiable flashes paired with short
  clicks and no server URL or account data.
- Record the macOS build, default output transport, HomePod target, display
  refresh rate, mpv path/version, Plezy version, and MPVKit pin in the
  evidence log. Keep host-specific values in `.agents/machines.md` and point
  to them from this plan.
- Derive the standalone argument baseline directly from Vela's command
  construction. Do not enable the owner's mpv config and do not use the
  persisted Advanced arguments field for the experiment.
- Use an owner-private per-run IPC endpoint and log path. Capture only
  selected AO/device/timing lines; never retain a full media-server playback
  log.

### Stage 2 — force the two upstream macOS AOs

For each of built-in output and the selected AirPlay/HomePod output:

1. launch the fixture with Vela-equivalent arguments plus forced
   `--ao=coreaudio`;
2. capture the selected AO and properties through the temporary IPC endpoint;
3. measure physical flash/click alignment;
4. repeat with forced `--ao=avfoundation`;
5. repeat a result if the physical observation is ambiguous.

The primary comparison is physical synchronization. Startup success,
dropouts, seeking, pause/resume, volume response, and format acceptance are
secondary compatibility observations that must still be recorded.

### Stage 3 — identify Plezy's working runtime path

- First attempt a narrow, non-persistent diagnostic using Plezy's existing
  mpv log/event path, filtering to AO selection only.
- Do not capture unfiltered output from a server-backed play because it can
  contain token-bearing URLs or headers.
- If the release build does not expose `current-ao`, stop and request owner
  authority before modifying or building the sibling Plezy repository. The
  fallback is a disposable diagnostic-only Plezy change that queries
  `current-ao` once after `file-loaded`; it is never committed or shipped.
- Record the selected AO, whether fallback occurred, and the physical sync
  observation. Do not infer the AO from the static priority list.

### Stage 4 — test live route changes only after launch behavior is known

- For every AO shown synchronized at launch, switch built-in -> AirPlay and
  AirPlay -> built-in during the same play.
- Record `audio-device`, `current-ao`, discontinuity/reinitialization events,
  and physical sync after stabilization.
- This stage answers whether the eventual fix must react to route changes or
  whether the selected Apple backend already owns them. It does not authorize
  a Vela listener or polling loop.

### Stage 5 — choose the smallest evidence-backed design

Use the first matching branch:

- **AVFoundation correct; CoreAudio wrong:** propose an explicit macOS AO
  preference in Vela. Verify fallback and common local/HDMI/USB/compressed
  formats. Do not add device detection or a delay setting.
- **Both standalone AOs correct; Vela wrong:** compare Vela's effective argv,
  process environment, launch timing, and window/presentation context against
  the controlled command. Fix the divergent launch behavior.
- **Plezy correct on CoreAudio while standalone CoreAudio is wrong:** isolate
  MPVKit, embedded-renderer, app-bundle, and patch differences one at a time.
  Preserve the external-mpv product decision unless the owner explicitly
  changes it.
- **Neither standalone AO correct:** only then reopen measured route-aware
  correction. The design must use mpv's actual AO/device and baseline delay,
  not merely the system default, and must honor zero/own-config/expert
  ownership. The retained Claude findings become active constraints again.

Each branch requires a new cold-implementable production plan, independently
proved guards, regression analysis, and owner approval before code changes.

## Evidence log

### 2026-08-20 — AirPlay launch A/B, physical observation pending

- Generated a credential-free 12-second 1280x720 60 fps flash/click fixture.
  The valid comparison fixture contains stereo 48 kHz AAC audio and has
  SHA-256 `f5e5b5641234a3876ad535bb41e8970f005730d194039d1cef40c7716262877e`.
- An initial mono fixture was invalid for the sync comparison: forced
  CoreAudio rejected its input channel layout with macOS `paramErr (-50)` and
  produced no audio. This independently reproduces the failure that motivated
  Plezy's AVFoundation fallback, but it says nothing about physical A/V sync.
- With the stereo fixture and system output already AirPlay, forced CoreAudio
  opened device `AirPlay (126)`. IPC reported `current-ao=coreaudio`,
  `audio-device=auto`, `audio-delay=0`, and `avsync=-0.005736` seconds. The AO
  reported 11,609,977 ns device latency, 354,166 ns AudioUnit latency, and a
  1,148-sample device buffer.
- With every other argument and route condition unchanged, forced
  AVFoundation opened `AVSampleBufferAudioRenderer`. IPC reported
  `current-ao=avfoundation`, `audio-device=auto`, `audio-delay=0`, and
  `avsync=-0.000142` seconds. It allocated a 96,000-sample device buffer.
- Both valid runs completed naturally. The owner observed that CoreAudio
  swallowed the first click and then kept every remaining click synchronized
  with its flash. AVFoundation swallowed roughly half of the first click and
  then kept every remaining click synchronized with its flash. Neither backend
  had a steady-state physical offset on this stereo fixture.
- Installed Vela 1.0.60 has no stored `mpv_extra_args`, does not enable the
  own-config path, and has no stored autocrop override. Those sanitized fields
  exclude the three user-controlled launch differences without exposing the
  connection document's other fields.
- The owner then reproduced the persistent defect through installed Vela
  1.0.60 on the same AirPlay output: physical audio was roughly two seconds
  late. Sanitized live IPC showed `current-ao=coreaudio`,
  `audio-device=auto`, `audio-delay=0`, and `avsync=0.000006` seconds. The input
  was a network-backed MKV with H.264 video at 23.976025 fps and E-AC-3
  `5.1(side)` audio at 48 kHz, decoded to float planar and output by CoreAudio
  as 48 kHz stereo. Video used VideoToolbox, display timing was 240 Hz, and
  `video-sync=audio`. mpv therefore believed the stream was synchronized while
  the AirPlay endpoint physically presented its audio about two seconds late.
- With that exact play still running, the owner separately approved one IPC
  write setting `ao=avfoundation`. mpv accepted it and changed
  `current-ao` from CoreAudio to AVFoundation without an explicit audio reload,
  seek, or relaunch. IPC then reported `avsync=0.000359` seconds. The owner
  observed that physical A/V synchronization became correct. No persisted Vela
  or mpv setting changed; the selection belongs only to that live process.
- Interim disposition: the audio-output hypothesis is rejected for
  general stereo playback but confirmed for the affected server-backed stream:
  AVFoundation fixes the physical offset CoreAudio leaves invisible to mpv.
  The backend-gap hypothesis is rejected because an existing upstream backend
  is correct. The embedded-path hypothesis is unnecessary for this
  reproduction. Codec/channel layout/input path still determine why CoreAudio
  fails only on the real stream, but they no longer block selecting the
  working backend. Route-change and non-AirPlay compatibility remain unproved.

## Result matrix

Fill this table during the separately approved diagnostic run:

| Player/path | Output at launch | Forced/actual AO | Physical offset | mpv-reported state | Mid-play switch | Result |
| --- | --- | --- | --- | --- | --- | --- |
| Standalone Vela-equivalent | Built-in | CoreAudio | pending | pending | pending | pending |
| Standalone Vela-equivalent | AirPlay | CoreAudio | steady-state synchronized; first click absent | `current-ao=coreaudio`; delay 0; `avsync=-5.736 ms`; reported device latency 11.610 ms | pending | Correct after initial preroll |
| Standalone Vela-equivalent | Built-in | AVFoundation | pending | pending | pending | pending |
| Standalone Vela-equivalent | AirPlay | AVFoundation | steady-state synchronized; about half of first click absent | `current-ao=avfoundation`; delay 0; `avsync=-0.142 ms`; 96,000-sample buffer | pending | Correct after initial preroll |
| Installed Vela 1.0.60 server-backed play | AirPlay | CoreAudio | audio late roughly 2 seconds | E-AC-3 5.1 -> stereo; `avsync=0.006 ms`; delay 0 | not tested | Defect reproduced |
| Same installed Vela play after live AO change | AirPlay | AVFoundation | synchronized | E-AC-3 5.1 -> stereo; `avsync=0.359 ms`; delay 0 | AO changed live without relaunch | Defect corrected |
| Plezy 2.16.0 | AirPlay | pending runtime proof | observed synchronized | pending | pending | pending |

## Prior review disposition

The two retained Claude verdicts in
`.agents/review/openreview-2026-08-20-airplay-plan.md` reviewed the withdrawn
route-detection/constant-delay design. Their records and provenance remain
intact. Their findings are historical constraints if evidence later forces a
compensation design; they are not an active polling-versus-listener decision
queue now.

No further Claude session is authorized by this revision. The owner's durable
review-process constraint remains: never start more than one Claude session
without explicit approval; never impose a low timeout, terminate a Claude
process, or discard a verdict without explicit approval.

## Verification for this planning slice

This revision changes no shipped behavior. Run `git diff --check`, inspect the
plan/review/state/machine record as one coherent diff, and commit the planning
record. No product build or platform CI is required for this docs-only slice.
