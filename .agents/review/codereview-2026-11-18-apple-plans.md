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
| tvp-1 | HIGH | Jellyfin/Emby tokens in saved item snapshots would enter `UserDefaults` | `[x]` closed by the repair hunt | `3f9157c` |
| tvp-2 | HIGH | One Keychain item per connection breaks exact rollback of connections | `[x]` closed in round 2 | `f548b4d` |
| tvp-3 | HIGH | One history copy per document removes two approved recovery versions | `[x]` closed by the repair hunt | `5107e1d` |
| tvp-4 | MEDIUM | A 400 KB cap is an unapproved limit on healthy playlists | `[~]` reopened in round 2, repaired again `8b38df9` | `5d5737c`, `f966e57` |
| tvp-5 | HIGH | The recovery screen omits "Try again" (`retry_durable_state`) | `[-]` reopened; declined (owner ruling 2026-11-18, tests) | `980c7fc` |
| tvp-6 | HIGH | The Swift interface has no core-to-engine direction (play, commands) | `[x]` closed by the repair hunt | `0f4339e` |
| tvp-7 | HIGH | The step-down and completion workers stay in Tauri's `.setup` | `[x]` closed by the repair hunt | `e03c407` |
| tvp-8 | HIGH | SwiftUI has no route for Plex's `vela-artwork:` artwork | `[x]` closed by the repair hunt | `8169b46` |
| tvp-9 | HIGH | Plex sign-in stops before server selection (`link_select_server`) | `[x]` closed by the repair hunt | `2fa00f0` |
| tvp-10 | LOW | The remote's short jump is ±10 s; the desktop's arrows are ±5 s | `[x]` closed by the repair hunt | `ef1978a` |
| tvp-11 | HIGH | Profile 8.1 to 5 conversion has no base-layer conversion | `[x]` closed by the repair hunt | `4785665` |
| tvp-12 | HIGH | The LG's Dolby Vision indicator cannot prove the metadata reached the picture | `[x]` closed by the repair hunt | `2379345` |
| tvp-13 | MEDIUM | B4 treats an encoder content-type value as HDMI passthrough | `[x]` closed by the repair hunt | `6a2869f` |
| tvp-14 | HIGH | T0 builds a standalone core before T1 creates one | `[x]` closed by the repair hunt | `9d4c3d1` |
| tvp-15 | MEDIUM | A physical Apple TV cannot reach the loopback-bound Plex TLS mock | `[x]` closed by the repair hunt | `d31477e` |
| tvp-16 | MEDIUM | `cargo test` from `src-tauri` can skip the extracted core's tests | `[x]` closed by the repair hunt | `be7e706` |
| tvp-17 | MEDIUM | The command guards misclassify shell-only commands and page-only features | `[-]` reopened; declined (owner ruling 2026-11-18, tests) | `a5aeb9c` |
| tvp-18 | MEDIUM | One-way DTO decoding passes after a breaking contract change | `[-]` reopened; declined (owner ruling 2026-11-18, tests) | `6cc0fd7` |
| tvp-19 | MEDIUM | No privacy manifest for `UserDefaults`, so the TestFlight upload is rejected | `[x]` closed by the repair hunt | `4b01e43`, `66881a1` |

### iPhone plan (`ios-port.md`)

| ID | Severity | Finding (one line) | Status | Commit |
|----|----------|--------------------|--------|--------|
| iph-1 | HIGH | Heat as a step-down trigger is not one of the owner's two approved triggers | `[x]` closed by the repair hunt | `17e3f9d` |
| iph-2 | HIGH | I0 and I1 need an iPhone build before I2 enables the iPhone family | `[x]` closed by the repair hunt | `c501fca` |
| iph-3 | HIGH | The phone's by-eye checks can pass with required formats broken | `[x]` closed in round 2 | `0807077` |
| iph-4 | HIGH | "HDR from the iPhone 12 on" excludes HDR iPhones (XS, 11 Pro) | `[x]` closed by the repair hunt | `db98de7` |
| iph-5 | MEDIUM | I0 can pass without exercising the software decoder | `[x]` closed by the repair hunt | `bc21ab2` |
| iph-6 | MEDIUM | An injected memory warning cannot prove the build survives pressure | `[x]` closed by the repair hunt | `96092ec` |
| iph-7 | MEDIUM | "Every end-to-end scenario" is impossible on iPhone (`mpvargs`, full-page rect) | `[x]` closed by the repair hunt | `906646d` |
| iph-8 | MEDIUM | iPad-only approvals are extended to the iPhone | `[x]` closed in round 2 | `25e89e7` |
| iph-9 | MEDIUM | `dvh` does not give the area above the on-screen keyboard | `[x]` closed by the repair hunt | `e7c43dc` |
| iph-10 | MEDIUM | The width breakpoints miss landscape phones (932 x 430) | `[x]` closed by the repair hunt | `5499602` |
| iph-11 | LOW | "A push" per slice contradicts the push-after-each-commit policy | `[x]` closed by the repair hunt | `6c8f279` |

