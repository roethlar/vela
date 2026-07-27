# Plan: `tr-12` Plex decision diagnostics

## Status

**Revision 1 APPROVED and ACTIVATED by the owner 2026-07-26. Implementation is
in progress and owns Vela 1.0.57.**

The approved behavior is:

- when the on-demand quality submenu cannot complete a Plex decision request,
  use its existing inline error surface to show a credential-safe reason instead
  of claiming that the server refused conversion;
- when an explicit tier play encounters the same failure, keep the existing
  fail-closed fallback to Original and write the credential-safe reason once to
  Vela's log rather than failing a play that can still succeed; and
- keep a valid Plex negative decision quiet and render the existing
  “This server won't convert this title” state.

The ruling is recorded in `.agents/decisions.md`. The owner supplied the
separate implementation go on 2026-07-26.

This plan fixes `tr-12` only. It does not refactor the duplicated Plex
universal-transcode query builders (`tr-13`).

## Goal

Preserve the safe distinction between a valid Plex conversion refusal and a
failed capability request without exposing a server address, request URL,
rating key, session id, token, or response body.

Acceptance requires all of the following:

1. Plex decision transport, HTTP-status, response-read, XML-parse, and
   semantically incomplete-response failures become typed, credential-safe
   errors at `PlexLibrary::transcode_decision`.
2. The quality submenu surfaces that safe error through its existing inline
   alert rather than returning an Original-only capability result.
3. An explicit tier play still degrades to Original after a decision failure,
   and its log distinguishes the failed request from a valid server refusal.
4. A valid negative decision remains an ordinary, quiet capability refusal.
5. A successful decision remains unchanged and still permits the selected tier.
6. Split-file refusal, direct play, tier resolution, profile selection, header
   authentication, session ownership, teardown, and Automatic behavior remain
   unchanged.

## Evidence and constraints

- Canonical finding: `.agents/review/findings/tr-12.md`.
- `PlexSource::playback_options` currently maps every
  `transcode_decision` error to `false` with `.unwrap_or(false)`. The frontend
  therefore receives an ordinary Original-only result and renders “This server
  won't convert this title.”
- `PlexSource::resolve_stream_version` has a second `.unwrap_or(false)`. It
  falls back safely, but its only log is the same generic message emitted for a
  valid refusal.
- `PlexLibrary::transcode_decision` currently returns
  `Box<dyn std::error::Error>` from a chained reqwest/XML path. A caller must
  not render that error directly: reqwest's `Display` includes the full request
  URL, including the server address, rating key, and decision session id.
- `src/routes/+page.svelte` already catches a rejected `quality_options`
  command and renders `friendlyError(qualityError)` in a `role="alert"`.
  No production frontend change is expected. The backend error must still be
  intrinsically safe; the frontend redaction regex is defense in depth, not the
  security boundary.
- A parsed `DecisionContainer` with no `generalDecisionCode` is fail-closed
  today but indistinguishable from a real refusal. Production decision parsing
  must treat a missing code as an invalid response while leaving the raw serde
  struct's conservative `conversion_ok()` behavior intact.
- The current source version is 1.0.56. This shipped-code repair owns 1.0.57
  through `scripts/bump.sh 1.0.57`.
- The live venue and its owner-approved controls are canonical in
  `.agents/machines.md`. The detached `~/dev/vela-main` worktree must be clean
  before it moves.

## Implementation

### 1. Add a payload-free Plex decision error

In `src-tauri/src/plex_library.rs`, add a public error type beside
`DecisionContainer`:

```text
PlexDecisionError
  NoServer
  TimedOut
  Unreachable
  RequestFailed
  HttpStatus(u16)
  BodyRead
  InvalidResponse
```

Implement `Display` and `std::error::Error`. The display strings are the
complete diagnostic contract:

- `Plex could not check conversion because no server is available.`
- `Plex could not check conversion because the request timed out.`
- `Plex could not check conversion because the server could not be reached.`
- `Plex could not check conversion because the request failed.`
- `Plex could not check conversion because the server returned HTTP <code>.`
- `Plex could not check conversion because the response could not be read.`
- `Plex could not check conversion because the response was invalid.`

The type may retain only enum discriminants and the numeric HTTP status. It
must never own or borrow a reqwest error, URL, host, rating key, session id,
token, response body, serde error, or server reason phrase.

