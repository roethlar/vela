# pw-8: the other end-to-end scenarios, gone through for the same race

Written 2026-11-14. `pw-8` found that the scenario `windowsetting` changed
and ended a play before Vela had connected to that play's mpv, and its
record listed as a known gap that no other scenario had been gone through
for the same race. This file is that going-through. It was made by codex
(gpt-6-astra) as a coding task, under the owner's request of that day to
split the coding with codex, and landed at `71395c3` after the
coordinating agent's review. Codex cannot verify code it wrote, so the
review here is the coordinating agent's.

## The rule

A scenario that changes mpv's state or ends a play, and then checks
something Vela can only know from its own connection to that mpv, waits
first until mpv has accepted Vela's connections:
`velaConnected(socketPath, connections)` in `tests/e2e/helpers.mjs`, which
counts accepted connections on the play's socket from `/proc/net/unix`
(`acceptedConnections` in `tests/e2e/judge.mjs`). The count includes the
scenario's own connection: three for a play in mpv's own window, four for
an embedded play. Nineteen calls in sixteen files; `windowsetting`'s own
wait became that helper.

## The survey (codex's, as reported)

Line numbers are those of the tree codex worked on (`5440907` plus its
change) and have moved since. The letters name Vela's connections. Its
last row says that no helper `positionsWhile` exists: it came to `main`
after that tree (`d504a1e`), reads the position while a play is being
stopped, and is reached only through `playAndQuit`, which has the wait.

Vela’s connections, derived from the product:

- **R — position/window reader:** `spawn_position_reader` (`src-tauri/src/playback.rs:967`), used by `start_tracking_plex`, `start_tracking_jellyfin`, and `spawn_end_watcher`. One connection handles position and window properties.
- **E — clean-EOF watcher:** `spawn_eof_watcher` (`src-tauri/src/playback.rs:915`), always started by `play`.
- **V — embedded view reporter:** `spawn_reporter` (`src-tauri/src/player/report.rs:432`), started only for an actual embedded play.
- **H — Automatic health sampler:** `spawn_health_sampler` (`src-tauri/src/playback.rs:1116`), conditional on `step_down`; `automatic_manages` (`src-tauri/src/commands.rs:5462`) decides this.
- **N — notice sender:** `spawn_osd_notice` (`src-tauri/src/playback.rs:1074`), temporary: connects, waits 1.5 seconds, writes, and closes.
- Embedded controls additionally make temporary connections through `exchange` (`src-tauri/src/player/control.rs:395`). Cloned stream descriptors do not represent additional accepted connections.

Including the scenario’s client, ordinary plays therefore require **3 external / 4 embedded** connections. Automatic adds one; a notice adds a temporary client that must not substitute for a missing reader. All changed callers use Original quality without notices; Original is the default in `playback_quality` (`src-tauri/src/config.rs:168`).

Embedded mpv **does expose this socket**: `build_launch_for` (`src-tauri/src/player/launch.rs:924`) sets `input-ipc-server`, and `embedded::session` (`src-tauri/src/player/embedded/session.rs:470`) checks it. The helper therefore has no embedded bypass. Each player-dependent count has a comment explaining V.

The survey below covers all **54 scenarios and three live scenarios**. Paths are relative to `tests/e2e/`; line numbers describe the final tree. **† marks a changed file**, and its row explains the change.

