# Plan: preserve mpv window state across automatic continuation

Status: **DRAFT — CLAUDE OPENREVIEW ROUTING AND OWNER APPROVAL PENDING.**

The machine-local Claude Code 2.1.214 cache currently confirms only the
standard code-review pair. The `openreview` playbook requires a separately
owner-confirmed frontier model/effort pair and grade before dispatch and
forbids inference from the code-review mapping. No plan-review verdict exists
until that routing decision is recorded.

## Goal

When a cleanly completed item is replaced by the next item in a Vela playlist,
a server playlist, or Continue Playing, the new mpv process retains the actual
fullscreen and maximized state of the completed process. A user must not have
to re-enter fullscreen or maximize every automatically advanced item.

The implementation must preserve Vela's exact-session replacement rules,
configured mpv options, failed-launch behavior, backend-neutral playback path,
and stale-reader isolation. It must not persist desktop geometry or turn a
manual play into an implicit continuation.

## Evidence and existing boundary

- `playback::play` starts a new mpv process for every item. With Vela's default
  `--no-config`, no runtime window state can survive process replacement.
- `spawn_position_reader` already connects to every launched mpv IPC endpoint
  and observes `time-pos`; Plex, Jellyfin/Emby, and the untracked end path all
  use it.
- `play_by_key` receives `replace_session: Some(ended_session)` only for an
  exact-session automatic replacement. Backend playlist advancement passes it
  from `advance_playlist`; frontend Continue Playing passes the same completed
  UUID through `play_item(expectedSession)`. Manual item, Vela-playlist, and
  server-playlist starts pass `None`.
- `playback::play` appends configured `mpv_extra_args` and autocrop arguments
  before its protected IPC/title/auth/start/URL block. mpv resolves repeated
  options last-wins, so inherited runtime state must be appended after user and
  autocrop options but before that protected block.
- The current IPC reader can outlive session replacement briefly. A single
  mutable global window value would let an old reader overwrite the new
  process's observation.

## Owner decision required

Recommended boundary: inherit window state only when `replace_session` names
the exact active session. Every manual play starts from current Vela/mpv
configuration and does not inherit the previous process. This makes the change
match automatic continuation without making unrelated clicks share window
state. Fullscreen and maximized are both retained; size, position, monitor,
workspace, and border state remain out of scope.

## State model

Add backend-owned window state with three values per property: unknown, false,
or true.

- `PlaybackWindowState` contains `fullscreen: Option<bool>` and
  `maximized: Option<bool>`.
- `WindowStateObservation` is a fresh thread-safe handle for one launched mpv.
  Its IPC reader updates only that handle; callers can take a consistent
  snapshot. Unknown is never coerced to false.
- `AppState` stores at most one published `{ session_id, observation }` record.
  The record is not durable configuration. It exists only to transfer observed
  state from one exact playback session to its authorized successor.

Session identity belongs beside the observation rather than inside one global
pair of booleans. An old reader can continue mutating its old handle without
affecting the record published for a newer process.

## Playback flow

### Observe the current process

Extend `spawn_position_reader` to send three mpv observations with distinct
request IDs: `time-pos`, `fullscreen`, and `window-maximized`.

- Position handling remains numeric and unchanged.
- Window handling accepts only `property-change` events whose `name` is one of
  the two owned properties and whose `data` is a JSON boolean.
- Null, strings, numbers, command responses without a property name, and
  unknown properties leave the relevant state unknown or unchanged.
- All progress targets, including the no-server-progress watcher, pass the
  launch's fresh observation handle into this shared reader.

### Select inherited state

While holding the existing `play_lock`, `play_by_key` validates
`replace_session` against `active_playback_session`, resolves the new stream,
and then snapshots window state only if the published window record carries the
same exact session ID. `None`, a stale UUID, a missing record, or an unknown
property produces no inherited flag for that property.

Unavailable playlist entries do not consume or replace the prior observation:
the same ended session remains eligible while the loop tries the next entry.
A slow stream resolution may continue receiving final events on the prior
handle; the snapshot is taken only after resolution and before replacement.

### Launch and publish the successor

Put the inherited snapshot and a fresh observation handle in `PlaySpec` (or an
equivalent playback launch input). In `playback::play`:

1. append Vela render defaults;
2. append configured `mpv_extra_args` and autocrop arguments;
3. append known inherited flags in the order
   `--window-maximized=yes|no`, then `--fullscreen=yes|no`;
4. append the protected IPC/title/auth/start/URL block.

Known false values are emitted explicitly so a prior runtime exit from
fullscreen/maximized overrides configured `yes`. Unknown values are omitted so
normal mpv or user configuration remains authoritative.

