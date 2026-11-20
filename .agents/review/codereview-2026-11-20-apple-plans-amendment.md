# Codereview 2026-11-20 — the iPad and iPhone plans, after the probes' amendment

Outcome: two `codereview codex` hunts, design only, one per plan,
dispatched in parallel on 2026-11-20. They cover the amendment the probes
called for (`bee0c00`), everything else written into the plans since the
last rounds, and, for the iPadOS plan, the round 5 repairs of 2026-11-18
that had no verdict.

| Hunt | Range | Result |
|------|-------|--------|
| iPadOS plan | `2ffdd13..bee0c00` | **findings**: 10 new, 10 admitted. Round 5's repairs: 3 closed, 5 reopened |
| iPhone plan | `510a619..bee0c00` | **findings**: 15, of which 8 are the iPadOS report's again and 7 its own; all admitted |

The reviewers' reports, verbatim, and the prompts they were given are
beside this file in `codereview-2026-11-20-apple-plans-amendment/`
(`ipados.md`, `ios.md`, `prompt-ipados.txt`, `prompt-ios.txt`). Each
finding below has a one-line summary; the evidence is the reviewer's, in
that report.

## Dispatch record

- The owner's word: asked "Shall I go ahead with the amendment and that
  review round?", after being told codex would be called plainly unless
  the owner named a model, the owner answered "go".
- Who reviews: the amendment was written by Claude (Fable 5.2), so the
  reviewer is codex (`.agents/decisions.md`, 2026-11-20, "Review crosses
  model families").
- Harness: codex-cli 0.171.0, CLI transport, headless one-shot, run from
  the repo root on `nagatha`:
  `codex --search exec --sandbox read-only --json -o <outfile> <prompt>
  < /dev/null`. No model and no effort were named.
- Resolved identity: the `--json` event stream carries no model field.
  Codex's own configuration on this Mac (`~/.codex/config.toml`) names
  `gpt-6.2-astra` at `xhigh`, which a plain call takes. That is what was
  dispatched by default, not a transcript-confirmed identity. Codex
  printed that the project's own configuration was not loaded (the
  project is not marked trusted for it).
- Tier: none declared (a plain call). Grade: none declared.
- Capability proof: each reviewer read `AGENTS.md` and ran
  `git rev-parse <head>`, and reported that both succeeded.
- Prompt: the bounded defect-hunt mandate, with the pins and the
  read-only boundary; design only, by the owner's ruling of 2026-11-18 on
  tests. The iPadOS prompt also named round 5's findings file and repair
  commits, for verdicts. Neither prompt summarized the plans or named a
  candidate risk.
- Runs: iPadOS 20:19:33Z to 20:28:30Z, exit 0, 28 web searches. iPhone
  20:19:33Z to 20:28:46Z, exit 0, 29 web searches.

## What the working agent checked before admitting

- ipd-59: `tauri` 2.12.7 declares `rust-version = "1.90"` in its
  manifest, as do `tauri-build` 2.7.6 and `tauri-runtime-wry` 2.12.5 (tao
  0.37.3 and wry 0.57.3 declare 1.85); `src-tauri/Cargo.toml` declares
  1.89 and CI runs `cargo +1.89.0 check --locked`. The probe built with
  stable 1.99, which is why P1 did not see it.
- ipd-61: 3840 x 2160 at 4:2:0 with 16 bits a component is 24,883,200
  bytes. The amendment's "about 12 MB" was the agent's arithmetic error.
- ipd-60: a probe run on the Mac, after the report: the decoder's
  `DolbyVisionRPUData` attachment on the profile 5 frame is, byte for
  byte, the stream's RPU as dovi_tool writes it without its start code
  (267 bytes), which is the raw form FFmpeg exposes as frame side data.
  The key itself is not a constant in Apple's public headers (iOS 27.2
  SDK: CoreVideo, CoreMedia, VideoToolbox and AVFoundation were searched).
- ipd-36 (reopened): the pinned FFmpeg's `configure` stops when `libxevd`
  and `libxevdb` are both enabled (lines 4860 to 4861).
- iph-22: the probe's archived `Vela.app` does contain
  `assets/mpv-scripts/` with `autocrop.lua` and `LICENSE.GPL`. The Log's
  claim that the overlay left them out was false: the agent listed the
  bundle's top folder and did not look inside it. Tried after the report:
  with `"resources": []` in `tauri.ios.conf.json` and the generated
  `gen/apple/assets` folder emptied, a fresh simulator build has no Lua
  and no GPL file in it. With the folder not emptied, the same build
  still carried the old copies: Tauri does not clear what an earlier
  build put there.
- iph-26: the pinned FFmpeg's `hls.c` extends one open's `end_offset`
  across consecutive byte ranges of the same resource (lines 1481 to
  1500).

## Findings

Status: `[ ]` admitted and open; `[~]` repaired, no verdict yet; `[x]`
closed by a reviewer's verdict. "r2" is the round of verdicts on the
first repairs (below).

