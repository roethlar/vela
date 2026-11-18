# Codereview 2026-11-18 — the three Apple ports' plans

Outcome: three `codereview codex` hunts, one per plan, dispatched in
parallel on 2026-11-18 at the owner's request ("make plans for each and
review them with codex gpt-6.2-astra xhigh").

| Hunt | Range | Result |
|------|-------|--------|
| tvOS plan | `b00acd50..a06b5568` | **findings**: 19 candidates, 19 admitted |
| iPhone plan | `09c58efb..b00acd50` | **findings**: 11 candidates, 11 admitted |
| iPadOS plan, with the decision it rests on | `a7d01475..09c58efb` | **findings**: 20 candidates, 20 admitted |

The reviewers' reports, verbatim, are kept beside this file in
`codereview-2026-11-18-apple-plans/` (`tvos.md`, `ios.md`, `ipados.md`).
Each finding below has a one-line summary. The evidence (file and line,
condition, predicted failure, better approach) is the reviewer's, in that
report under the same number.

## Dispatch record

- Owner's dispatch word, verbatim: "review them with codex gpt-6.2-astra
  xhigh". The model and effort were named by the owner for this dispatch
  only (inline, session-only). The standing rule of 2026-07-26 is to call
  codex plainly, and it is unchanged. Nothing is stored as a default.
- Harness: codex-cli 0.170.0, CLI transport, headless one-shot, run from
  the repo root on `nagatha`:
  `codex --search exec -m gpt-6.2-astra -c model_reasoning_effort="xhigh"
  --sandbox read-only --json -o <outfile> <prompt>`.
- Resolved identity: the `--json` event stream carries no model field.
  This records the dispatched pair, not a transcript-confirmed one.
- Tier: owner-named (inline). Grade: none declared.
- Capability proof: each reviewer read `AGENTS.md` and ran
  `git rev-parse <head>`, and reported that both succeeded.
- Prompt: the bounded defect-hunt mandate of the codereview playbook,
  with the pins and the read-only boundary. It included no summary of
  the plans and no candidate risks.
- Runs:
  - tvOS: 13:30:31Z to 13:36:46Z, exit 0, 22 web searches.
  - iPhone: 13:30:31Z to 13:37:27Z, exit 0, 14 web searches.
  - iPadOS: 13:30:31Z to 13:41:31Z, exit 0, 130 web searches.

## Intake

Each candidate gave file and line evidence, a triggering condition, a
predicted failure and a severity tied to impact, so each one qualifies.
The working agent checked these claims against the repository:
- tvp-3: `.agents/decisions.md` 2026-07-23 requires three versions;
- tvp-7: the step-down loop and the sequence dispatcher are in
  `lib.rs` `.setup`;
- tvp-8: `artwork.rs` builds a marker, not a URL;
- tvp-1: `source/jellyfin.rs` puts `api_key` in its URLs;
- tvp-9: `commands.rs` has `ChooseServer`.

All are admitted. None is disputed.

Severity is the reviewer's unless noted. Repairs are docs-only. For each,
the check is a re-read of the repaired text against the evidence, and a
fresh hunt over the repair range closes the round.

### tvOS plan (`tvos-port.md`)

