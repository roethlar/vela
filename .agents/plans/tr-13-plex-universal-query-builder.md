# Plan: `tr-13` shared Plex universal-transcode query contract

## Status

**Revision 1 APPROVED; IMPLEMENTATION LANDED; VERIFICATION BLOCKED
2026-07-27.**

The owner approved Revision 1 and supplied the implementation go on 2026-07-26.
No product decision is outstanding: this is a Plex-private refactor that must
preserve the current decision and `start.m3u8` wire contract.

This plan fixes `tr-13` only. It does not change the behavior closed under
`tr-10`, `tr-11`, or `tr-12`.

Implementation landed in 1.0.58 at `81d5497`. The complete local gate and all
six independent guard mutations passed. The required clean Linux 39/39 gate is
not yet available because unchanged, unrelated E2E scenarios repeatedly hit
pre-existing harness races:

- the first full run reached 34/39; `plexdecision` passed, four scenarios
  received `mpv: property unavailable`, and `refresh` lost its mock hold
  window (`/Users/michael/.ptk/jobs/job-7397-49.log`);
- the unchanged five-scenario retry passed 5/5
  (`/Users/michael/.ptk/jobs/job-7397-50.log`);
- a second full run reached 20 pass / 4 fail before `refresh` held one
  WebDriver call for 38 minutes and was terminated through the runner's
  SIGTERM cleanup path (`/Users/michael/.ptk/jobs/job-7397-51.log`);
- after a graceful VM reboot, the four prior mpv/WebDriver failures passed,
  while `refresh` failed the same 15-second listing assertion both in the
  five-scenario pass and alone
  (`/Users/michael/.ptk/jobs/job-7397-56.log`,
  `/Users/michael/.ptk/jobs/job-7397-57.log`).

That repeated blocker crossed the repo's three-cycle stall threshold. Do not
waive the 39/39 requirement or proceed to live Plex / finding review. The next
action needs owner approval for a separate narrow plan to stabilize the
pre-existing E2E races, after which this plan resumes at the full Linux gate.

## Goal

Represent the common Plex universal-transcode query exactly once, with the
legitimate decision/start differences expressed as typed inputs, so a future
parameter change cannot silently make the capability decision describe a
different delivery from the stream Vela starts.

Acceptance requires all of the following:

1. `PlexLibrary::transcode_decision` and `PlexLibrary::transcode_url` obtain
   their universal-transcode query pairs from one private production builder.
2. A tier decision and its corresponding start URL carry the same exact,
   duplicate-sensitive common query multiset after only the documented
   endpoint differences are normalized.
3. A direct-play decision retains `directPlay=1`, `directStream=1`, no tier
   bounds, and offset zero; a tier decision/start retains `0`, `0`, and both
   tier bounds.
4. The current endpoint, offset, session, client-identity, Accept-header, and
   token-header differences remain explicit and unchanged.
5. The shared builder cannot accept a Plex token. No token enters either query
   or the credential-free start URL.
6. Both production callers are guarded through their real serialized wire
   output, and a source-wiring guard proves the common contract is represented
   once rather than duplicated in two currently matching vectors.
7. Direct play, selected-version targeting, split-file refusal, the `Web`
   profile, decision diagnostics, session ownership, header authentication,
   teardown, resume, the quality ladder, and Automatic behavior remain
   unchanged.

## Evidence and constraints

- Canonical finding: `.agents/review/findings/tr-13.md`.
- `PlexLibrary::transcode_decision` currently constructs its query vector by
  hand before passing it to reqwest.
- `PlexLibrary::transcode_url` separately constructs and serializes the matching
  start query.
- Finding `or-7` records a real drift: the decision once sent
  `directStream=1` while the start URL sent `0`.
- Finding `tr-11` records a second failure: both duplicated builders omitted
  the required `X-Plex-Client-Profile-Name=Web` selector. Sharing only that
  selector's value did not remove the duplicated parameter contract.
- The current `direct_flags` helper shares only two values. The existing
  `the_decision_asks_for_the_delivery_it_would_start` test checks that helper
  and the start URL, but never captures the production decision request. It
  therefore does not guard the complete two-caller wire contract.
- Finding `tr-10` changed the credential boundary after `tr-13` was admitted:
  both requests authenticate by private header, and the start URL must remain
  token-free. The shared query builder must not take a token merely to mirror
  the finding's pre-`tr-10` evidence.