Create the successor's observation before spawning, but publish its
`{ session_id, observation }` record only after `playback::play` succeeds. Do
that before releasing the recents/start gate. A resolve, spawn, or tracker
setup failure therefore cannot publish attempted state. The prior record may
remain, but exact active-session matching prevents it from authorizing later
inheritance after the failed replacement clears the attempted session.

## Concurrency and failure rules

- Existing `play_lock` serialization remains the authority for replacement;
  no new lock spans network resolution or process lifetime.
- A delayed automatic continuation whose expected UUID is stale remains
  rejected before it can snapshot or launch anything.
- A newer manual play gets a fresh observation handle and publishes it only on
  successful launch, but inherits no prior state.
- Old IPC tails mutate only their per-session handles and cannot overwrite the
  current record.
- If one or both properties were never observed, only the known property is
  inherited. No observation failure blocks playback.
- Failed stream resolution leaves the currently playing process and its window
  record intact. Failed spawn/tracker setup follows the existing terminated-old
  failure contract and does not publish the attempted observation.
- User close remains non-continuing. Only existing clean-EOF sequence paths can
  consume the state.

## Files and implementation slice

One code slice, one version bump, one external code-review verdict:

- `src-tauri/src/playback.rs`: state/observation types, strict IPC property
  parsing, observation registration, inherited launch flags, focused unit
  tests.
- `src-tauri/src/commands.rs`: exact-session snapshot selection, fresh-handle
  creation, successful-launch publication, and focused session/isolation tests.
- `src-tauri/src/lib.rs`: initialize the current window-session record.
- `tests/e2e/scenarios/playlistplay.mjs`: manual non-inheritance, automatic
  true/false inheritance, configured-option precedence, and successor-handle
  rebinding on the real mpv IPC path.
- `tests/e2e/scenarios/continuetv.mjs`: frontend `expectedSession` continuation
  inherits fullscreen state outside a persistent playlist.
- Version surfaces maintained by `scripts/bump.sh`: Vela 0.1.59.

## Guard design

### Focused Rust guards

- Strictly parse boolean property-change events for fullscreen and maximized;
  reject null/wrong-type/wrong-event/wrong-name inputs.
- Preserve explicit true, explicit false, and unknown independently.
- Generate inherited arguments in maximized-then-fullscreen order and omit
  unknown properties.
- Select a snapshot only for the exact expected session; manual `None` and
  stale sessions inherit nothing.
- Prove two observation handles are isolated and a successor record rebinds to
  its fresh handle.
- Prove failed launch publication is not called through the existing
  success-only boundary.

### Real-app guards

Use configured `--fullscreen=yes --window-maximized=yes` as a hostile baseline
in `playlistplay`:

1. change the first process to explicit false/false;
2. manually replace it and require the new process to use configured true/true,
   proving manual starts do not inherit;
3. change that process to false/false, finish at clean EOF, and require the
   automatic successor to start false/false, proving observed `no` overrides
   configured `yes`;
4. change the successor to true/false, finish it, and require the following
   successor to start true/false, proving both properties and the newly
   published handle own the next transition.

In `continuetv`, set fullscreen true on a naturally completed episode and
require the frontend-selected next episode's new mpv process to report true.
This proves `play_item(expectedSession)` uses the same continuity path as
backend playlist advancement.

Wait for each `set_property` readback before ending the process so the test does
not mistake an IPC timing race for application behavior.

## Production mutation proofs

After implementation, mutate production only and restore exact head after each
red result:

1. stop observing `fullscreen`;
2. stop observing `window-maximized`;
3. append inherited flags before user options, allowing configured `yes` to win;
4. let manual `replace_session: None` inherit the current observation;
5. drop the exact session-ID comparison;
6. reuse the old observation or omit successful successor publication;
7. coerce unknown to false;
8. bypass inheritance for frontend `play_item(expectedSession)` continuation.

Each mutation must fail the focused Rust guard or the exact real-app leg that
predicts its observable defect, then pass after restoration. A source-only
assertion does not substitute for the real mpv transitions.

## Verification

- canonical frontend and Rust verification from `.agents/repo-guidance.md`;
- focused Rust window-state tests on the macOS development host;
- syntax-check the changed `.mjs` scenarios;
- fresh-build Linux real-app `playlistplay` and `continuetv` scenarios;
- full fresh-build Linux real-app suite;
- Claude `codereview` against pinned base/head with an independent
  production-only mutation, red/restored-green proof, and clean exact-head
  worktree.

## Non-goals

- Persisting state across app restarts or unrelated manual plays.
- Window size, position, monitor, workspace, border, always-on-top, or display
  mode persistence.
- Reusing one mpv process between items.
- Changing clean-EOF detection, playlist ordering, unavailable-entry skipping,
  Continue Playing policy, resume behavior, autocrop, or user mpv settings.
- Treating missing IPC observations as false or making observation a playback
  prerequisite.
