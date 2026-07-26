# Plan: `tr-10` Plex transcode header authentication

## Status

**APPROVED 2026-07-26.** The owner authorized implementation and directed the
completed code change to the Claude reviewer harness with no model or effort
override. No push is authorized.

No product decision is outstanding. The active 2026-07-03 and 2026-07-23
credential decisions already require every Plex media URL handed to mpv to be
credential-free and require Plex authentication to use the private mpv header
include.

This plan fixes `tr-10` only. It does not change Plex capability-decision error
reporting (`tr-12`) or refactor the duplicated universal-transcode query
builders (`tr-13`).

## Goal

Make a Plex tier play authenticate every HLS request through the existing
private `X-Plex-Token` header include while the transcode URL, mpv's `path`, and
mpv's argv remain free of both the token parameter and the token value.

Acceptance requires all of the following:

1. `PlexLibrary::transcode_url` emits no `X-Plex-Token` query pair and no active
   token value under any query key.
2. A successful Plex transcode resolution returns exactly one
   `X-Plex-Token` entry in `StreamResolution.http_headers`.
3. Direct play and a tier request use the same header-authentication boundary;
   direct-play preflight behavior remains unchanged.
4. During a real tier play, mpv reports a token-free universal-transcode path,
   its process arguments contain no Plex credential, and its final Vela-owned
   include is a regular owner-only file containing the one expected token
   header.
5. The real HLS play opens a Plex transcode session and quitting mpv removes
   that session, proving header authentication works through delivery rather
   than only at URL construction.
6. The existing profile, capability-decision, selected-version, split-file
   refusal, resume offset, session ownership, and teardown behavior remain
   unchanged.

## Evidence and constraints

- Canonical finding: `.agents/review/findings/tr-10.md`.
- `PlexLibrary::transcode_url` currently appends
  `("X-Plex-Token", self.auth_token.clone())` to the start query.
- `PlexSource::resolve_stream_version` currently initializes
  `stream_headers` empty and populates it only in the direct-play/fallback
  branch. A successful transcode therefore returns empty
  `StreamResolution.http_headers`.
- `commands.rs` already moves `StreamResolution.http_headers` into
  `PlaySpec.http_headers`, and `playback.rs` already writes non-empty headers
  to a per-launch 0600 include asserted after user options. Neither layer needs
  a production change.
- The source-text assertion named “Plex credentials never enter query strings
  or frontend artwork URLs” in `tests/config-integrity.test.mjs` misses the
  current tuple-built query. Its existing `insert`/`append_pair` and inline
  `.query(&[(...)])` negatives stayed green while `tr-10` shipped.
- The stale `transcode_url` doc comment says an initial header would not travel
  to generated HLS requests. The owner's Plex disproved that premise on
  2026-07-26: a token-free master playlist and child playlist accepted
  `X-Plex-Token` headers with 200 responses, a token-free segment accepted the
  header with 206, neither child URI contained a token, and teardown returned
  204 with no probe session left.
- The current source version is 1.0.55. This shipped-code repair owns 1.0.56
  through `scripts/bump.sh 1.0.56`.
- The live venue and its owner-approved controls are canonical in
  `.agents/machines.md`. The detached `~/dev/vela-main` worktree must be clean
  before it moves.
- Test failures must never print the real token, the private include contents,
  or the credential-adjacent include path. Security assertions over those
  values must reduce to booleans with fixed safe messages.

## Implementation

### 1. Make the Plex transcode URL credential-free

In `src-tauri/src/plex_library.rs`, remove only the `X-Plex-Token` pair from
the query assembled by `PlexLibrary::transcode_url`.

Keep all non-credential request fields unchanged, including:

- `X-Plex-Client-Identifier`;
- `X-Plex-Client-Profile-Name=Web`;
- media and part indexes;
- direct-play/direct-stream flags;
- tier bitrate and resolution;
- offset, `fastSeek`, and `copyts`; and
- the client-generated session id returned with the URL.

Correct the method's doc comment: the returned URL is credential-free, and its
caller must carry `X-Plex-Token` through `StreamResolution.http_headers`; the
header-only master/child/segment chain is live-proven. Do not share or
otherwise refactor the decision/start query builders in this slice.

### 2. Put Plex stream authentication outside the delivery branch

In `PlexSource::resolve_stream_version` in
`src-tauri/src/source/plex.rs`, construct the one-element
`stream_headers` vector from `lib.auth_token_clone()` before choosing direct
play versus transcode. Return that same vector for both delivery modes.

