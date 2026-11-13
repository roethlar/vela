# Codereview 2026-11-13 — the embedded player's core (embedded player, Slice 3)

Outcome: codereview codex over `053cb36..0a63799` together with `a02db80`:
**findings**, nine candidates, nine admitted (`ec-1`, `ec-2` HIGH; `ec-3`
to `ec-7` MEDIUM; `ec-8`, `ec-9` LOW), none declined.

## Dispatch record

- Owner's dispatch word, verbatim (2026-11-12, the session goal):
  "codereview slices with codex". This is the generation half: a defect
  hunt over a pinned landed range.
- Reviewer: codex / host default / host default / standard
- Harness: codex-cli 0.167.2, CLI transport, headless one-shot, stdin
  closed: `codex exec --sandbox workspace-write --json --output-schema
  <schema> -o <outfile> <prompt>`. Called plainly, with no model and no
  effort argument, per `.agents/machines.md`. The `--json` stream names no
  model; the host's `~/.codex/config.toml` reads `model = "gpt-6-astra"`,
  `model_reasoning_effort = "max"`, so that is what a plain call resolves
  to on `continuum`.
- Run from `~/dev/vela-probe-wt`, a detached `git worktree` of this clone
  checked out at the head SHA, and declared disposable in the prompt.
- Tier: standard. T1 evaluated against the range before dispatch: no
  changed path matches a sensitive-path glob; the repo has no
  `.agents/review/sensitive-paths` override.
- Thread `01a1d3e2-a70c-7362-a475-ceec149241d4`; the event stream ends in
  `turn.completed`; 55 tool calls. Dispatched 2026-11-13 07:50 UTC,
  finished 08:09 UTC.

## Reviewed range

- base: `053cb36ea59d095a20e9ed5faf5145556c061b19`
- head: `0a6379975a43940ba606f67284cdad3803ec8b7a`

One commit in the range, `0a63799` (the player), reviewed together with
`a02db80af65a72ec77008bada8c3371142e9ce03` (what an embedded launch is made
of), which landed earlier in the same slice. The commits between the two
belong to other review loops and were declared out of scope in the prompt.

## Acceptance (computed by the orchestrator)

The payload matches the schema; `verdict` is `findings` and `findings` is
non-empty; `reviewed_sha` and `base_sha` equal the dispatched pins;
`capability_ok` is literally `true`. Accepted as a valid findings verdict.

Payload, verbatim:

