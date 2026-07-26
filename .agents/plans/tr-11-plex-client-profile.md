# Plan: `tr-11` Plex HLS client profile

## Status

**APPROVED 2026-07-26 — owner said “go” after reviewing revision 2.**

No product decision is outstanding. For this repair, Vela will identify its
universal HLS transcode requests with
`X-Plex-Client-Profile-Name=Web`. The owner's current Plex installation has
that HLS profile; the same request without a profile returns 400, while adding
only `Web` returns decision code 1001 (“Conversion OK”) and produces usable
HLS. Availability of a profile literally named `Web` on other Plex versions
and installations is an explicit, unverified portability assumption.

This plan fixes `tr-11` only. `tr-10` remains a separate finding and commit.
Decision-error observability and the duplicated universal-transcode query
builders are tracked separately as `tr-12` and `tr-13`; neither expands this
repair.

## Plan review

An owner-directed one-off `openreview claude` used Claude CLI defaults, with no
model or effort override. Claude Code 2.1.220 resolved
`claude-opus-5[1m]` and returned five schema-valid findings over exact range
`dbdbbdd78c1dd23fca0d53ef6274be40d5620e6a..2c85864d03fe743ed126830c414721a03af76459`
on 2026-07-26.

Revision 2 admits and resolves the plan-level findings by:

- correcting the canonical Plex provider contract rather than leaving the
  incomplete parameter list in place;
- proving that the now-successful capability decisions create no server-side
  transcode sessions before the playback baseline is taken;
- scoping `Web` to the one live-proven installation and recording the silent
  decision-error path as `tr-12`; and
- moving the header-form negative into the canonical finding and provider
  contract.

The duplicated query-builder risk is real but is not part of the narrow
`tr-11` repair; it is admitted separately as `tr-13`. Full provenance and
triage are in `.agents/review/findings/tr-11.md`.

## Goal

Make Vela's Plex decision and start requests select the same real HLS client
profile so Plex can offer and launch conversion.

Acceptance requires all of the following:

1. A tier decision carries `X-Plex-Client-Profile-Name=Web`.
2. The matching `start.m3u8` URL carries the identical key and value.
3. Every successful capability decision executed while the live scenario
   searches for a candidate creates no server-side transcode session.
4. The existing direct-play, split-file refusal, quality ladder, session
   ownership, teardown, and Automatic behavior remain unchanged.
5. `npm run e2e:live live-transcode` finds a tier, launches mpv on Plex's
   universal transcode endpoint, observes a new real server session, quits mpv,
   and observes that session disappear.

## Evidence and constraints

- Canonical finding: `.agents/review/findings/tr-11.md`.
- On 2026-07-26, the production decision query returned 400 for all 12 eligible
  live candidates. Plex logged “unable to find a matching profile.”
- Adding only `X-Plex-Client-Profile-Name=Web` made the decision return 200 /
  code 1001 and made the token-free master, child playlist, and segment return
  200, 200, and 206.
- The selector is a **query parameter**, not a request header. A live probe that
  supplied `X-Plex-Client-Profile-Name` as a header still returned 400.
- `Web` is live-proven only on the owner's current Plex installation. The
  repair deliberately assumes the same built-in profile exists elsewhere; it
  does not claim cross-version or cross-installation verification.
- Use one Rust constant for the production value. The guards must spell the
  expected wire value `Web` independently so changing the constant to another
  installed profile cannot move the tests with the implementation.
- Keep the repair narrow: do not add platform/device/model heuristics, custom
  profile XML, server-version branching, or profile discovery.
- Do not fix `tr-10` in this slice. In particular, leave the token's current
  transcode-query placement and `stream_headers` behavior unchanged so that
  finding keeps an isolated diff and guard proof.
- Current source version is 1.0.54. This shipped-code repair owns 1.0.55 through
  `scripts/bump.sh 1.0.55`.
- The live venue and its owner-approved controls are canonical in
  `.agents/machines.md`. The detached VM worktree must be clean before it moves.

## Implementation

### 1. Add the shared profile value

In `src-tauri/src/plex_library.rs`, define one private constant near the
universal-transcode builders:

```rust
const PLEX_HLS_CLIENT_PROFILE: &str = "Web";
```

Use that constant in both requests below. Do not introduce a new config field or
provider-neutral abstraction: this is a Plex protocol requirement, not a user
setting.

### 2. Repair the decision request

In `PlexLibrary::transcode_decision`, append
`X-Plex-Client-Profile-Name=Web` to the query vector passed to reqwest.

Do not change the existing token and client-identifier headers, delivery flags,
tier bounds, media index, part index, session id, or response parsing.

### 3. Repair the start request

In `PlexLibrary::transcode_url`, append the same
`X-Plex-Client-Profile-Name=Web` pair to the encoded query.

Do not change the URL shape or session generation. Do not remove
`X-Plex-Token`; that is `tr-10`.

### 4. Add two independent guards

Add both guards beside the existing Plex transcode tests in
`src-tauri/src/plex_library.rs`.

