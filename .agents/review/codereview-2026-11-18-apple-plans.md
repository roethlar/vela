# Codereview 2026-11-18 — the three Apple ports' plans

Outcome: three `codereview codex` hunts, one per plan, dispatched in
parallel on 2026-11-18 at the owner's request ("make plans for each and
review them with codex gpt-6.2-astra xhigh").

| Hunt | Range | Result |
|------|-------|--------|
| tvOS plan | `b00acd50..a06b5568` | **findings**: 19 candidates, 19 admitted |
| iPhone plan | `09c58efb..b00acd50` | **findings**: 11 candidates, 11 admitted |
| iPadOS plan, with the decision it rests on | `a7d01475..09c58efb` | in progress |

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
  - iPhone: 13:30:31Z to 13:37:27Z, exit 0.

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
| tvp-1 | HIGH | Jellyfin/Emby tokens in saved item snapshots would enter `UserDefaults` | `[ ]` | |
| tvp-2 | HIGH | One Keychain item per connection breaks exact rollback of connections | `[ ]` | |
| tvp-3 | HIGH | One history copy per document removes two approved recovery versions | `[ ]` | |
| tvp-4 | MEDIUM | A 400 KB cap is an unapproved limit on healthy playlists | `[ ]` | |
| tvp-5 | HIGH | The recovery screen omits "Try again" (`retry_durable_state`) | `[ ]` | |
| tvp-6 | HIGH | The Swift interface has no core-to-engine direction (play, commands) | `[ ]` | |
| tvp-7 | HIGH | The step-down and completion workers stay in Tauri's `.setup` | `[ ]` | |
| tvp-8 | HIGH | SwiftUI has no route for Plex's `vela-artwork:` artwork | `[ ]` | |
| tvp-9 | HIGH | Plex sign-in stops before server selection (`link_select_server`) | `[ ]` | |
| tvp-10 | LOW | The remote's short jump is ±10 s; the desktop's arrows are ±5 s | `[ ]` | |
| tvp-11 | HIGH | Profile 8.1 to 5 conversion has no base-layer conversion | `[ ]` | |
| tvp-12 | HIGH | The LG's Dolby Vision indicator cannot prove the metadata reached the picture | `[ ]` | |
| tvp-13 | MEDIUM | B4 treats an encoder content-type value as HDMI passthrough | `[ ]` | |
| tvp-14 | HIGH | T0 builds a standalone core before T1 creates one | `[ ]` | |
| tvp-15 | MEDIUM | A physical Apple TV cannot reach the loopback-bound Plex TLS mock | `[ ]` | |
| tvp-16 | MEDIUM | `cargo test` from `src-tauri` can skip the extracted core's tests | `[ ]` | |
| tvp-17 | MEDIUM | The command guards misclassify shell-only commands and page-only features | `[ ]` | |
| tvp-18 | MEDIUM | One-way DTO decoding passes after a breaking contract change | `[ ]` | |
| tvp-19 | MEDIUM | No privacy manifest for `UserDefaults`, so the TestFlight upload is rejected | `[ ]` | |

### iPhone plan (`ios-port.md`)

| ID | Severity | Finding (one line) | Status | Commit |
|----|----------|--------------------|--------|--------|
| iph-1 | HIGH | Heat as a step-down trigger is not one of the owner's two approved triggers | `[ ]` | |
| iph-2 | HIGH | I0 and I1 need an iPhone build before I2 enables the iPhone family | `[ ]` | |
| iph-3 | HIGH | The phone's by-eye checks can pass with required formats broken | `[ ]` | |
| iph-4 | HIGH | "HDR from the iPhone 12 on" excludes HDR iPhones (XS, 11 Pro) | `[ ]` | |
| iph-5 | MEDIUM | I0 can pass without exercising the software decoder | `[ ]` | |
| iph-6 | MEDIUM | An injected memory warning cannot prove the build survives pressure | `[ ]` | |
| iph-7 | MEDIUM | "Every end-to-end scenario" is impossible on iPhone (`mpvargs`, full-page rect) | `[ ]` | |
| iph-8 | MEDIUM | iPad-only approvals are extended to the iPhone | `[ ]` | |
| iph-9 | MEDIUM | `dvh` does not give the area above the on-screen keyboard | `[ ]` | |
| iph-10 | MEDIUM | The width breakpoints miss landscape phones (932 x 430) | `[ ]` | |
| iph-11 | LOW | "A push" per slice contradicts the push-after-each-commit policy | `[ ]` | |

### iPadOS plan (`ipados-port.md`)

Pending its hunt.