Classify transport errors from reqwest predicates only:

- `is_timeout()` -> `TimedOut`;
- `is_connect()` -> `Unreachable`; and
- every other send failure -> `RequestFailed`.

Do not reuse an error's `Display` or `Debug` representation.

### 2. Classify every decision-response stage before parsing

Change `PlexLibrary::transcode_decision` to return
`Result<DecisionContainer, PlexDecisionError>`.

Keep its request URL, token/client headers, `Web` profile query, media and part
indexes, delivery flags, tier constraints, session, and Accept header
unchanged.

Replace the chained `send` / `error_for_status` / `text` / XML parse with
explicit stages:

1. map a missing selected server to `NoServer`;
2. map `.send().await` through the transport classification above;
3. inspect `response.status()` and return `HttpStatus(status.as_u16())` for
   every non-success status without reading or retaining its body;
4. map `.text().await` failure to `BodyRead`;
5. map `serde_xml_rs::from_str` failure to `InvalidResponse`; and
6. after parsing, return `InvalidResponse` when
   `general_decision_code` is absent.

A present 1xxx code remains permission. Any other present code remains a valid
negative decision. Do not log in `PlexLibrary`: callers own whether a failed
probe becomes a UI error or a playback fallback.

### 3. Surface a failed quality-menu probe

In `PlexSource::playback_options` in `src-tauri/src/source/plex.rs`, preserve
the split-file early refusal and the no-tier case exactly as they are.

For the actual decision request:

- `Ok(decision)` uses `decision.conversion_ok()` as today;
- `Err(error)` returns `Err(error.to_string())` from `playback_options`.

That error already crosses `quality_options` into the existing
`qualityError`/`role="alert"` branch. Do not add a toast, modal, retry loop, or
new DTO field. A user can close and reopen the submenu to retry, while the
ordinary Play/Resume action remains available.

A valid negative decision still returns `PlaybackOptions` with no transcode
tiers and therefore retains the existing quiet refusal copy.

### 4. Diagnose an explicit-tier fallback without failing playback

In `PlexSource::resolve_stream_version`, replace the error-collapsing
`.unwrap_or(false)` with an explicit match and retain an optional
`PlexDecisionError` for the fallback branch.

The fallback log precedence must be mutually exclusive:

1. split-file refusal keeps its current split-file explanation;
2. a failed decision emits the safe error followed by
   `Playing the original.` exactly once; and
3. a valid negative decision keeps the generic
   `plex: conversion unavailable for this copy; playing the original`.

All three return `Delivery::Original`, use the existing header-authenticated
part URL, and run the existing direct-play preflight. Do not return the
decision error from `resolve_stream_version`, because that would turn a safe
fallback into a failed play.

### 5. Add independent guards

#### 5.1 Guard the decision boundary

Add loopback tests beside the existing decision tests in
`src-tauri/src/plex_library.rs`:

1. `plex_decision_transport_error_is_sanitized`
   - use an unreachable loopback endpoint and distinctive rating/session
     sentinels;
   - require the appropriate safe transport variant and exact display text;
   - assert that the display contains no address, rating key, session, or
     synthetic token.
2. `plex_decision_http_error_is_sanitized`
   - answer 503 with a syntactically valid negative-decision body carrying a
     body sentinel;
   - require `HttpStatus(503)` and the exact safe display text;
   - assert that neither body nor request sentinels appear.
3. `plex_decision_malformed_body_is_sanitized`
   - answer 200 with malformed XML containing a distinctive sentinel;
   - require `InvalidResponse` and the exact safe display text;
   - assert that the response sentinel does not appear.
4. `plex_decision_missing_code_is_invalid`
   - answer 200 with `<MediaContainer/>`;
   - require `InvalidResponse`, proving semantic unreadability does not become
     a quiet refusal.
5. `plex_decision_valid_refusal_is_quiet`
   - answer 200 with a present 2xxx `generalDecisionCode`;
   - require `Ok` and `conversion_ok() == false`, not an error.

Keep or update the existing success/profile guard so a 1xxx decision remains
`Ok` and the real request still carries `Web`.

#### 5.2 Guard both source call sites