| ID | Severity | Finding (one line) | Status | Commit |
|----|----------|--------------------|--------|--------|
| tvp-1 | HIGH | Jellyfin/Emby tokens in saved item snapshots would enter `UserDefaults` | `[~]` repaired, awaiting the repair hunt | `3f9157c` |
| tvp-2 | HIGH | One Keychain item per connection breaks exact rollback of connections | `[~]` repaired, awaiting the repair hunt | `f548b4d` |
| tvp-3 | HIGH | One history copy per document removes two approved recovery versions | `[~]` repaired, awaiting the repair hunt | `5107e1d` |
| tvp-4 | MEDIUM | A 400 KB cap is an unapproved limit on healthy playlists | `[~]` repaired, awaiting the repair hunt | `5d5737c`, `f966e57` |
| tvp-5 | HIGH | The recovery screen omits "Try again" (`retry_durable_state`) | `[~]` repaired, awaiting the repair hunt | `980c7fc` |
| tvp-6 | HIGH | The Swift interface has no core-to-engine direction (play, commands) | `[~]` repaired, awaiting the repair hunt | `0f4339e` |
| tvp-7 | HIGH | The step-down and completion workers stay in Tauri's `.setup` | `[~]` repaired, awaiting the repair hunt | `e03c407` |
| tvp-8 | HIGH | SwiftUI has no route for Plex's `vela-artwork:` artwork | `[~]` repaired, awaiting the repair hunt | `8169b46` |
| tvp-9 | HIGH | Plex sign-in stops before server selection (`link_select_server`) | `[~]` repaired, awaiting the repair hunt | `2fa00f0` |
| tvp-10 | LOW | The remote's short jump is ±10 s; the desktop's arrows are ±5 s | `[~]` repaired, awaiting the repair hunt | `ef1978a` |
| tvp-11 | HIGH | Profile 8.1 to 5 conversion has no base-layer conversion | `[~]` repaired, awaiting the repair hunt | `4785665` |
| tvp-12 | HIGH | The LG's Dolby Vision indicator cannot prove the metadata reached the picture | `[~]` repaired, awaiting the repair hunt | `2379345` |
| tvp-13 | MEDIUM | B4 treats an encoder content-type value as HDMI passthrough | `[~]` repaired, awaiting the repair hunt | `6a2869f` |
| tvp-14 | HIGH | T0 builds a standalone core before T1 creates one | `[~]` repaired, awaiting the repair hunt | `9d4c3d1` |
| tvp-15 | MEDIUM | A physical Apple TV cannot reach the loopback-bound Plex TLS mock | `[~]` repaired, awaiting the repair hunt | `d31477e` |
| tvp-16 | MEDIUM | `cargo test` from `src-tauri` can skip the extracted core's tests | `[~]` repaired, awaiting the repair hunt | `be7e706` |
| tvp-17 | MEDIUM | The command guards misclassify shell-only commands and page-only features | `[~]` repaired, awaiting the repair hunt | `a5aeb9c` |
| tvp-18 | MEDIUM | One-way DTO decoding passes after a breaking contract change | `[~]` repaired, awaiting the repair hunt | `6cc0fd7` |
| tvp-19 | MEDIUM | No privacy manifest for `UserDefaults`, so the TestFlight upload is rejected | `[~]` repaired, awaiting the repair hunt | `4b01e43`, `66881a1` |

### iPhone plan (`ios-port.md`)

| ID | Severity | Finding (one line) | Status | Commit |
|----|----------|--------------------|--------|--------|
| iph-1 | HIGH | Heat as a step-down trigger is not one of the owner's two approved triggers | `[~]` repaired, awaiting the repair hunt | `17e3f9d` |
| iph-2 | HIGH | I0 and I1 need an iPhone build before I2 enables the iPhone family | `[~]` repaired, awaiting the repair hunt | `c501fca` |
| iph-3 | HIGH | The phone's by-eye checks can pass with required formats broken | `[~]` repaired, awaiting the repair hunt | `0807077` |
| iph-4 | HIGH | "HDR from the iPhone 12 on" excludes HDR iPhones (XS, 11 Pro) | `[~]` repaired, awaiting the repair hunt | `db98de7` |
| iph-5 | MEDIUM | I0 can pass without exercising the software decoder | `[~]` repaired, awaiting the repair hunt | `bc21ab2` |
| iph-6 | MEDIUM | An injected memory warning cannot prove the build survives pressure | `[~]` repaired, awaiting the repair hunt | `96092ec` |
| iph-7 | MEDIUM | "Every end-to-end scenario" is impossible on iPhone (`mpvargs`, full-page rect) | `[~]` repaired, awaiting the repair hunt | `906646d` |
| iph-8 | MEDIUM | iPad-only approvals are extended to the iPhone | `[~]` repaired, awaiting the repair hunt | `25e89e7` |
| iph-9 | MEDIUM | `dvh` does not give the area above the on-screen keyboard | `[~]` repaired, awaiting the repair hunt | `e7c43dc` |
| iph-10 | MEDIUM | The width breakpoints miss landscape phones (932 x 430) | `[~]` repaired, awaiting the repair hunt | `5499602` |
| iph-11 | LOW | "A push" per slice contradicts the push-after-each-commit policy | `[~]` repaired, awaiting the repair hunt | `6c8f279` |

### iPadOS plan (`ipados-port.md`, and the decision of 2026-11-18)

Run 13:30:31Z to 13:41:31Z, exit 0, 130 web searches. The working agent
checked two claims: the fallback `cfg` of `rename_noreplace`
(`storage.rs`, ipd-13), and `check_mpv` inside Settings' load
(`Settings.svelte`, ipd-10). Both hold.

