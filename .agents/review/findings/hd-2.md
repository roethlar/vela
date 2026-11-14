# hd-2: a profile mpv applies by itself when a file loads can undo the HDR target

**Severity**: MEDIUM — mpv then draws something other than what the surface is tagged as, which is wrong brightness and colour for the whole play. It needs own-configuration mode and a protocol or extension profile in the user's mpv.conf that sets one of the three target options.
**Status**: In progress
**Branch**: —
**Commit**: `7639c0a`

## Evidence

Reviewer, verbatim: "session.rs:284-292 sets and tags the HDR target before `loadfile` at line 315. mpv applies automatic protocol/extension profiles during file loading. A user profile such as `[protocol.https]` with `target-trc=bt.1886` therefore overrides PQ after tagging. A headless libmpv test using the equivalent `[protocol.memory]` profile reproduced this override; no profile-contained include was involved."

## Predicted observable failure

Reviewer, verbatim: "mpv renders SDR-encoded pixels onto a surface still tagged PQ, producing visibly incorrect brightness and colour. With no compositor change, Vela never restores agreement."

Reviewer's suggested direction, verbatim: "Apply and verify the protected targets after automatic file profiles execute but before presenting video, for example through a load hook. Keep subsequent target changes coordinated with the surface description."

## What

In HDR, Vela tells mpv what it is drawing for (`target-trc`, `target-prim`, `target-peak`) and tells the compositor the same about the surface. Vela set them once, before the file was handed to mpv, and never looked again. Other things can set them afterwards: a profile in the user's own mpv.conf that mpv applies when a file loads (`[protocol.https]`, `[extension.mkv]`) or when a condition becomes true (`profile-cond`), and any command on mpv's IPC socket. mpv then drew for one thing while the surface said another, for the rest of the play, and nothing noticed. It needs the "use my own mpv configuration" setting, or something talking to the socket.

## Approach

The design this started from is `.agents/review/hd-3-hd-2.design.md`. One of its rules turned out to be wrong and was replaced while the fix was being attacked (see below); that file now says so.

- **mpv is asked to report its three targets** (`mpv_observe_property`, as text) as soon as it has started. A report reaches `hdr::target_changed`, the only thing that acts on it; `session.rs` passes it on and decides nothing.
- **While HDR is in effect, a reported value other than the one Vela asks for is put back**: the same three asks again, without touching the surface (it already says the right thing, and a newer description may be on its way, which would make tagging fail). Presentation is held meanwhile, as for any change of target (`hd-3`), and the finish is presented even on a paused picture. The peak is compared as a number, the other two as text. A value mpv cannot read counts as not Vela's.
- **In standard range nothing is defended.** The user's own targets are theirs, as they always were.
- **Which reports count.** The design said: a report counts as someone else's once mpv has answered Vela's ask for that option. An attacking helper showed against the shipped library that this misses a case that matters: mpv reads an observed option once per pass of its core and reports it only when it differs from what it read last time. If something sets the option back to the value Vela was last told, in the same pass as Vela's ask, nothing is ever reported (300 of 300 tries idle; while playing, every try within about a frame of Vela's ask), and Vela would finish believing its target holds. So now each answer to an ask makes mpv observe that option ANEW, under that ask's own number; mpv then reports the option once more as it reads it after the answer, whatever it is. A change of target finishes only when all three options have been answered and reported afresh, and a report counts only under its option's newest registration. This does not lean on the order of mpv's events at all.
- **A change made while one of Vela's own is in flight** is remembered, and the change is asked once more when it finishes, still held.
- **A fight ends.** More than five restorations within ten seconds, and Vela gives up: standard range on both sides, the ordinary way (untag, three `auto` asks), for the rest of the play; no later description from the compositor brings HDR back. One line on stderr, naming nothing from the user's configuration.
- **With mpv's queue for Vela full** (`hd-3`'s fault path): a restoration that cannot be sent goes to standard range like any refused HDR change; a give-up that cannot be sent waits, held, and is asked again once the queue has been read empty.

How it was made: written by one helper agent from the design, attacked by three (event ordering; guards that do not bite; mpv itself), which raised twelve findings, one of them the hole above; repaired in three rounds. The coordinator then read the product code, ran the full check, injected its own regressions (below) and ran it on a real HDR desktop.

## Files changed

- `src-tauri/src/player/embedded/hdr.rs` — `observe_targets`, `target_changed`, `agrees`, `values`; `Transition.unreported` and `.again`; `Tagging.{restorations, given_up, observing}`, `finish_when_done`, `restore`, `give_up`; `answered` observes its option anew; `output_described` honours the give-up. The rig gains someone else's sets (drawn at once, reported later, as mpv does) and the search over every order includes them. Five more tests against the real library (ten in all).
- `src-tauri/src/player/embedded/libmpv.rs` — `mpv_observe_property`, `mpv_unobserve_property`; `Event::TargetChanged`; `property_change`, which copies the name and the text out before the next call into mpv.
- `src-tauri/src/player/embedded/session.rs` — `set_up` observes right after mpv has started; `set_up`'s loop and `Live::run` each pass a report to `hdr::target_changed`.
- `tests/embedded-player.test.mjs` — the wiring, and that every line `hdr.rs` writes is fixed text. `tests/e2e/README.md` — what the real-library tests now cover.