| Place | Action and later check | Vela connection needed | Existing coverage; exposure and change |
|---|---|---|---|
| † `helpers.mjs:440`, `playAndQuit` | Seek to 6 seconds, stop; callers check progress, recents, resume and timeline reports | R | `time-pos > 0.5` observes mpv; the following 1.5 seconds is elapsed time. Neither proves R connected under delayed scheduling. Added 3/4-client wait before callbacks and seek. |
| `curation.mjs:25`; `watchcurate.mjs:23`; `resume.mjs:59,71`; `playverbs.mjs:141,180`; `multiplex.mjs:211` | Drive plays through `playAndQuit`; check recents, resume/watch state, or final Plex position | R | Covered by the helper change. Multiplex’s additional “playing” report wait does not itself prove a sampled position: `start_tracking_plex` can report its initial counter. |
| `helpers.mjs:319`, `stopPlayAsUser` | Quit external mpv or close embedded playback; await shutdown and uncovered page | Depends on caller | Its waits occur **after** stopping and prove shutdown, not observer readiness. Kept generic; readiness is established where the caller needs observed state. |
| † `windowsetting.mjs:95,121,125,136` | Change fullscreen, quit mpv or Vela; check saved settings and subsequent window | R; E also included in count | Already covered by inline three-client wait. Replaced it with `velaConnected(socket, 3)`, still the final step before handoff. |
| † `playlistplay.mjs:130,272,287` | Change window flags, replace a play, release successive plays to EOF; check inheritance and completion | R, E | Path/window readback and recorded starts are not Vela-reader readiness. Added 3/4-client wait before `nextMpv` returns. |
| † `continuetv.mjs:80,88,117,265,540` | Change fullscreen, release episodes to EOF, quit/manual-replace plays; check continuation and window lifecycle | R, E | Pausing and reading mpv’s path/property cover only mpv. Added 3/4-client wait before the initial pause and handoff. |
| † `continueon.mjs:48,55,65` | Seek each movie/playlist item near EOF; check successors, recents and refresh ordering | E, R | Approximately 0.8 seconds of remaining playback is time, not connection evidence. Added 3/4-client wait. |
| † `continueoff.mjs:56,131,136` | Seek near EOF; assert no continuation | E | Missing E could make the negative continuation check pass without observing clean EOF. Added 3/4-client wait before pause/seek. |
| † `completionhub.mjs:87,100,115` | Complete episodes; check watched-state curation and exact refresh behavior | E, R | Path readback and remaining playback time do not establish observers. Added 3/4-client wait. |
| † `serverplaylists.mjs:186,201,221,257` | Complete first and second items; check successor repaint and owner-loss behavior | E, R | Home/recents waits prove launch bookkeeping, not E readiness. Added 3/4-client wait before returning each player. |
| † `mergedview.mjs:17,50` | Finish one backing near EOF; check watched fan-out to both servers | E, R | Only mpv’s path was awaited. Added 3/4-client wait in the `finish` arm. |
| `mergedview.mjs:58` | Stop ordinary backing-selection probes; check selected stream and persisted override | No sampled mpv state | No exposure: these checks concern launch selection/persistence; shutdown is awaited. |
| † `playback.mjs:90,127,137` | Seek, stop; check final recent offset and Continue Watching | R | mpv progression plus 1.5-second delay normally spans retries, but is not readiness proof. Added 3/4-client wait before seek. |
| † `watchstate.mjs:69,83,89` | Seek, stop; check Jellyfin Stopped position and UI progress | R | Same timing-only coverage. Added 3/4-client wait before seek. |
| † `replaceplay.mjs:63,71,128` | Seek/pause A, start B; check A’s final position and B’s survival | R | Progression and 1.5 seconds do not prove R connected. `playing` now waits for 3/4 clients before handoff. B also uses this helper. |
| † `autocrop.mjs:67,106,111` | Park fresh play near 6 seconds, quit; subsequent play must resume there | R | Crop completion observes the script; 1.5 seconds permits observation but does not prove connection. Added three-client wait before parking/quitting. |
| `autocrop.mjs:136,140,148` | Off/Manual crop legs; quit after checking crop behavior | No sampled Vela state | No exposure: crop checks read mpv directly. Stopped existence does not require a sampled position. |
| † `embeddedcrop.mjs:68,93,180` | Seek/stop fresh play; check resumed position; exercise crop controls | R for resume; V for controls | Visible crop state proves V, not independent R. Added four-client wait after capturing the initial position, before handoff. |
| † `embeddedfullscreenend.mjs:288,326,350,413` | Three near-EOF releases and one explicit quit; check fullscreen/end lifecycle | E, R; V supplies view state | `viewReady` proves V, not E/R. Added four-client waits at all four connected-play boundaries. The deliberate failed-stream successor and Close-only leg were not given impossible startup waits. |
| `embeddedcontrols.mjs:95` | Pause, seek, mute, volume, fullscreen, stop | V | Covered: waits for the view’s first state before controls and checks subsequent UI state. Stop bookkeeping does not assert R’s sampled position. |
| `embeddedskip.mjs:84,117,123,148` | Pause and skip through page; verify marker/button disappears | V | Covered: the visible Skip Intro button proves V is receiving marker state before the mutation. |
| `markers.mjs:76,96,146` | Auto-skip, rewind/re-arm, quit; check jump and script injection | No Vela receiving connection for skip verdict | No exposure: `proveSkippedPast` reads mpv directly. The cleanup Stopped check asserts existence, not sampled position. |
| `markerbutton.mjs:132,174` | Seek into marker, inject mouse/Space input, quit | None for skip verdict | No exposure: script readiness, active marker, bounds and positions are read directly from mpv. |
| `markerpayload.mjs:38,48,78,88`; `cropscript.mjs:68,74,92,232` | Drive standalone mpv scripts, including toggles/loadfile/quit | None: Vela is not playing | A Vela-client count would never be reached. No wait added. |
| `embeddedplay.mjs:76,232,255` | Close one play, seek another toward EOF; check renderer/player/view shutdown | No sampled R value or clean-EOF distinction | Its two-second load delay precedes tracker startup in `Session::start`, so it is not readiness proof. These assertions concern internal player lifecycle; tracker settlement also completes on reader retry expiry. |
| `embeddedownconfig.mjs:69,153` | Check options, seek near EOF; check process/view ends | No sampled Vela state | No exposure to missing a sampled position/window/clean-EOF verdict; player lifecycle is driven internally. |
| `playerfallback.mjs:68,74,84,87` | Observe notice log, then quit | N sends notice; no receiving-state assertion | Existing 3.5-second notice allowance is time-based. This is sender delivery, not the missing-observer race. No persistent count imposed on transient N. |
| `embeddedlatestart.mjs:56,59,212,241` | Inspect abandoned/fallback players; check notice and quit | N for delivery; internal startup lifecycle | Positive notice delivery is awaited; second-play absence uses 3.5 seconds. Failed embedded startup and fallback do not have normal embedded-client counts. No wait added. |
| `exitplay.mjs:99,123` | Exit Vela; check app/player processes end | None for asserted outcome | Slot/process shutdown does not require IPC observations. Stopped reports are explicitly not asserted. |
| `mpvargs.mjs:80,108`; `search.mjs:73,80` | Read launch options/path, then stop | None for asserted launch facts | No observation-dependent post-stop assertion. |
| `sourcepolicy.mjs:150,155,187,190` | Read chosen stream, stop, check source policy/override | None for asserted selection facts | Selection and persisted override precede playback observation. |
| † `live/plex.mjs:392,406,407` | Seek episode to duration minus 0.8 seconds; check watched state and successor | E, R | Secure-path/duration reads observe mpv only. Added 3/4-client wait before seek. |
| `live/plex.mjs:413,423,469,495,520` | Failure cleanup, successor cleanup, paused movie probe | No sampled final-state assertion | No additional wait needed: relevant episode-completion checks precede successor cleanup; movie probe asserts its direct mpv reads. |
| `live/transcode.mjs:314,338` | Quit explicit-tier play; await encoder removal | R termination/on-end, not sampled state | `start_tracking_plex` invokes `on_end` even without a sampled position; encoder teardown follows that callback. The existing 90-second state wait covers retry-expiry cleanup. Explicit tier does not enable H. |
| `live/jellyfin.mjs` | Server failure/reporting checks | None | Plays nothing. |
| `../e2e-harness.test.mjs:529,546` | Connect to synthetic test sockets | None | No Vela play; deliberately excluded from the helper. |
| `positionsWhile` | — | — | No such helper exists under `tests/` at local head `5440907`. |