1. `transcode_decision_sends_the_hls_client_profile`
   - use the existing loopback request-capture helper;
   - return a valid decision XML response;
   - call the production `transcode_decision`;
   - inspect the captured request target and assert the exact query pair
     `X-Plex-Client-Profile-Name=Web`.
2. `transcode_start_uses_the_same_hls_client_profile`
   - call the production `transcode_url`;
   - parse the resulting query with `url::Url`;
   - assert exactly one profile pair and the exact `Web` value;
   - keep the expected `Web` literal independent of
     `PLEX_HLS_CLIENT_PROFILE`.

The tests must reach the two production builders separately. A helper-only test
is insufficient because it would not prove that either caller uses the value.

### 5. Correct and strengthen the live scenario

In `tests/e2e/live/transcode.mjs`:

1. change the stale leading invocation from `npm run e2e:live transcode` to
   `npm run e2e:live live-transcode`;
2. snapshot `/transcode/sessions` immediately before the candidate loop;
3. after the candidate decisions finish, observe the session list for a short,
   bounded window and assert that none of the candidate decisions added a new
   session; and
4. only after that assertion, take the existing fresh baseline used to identify
   the session opened by playback.

The pre-existing 400 responses made this side effect unreachable, so the
successful post-repair behavior must be established against the real server.
If a decision creates a session, stop the slice before playback and record a
new finding; do not silently absorb session retention or teardown into
`tr-11`, and do not delete any session that cannot be proven to belong to this
scenario.

### 6. Bump once

Run `scripts/bump.sh 1.0.55` after the code and guards are in place. Accept only
the version-surface set owned by that script.

## Verification

### Targeted green checks

From `src-tauri/`:

```text
cargo +stable test --locked transcode_decision_sends_the_hls_client_profile
cargo +stable test --locked transcode_start_uses_the_same_hls_client_profile
```

Run both together once as well, using a shared filter if their final names
permit it.

### Canonical verification

Run the complete command set from `.agents/repo-guidance.md` because the version
bump touches both Rust and frontend-visible version surfaces:

```text
node scripts/check-js-toolchain.mjs
npm ci
npm audit
npm run check
npm run build
cd src-tauri
cargo +1.89.0 check --locked
cargo +stable check --locked
cargo +stable clippy --all-targets --locked -- -D warnings
cargo +stable test --locked
cargo audit --file Cargo.lock
```

Run the command set above before the implementation commit. The Linux E2E entry
point is deferred until the exact committed SHA is aligned to the VM under
**Venue and live proof** below. Canonical verification is not complete until
that suite also passes; do not substitute a macOS browser run.

### Commit and red-proof

Commit the 1.0.55 implementation before injecting regressions; the commit must
contain only `tr-11`.

Then prove the two guards independently:

1. remove the profile pair from `transcode_decision` only; its targeted test
   must fail because the captured decision request lacks the pair;
2. restore the committed file and confirm a clean worktree;
3. remove the profile pair from `transcode_url` only; its targeted test must
   fail because the start URL lacks the pair;
4. restore the committed file and confirm a clean worktree;
5. change the shared production value from `Web` to `Plex Desktop`; both
   targeted tests must fail because the wire value is no longer the live-proven
   HLS profile;
6. restore the committed file, confirm a clean worktree, and rerun both targeted
   tests green.

Do not combine the regressions. Preserve the failing output in the finding's
guard-proof record.

### Venue and live proof

Align the clean detached `~/dev/vela-main` worktree to the exact implementation
commit. If that commit is not pushed, transfer it with a temporary git bundle;
do not copy individual source files and do not push without the separate push
approval required by `.agents/push-policy.md`.

On the VM, through its login shell, run the full hermetic suite:

```text
cd ~/dev/vela-main && npm run e2e
```

Then, from the macOS repo root, run the live scenario:

```text
npm run e2e:live live-transcode
```

Require all existing scenario assertions to pass. Afterward:

- confirm the candidate decision probes added no server-side session;
- confirm the VM worktree is clean and at the implementation commit;
- confirm Plex and `plex-watchdog.timer` are active; and
- confirm the scenario's session is absent without disturbing unrelated active
  sessions.

The existing 1.0.54 failure is the live red proof.

## Review and closeout

After local, hermetic, guard, and live verification:

1. run the repo's finding-specific `codereview codex` workflow over the
   implementation commit;
2. if it returns an actionable finding, record and address exactly that finding
   in its own commit before proceeding;
3. update `.agents/review/findings/tr-11.md`, `.agents/review/index.md`,
   `.agents/plans/server-transcoding.md`, and `.agents/state.md` with the commit,
   guard proof, live result, and review verdict;
4. commit that record-only closeout immediately; and
5. leave `tr-10`, `tr-12`, and `tr-13` open; `tr-10` remains next.

Do not push any commit without a separate explicit go.

## Expected files

- `src-tauri/src/plex_library.rs`
- `tests/e2e/live/transcode.mjs`
- version surfaces maintained by `scripts/bump.sh`
- `.agents/review/findings/tr-11.md`
- `.agents/review/index.md`
- `.agents/plans/server-transcoding.md`
- `.agents/state.md`

No other production or test file is in scope.