| ID | Severity | Finding (one line) | Status | Commit |
|----|----------|--------------------|--------|--------|
| ipd-1 | HIGH | Profile 5 conversion and the profile 5 test fixture omit the base layer's pixel conversion | `[~]` repaired, awaiting the repair hunt | `5c623b7` |
| ipd-2 | HIGH | P2 can pass without the engine displaying Dolby Vision metadata | `[~]` repaired, awaiting the repair hunt | `fd5a408` |
| ipd-3 | HIGH | Profiles and minimum OS tested are narrower than the support promised (8.4, HLS on iPadOS 18) | `[~]` repaired, awaiting the repair hunt | `39749fc` |
| ipd-4 | HIGH | Loopback HLS leaves software-decoded video and LPCM audio no presentation path | `[~]` repaired, awaiting the repair hunt | `4d01c24` |
| ipd-5 | HIGH | The media build omits AAC and HLS input, both of which Vela already plays | `[~]` repaired, awaiting the repair hunt | `5cb730a` |
| ipd-6 | HIGH | A Jellyfin transcode URL already carries `api_key`, so row 7 fails | `[~]` repaired, awaiting the repair hunt | `2569bfe` |
| ipd-7 | MEDIUM | SRT, WebVTT and mov_text need converting to ASS events before libass | `[~]` repaired, awaiting the repair hunt | `f1d05df` |
| ipd-8 | MEDIUM | Cropping has no frame source on the hardware sample-buffer path | `[~]` repaired, awaiting the repair hunt | `d4939b8` |
| ipd-9 | HIGH | The seam drops the desktop's window and screen state from launches and events | `[~]` repaired, awaiting the repair hunt | `ef837e7` |
| ipd-10 | HIGH | Compiling out `check_mpv` breaks Settings' whole load; crop settings live in mpv commands | `[~]` repaired, awaiting the repair hunt | `d5ce525` |
| ipd-11 | HIGH | Backgrounding sends a terminal report for a play the plan only pauses | `[~]` repaired, awaiting the repair hunt | `6409f9c` |
| ipd-12 | MEDIUM | Rows 12b (exit stops playback) and 14 (ends at end of file) dropped from the parity list | `[~]` repaired, awaiting the repair hunt | `9ef23e5` |
| ipd-13 | MEDIUM | Widening only the macOS rename branch leaves the fallback returning failure after the move | `[~]` repaired, awaiting the repair hunt | `220f489` |
| ipd-14 | HIGH | A third player cannot produce the two-player parity verdict; the driver and waits are mpv's | `[~]` repaired, awaiting the repair hunt | `1b77c38` |
| ipd-15 | MEDIUM | A6 proves the step-down notice before A9 builds Automatic quality | `[~]` repaired, awaiting the repair hunt | `c7d154b` |
| ipd-16 | MEDIUM | By-eye checks cannot tell stereo, multichannel LPCM and Atmos apart | `[~]` repaired, awaiting the repair hunt | `7e20fc3` |
| ipd-17 | MEDIUM | The signing recipe covers neither channel O2 offers | `[~]` repaired, awaiting the repair hunt | `32cf784` |
| ipd-18 | MEDIUM | `dvh` does not give a keyboard-safe layout on iPad WebKit | `[~]` repaired, awaiting the repair hunt | `3f804c3` |
| ipd-19 | LOW | The device gate excludes iPads that have EDR headroom | `[~]` repaired, awaiting the repair hunt | `c5bbf55` |
| ipd-20 | LOW | The decision record says mpv always outputs HDR10 from Dolby Vision; macOS EDR does not | `[~]` repaired, awaiting the repair hunt | `3fbd6ef` |

The tvOS hunt's tvp-19 (privacy manifest) applies to the iPad app too.
The iPadOS hunt did not raise it. The iOS app also uses required-reason
APIs (UserDefaults, and file timestamps through `stat`), so the same
repair is made in `ipados-port.md` under tvp-19.

## Repairs

All fifty findings were repaired on 2026-11-18, one finding per commit,
each pushed. Commits are listed in the tables above. Some repairs reach
past their own plan, because the defect sits in shared text:
- tvp-19 adds the privacy manifest to both the tvOS and the iPadOS plans;
- iph-8 changes the iPadOS plan's O4;
- iph-11 corrects the slice wording in all three plans;
- ipd-3 changes the iPhone plan's minimum-OS line;
- ipd-20 corrects `.agents/decisions.md`.

The round closes with a repair hunt per plan over `bad7fa1..<repaired
head>`. Each hunt is given its plan's findings and judges each one closed
or reopened, plus anything new in the text the repairs touched. The
statuses above move to `[x]` only on a closed verdict.