The remaining scenario files contain no running-play mutation:

| Places | What they test | Connection/wait/exposure |
|---|---|---|
| `combinedrecovery:22`, `configrecovery:45`, `configrollback:46`, `connectionrecovery:42`, `connectionrollback:35`, `recoveryresume:29` | Configuration recovery | None; no play |
| `imagepolish:269`, `uifoundation:218`, `uimotion:450`, `smoke:11` | UI and startup | None; no play |
| `markwatched:15`, `watchposition:281`, `mergedrefresh:67`, `pagefail:245`, `refresh:214`, `scanlib:76`, `sortpersist:16`, `sourcedeadend:21`, `playlistedit:82`, `plexdecision:28` | Server/UI actions and persistence | None; no play |
| `surfaces.mjs:66,110` | Deliberately failed launch diagnostics | Launch fails before a usable play/socket; no observer wait |

The 16 † files changed for the reasons above. The seventeenth, `tests/e2e-judge.test.mjs` (`tests/e2e-judge.test.mjs:476`), adds helper/wiring guards and updates the existing windowsetting guard. `velaConnected` (`tests/e2e/helpers.mjs:108`) polls `acceptedConnections`, accepts an injected `/proc` reader, and reports timeout context. Existing assertions and post-seek observation delays remain; no sleeps were added.

The six new helper tests are:

- `pw-8: velaConnected waits for all 3 clients on this socket`
- `pw-8: velaConnected waits for all 4 clients on this socket`
- `pw-8: velaConnected waits for all 5 clients on this socket`
- `pw-8: velaConnected accepts extra clients and reads /proc/net/unix by default`
- `pw-8: velaConnected times out naming the socket and required count`
- `pw-8: velaConnected rejects invalid counts before reading the kernel table`