- Finding `tr-12` changed only decision error classification/reporting. The
  refactor must leave its staged request/response handling and safe errors
  byte-for-byte equivalent outside query construction.
- The source version is 1.0.57. This shipped-code refactor owns 1.0.58 through
  `scripts/bump.sh 1.0.58`.
- The Linux and live-Plex venue, controls, and cleanup requirements are
  canonical in `.agents/machines.md`. The detached `~/dev/vela-main` worktree
  must be clean before it moves.

### Current request boundary

The implementation must preserve this contract:

| Field | Decision request | Start request | Classification |
|---|---|---|---|
| endpoint | `/video/:/transcode/universal/decision` | `/video/:/transcode/universal/start.m3u8` | endpoint-only |
| `path` | selected rating key | same | common |
| `mediaIndex` | selected media index | same | common |
| `partIndex` | `0` | `0` | common |
| `protocol` | `hls` | `hls` | common |
| `X-Plex-Client-Profile-Name` | `Web` | `Web` | common |
| `directPlay` / `directStream` | `1` / `1` for direct, `0` / `0` for a tier | `0` / `0` | delivery |
| `fastSeek` | `1` | `1` | common |
| `copyts` | `1` | `1` | common |
| `offset` | `0` | requested resume seconds | endpoint value |
| `maxVideoBitrate` / `videoResolution` | absent for direct, tier values for conversion | tier values | delivery |
| `session` | caller-supplied probe id | newly generated id returned for teardown | endpoint value |
| `X-Plex-Client-Identifier` | request header, not query | query pair | endpoint-only identity |
| `X-Plex-Token` | request header | mpv private HTTP header outside this builder | credential boundary |
| `Accept` | `application/xml` header | unchanged start behavior | endpoint-only header |

Query order is not a Plex contract. The new builder may converge the two
currently different pair orders into one canonical order, but it must preserve
the exact pair names, values, multiplicity, and encoding behavior.

## Implementation

### 1. Model delivery and endpoint differences privately

In `src-tauri/src/plex_library.rs`, add two private enums beside the existing
universal-transcode code:

```text
UniversalTranscodeDelivery
  Direct
  Tier(QualityTier)

UniversalTranscodeEndpoint<'a>
  Decision
  Start {
    offset_seconds: u64,
    client_identifier: &'a str,
  }
```

Add one private `PlexLibrary::universal_transcode_query` helper. Its inputs are:

- rating key;
- selected media index;
- session id;
- `UniversalTranscodeDelivery`; and
- `UniversalTranscodeEndpoint`.

It returns the ordered `Vec<(&'static str, String)>` consumed by the existing
reqwest and start-URL serializers.

The helper must construct these pairs once:

- `path`;
- `mediaIndex`;
- `partIndex`;
- `protocol`;
- `X-Plex-Client-Profile-Name`;
- `directPlay`;
- `directStream`;
- `fastSeek`;
- `copyts`;
- `offset`;
- optional `maxVideoBitrate`;
- optional `videoResolution`;
- `session`; and
- the start-only `X-Plex-Client-Identifier`.

`Direct` emits flags `1` / `1` and no tier bounds. `Tier` emits `0` / `0` and
both tier bounds. `Decision` fixes offset to zero and emits no client-identifier
query pair. `Start` uses its supplied offset and appends exactly one
client-identifier pair.

Do not accept an auth token, URL, endpoint path, request headers, response
parser, or mutable caller-supplied pair list. The abstraction is Plex-private;
do not move it into the provider-neutral source layer.

### 2. Route both production callers through the builder

In `PlexLibrary::transcode_decision`:

- map `None` to `UniversalTranscodeDelivery::Direct`;
- map `Some(tier)` to `UniversalTranscodeDelivery::Tier(tier)`;
- call the shared builder with `UniversalTranscodeEndpoint::Decision`; and
- pass the returned pairs to the existing reqwest `.query`.

Keep the decision endpoint, token/client-identifier/Accept headers, safe error
classification, status handling, body read, XML parse, semantic validation, and
return type unchanged.

In `PlexLibrary::transcode_url`:

- preserve the split-file refusal, server-base check, and one fresh UUID per
  start;