## Guard proof

Run 2026-11-13 by the coordinator in a disposable worktree at the fix's content; `cargo +stable test --locked --lib embedded`, `node --test tests/embedded-player.test.mjs`, and `npm run test:libmpv` against the shipped library. The table is filled in from that run.

| Injection | Rust (stand-ins) | Node | Real library |
| --- | --- | --- | --- |
| Unchanged | PASS (130) | PASS | PASS (10) |
| Someone else's change, with nothing of Vela's in flight, is not put back | FAIL, 13 tests, among them `a_profile_applied_as_the_file_loads_is_undone_before_the_first_frame`, `someone_elses_change_is_put_back_in_every_order` | | |
| An answer no longer makes mpv observe its option anew (the hole the attack found) | FAIL, 9 tests, among them `a_value_set_back_in_the_same_pass_as_velas_ask_is_seen_and_put_back` | | FAIL `real_library_reports_a_value_set_back_in_the_same_pass_as_velas_ask` |
| Giving up is not remembered | FAIL `once_given_up_standard_range_stays_for_the_rest_of_the_play` and two more | | |
| The running play drops mpv's reports | | FAIL, two tests | |
| mpv is never asked to report its targets | (this injection did not compile, so the Rust side says nothing) | FAIL `mpv's reports of a changed target reach hdr from the start of a play to its end, and nothing else` | |
| A report's text is read as the pointer itself, not through it | FAIL `a_property_change_is_read_as_its_name_and_its_text` | | |

Also run by the coordinator at the fix's content: the full check (JS, Rust 1.89, stable and 1.100 clippy, 521 Rust tests); six scenarios in the embedded venue (6 of 6); and a scripted play of an HDR clip on `continuum`'s desktop (KDE, HDR on) in which a second IPC client changed each target in turn: `target-peak`, `target-trc` and `target-prim` were back at Vela's values after 82, 46 and 48 ms, with no dropped frame; nine more changes in quick succession then made Vela give up as designed (standard range, the play carried on).

The helper that wrote the fix ran 29 injections, each failing its named test, and an independent helper re-ran those belonging to the twelve findings. Among them, each alone: a value equal to Vela's own is put back; the peak is compared as text; standard range is defended; a report before its option's answer is acted on; a report under a replaced registration is acted on; the change remembered during a change in flight is forgotten at the finish; a restoration tags the surface; the sixth restoration in ten seconds does not give up, or the fifth does; the window never slides; the give-up leaves the surface tagged or mpv's targets as they are; a refusal is overridden by a remembered change; an extra restoration is made that nobody else's change called for (caught in every order); an option is not observed, or observed before mpv has started.

## Coder dispute (if any)

## Known gaps

- **A change in the middle of a play is seen a frame or two late.** mpv gives the thread that draws no notice: the next frame simply uses the new value, and the report can only be read after it. Measured before the fix was written: with a 24 frames a second file, one frame could be shown for the foreign target under the old description. A profile applied as the file LOADS is reported before the first frame (0.2 ms after the hand-over, in every run measured) and is put back before anything is shown.
- **Standard range is never defended, the give-up included.** If whatever Vela was fighting then sets an HDR transfer, mpv draws that under an untagged surface for the rest of the play. That is the user's own configuration doing what it says; it was so before this fix whenever Vela was not asserting HDR.
- **Giving up lasts for the rest of the play**, whatever the display does later.
- **Each restoration logs "HDR output, peak N nits" again** (at most five in ten seconds).
- **The tests against the real library run without a window and without a file.** There the order of mpv's events cannot show a per-option mistake; the stand-ins hold that part, in every order. The hole described above is held by both.
- **That nothing from the user's configuration is logged** is held for the ordinary mistake (a value put into a line) by a test that reads `hdr.rs`'s text; it is a tripwire, not a proof, and says so.
- **No end-to-end scenario reaches any of this**: the headless test compositor has no colour management. A scripted run on a real HDR desktop stands in (result in the plan's log).

## Reviewer comments

Raised by: codex / host default / host default / standard, codereview
generation pass over `74f44b6..e8db400`, 2026-11-13 08:38 UTC
(`.agents/review/codereview-2026-11-13-embedded-hdr.md`). Reviewer's
title: "Apply protected HDR targets after automatic file profiles". Reviewer's severity: MEDIUM.