The other 16 new test names are exactly **`pw-8: <file> <function> awaits Vela before <step>`**, with these substitutions:

| File | Function | Step |
|---|---|---|
| `helpers.mjs` | `playAndQuit` | `if (during) await during(mpv);` |
| `scenarios/windowsetting.mjs` | `play` | `return { mpv, session: recent.session_id };` |
| `scenarios/continueoff.mjs` | `run` | `await mpv.setProp('pause', true);` |
| `scenarios/continueon.mjs` | `nextMpv` | `await mpv.setProp('pause', true);` |
| `scenarios/continuetv.mjs` | `nextMpv` | `await mpv.setProp('pause', true);` |
| `scenarios/completionhub.mjs` | `startCompletedEpisode` | `await mpv.setProp('pause', true);` |
| `scenarios/playlistplay.mjs` | `nextMpv` | `return { mpv, socket };` |
| `scenarios/serverplaylists.mjs` | `nextPlayer` | `return mpv;` |
| `scenarios/replaceplay.mjs` | `playing` | `return mpv;` |
| `scenarios/playback.mjs` | `run` | `await mpv.setProp('time-pos', 6);` |
| `scenarios/watchstate.mjs` | `run` | `await mpv.setProp('time-pos', 6);` |
| `scenarios/mergedview.mjs` | `playFromMenu` | `await mpv.setProp('pause', true);` |
| `scenarios/autocrop.mjs` | `playAndAssertCrop` | `if (park !== undefined)` |
| `scenarios/embeddedcrop.mjs` | `play` | `return { mpv, socketPath, first };` |
| `scenarios/embeddedfullscreenend.mjs` | `run` | `await viewReady(driver);` |
| `live/plex.mjs` | `completeEpisodeAndContinue` | `await first.setProp("time-pos", Math.max(0, duration - 0.8));` |

## How it was checked

By codex, in its sandbox (no display, no listening socket): the helper's
six unit tests and one wiring test per place changed, all in
`tests/e2e-judge.test.mjs`; `npm run check`; and 36 red-proofs, each
restored from its own copy of the file and compared byte for byte (the
helper releasing one client early at 3, 4 and 5; not waiting; returning an
unawaited promise; the comparison made strict either way; the wrong
kernel table; the count not checked; each of the nineteen calls removed in
turn; and six ways of weakening single places).

By the coordinating agent:

- Read the diff; it merged onto `main` without conflict, and `npm run
  check` was green there (518 Node tests).
- Eight red-proofs of its own on the merged content, each failing the
  test meant for it: the helper letting a play go one connection early;
  the helper not waiting at all; the wait taken out of `playAndQuit` and
  of `playlistplay`; `windowsetting` asking for two connections, and
  waiting under a condition; `completionhub` waiting after its pause;
  `embeddedcrop` asking for three.
- One full run of each suite on the merged content (`main` at `33ea46a`
  plus the change). External: 47 of 48, the one failure
  `embeddedownconfig` (the recorded own-config defect). Embedded: 48 of
  50: `pagefail` (the recorded race) and `mergedview` once, on a card it
  had not waited for, a fault of that scenario and not of this change;
  five reruns passed, and `ad3b82d` makes the scenario wait for its card.
  No wait for a count hung in either player, so three and four are
  reached in every scenario changed.

`completionhub` had failed once in the embedded suite at `26660a9`
("timed out waiting for local suppression with the server transition
still pending") and that failure had not been looked into. The survey's
row for it fits: the scenario released the play to its end without
knowing that Vela's watcher for the end of the file was connected. That
this was the cause of that one failure is an inference; the scenario
passed in this full run.

## Not done, and what the count does not prove

- The repeated runs codex proposed for the most exposed scenarios
  (`windowsetting` 25 times; `playlistplay` and `continuetv` 10 times per
  player; a dozen others 3 times per player) were not made; each scenario
  ran once per player.
- The count proves that Vela is connected. It does not prove which client
  is which, that a subscription is complete, or that an event already
  sent has been read. A connection Vela may open later for such a play
  would let a wait be met early; that would show as the old failure, not
  as a false pass.
- `/proc/net/unix` makes the wait Linux-only, as the end-to-end suite is.
- The live scenarios (`tests/e2e/live/`) were changed in one place
  (`plex.mjs`) and were not run: they need a real server.