- call the shared builder with `UniversalTranscodeDelivery::Tier(tier)` and
  `UniversalTranscodeEndpoint::Start`;
- serialize the returned pairs with the existing value encoder; and
- return the same credential-free URL plus the generated teardown session.

Keep both public function signatures unchanged. In particular, do not reuse the
decision probe id as the start session and do not move session generation into
`PlexSource`.

Remove `PlexLibrary::direct_flags` once the delivery enum owns that mapping.
Replace its helper-only test with the real-wire guards below.

No change is expected in `src-tauri/src/source/plex.rs`.

### 3. Guard the exact production wire contract

Add test-only query parsing/normalization helpers beside the existing
loopback-request fixture in `src-tauri/src/plex_library.rs`. They must preserve
duplicate pairs rather than collapsing into a map.

Add `universal_transcode_endpoints_share_one_query_contract`:

1. run the real tier `transcode_decision` against the loopback capture server;
2. build the real tier `transcode_url` for the same rating key, media index,
   tier, and zero offset;
3. parse both serialized request targets;
4. assert the exact decision and start endpoint paths;
5. assert the decision request carries the synthetic token and client id only
   as headers;
6. assert the start query carries exactly one client-id pair, then remove that
   documented start-only pair for comparison;
7. assert neither query contains a token key or synthetic token value;
8. assert each query carries exactly one session matching the id supplied or
   returned by its production caller, then normalize only the two session
   values; and
9. sort and compare the remaining duplicate-sensitive pairs against each other
   and against an independently spelled literal expected contract.

The literal expected tier contract must include:

```text
path=/library/metadata/42
mediaIndex=<selected index>
partIndex=0
protocol=hls
X-Plex-Client-Profile-Name=Web
directPlay=0
directStream=0
fastSeek=1
copyts=1
offset=0
maxVideoBitrate=<tier bitrate>
videoResolution=<tier WxH>
session=<normalized>
```

Do not derive expected names or values from the new enums, helper, profile
constant, or returned production pairs.

Add `universal_transcode_direct_decision_preserves_direct_delivery`:

- run the real `transcode_decision` with `tier=None`;
- parse the captured request target;
- require exactly one `directPlay=1` and one `directStream=1`;
- require offset zero;
- require no `maxVideoBitrate` or `videoResolution`; and
- retain the existing header-only token/client-id assertions.

Keep the existing independent guards for the literal `Web` value, credential-
free start URL, selected media index, resume offset, unique returned session,
and split-file refusal.

### 4. Guard that the shared builder is actually wired

In `tests/transcoding-ui.test.mjs`, add a source-wiring test named
`Plex universal-transcode endpoints use one query contract`.

The test must inspect only the production portion of
`src-tauri/src/plex_library.rs` and require:

- exactly one `universal_transcode_query` definition;
- one call from `transcode_decision`;
- one call from `transcode_url`;
- no common-pair construction in either caller; and
- exactly one production pair-construction site for every common query key.

It must also require the start-only client-identifier branch and reject any
`X-Plex-Token` query-pair construction in the universal-transcode region.

This source guard supplements the behavior tests. The Rust tests prove both
real wire outputs; the source guard proves those currently equal outputs are
not still produced by duplicated vectors.

### 5. Bump once

Run `scripts/bump.sh 1.0.58` after production code and guards are in place.
Accept only the version surfaces owned by that script.

## Verification

### Targeted green checks

From `src-tauri/`:

```text
cargo +stable test --locked universal_transcode_endpoints_share_one_query_contract
cargo +stable test --locked universal_transcode_direct_decision_preserves_direct_delivery
cargo +stable test --locked transcode_
```

From the repo root:

```text
node --test --test-name-pattern="Plex universal-transcode endpoints use one query contract" tests/transcoding-ui.test.mjs
```

Inspect the executed test counts; a zero-test filtered success is a failure.

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

Run this set before the implementation commit. Linux E2E is deferred until the
exact committed SHA is aligned to the VM. Canonical verification is incomplete
until the full Linux suite passes.

### Commit and red-proof

Commit the 1.0.58 implementation before injecting regressions. The commit must
contain only `tr-13`, its guards, and script-owned version surfaces.

Prove each claimed guard independently, restoring from the committed
implementation and confirming a clean worktree between mutations:

1. bypass the shared builder only in `transcode_decision` by removing `copyts`
   from its returned pair list; require the Rust parity guard to fail on the
   captured decision wire;
2. restore, then bypass it only in `transcode_url` by removing `fastSeek` from
   its returned pair list; require the Rust parity guard to fail on the parsed
   start wire;
3. restore, then remove `protocol` from the shared builder; require the
   independently spelled expected-contract assertion to fail even though the
   two production outputs still match each other;
4. restore, then duplicate the current common vector back into both callers and
   stop calling the shared builder while keeping the serialized pair sets
   correct; require the Node wiring guard to fail;
5. restore, then route a direct decision through tier delivery; require the
   direct-decision wire guard to fail on flags and tier bounds;
6. restore, then omit the start-only client identifier; require the parity
   guard's endpoint-difference assertion to fail;
7. restore the committed implementation, confirm a clean worktree, and rerun
   all targeted Rust and Node guards green.

Do not combine mutations. Record only the focused failure reason in
`.agents/review/findings/tr-13.md`.

### Venue and live proof

Align the clean detached `~/dev/vela-main` worktree to the exact implementation
commit. If the commit is not pushed, transfer it with a temporary git bundle;
do not copy individual source files and do not push without the separate
approval required by `.agents/push-policy.md`.

On the VM, through its login shell, run the full hermetic suite:

```text
cd ~/dev/vela-main && npm run e2e
```

Then run the existing positive real-Plex regression:

```text
npm run e2e:live live-transcode
```

Require the live scenario to find an approved tier, create no session during
decision probes, hand mpv a credential-free `start.m3u8` URL with private header
auth, observe its own new server session, and remove that session on teardown.
Do not inject a broken query against the owner's Plex server for a live red
case.

Afterward:

- confirm the VM worktree is clean and at the implementation commit;
- confirm the temporary transfer artifacts are removed;
- confirm Plex and `plex-watchdog.timer` are active;
- confirm the scenario left no mpv process/listener or credentials file; and
- return the VM to its prior power state.

## Review and closeout

After local, guard, Linux, and live verification:

1. run the finding-specific `codereview claude` workflow over the exact
   implementation range, with no model argument; use the playbook's standard
   omitted-effort default;
2. require the reviewer to inspect both production wire guards and independently
   prove one caller-specific drift red/restored-green;
3. if the review returns an actionable finding, record and address exactly that
   finding in its own commit, rerun affected and canonical verification, and
   dispatch a fresh Claude review under the same owner-selected routing;
4. update `.agents/review/findings/tr-13.md`, `.agents/review/index.md`,
   `.agents/plans/server-transcoding.md`, this plan, `.agents/state.md`, and
   `.agents/machines.md` with the implementation/correction commits, independent
   guard proof, Linux/live result, venue state, and final review verdict;
5. commit the record-only closeout immediately.

The owner-directed Claude reviews of `tr-12` and `tr-13` are finding-specific
exceptions and do not change this repo's standing Codex routing. For `tr-13`,
the owner explicitly supplied no model; do not add one. Do not push any commit
without a separate explicit go.

## Expected files

- `src-tauri/src/plex_library.rs`
- `tests/transcoding-ui.test.mjs`
- version surfaces maintained by `scripts/bump.sh`
- `.agents/review/findings/tr-13.md`
- `.agents/review/index.md`
- `.agents/plans/server-transcoding.md`
- `.agents/plans/tr-13-plex-universal-query-builder.md`
- `.agents/state.md`
- `.agents/machines.md`

No other production or test file is in scope.

## Explicit non-goals

- No Plex query key, value, multiplicity, or endpoint change.
- No change from `X-Plex-Client-Profile-Name=Web`, no profile discovery, no
  fallback profile, and no custom profile.
- No token query parameter, raw credential in a URL, new mpv header, or change
  to the private-header include.
- No reuse of a decision probe id as the started session; no change to session
  generation, ownership, or teardown.
- No URL serializer or percent-encoding rewrite.
- No change to decision failure classification, safe diagnostic text, valid
  refusals, or Original fallback.
- No change to selected-version targeting, split-file refusal, tier filtering,
  resume semantics, direct-play preflight, or Automatic.
- No Jellyfin, Emby, frontend UI, settings, or provider-neutral abstraction.
- No deliberate failed request against the owner's real Plex server.
