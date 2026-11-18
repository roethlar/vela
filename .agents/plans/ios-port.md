# Plan: Vela on iPhone (second of three Apple ports)

## Status

**DRAFT 2026-11-18. Not approved; no code is authorized.** Review:
pending (`codereview codex gpt-6.2-astra xhigh`, owner's request of
2026-11-18).

This plan comes after `.agents/plans/ipados-port.md` and before
`.agents/plans/tvos-port.md` (`.agents/decisions.md` 2026-11-18). It
starts only when the iPadOS plan has landed. It adds no engine, seam or
library work of its own; those are the iPadOS plan's Part A.

## Owner rulings that bind this plan

Everything in `ipados-port.md`, "Owner rulings that bind this plan",
applies here unchanged: the Apple engine, no label, parity, the proper
way, HDR, licensing, and the working rules.

## What this plan is

iPhone and iPad run one app, from one Tauri iOS target. The iPadOS plan
builds that app for the iPad family only, and makes the page work at
every iPad window size, Split View and Slide Over included, which are
already compact widths. This plan:
- adds the iPhone family to the same app;
- builds what is specific to the phone: phone heights, landscape at
  small heights, the notch, the Dynamic Island and the home indicator,
  rotation during play, one-handed reach of the player's controls, and
  memory and heat;
- proves it on the owner's iPhone.

## Scope

When this plan is done, Vela installs on the owner's iPhone and works as
it does on the iPad. Every row of the iPadOS plan's parity list passes on
iPhone, and the page is usable:
- in portrait from 375 pt to 440 pt wide;
- in landscape at heights from 375 pt to 440 pt.

## Non-goals (of this plan)

- Features the desktop does not have: picture in picture, AirPlay of
  video, offline downloads, a cellular-data setting, and CarPlay.
- iPhones whose iOS is below the iPadOS plan's minimum (iOS 18 is
  assumed there).
- Apple TV (`tvos-port.md`).

## Owner inputs needed (one at a time)

- **P1. The owner's iPhone (a fact):** model and iOS version. Which
  iPhones have HDR screens comes from Apple's list (support.apple.com
  109039), which includes the iPhone X, XS and 11 Pro as well as the
  12 and later, and from the screen's measured EDR headroom at run time
  (the iPadOS plan's B-1), never from a model cutoff. iPhones without one
  play through Apple's tone mapping; Vela writes no SDR path of its own (the
  HDR rule).
- **P2. Differences that need approval:** none new. The four of
  `ipados-port.md`, as ruled there, apply to the phone.

## Assumptions (each proven or overturned by a probe)

- **C1 (I1).** The engine plays 4K HDR10 and Dolby Vision on the owner's
  iPhone within its memory limit: the demux and loopback buffers, or the
  sample-buffer queue, whichever P2 chose. It also does so on the
  software-decode path for a codec the phone has no decoder for.
  Measured: peak memory, and thermal state over a 30-minute play.
- **C2 (I1).** With the on-screen keyboard up in landscape (about half
  the height), the page's forms (the server address, sign-in, search,
  playlist names) scroll their focused field into view.

## Design

### The page on a phone

These build on the iPadOS plan's B-3 compact layout (the drawer, stacked
detail pages, top-tab Settings, the player's single sheet):
- **Safe areas:** every fixed or sticky element (the header, the footer,
  the player bar, the skip button, the notice) is inset by
  `env(safe-area-inset-*)` in both orientations. An end-to-end check in
  the iPhone simulator venue reads each element's position against the
  simulator's safe areas.
- **Small heights:** in landscape below about 500 pt of height, the
  header collapses to one row, and the Continue Watching cover-flow
  sizes by height. Dialogs and Settings scroll inside the visible
  viewport (`dvh`).
- **The player in portrait:** the picture is letterboxed at the top
  third, with the controls below it, reachable with one hand. Turning to
  landscape fills the screen. Rotation does not interrupt the play, and
  the engine's layer follows the rectangle the page reports.