Generalize the existing sequential transcode-resolution fixture in
`src-tauri/src/source/plex.rs` only enough to choose the decision status/body
and to answer the direct-part HEAD used after a fallback.

Add:

1. `plex_quality_options_surface_http_decision_failure`;
2. `plex_quality_options_surface_malformed_decision_failure`; and
3. `plex_quality_options_keep_a_valid_refusal_quiet`.

The first two call the real `MediaSource::playback_options` and require the
exact safe error. The third requires an ordinary options result with no tiers.
All three must prove the fixture received the real universal-decision request.

Add `plex_tier_play_falls_back_after_decision_failure`, which calls the real
`resolve_stream_version` with a malformed decision response and asserts:

- delivery is `Original`;
- the resolved URL is the original part URL, not `start.m3u8`;
- the token remains in exactly one private HTTP header and not in the URL; and
- no transcode session handle is returned.

#### 5.3 Guard the reporting wiring and existing UI surface

In `tests/transcoding-ui.test.mjs`, add one source-wiring guard that requires:

- `playback_options` to propagate a decision error rather than use
  `.unwrap_or(false)`;
- the explicit-tier path to retain and print the safe decision error before
  fallback;
- the valid-refusal generic message to remain in a distinct branch; and
- the existing quality-menu catch to assign `qualityError`, with the submenu
  rendering it through `friendlyError` in a `role="alert"`.

This source guard supplements the Rust behavior tests. Its purpose is to catch
the otherwise unobservable deletion of the explicit fallback log call site;
it is not the authority for HTTP classification or credential safety.

#### 5.4 Guard the user-visible boundary in the real app

Extend `tests/e2e/mockplex.mjs` with an optional canned decision response.
Handle `/video/:/transcode/universal/decision` as an authenticated Plex route,
record it without secrets, and default to a valid 2xxx refusal when no override
is supplied.

Add `tests/e2e/scenarios/plexdecision.mjs`:

- seed one TLS mock Plex source whose decision endpoint returns HTTP 503 with a
  body sentinel;
- open its Movies library and the single-copy `Play at Quality` submenu;
- require an inline alert containing exactly the safe HTTP-status diagnostic;
- require that the alert contains no token, address/port, machine id, rating
  key, session id, response body, or reqwest-style URL;
- require that the ordinary Play/Resume action remains available;
- require the mock to have received the authenticated, token-free-query
  universal-decision request; and
- save a screenshot of the alert state.

The scenario must not start mpv or a server-side transcode.

### 6. Bump once

Run `scripts/bump.sh 1.0.57` after production code and guards are in place.
Accept only the version surfaces owned by that script.

## Verification

### Targeted green checks

From `src-tauri/`:

```text
cargo +stable test --locked plex_decision_
cargo +stable test --locked plex_quality_options_
cargo +stable test --locked plex_tier_play_falls_back_after_decision_failure
```

From the repo root:

```text
node --test --test-name-pattern="Plex decision failures" tests/transcoding-ui.test.mjs
```

The filters must execute every named guard; inspect the test counts rather than
accepting a zero-test success.

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
exact committed SHA is aligned to the VM. Canonical verification is not
complete until the full Linux suite passes.

### Commit and red-proof

Commit the 1.0.57 implementation before injecting regressions. The commit must
contain only `tr-12`, its guards, the mock scenario, and script-owned version
surfaces.

Prove each behavior independently, restoring each target from the committed
implementation and confirming a clean worktree between mutations:

1. bypass the non-success HTTP-status return so the 503 fixture's valid XML is
   read as a quiet refusal; require `plex_decision_http_error_is_sanitized` to
   fail for the status/refusal distinction;
2. restore, then route transport failure through a temporary raw
   `error.to_string()` variant; require
   `plex_decision_transport_error_is_sanitized` to fail because request
   context appears;
3. restore, then make malformed XML collapse to a default decision and bypass
   the missing-code validation; require
   `plex_decision_malformed_body_is_sanitized` to fail because the malformed
   response became a quiet refusal;
4. restore, then remove only the missing-code validation; require
   `plex_decision_missing_code_is_invalid` to fail;
5. restore, then classify every non-1xxx decision as `InvalidResponse`; require
   `plex_decision_valid_refusal_is_quiet` to fail;