### iPadOS plan (`ipados-port.md`, and the decision of 2026-11-18)

Run 13:30:31Z to 13:41:31Z, exit 0, 130 web searches. The working agent
checked two claims: the fallback `cfg` of `rename_noreplace`
(`storage.rs`, ipd-13), and `check_mpv` inside Settings' load
(`Settings.svelte`, ipd-10). Both hold.

| ID | Severity | Finding (one line) | Status | Commit |
|----|----------|--------------------|--------|--------|
| ipd-1 | HIGH | Profile 5 conversion and the profile 5 test fixture omit the base layer's pixel conversion | `[x]` closed by the repair hunt | `5c623b7` |
| ipd-2 | HIGH | P2 can pass without the engine displaying Dolby Vision metadata | `[x]` closed by the repair hunt | `fd5a408` |
| ipd-3 | HIGH | Profiles and minimum OS tested are narrower than the support promised (8.4, HLS on iPadOS 18) | `[x]` closed in round 2 | `39749fc` |
| ipd-4 | HIGH | Loopback HLS leaves software-decoded video and LPCM audio no presentation path | `[x]` closed by the repair hunt | `4d01c24` |
| ipd-5 | HIGH | The media build omits AAC and HLS input, both of which Vela already plays | `[x]` closed by the repair hunt | `5cb730a` |
| ipd-6 | HIGH | A Jellyfin transcode URL already carries `api_key`, so row 7 fails | `[~]` reopened in round 2, repaired again `06c7f04` | `2569bfe` |
| ipd-7 | MEDIUM | SRT, WebVTT and mov_text need converting to ASS events before libass | `[x]` closed by the repair hunt | `f1d05df` |
| ipd-8 | MEDIUM | Cropping has no frame source on the hardware sample-buffer path | `[x]` closed by the repair hunt | `d4939b8` |
| ipd-9 | HIGH | The seam drops the desktop's window and screen state from launches and events | `[x]` closed by the repair hunt | `ef837e7` |
| ipd-10 | HIGH | Compiling out `check_mpv` breaks Settings' whole load; crop settings live in mpv commands | `[x]` closed by the repair hunt | `d5ce525` |
| ipd-11 | HIGH | Backgrounding sends a terminal report for a play the plan only pauses | `[~]` reopened in round 2, repaired again `a3c74f7` | `6409f9c` |
| ipd-12 | MEDIUM | Rows 12b (exit stops playback) and 14 (ends at end of file) dropped from the parity list | `[x]` closed by the repair hunt | `9ef23e5` |
| ipd-13 | MEDIUM | Widening only the macOS rename branch leaves the fallback returning failure after the move | `[x]` closed by the repair hunt | `220f489` |
| ipd-14 | HIGH | A third player cannot produce the two-player parity verdict; the driver and waits are mpv's | `[x]` closed by the repair hunt | `1b77c38` |
| ipd-15 | MEDIUM | A6 proves the step-down notice before A9 builds Automatic quality | `[x]` closed by the repair hunt | `c7d154b` |
| ipd-16 | MEDIUM | By-eye checks cannot tell stereo, multichannel LPCM and Atmos apart | `[x]` closed by the repair hunt | `7e20fc3` |
| ipd-17 | MEDIUM | The signing recipe covers neither channel O2 offers | `[x]` closed by the repair hunt | `32cf784` |
| ipd-18 | MEDIUM | `dvh` does not give a keyboard-safe layout on iPad WebKit | `[x]` closed by the repair hunt | `3f804c3` |
| ipd-19 | LOW | The device gate excludes iPads that have EDR headroom | `[x]` closed by the repair hunt | `c5bbf55` |
| ipd-20 | LOW | The decision record says mpv always outputs HDR10 from Dolby Vision; macOS EDR does not | `[x]` closed in round 2 | `3fbd6ef` |

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

## Repair hunt, round 1 (2026-11-18, `bad7fa1..fe7fea2`)

Three fresh codex sessions, the same owner-named model and effort, one
per plan. Each judged its plan's findings and looked for new defects in
the repaired text. Reports, verbatim: `ios-r1.md`, `ipados-r1.md` and
`tvos-r1.md` beside this file. Runs 13:55:45Z to 14:07:52Z, exit 0 each.

- iPhone: 9 closed, 2 reopened (iph-3, iph-8), 8 new (iph-12 to iph-19).
- iPadOS: 16 closed, 4 reopened (ipd-3, ipd-6, ipd-11, ipd-20), 9 new
  (ipd-21 to ipd-29).