### iPadOS plan

| ID | Severity | Finding (one line) | Status | Commit |
|----|----------|--------------------|--------|--------|
| ipd-59 | HIGH | T0's Tauri 2.12.7 needs Rust 1.90; Vela's floor and CI are 1.89 | `[x]` closed in r2 | `675d6cd` |
| ipd-60 | HIGH | A Dolby Vision stream that falls back to software decoding loses its RPU (profile 5 turns green) | `[x]` reopened in r2, closed in r3 | `0e73e0b`, `253c903` |
| ipd-61 | HIGH | The picture budget counts a 2160p 10-bit picture at 12 MB; uncompressed it is 24.9 MB | `[~]` reopened in r2 and in r3, repaired again | `c8cc601`, `14a6960`, `b952812` |
| ipd-62 | MEDIUM | Counting each E-AC-3 syncframe advances the clock twice when a dependent substream is present | `[x]` reopened in r2, closed in r3 | `3c6fc64`, `f1b2be5` |
| ipd-63 | MEDIUM | A format change later in the stream (an HLS discontinuity) has no decoder recovery | `[x]` closed in r2 | `152d26a` |
| ipd-64 | MEDIUM | "Exact" seek skips the picture that covers the target | `[~]` reopened in r2 and in r3, repaired again | `acefe81`, `95331c5`, `32f7a3a` |
| ipd-65 | MEDIUM | A gap in the sound's time flushes sound that was still to be heard | `[~]` reopened in r2 and in r3, repaired again | `f8895f9`, `89d3c02`, `3794375` |
| ipd-66 | MEDIUM | A hardware session may decode and still refuse to deinterlace | `[x]` reopened in r2, closed in r3 | `33deafa`, `1a73731` |
| ipd-67 | MEDIUM | A track's display rotation is lost (portrait iPhone recordings play sideways) | `[x]` closed in r2 | `9fc7392` |
| ipd-68 | MEDIUM | The plan says P2 is done on the iPad and A2 holds, beyond what the Log shows | `[x]` closed in r2 | `787c2c9` |
| ipd-69 | MEDIUM | (r2) An unchanged file restarts on every seek when the server answers a ranged request with the whole file | `[x]` closed in r3 | `986478e` |
| ipd-70 | MEDIUM | (r2, both reports) The software path expects a raw RPU from dav1d for profile 10; FFmpeg gives parsed metadata there | `[x]` closed in r3 | `253c903` |
| ipd-71 | MEDIUM | (r3) With no validator the forward-only open cannot read an MP4 whose index follows its media | `[~]` | `9fed125` |
| ipd-72 | MEDIUM | (r3) Reporting the position back through the timeline offset rewinds progress after a reset | `[~]` | `3794375` |
| ipd-73 | MEDIUM | (r3) After a changed redirect target, two resources with the same ETag can be spliced | `[~]` | `a51a84b` |
| ipd-74 | MEDIUM | (r3) The copy into the deinterlacer loses the field order | `[~]` | `5a3230e` |

Round 5's repairs (findings in `codereview-2026-11-18-apple-plans.md`):

| ID | Verdict | What was still wrong | Status | Commit |
|----|---------|----------------------|--------|--------|
| ipd-55 | closed | | `[x]` | `fd4461e` |
| ipd-56 | closed | | `[x]` | `1dbb1d6` |
| ipd-58 | closed | | `[x]` | `24cf277` |
| ipd-32 | reopened | A reset on another device (an offset, then none) is both "the server changed" and "no resume state, Vela wins" | `[x]` closed in r2 | `f266d93` |
| ipd-36 | reopened | "Every permitted external decoder" cannot be built: `libxevd` and `libxevdb` exclude each other | `[x]` closed in r2 | `2cb5d51` |
| ipd-52 | reopened | A close already sent, whose answer was lost, can still clear the next play | contested after r3 (`ipd-52.contested.md`) | `1c59006`, `c9cfd9b`, `647c8c2` |
| ipd-53 | reopened | A read that starts exactly at the end can get 416, which the plan calls an error | `[x]` closed in r2 | `56b3b76` |
| ipd-57 | reopened | `If-Range` needs a strong validator; with a weak one or none, two versions can still be spliced | `[x]` reopened again in r2, closed in r3 | `696c8a4`, `5203659` |

### iPhone plan

Its findings 1, 3, 4, 8, 9, 10, 11 and 13 are ipd-59, ipd-61, ipd-60,
ipd-63, ipd-66, ipd-64, ipd-62 and ipd-57 again, and share their repairs.
Its own:

| ID | Severity | Finding (one line) | Status | Commit |
|----|----------|--------------------|--------|--------|
| iph-22 | HIGH | The iOS overlay's `"resources": {}` does not remove the desktop's GPL mpv scripts from the app | `[x]` closed in r2 | `9e51f31` |
| iph-23 | MEDIUM | FEL composition, once landed for the iPads, would run on the iPhone without a phone's own measurement | `[x]` closed in r2 | `2aaedf8` |
| iph-24 | MEDIUM | Software decoders already give pictures in display order; a second reorder queue doubles delay and memory | `[x]` reopened for the phone's rule in r2, closed in r3 | `c5065a3`, `0731be8`, `add36fd` |
| iph-25 | MEDIUM | The renderer's dropped count includes display misses, which could trigger an Automatic step-down the owner did not approve | `[x]` closed in r2 | `6f96695` |
| iph-26 | MEDIUM | The exact-length rule for HLS byte ranges rejects FFmpeg's read across adjacent segments | `[x]` closed in r2 | `39b3e74` |
| iph-27 | LOW | I2 keeps row 13a open for a ruling the owner already gave | `[x]` closed in r2 | `f1ca934` |
| iph-28 | LOW | "Every slice gets a `codereview codex`" contradicts the 2026-11-20 rule when codex writes the slice | `[x]` closed in r2 | `ebe5773` |
| iph-29 | MEDIUM | (r2) A server may answer a ranged request with a shorter `206`; the "whole range or error" rule breaks on it | `[~]` closed by the iPadOS report and reopened by the iPhone report in r3 (a `206` may also begin later than asked); repaired again | `4250781`, `bbf89a8` |
| iph-30 | HIGH | (r3) A Dolby Vision stream Apple will not decode ends in an error, though the owner's ruling is that every codec plays on the device | `[~]` | `67189a2` |
| iph-31 | MEDIUM | (r3) The pinned FFmpeg's MP4 demuxer gives a `dav1` track no codec | `[~]` | `d1d2274` |

Nothing was declined. iph-28's repair also corrects the same sentence in
`tvos-port.md`, which was outside the round's scope.

## Round 2: verdicts on the repairs (2026-11-20, `bee0c00..ebe5773`)

Two codex sessions, called the same way, 23:20:07Z to 23:25:04Z (iPhone)
and to 23:29:00Z (iPadOS), exit 0 each. Each read `AGENTS.md` and ran
`git rev-parse` on the head. Reports, verbatim: `ipados-r2.md` and
`ios-r2.md`; prompts beside them. The prompts named each finding's
repair commit and no more.

- iPadOS: 12 closed, 8 reopened (ipd-60, ipd-61, ipd-62, ipd-64, ipd-65,
  ipd-66, ipd-52, ipd-57), 2 new (ipd-69, ipd-70).
- iPhone: 7 closed, 3 reopened (ipd-60, ipd-61, and iph-24 for the
  phone's own rule), 2 new (iph-29, and ipd-70 again).
- What the working agent checked: FFmpeg's AV1 path attaches only parsed
  Dolby Vision metadata (`itut35.c`), its HEVC decoder the raw RPU as
  well (`hevc/hevcdec.c`); the owner's Plex answers a ranged request for
  a media part with a strong ETag and a Last-Modified date.
- ipd-60's reopening turns on the rule that the Apple apps use public API
  only (`.agents/decisions.md`, 2026-11-18, the agent's reading of "do
  not close that door"): the attachment that carries the RPU has no
  declared key. The second repair stops the engine writing it at all,
  which also changes how a composed FEL picture could be shown; that
  choice is the owner's, after probe P5.

## Round 3: verdicts on the second repairs (2026-11-20, `ebe5773..add36fd`)

Two codex sessions, called the same way, 23:33:20Z to 23:38:20Z (iPhone)
and to 23:39:51Z (iPadOS), exit 0 each, with the capability proof as
before. Reports, verbatim: `ipados-r3.md` and `ios-r3.md`; prompts
beside them.

- iPadOS: 7 closed (ipd-60, ipd-70, ipd-62, ipd-66, ipd-57, ipd-69,
  iph-29), 4 reopened (ipd-52, ipd-61, ipd-64, ipd-65), 4 new (ipd-71 to
  ipd-74).
- iPhone: 3 closed (iph-24, ipd-60, ipd-70), 2 reopened (ipd-61, and
  iph-29, which the iPadOS session closed), 2 new (iph-30, iph-31).
- What the working agent checked: the pinned dav1d wrapper's allocator
  rounds to 128 and adds two alignment units (`libdav1d.c`), and the
  pinned MOV tag table has no `dav1` entry (`isom_tags.c`).

## Where it stands (2026-11-20, head `647c8c2`)

Every finding admitted in the three rounds has a repair in the plans.
Closed by a verdict: 27. Repaired with no verdict yet (`[~]`): ipd-61,
ipd-64, ipd-65, ipd-71, ipd-72, ipd-73, ipd-74, iph-29, iph-30, iph-31.
Contested: ipd-52, for the owner (`.agents/review/ipd-52.contested.md`).

Each round closed most of what it was given and raised about as many new
points in the text the repairs touched, mostly in the transport and
timing rules: 22, then 12, then 11. The working agent's recommendation,
put to the owner: stop reviewing the plans as documents here. What is
left is of the kind that the code and its tests settle, and every slice
gets its own review when it is written.