6. restore, then reintroduce `.unwrap_or(false)` only in
   `playback_options`; require both quality-option failure tests and the Node
   propagation guard to fail while the valid-refusal test remains green;
7. carry that same menu-collapse mutation to the clean Linux venue and run
   only `plexdecision`; require the scenario to fail because it sees the
   ordinary refusal copy instead of an alert;
8. restore the committed implementation on both machines and rerun those
   guards green;
9. reintroduce `.unwrap_or(false)` and the generic-only fallback only in
   `resolve_stream_version`; require the Node reporting-wiring guard to fail;
10. restore, confirm a clean worktree, and rerun all targeted Rust and Node
    guards green.

Do not combine unrelated mutations. Preserve only credential-safe failing
output in the finding record. The sentinels must be synthetic.

### Venue and live proof

Align the clean detached `~/dev/vela-main` worktree to the exact implementation
commit. If that commit is not pushed, transfer it with a temporary git bundle;
do not copy individual source files and do not push without the separate
approval required by `.agents/push-policy.md`.

On the VM, through its login shell, run the focused scenario and then the full
hermetic suite:

```text
cd ~/dev/vela-main && npm run e2e -- plexdecision
cd ~/dev/vela-main && npm run e2e
```

Inspect the focused scenario screenshot for the inline, non-overflowing alert.

Then, from the macOS repo root, run:

```text
npm run e2e:live live-transcode
```

The live run is a positive regression check only. Do not deliberately break the
owner's Plex profile or send malformed traffic to create a live red case.
Require the successful decision, play, and teardown assertions to remain green.

Afterward:

- confirm the VM worktree is clean and at the implementation commit;
- confirm the focused mock created no transcode session or mpv residue;
- confirm the real scenario's decision probes created no session;
- confirm its play opened and then removed only its own session; and
- confirm Plex and `plex-watchdog.timer` remain active without disturbing any
  unrelated session.

## Review and closeout

After local, guard, hermetic, and live verification:

1. run the finding-specific `codereview` workflow over the implementation
   commit with the **Claude** harness, as explicitly directed by the owner;
2. pass no model or effort override, and record the model/effort actually
   resolved from the dispatch transcript rather than predicting it;
3. if Claude returns an actionable finding, record and address exactly that
   finding in its own commit, rerun affected and canonical verification, and
   redispatch a fresh Claude review without a model/effort override;
4. update `.agents/review/findings/tr-12.md`, `.agents/review/index.md`,
   `.agents/plans/server-transcoding.md`, this plan, `.agents/state.md`, and
   `.agents/machines.md` with the implementation and correction commits,
   independent guard proof, Linux/live result, venue state, and final review
   verdict;
5. commit that record-only closeout immediately; and
6. leave `tr-13` open and separately gated.

This owner-directed Claude review is a one-off for `tr-12`; it does not change
the repository's standing reviewer routing. Do not push any commit without a
separate explicit go.

## Expected files

- `src-tauri/src/plex_library.rs`
- `src-tauri/src/source/plex.rs`
- `tests/transcoding-ui.test.mjs`
- `tests/e2e/mockplex.mjs`
- `tests/e2e/scenarios/plexdecision.mjs`
- version surfaces maintained by `scripts/bump.sh`
- `.agents/review/findings/tr-12.md`
- `.agents/review/index.md`
- `.agents/decisions.md`
- `.agents/plans/server-transcoding.md`
- `.agents/plans/tr-12-plex-decision-diagnostics.md`
- `.agents/state.md`
- `.agents/machines.md`

No other production or test file is in scope.

## Explicit non-goals

- No `tr-13` shared universal-transcode query builder or query refactor.
- No Plex profile discovery, profile fallback list, custom client profile,
  platform/device heuristic, or server-version branching.
- No retry or backoff for capability decisions.
- No popup, modal, toast, persistent warning, telemetry, or new frontend DTO.
- No change to Jellyfin or Emby capability/error behavior.
- No rendering of raw reqwest, serde, URL, server-response, token, rating-key,
  or session data.
- No change to the quality ladder, selected-version targeting, split-file
  refusal, direct-play preflight, transcode URL, token header, session
  generation, teardown, resume semantics, or Automatic behavior.
- No deliberate failure probe against the owner's real Plex server.