- tvOS: 14 closed, 5 reopened (tvp-2, tvp-4, tvp-5, tvp-17, tvp-18), 13
  new (tvp-20 to tvp-32).

The owner's ruling of 2026-11-18 came while this round was triaged:
"don't spend tokens on developing tests for code that hasn't been
written. don't review tests that shouldn't be conceived of yet"
(`.agents/decisions.md`). From then on, findings about tests for
unwritten code are declined with that ruling as the reason. Design
findings are repaired. The iPhone round's repairs (iph-12 to iph-19)
were made before the ruling, and several are test details, which the
plans now say are not binding.

### New findings

| ID | Severity | Finding (one line) | Status | Commit |
|----|----------|--------------------|--------|--------|
| iph-12 | HIGH | I0's install could require an App Store release | `[x]` closed in round 2 | `d043ad5` |
| iph-13 | MEDIUM | Sustained memory runs bypass live profile 7 conversion | `[x]` closed in round 2 | `df59b63` |
| iph-14 | MEDIUM | Bounds on Apple's internal renderer queue cannot be checked | `[x]` closed in round 2 | `9561e16` |
| iph-15 | MEDIUM | The rotation test passes with a full-screen portrait picture | `[x]` closed in round 2 | `5e5e65a` |
| iph-16 | MEDIUM | A scenario exclusion can hide parity row 13a | `[x]` closed in round 2 | `2a28228` |
| iph-17 | MEDIUM | The landscape test rejects the bottom-bar outcome | `[x]` closed in round 2 | `00c3aa7` |
| iph-18 | LOW | 932 x 430 is not the scope's landscape maximum | `[x]` closed in round 2 | `e4cbf21` |
| iph-19 | LOW | The keyboard check asks for a Search button that does not exist | `[x]` closed in round 2 | `2e3914d` |
| ipd-21 | HIGH | HLS child requests and redirects could carry the token to another origin | `[~]` reopened in round 2, repaired again `06c7f04` | `676b522` |
| ipd-22 | HIGH | Profile 10 has no probe | `[-]` duplicate of ipd-3 (reopened), repaired there | `a01dd22` |
| ipd-23 | HIGH | One app manifest cannot cover each shipped framework | `[x]` closed in round 2 | `2fe1eba` |
| ipd-24 | MEDIUM | The OS floor applies to any loopback Dolby Vision, not only 8.1 | `[x]` closed in round 2 | `da2c683` |
| ipd-25 | MEDIUM | The Settings check precedes the first runnable app | `[-]` declined (owner ruling 2026-11-18, tests) | |
| ipd-26 | MEDIUM | A6 must pass a lifecycle row A10 builds | `[-]` declined (owner ruling 2026-11-18, tests) | |
| ipd-27 | MEDIUM | A7 needs an Automatic transition before A9 builds it | `[-]` declined (owner ruling 2026-11-18, tests) | |
| ipd-28 | MEDIUM | libass tracks need the ASS header and font attachments | `[x]` closed in round 2 | `221dbc8` |
| ipd-29 | MEDIUM | Keyframe-only crop detection measures a different picture | `[x]` closed in round 2 | `7e9faa8` |
| tvp-20 | HIGH | A cache purge erases playlist display and routing information | `[x]` closed in round 2 | `aad77ed` |
| tvp-21 | HIGH | Reference-only recents lose Vela's resume position | `[x]` closed in round 2 | `9cbcb7c` |
| tvp-22 | HIGH | CloudKit adds an unapproved account dependency | `[~]` reopened in round 2, repaired again `8b38df9` | `10a8b5e` |
| tvp-23 | HIGH | The separate engine `stop` has no session id | `[x]` closed in round 2 | `0d54247` |
| tvp-24 | HIGH | Continue Playing is assigned to the core, not the screen | `[x]` closed in round 2 | `e004a28` |
| tvp-25 | MEDIUM | Saved artwork can be rebound to the wrong server | `[x]` closed in round 2 | `804a630` |
| tvp-26 | MEDIUM | T1 cannot run its worker proof; an existing guard pins the dispatcher in `.setup` | `[-]` declined (owner ruling 2026-11-18, tests). For the implementer: `tests/clean-eof-refresh-order.test.mjs` moves with the dispatcher. | |
| tvp-27 | MEDIUM | The workspace clippy command drops `--locked` and `-D warnings` | `[x]` closed in round 2 | `efca5dc` |
| tvp-28 | MEDIUM | T0 probes different TLS features from production | `[x]` closed in round 2 | `ba25770` |
| tvp-29 | LOW | The TLS probe provisions trust on the TV only | `[-]` declined (owner ruling 2026-11-18; probe procedure) | |
| tvp-30 | HIGH | Display criteria cannot name AV1 Dolby Vision (profile 10) | `[x]` closed in round 2 | `10d51a8` |
| tvp-31 | MEDIUM | LPCM decoding does not establish lossless delivery | `[x]` closed in round 2 | `9920d71` |
| tvp-32 | MEDIUM | One app manifest cannot cover each shipped framework | `[x]` closed in round 2 | `2fe1eba` |