- **Navigation:** the iPadOS plan's drawer, unless I2's check on the
  owner's phone shows it is hard to reach with one hand. Then the same
  destinations move to a bottom bar at phone sizes. That is a layout
  change, not a feature difference.

### The app

- **Device family:** iPhone and iPad (1,2), in the tracked Tauri
  configuration (the iPadOS plan's B-2 rule: nothing in
  `src-tauri/gen/apple` is edited).
- **Orientation:** every orientation but upside-down portrait on iPhone,
  which Apple's guidelines discourage.
- **Icons:** the iPhone sizes, from the same 1024 x 1024 source as the
  iPad's.

### The engine on a phone

- No engine features are added.
- **Buffer limits** are set from C1's measurements. When iOS sends a
  memory warning, the engine shrinks its buffers rather than being
  ended by the system.
- **Thermal state** (`ProcessInfo.thermalState`) is measured and
  recorded (I1, I4). It is not a step-down trigger.
  - The owner approved two triggers: sustained decoder frame drops and a
    repeatedly starving cache (`.agents/decisions.md` 2026-07-25,
    implemented by `AutomaticDetector` in `automatic.rs`). They were
    chosen because a step-down interrupts the play and can lose HDR.
  - A phone that overheats and starts dropping frames already steps down
    through the first trigger.
  - Heat as a trigger of its own would be a third. That needs the
    owner's ruling, asked only if the measurements show plays failing
    from heat without drops.

## Slices

Each slice is one or more commits with tests and red-proofs, a
`codereview codex` hunt and a push.

- **I0, the iPhone family.** Device family 1,2 and the orientations go
  in the tracked configuration first. Apple's device family `2` alone
  prevents installing on an iPhone, and every later slice needs the app
  on the phone and in the iPhone simulator. CI builds the iPhone
  simulator app, and the app is installed on the owner's iPhone through
  O2's channel.
- **I1, probes (throwaway; Log).** C1 on the owner's iPhone with the
  owner's 4K HDR10, Dolby Vision 8.1 and converted profile 7 files. C2
  in the iPhone simulator.
- **I2, the page on a phone.** Safe areas, small heights, the player in
  both orientations, and the forms with the keyboard up. The iPhone
  simulator venue runs every end-to-end scenario at two phone sizes. The
  desktop suites run too, to show the desktop is unchanged.
- **I3, icons and release packaging.** The iPhone icon sizes. The
  release workflow's iOS job, if O2 made one, ships the universal app.
- **I4, the engine's memory and thermal response**, with XCTests that
  inject memory warnings and thermal states.
- **I5, the owner's by-eye checks** on the iPhone, one by one from a
  checklist, each recorded with what was seen:
  - HDR10, HLG, and Dolby Vision profiles 5 and 8.1;
  - an original profile 7 file, converted live. A file converted
    beforehand would bypass the converter.
  - a Dolby Vision negative control: the reference clip of
    `tvos-port.md` (one base layer with two RPUs that look visibly
    different, and one with the RPU stripped);
  - each audio format of the iPadOS plan's row 17, with the route named:
    speaker, headphones, AirPods or an HDMI adapter;
  - rotation during play.

  If the owner's iPhone has no HDR screen, tone-mapped playback is
  checked there, and the HDR rows stay pending until an HDR iPhone is
  used. An iPad's result does not stand in for the phone. Then docs and
  release notes.

## Risks

- **Phone layout changes can disturb the iPad's compact layout and the
  desktop.** Every page slice runs the iPad and desktop venues as well.
- **The owner's iPhone may not have an HDR screen.** Rows 15 and 16
  would then stay pending on the phone, with tone-mapped playback checked
  instead, and the record says so.

## Verification before claiming completion

- The repo-guidance Verification set.
- The desktop suites.
- The iPad simulator venue, and the iPhone simulator venue at two sizes.
- The iOS CI job.
- The owner's by-eye checks on the iPhone, recorded with what was seen.

## Log

- 2026-11-18: drafted after the owner's rulings of that day. The
  layout facts it builds on are in `ipados-port.md`, B-3.