```json
{"verdict":"findings","capability_ok":true,"reviewed_sha":"0a6379975a43940ba606f67284cdad3803ec8b7a","base_sha":"053cb36ea59d095a20e9ed5faf5145556c061b19","findings":[{"title":"Native window close can invalidate a starting player's Wayland pointer","evidence":"host.rs:262-268 retains borrowed WindowHandles, consumed by session.rs:179-189 and surface.rs:208-211. The exit sweep in src-tauri/src/lib.rs:409 handles only ExitRequested/Exit. Pinned tauri-runtime-wry 2.11.4 delivers ExitRequested after destroying the last window. Closing after playback.rs:365 acquires the slot, but before the worker consumes the parent pointer, therefore destroys the parent before the sweep can acquire that lock.","predicted_failure":"The worker can pass a freed wl_surface pointer to ObjectId::from_ptr, which calls wl_proxy_get_class on it: a use-after-free capable of crashing Vela.","severity":"HIGH","better_approach":"In embedded mode, intercept CloseRequested and prevent native destruction until all starting and running workers have released their window references. Coordinate completion asynchronously without blocking the window thread on the player-slot lock."},{"title":"Timeout cleanup detaches a worker that still owns player resources","evidence":"src-tauri/src/player/embedded/session.rs:358-366 and 433-439 ignore false from stop_and_wait. A setup operation that remains blocked, such as reading an Advanced --include pointing to an unwritten FIFO, survives the timeout. A review probe confirmed that dropping Session returns after five seconds while its worker remains alive; the existing teardown wiring test nevertheless passes.","predicted_failure":"Failed starts leave untracked threads, descriptors and graphics resources alive. If setup later resumes, it still loads the file and emits Opened before checking stop, despite the play already having failed; borrowed window lifetimes are no longer protected.","severity":"HIGH","better_approach":"Retain ownership of timed-out workers until teardown actually completes, and cancel setup before loading or publishing lifecycle events. A timeout must not authorize releasing the worker's dependencies."},{"title":"The new startup read bypasses nonregular-file protection","evidence":"src-tauri/src/player/embedded/startup.rs:138-146 unconditionally calls read_to_string before GTK or durable::load. With config.json replaced by a FIFO, a review subprocess blocked here; an ordinary external-settings file returned immediately. The previous storage path rejects nonregular files in storage.rs:62-72.","predicted_failure":"Vela never opens its window or recovery UI for this invalid settings path, even when embedded playback was never selected. A symlink to an unbounded device also bypasses the previous rejection.","severity":"MEDIUM","better_approach":"Preserve the storage loader's regular-file and symlink checks before reading the startup setting. Bound this preliminary read and treat rejected input as ordinary window mode."},{"title":"Includes inside user profiles override the protected option tail","evidence":"src-tauri/src/player/embedded/options.rs:101-108 separates direct include options but batches profile selection with Vela's protected options. With own mpv.conf defining [example] containing include=/path/override.conf, Advanced --profile=example, and override.conf containing idle=yes, a headless libmpv 0.42 probe of the same include sequence returned idle=yes while vo and input-ipc-server retained Vela's values.","predicted_failure":"The startup verification passes, but EOF leaves mpv running indefinitely. Final playback bookkeeping and continuation do not complete.","severity":"MEDIUM","better_approach":"Finish applying user options and their nested profiles before applying Vela's protected tail in a separate step. Test the effective values through libmpv, including profile-contained includes."},{"title":"Close can stop a successor after validating the predecessor","evidence":"src-tauri/src/commands.rs:5361-5377 releases active_playback_session before acquiring current_child and never acquires play_lock. After Close validates session A, a concurrent replacement can install and publish B before Close obtains the player-slot lock.","predicted_failure":"A delayed Close belonging to A kills B, contradicting the command's session-isolation promise.","severity":"MEDIUM","better_approach":"Serialize the session check and stop operation with the same play_lock used by replacement plays, or store and validate the session identity atomically alongside the player handle."},{"title":"An embedded setup failure does not fall back for the current play","evidence":"src-tauri/src/player/embedded/egl.rs:166-173 checks available formats without creating the required OpenGL 3.3 context. If context or renderer creation subsequently fails, host.rs:375-381 only marks future starts unavailable, and playback.rs:376 propagates the error. No external launch is attempted for that play.","predicted_failure":"On a machine that passes the preliminary probe but cannot create the renderer, the first Play fails outright; only a later Play falls back. This contradicts D3 and the new development documentation's fallback promise.","severity":"MEDIUM","better_approach":"After confirming complete cleanup of the failed embedded start, rebuild the launch for the external player and complete the original play with its fallback notice."},{"title":"Failed auth-file deletion loses its cleanup owner","evidence":"src-tauri/src/player/embedded/session.rs:423-427 takes HeaderInclude out of Session and drops it when remove_now fails, unlike external.rs:47-57, which restores ownership. A review probe made the include directory temporarily non-writable, called removal, restored permissions, and dropped Session; the credential file remained.","predicted_failure":"A temporary deletion failure leaves the stream-auth include on disk permanently because reaping and exit no longer retain its path for retry.","severity":"MEDIUM","better_approach":"Restore the include to Session when removal fails, matching ManagedChild, so reaping or exit can retry after the transient failure clears."},{"title":"Failed embedded starts abandon marker payload files","evidence":"src-tauri/src/player/embedded/host.rs:346-351 discards LaunchPlan.marker_payload during destructuring, and the Session::start error path never calls discard_marker_payload. An EGL failure before mpv initialization occurs before the marker script can consume and unlink the file.","predicted_failure":"The marker JSON survives the failed play and application exit, unless a later marker-bearing launch happens to prune it.","severity":"LOW","better_approach":"Keep a cleanup guard for the marker payload through startup and remove it on every failure before the script consumes it."},{"title":"Every embedded play leaks another server-side Wayland registry","evidence":"src-tauri/src/player/embedded/surface.rs:118 requests a new registry each time join runs, including every VideoSurface::create. The guest connection deliberately never disconnects. The installed Wayland protocol specifies that get_registry server resources survive destruction of the client proxy and are released only when the connection disconnects.","predicted_failure":"Repeated plays accumulate compositor-side registry resources for the lifetime of Vela, despite dropping each VideoSurface.","severity":"LOW","better_approach":"Reuse a registry and global bindings owned for the lifetime of the shared display, while keeping per-play surfaces and event handling separately owned."}]}
```