Declined reopened findings: tvp-5, tvp-17 and tvp-18 now ask only for
test designs (recovery interruption, coverage guards, argument
round-trips), so they are declined under the same ruling.

Open: the `[~]` rows above have no verdict on their latest repair. The
next codex round, design-only, over the updated plans, gives them their
verdicts (owner, 2026-11-18).

## Design round 2 (2026-11-18, `fe7fea2..16c787e`)

Design only, by the owner's ruling on tests. Three codex sessions, the
same model and effort, 15:28Z to 15:35Z, exit 0 each. Reports, verbatim:
`ios-r2.md`, `ipados-r2.md` and `tvos-r2.md`.

- iPhone: every pending row closed. New: iph-20 (the loopback's segment
  producer had no memory bound), repaired at `a80b148`; iph-21 (the
  footprint measures only Vela's process), repaired at `5284fe7`.
- tvOS: 11 closed; tvp-4 and tvp-22 reopened (playlist storage still
  had two homes), repaired at `8b38df9`. New, all from the owner's
  CloudKit design, each repaired:
  - tvp-33, portable identities across TVs, `d7466cb`;
  - tvp-34, edits bound to their iCloud account, `530a39e`;
  - tvp-35, merging two TVs' edits, `9eb1f49`;
  - tvp-36, all three Continue Playing modes, `e67c881`;
  - tvp-37, the CloudKit entitlement and schema, `76c8bf7`;
  - tvp-38, the offline state, `91e0211`;
  - tvp-39, quota, `9a0354d`;
  - tvp-40, the soundbar on direct HDMI, `492b00d`.
- iPadOS: 6 closed; ipd-6, ipd-21 and ipd-11 reopened. FFmpeg's own HTTP
  follows redirects and reuses connections without asking the engine,
  and a return after finalization had begun. Repaired at `06c7f04` (the
  engine makes every network request itself) and `a3c74f7`. New, each
  repaired:
  - ipd-30, credentials in FFmpeg's logs, `f51e2d8`;
  - ipd-31, a return after finalization had begun, `a3c74f7`;
  - ipd-32, old progress overwriting newer progress, `a3c74f7`;
  - ipd-33, same-origin credential URIs, `5e3eb90`;
  - ipd-34, position saves using up the settings history, `a3c74f7`;
  - ipd-35, the loopback's HLS attributes, `a80b148`.

Open: this round's repairs have no verdict yet.

## Design round 3 (2026-11-18, `16c787e..510a619`)

Design only. Three codex sessions, the same model and effort, 17:19Z to
17:27Z, exit 0 each. Reports, verbatim: `ios-r3.md`, `ipados-r3.md`,
`tvos-r3.md`.

Verdicts on round 2's repairs:
- iPhone: iph-20 and iph-21 closed.
- iPadOS: 7 closed. ipd-11 and ipd-32 were reopened and repaired again
  (`51c64f2`, `f71f3c1`).
- tvOS: tvp-22, tvp-34, tvp-38 and tvp-40 closed. Six were reopened and
  repaired again:
  - tvp-4, `ac56470`;
  - tvp-33, `5cd14bc`;
  - tvp-35 and tvp-39, `1fdc860`;
  - tvp-36, `8795422`;
  - tvp-37, `d81091a` and `fc0f1f1`.

New findings, numbered in the iPadOS report's order (the iPhone and tvOS
reports raised the same engine defects; their duplicates are not numbered
again):
- **ipd-36**, the build's codec allowlist: repaired at `37cf11c`, which
  enables every LGPL decoder and demuxer.
- **ipd-37, ipd-38, ipd-39, ipd-43 to ipd-46**, routing, and the remux
  fallback's encoder, Dolby Vision box, profile 5 signalling, seeking,
  display and Now Playing, and subtitle clock: withdrawn with the
  fallback's design (`463ce85`). The engine uses the sample-buffer
  renderer for everything. A remux path for Dolby Vision is designed in
  an amendment only if P2 shows it is needed, and these findings are its
  known requirements.
- **ipd-40, ipd-41, ipd-42**, App Transport Security, cookies, and the
  base URL of a redirected playlist: repaired at `eb7e59e`, which fetches
  media through the core's HTTP client. The iPhone report's FFmpeg HLS
  persistence and byte-range findings are covered by the same repair.
- **ipd-47**, the stale approval text: repaired at `3e26860`.
- **tvp-41**, an edit called saved before it is on disk: repaired at
  `267210d`.
- **tvp-42**, two rules for a return after finalization: repaired at
  `839e218`.

Open: this round's repairs have no verdict yet.