Keep `preflight_plex_stream` inside the direct-play/fallback branch. The
transcode branch remains verified by mpv and the live HLS scenario; this repair
does not add another production request. A failed capability decision or a
split-file refusal still falls back to the original and still receives the same
header.

Do not add `X-Plex-Client-Identifier` to the private include, change
`StreamResolution`, change `PlaySpec`, or alter the include-file implementation.

### 3. Add independent credential guards

#### 3.1 Guard the production start URL

Add `plex_transcode_auth_url_is_credential_free` beside the existing
`transcode_url` tests in `src-tauri/src/plex_library.rs`.

The test must call the real `PlexLibrary::transcode_url` with a distinctive
synthetic active token, parse the resulting URL with `url::Url`, and assert:

- no decoded query key equals `X-Plex-Token` case-insensitively;
- neither any decoded query key nor any decoded value contains the synthetic
  token; and
- the serialized URL does not contain the synthetic token.

Keep this separate from the session/tier/profile test so the credential
boundary has its own failure.

#### 3.2 Guard the source-to-playback handoff

Add `plex_transcode_auth_reaches_stream_resolution` in the
`src-tauri/src/source/plex.rs` test module. Use a small sequential loopback Plex
fixture that:

- requires the synthetic Plex token header on its metadata, resume, and
  decision requests;
- returns one single-part media version with a stable `Media` id;
- returns a valid resume/detail document; and
- returns `generalDecisionCode=1001` for the tier decision.

Construct a `PlexSource` with that already-selected, machine-pinned loopback
server, call the real `MediaSource::resolve_stream_version` for a named tier and
the fixture's exact media-version id, and assert:

- delivery is `Delivery::Transcode` at that tier;
- the URL is the Plex universal-transcode endpoint;
- a teardown session handle exists; and
- `http_headers` is exactly one pair:
  `X-Plex-Token: <synthetic token>`.

This guard must reach the production resolver. A helper-only assertion is
insufficient because the defect was branch wiring, not header construction.

#### 3.3 Close the source-text guard's demonstrated hole

Strengthen the existing Plex credential test in
`tests/config-integrity.test.mjs` with a negative that matches the current
Rust query-tuple form: an `X-Plex-Token` tuple whose value is
`self.auth_token` or its clone. Keep the behavioral Rust URL guard above as the
authority for aliases or differently named query keys; the source-text
negative exists to ensure the exact syntax that escaped the credential sweep
cannot return unnoticed.

### 4. Extend the real-mpv credential proof

In `tests/e2e/live/transcode.mjs`:

1. remove comments that describe a token-bearing transcode URL as expected;
2. after mpv publishes `path`, parse it and require the universal-transcode
   endpoint while checking, through safe booleans, that neither the
   `X-Plex-Token` key nor the active token value appears anywhere in the path;
3. locate the one mpv process whose `/proc/<pid>/cmdline` contains the scenario's
   unique `--input-ipc-server=<socketPath>` argument;
4. require no argv element to contain the active token or an
   `X-Plex-Token` query pair;
5. take the last `--include=` argument, without logging its path, and require
   the target to be a regular 0600 file whose contents equal exactly one
   `http-header-fields="X-Plex-Token: …"` line; and
6. retain the existing real-session open/quit/absent proof.

Use `assert.ok(<safe boolean>, <fixed message>)` for credential-bearing
comparisons. Do not pass the path, argv, expected header line, actual include
contents, or token to an assertion API that prints actual and expected values
on failure.

The scenario is Linux-only already, so `/proc` inspection does not add a
cross-platform production or test requirement.

### 5. Bump once

Run `scripts/bump.sh 1.0.56` after the code and guards are in place. Accept only
the version surfaces owned by that script.

## Verification

### Targeted green checks

From the repo root:

```text
node --test --test-name-pattern="Plex credentials never enter query strings or frontend artwork URLs" tests/config-integrity.test.mjs
```

From `src-tauri/`:

```text
cargo +stable test --locked plex_transcode_auth_url_is_credential_free
cargo +stable test --locked plex_transcode_auth_reaches_stream_resolution
cargo +stable test --locked plex_transcode_auth
```

The shared filter must execute both Rust guards.

### Canonical verification

Run the complete command set from `.agents/repo-guidance.md` because the
version bump touches Rust and frontend-visible version surfaces:

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

