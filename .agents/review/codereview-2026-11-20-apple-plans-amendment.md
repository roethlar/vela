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
closed by a reviewer's verdict.

### iPadOS plan

| ID | Severity | Finding (one line) | Status | Commit |
|----|----------|--------------------|--------|--------|
| ipd-59 | HIGH | T0's Tauri 2.12.7 needs Rust 1.90; Vela's floor and CI are 1.89 | `[ ]` | |
| ipd-60 | HIGH | A Dolby Vision stream that falls back to software decoding loses its RPU (profile 5 turns green) | `[ ]` | |
| ipd-61 | HIGH | The picture budget counts a 2160p 10-bit picture at 12 MB; uncompressed it is 24.9 MB | `[ ]` | |
| ipd-62 | MEDIUM | Counting each E-AC-3 syncframe advances the clock twice when a dependent substream is present | `[ ]` | |
| ipd-63 | MEDIUM | A format change later in the stream (an HLS discontinuity) has no decoder recovery | `[ ]` | |
| ipd-64 | MEDIUM | "Exact" seek skips the picture that covers the target | `[ ]` | |
| ipd-65 | MEDIUM | A gap in the sound's time flushes sound that was still to be heard | `[ ]` | |
| ipd-66 | MEDIUM | A hardware session may decode and still refuse to deinterlace | `[ ]` | |
| ipd-67 | MEDIUM | A track's display rotation is lost (portrait iPhone recordings play sideways) | `[ ]` | |
| ipd-68 | MEDIUM | The plan says P2 is done on the iPad and A2 holds, beyond what the Log shows | `[ ]` | |

Round 5's repairs (findings in `codereview-2026-11-18-apple-plans.md`):

| ID | Verdict | What is still wrong | Status | Commit |
|----|---------|---------------------|--------|--------|
| ipd-55 | closed | | `[x]` | `fd4461e` |
| ipd-56 | closed | | `[x]` | `1dbb1d6` |
| ipd-58 | closed | | `[x]` | `24cf277` |
| ipd-32 | reopened | A reset on another device (an offset, then none) is both "the server changed" and "no resume state, Vela wins" | `[ ]` | |
| ipd-36 | reopened | "Every permitted external decoder" cannot be built: `libxevd` and `libxevdb` exclude each other | `[ ]` | |
| ipd-52 | reopened | A close already sent, whose answer was lost, can still clear the next play | `[ ]` | |
| ipd-53 | reopened | A read that starts exactly at the end can get 416, which the plan calls an error | `[ ]` | |
| ipd-57 | reopened | `If-Range` needs a strong validator; with a weak one or none, two versions can still be spliced | `[ ]` | |

### iPhone plan

Its findings 1, 3, 4, 8, 9, 10, 11 and 13 are ipd-59, ipd-61, ipd-60,
ipd-63, ipd-66, ipd-64, ipd-62 and ipd-57 again, and share their repairs.
Its own:

| ID | Severity | Finding (one line) | Status | Commit |
|----|----------|--------------------|--------|--------|
| iph-22 | HIGH | The iOS overlay's `"resources": {}` does not remove the desktop's GPL mpv scripts from the app | `[ ]` | |
| iph-23 | MEDIUM | FEL composition, once landed for the iPads, would run on the iPhone without a phone's own measurement | `[ ]` | |
| iph-24 | MEDIUM | Software decoders already give pictures in display order; a second reorder queue doubles delay and memory | `[ ]` | |
| iph-25 | MEDIUM | The renderer's dropped count includes display misses, which could trigger an Automatic step-down the owner did not approve | `[ ]` | |
| iph-26 | MEDIUM | The exact-length rule for HLS byte ranges rejects FFmpeg's read across adjacent segments | `[ ]` | |
| iph-27 | LOW | I2 keeps row 13a open for a ruling the owner already gave | `[ ]` | |
| iph-28 | LOW | "Every slice gets a `codereview codex`" contradicts the 2026-11-20 rule when codex writes the slice | `[ ]` | |

Nothing was declined.
