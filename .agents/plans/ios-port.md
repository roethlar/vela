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
- iPhones whose iOS is below the iPadOS plan's minimum, which P2 sets
  there.
- Apple TV (`tvos-port.md`).

## Owner inputs needed (one at a time)

- **P1. The owner's iPhone (a fact):** model and iOS version. Which
  iPhones have HDR screens comes from Apple's list (support.apple.com
  109039), which includes the iPhone X, XS and 11 Pro as well as the
  12 and later, and from the screen's measured EDR headroom at run time
  (the iPadOS plan's B-1), never from a model cutoff. iPhones without one
  play through Apple's tone mapping; Vela writes no SDR path of its own (the
  HDR rule).
- **P2. Differences that need approval:** none new, but none carries
  over by itself. The iPadOS plan asks its four differences (O4) for the
  iPad and the iPhone together, since one app runs on both. A ruling
  whose words cover the iPhone carries over. One that names only the
  iPad leaves that difference open for the phone, and it is asked before
  I0. I0 puts the same build on the iPhone, so anything the iPad ruling
  hid would otherwise already be missing there.

## Assumptions (each proven or overturned by a probe)

- **C1 (I1).** The engine plays 4K HDR10 and Dolby Vision on the owner's
  iPhone within its memory limit: the demux and loopback buffers, or the
  sample-buffer queue, whichever P2 chose. It also does so on the
  software-decode path, with a fixture chosen to need it on the owner's
  model: VC-1 or MPEG-2, 10-bit H.264, or AV1 on a phone without an AV1
  decoder. The engine reports which decoder is active, and the probe
  asserts it. Measured separately for the hardware and software paths:
  peak memory, and thermal state over a 30-minute play.
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
- **Small heights:** a landscape phone (up to 932 x 430 pt) is wider
  than both the drawer's threshold (about 700 pt) and the player's compact
  form (about 900 px). So the drawer and the compact player also switch
  on compact height, not only on width. A test at 932 x 430 checks that
  the drawer and the compact player controls are what shows, and that
  every control is reachable. Below about 500 pt of height, the header
  also collapses to one row, and the Continue Watching cover-flow
  sizes by height.
  - Dialogs, Settings and forms are sized and scrolled by the visual
    viewport (`window.visualViewport`, its height and offset), not by
    `dvh`. WKWebView can shrink or pan the visual viewport for the
    keyboard while `dvh` layout keeps its height.
  - I2 checks that the focused field and the form's action button are
    both reachable with the software keyboard up, in both orientations.
- **The player in portrait:** the picture is letterboxed at the top
  third, with the controls below it, reachable with one hand. Turning to
  landscape fills the screen. Rotation does not interrupt the play.
  - The page reports the rectangle of a dedicated picture element, and
    the engine's layer follows it. Today `PlayerView.svelte` reports the
    whole player view (`player_set_rect`), which on the desktop is also
    the picture, so `embeddedcontrols`' full-page check still holds there.
  - A scenario checks the native layer's frame against that element in
    portrait and again after rotating, while the same session keeps
    playing.
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
- **Buffer limits** are set from C1's measurements and enforced before
  anything is allocated or enqueued: a byte and frame bound on every
  queue, checked on entry. A large allocation can end the app before a
  memory-warning handler runs, so the handler only shrinks the bounds
  further; it is not what keeps the app alive.
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

Each slice is one or more commits, each with its tests and red-proofs,
each pushed to `origin` and `github` as it lands
(`.agents/push-policy.md`). Each slice that lands repo code gets a
`codereview codex` hunt.

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
  simulator venue runs, at two phone sizes, every end-to-end scenario
  that applies to the Apple engine.
  - The list of applicable scenarios is kept with the iPadOS plan's Apple
    run set (`tests/e2e/parity.mjs`).
  - Each excluded scenario names its reason: mpv's own behaviour
    (`mpvargs`, the mpv lifecycle and window-state scenarios), or, for a
    feature difference, the owner's ruling. The
  desktop suites run too, to show the desktop is unchanged.
- **I3, icons and release packaging.** The iPhone icon sizes. The
  release workflow's iOS job, if O2 made one, ships the universal app.
- **I4, the engine's memory and thermal response.** XCTests check the
  bounds and inject memory warnings and thermal states. The proof is on
  the phone: after I4's changes, the sustained hardware and software
  plays of I1 are repeated on the owner's iPhone, recording peak
  footprint, the queue bounds reached, and that the play continued.
- **I5, the owner's by-eye checks** on the iPhone, one by one from a
  checklist, each recorded with what was seen. The checklist is the
  iPadOS plan's rows 15 to 17, run on the phone:
  - HDR10 and HLG (row 15);
  - Dolby Vision profiles 5, 8.1 and 8.4, profile 10 if the phone
    decodes AV1, and an original profile 7 file converted live, each on
    the minimum iOS as well as the current one (row 16). A file converted
    beforehand would bypass the converter.
  - the Dolby Vision reference clip of `tvos-port.md` and its
    stripped-RPU control;
  - each audio format, proved as row 17 says: the layout the engine
    reports, the channel-identification clips, and the route recorded with
    what it showed. Multichannel and Atmos results are recorded only from
    a route that can show them, such as an HDMI adapter to a receiver; a
    speaker or headphones prove only that sound plays.
  - rotation during play.

  A row with no device, OS or route that can prove it on the phone stays
  pending, named as such.

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