Run these before the implementation commit. Linux E2E is deferred until the
exact committed SHA is aligned to the VM under **Venue and live proof**.
Canonical verification is not complete until that suite passes; do not
substitute a macOS browser run.

### Commit and red-proof

Commit the 1.0.56 implementation before injecting regressions. The commit must
contain only `tr-10`, its guards, the live assertion, and script-owned version
surfaces.

Then prove the guards independently:

1. add the current vulnerable
   `("X-Plex-Token", self.auth_token.clone())` pair back to
   `transcode_url`; run only
   `plex_transcode_auth_url_is_credential_free` and require failure because the
   active token appears in the parsed production URL;
2. restore the committed file and confirm a clean worktree;
3. add that same tuple back again; run only the targeted Node credential test
   and require failure because the tuple-built query is now detected;
4. restore the committed file and confirm a clean worktree;
5. move the `stream_headers` construction back into the direct-play-only
   branch, leaving the transcode URL credential-free; run only
   `plex_transcode_auth_reaches_stream_resolution` and require failure because
   the transcoded resolution has no token header;
6. restore the committed file, confirm a clean worktree, and rerun all three
   targeted guards green.

Do not combine mutations. Preserve only credential-safe failing output in the
finding record; synthetic tokens are mandatory for every hermetic proof.

The live scenario is acceptance evidence, not the primary mutation guard. Do
not deliberately put the owner's real token back into mpv argv to red-proof the
live assertion; the independently mutated URL, source-wiring, and source-text
guards cover its two security claims without recreating the real exposure.

### Venue and live proof

Align the clean detached `~/dev/vela-main` worktree to the exact implementation
commit. If that commit is not pushed, transfer it with a temporary git bundle;
do not copy individual source files and do not push without the separate
approval required by `.agents/push-policy.md`.

On the VM, through its login shell, run the full hermetic suite:

```text
cd ~/dev/vela-main && npm run e2e
```

Then, from the macOS repo root, run:

```text
npm run e2e:live live-transcode
```

Require all existing and new assertions to pass. Afterward:

- confirm mpv's path and argv carried no token key or active token value;
- confirm the last Vela include was regular, 0600, and carried exactly the one
  expected header while mpv was alive;
- confirm candidate capability decisions added no server-side session;
- confirm the play opened a real transcode session and quitting mpv removed it;
- confirm the VM worktree is clean and at the implementation commit;
- confirm no mpv process or socket residue remains; and
- confirm Plex and `plex-watchdog.timer` are active without disturbing any
  unrelated session.

## Review and closeout

After local, hermetic, guard, and live verification:

1. run the repo's finding-specific `codereview` workflow over the
   implementation commit with the Claude harness, passing no model or effort
   override, as explicitly directed by the owner;
2. if it returns an actionable finding, record and address exactly that finding
   in its own commit before proceeding;
3. update `.agents/review/findings/tr-10.md`, `.agents/review/index.md`,
   `.agents/plans/server-transcoding.md`, `.agents/state.md`, and
   `.agents/machines.md` with the implementation commit, independent guard
   proof, live result, venue state, and review verdict;
4. commit that record-only closeout immediately; and
5. leave `tr-12` and `tr-13` open and separately gated.

Do not push any commit without a separate explicit go.

## Expected files

- `src-tauri/src/plex_library.rs`
- `src-tauri/src/source/plex.rs`
- `tests/config-integrity.test.mjs`
- `tests/e2e/live/transcode.mjs`
- version surfaces maintained by `scripts/bump.sh`
- `.agents/review/findings/tr-10.md`
- `.agents/review/index.md`
- `.agents/plans/server-transcoding.md`
- `.agents/state.md`
- `.agents/machines.md`

No other production or test file is in scope.

## Explicit non-goals

- No `tr-12` decision-error diagnostic or user-facing error change.
- No `tr-13` shared universal-transcode query builder or broader refactor.
- No Jellyfin or Emby credential-boundary change.
- No proxying, rewriting, or downloading HLS through Vela.
- No change to the `Web` client profile, tier policy, Automatic behavior,
  selected-version targeting, split-file refusal, resume semantics, session
  generation, ownership, or teardown.
- No change to the private include format, lifetime, permissions, mpv option
  ordering, or process-supervision design.
- No credential vault, encryption, logging, telemetry, or frontend exposure.