## Intake

Each candidate was checked against the code at `e8db400` (the head when
the verdict was read; Slice 4 had landed on top and touches none of the
places named).

| Candidate | Verdict | Why |
| --- | --- | --- |
| A window closed while a play is starting leaves the starting play a dead pointer | ADMITTED as `ec-1` | `Host::prepare` copies the window's raw `wl_surface` pointer on the window's thread; the play's own thread turns it into an object later, after the player slot was locked and any earlier player stopped. Nothing keeps the window alive in between. |
| A start that times out lets go of a worker that is still running | ADMITTED as `ec-2` | `Session::start` gives up after 20 s and `Session::drop` after 5 s more; the thread handle is then dropped. A worker that comes back after that still hands mpv its file and still reports `Opened`: it checks for a stop only once it is in its drawing loop. |
| The settings read at start-up has none of the loader's file checks | ADMITTED as `ec-3` | `saved_player_setting` is a bare `read_to_string` and runs for every start on Linux, chosen player or not. The loader it stands in for refuses anything that is not a regular file. This one is against the plan's first promise. |
| An include inside a user profile is applied after Vela's protected options | ADMITTED as `ec-4` | `steps` splits the list only at a direct `include`; a `--profile=` line shares its options file with Vela's tail, and the measured rule (an include inside a file takes effect after the rest of that file) then puts the profile's include last. |
| Close can stop the play that replaced the one it was for | ADMITTED as `ec-5` | `player_close` compares against the active session, releases that lock, and then stops whatever embedded player the slot holds. The slot's own player carries no identity to compare with. |
| A start that fails after the checks passed fails the play instead of falling back | ADMITTED as `ec-6` | Recorded as a deliberate shortcut in the plan's log ("redoing a half-started play in the other player ... was judged riskier than one failed play"), and the log itself says D3 reads otherwise. The reviewer read that entry and still reports it; D3 is the decision, the shortcut was the agent's. Admitted, and the shortcut is withdrawn. |
| A stream-auth file whose removal fails is forgotten | ADMITTED as `ec-7` | `Session::remove_consumed_header_include` takes the file's owner out and drops it on failure; `ManagedChild`, a few files away, puts it back so reaping can try again. |
| A failed embedded start leaves the marker payload file | ADMITTED as `ec-8` | `Prepared::start` takes the plan apart and drops `marker_payload` without the `discard_marker_payload` that the external launch calls on its failure path. |
| Every play adds a registry the compositor keeps for the life of Vela | ADMITTED as `ec-9` | `join` asks for a new `wl_registry` per play; the protocol has no request to release one, and Vela's connection is the window's, which is never closed. Small, and it grows with every play. |

Per-finding records: `.agents/review/findings/ec-1.md` to `ec-9.md`.

## Notes

- On severity: `ec-1` and `ec-2` are kept at the reviewer's HIGH. Each
  needs unusual timing (a window closed in the instant a play starts; a
  start that takes longer than 20 seconds), but what follows is memory
  that is no longer valid, in the process that holds the user's
  connections.
- `ec-3` is the only one that reaches a user who never selected the
  embedded player.
- Order of repair (dependencies, not severity): `ec-9` first, because
  joining the window's connection once is what lets `ec-1` create the
  surface on the window's thread without making that thread wait for the
  compositor on every play; `ec-8` and `ec-2` before `ec-6`, which may
  only start the external player once the failed start has left nothing
  behind.
- Not covered: the embedded player itself and its E2E scenario (no display,
  compositor or GPU in the reviewer's sandbox). The reviewer probed the
  options with a headless libmpv 0.42 and the session with unit probes.
