# Agent Decisions

Record durable repo decisions here. Do not use this as a chat log. Each entry
should make sense without conversation history and should name superseded
guidance when relevant.

## Decisions

### 2026-05-23 - Use external mpv playback for HDR

Status: Active

Decision:
Vela plays video through the system `mpv` binary in its own window. The webview
is used for browsing and controls, not embedded video playback.

Reason:
External mpv is the reliable path for HDR passthrough across Linux, macOS, and
Windows. The app records resume/progress over mpv IPC where the source supports
it.

Supersedes:
None.

### 2026-05-23 - Accept token-bearing media URLs as local-only exposure

Status: Active for server tokens (amended 2026-07-09: the SMB
mount-argument clause below is historical — SMB mounting was removed
2026-07-08 with the local/SMB/SSH sources; legacy SMB credentials persist
only as inert config fields under the same local-only stance)

Decision:
Plex, Jellyfin, and Emby poster/stream URLs may carry access tokens locally, and
SMB credentials may briefly appear in OS mount process arguments on platforms
that require it. Do not add new logs or error messages that expose those values.

Reason:
The current threat model accepts these as local-only exposure on the user's own
machine. Backend-only Plex calls use header auth where practical, but there is
no token proxy.

Supersedes:
The open policy question in `.review/deduped_action_list.md`.

### 2026-05-23 - Keep local media roots narrow

Status: Closed 2026-07-09 — code removed. The local-family sources this
governed were deleted in drop-local-sources slice 1 (0.1.33, `6855df5`;
decision 2026-07-08 "Vela is a multi-server client"). Historically
accurate while the code existed.

Decision:
Local folders and mounted remote folders must be validated as specific media
roots before the asset protocol or local source uses them. Filesystem roots and
the user's home directory are rejected, and listing/search/playback paths must
stay inside configured roots after canonicalization.

Reason:
The app intentionally serves local media artwork and files, but an overly broad
asset scope would increase the blast radius of a webview compromise or stale
config.

Supersedes:
The local asset scope concerns in `ISSUES.md` and `.review/gpt_review.md`.

### 2026-05-23 - Keep Linux SMB user-space only by default

Status: Closed 2026-07-09 — code removed. SMB and SSH sources were deleted
in drop-local-sources slice 1 (0.1.33, `6855df5`; decision 2026-07-08
"Vela is a multi-server client"). Previously partially superseded
(2026-07-04): the GVfs/KIO-FUSE resolution mechanism was replaced by the
native in-process client — see "Linux SMB goes native" (2026-07-04).

Decision:
On Linux, Vela resolves readable GVfs/KIO-FUSE SMB mounts created by the user's
desktop session and does not request root or invoke privileged CIFS mounting by
default. SSH/SFTP folders use `sshfs` with OpenSSH keys, agent, and config; Vela
does not store SSH passwords.

Reason:
Remote mounts should not require privilege escalation from the app. The local
source can browse a user-space mount once the OS or desktop session exposes one.

Supersedes:
None.

### 2026-06-10 - Standard agent guidance is canonical

Status: Active (amended 2026-07-08: `.agents/repo-map.json` was retired by
that day's governance refresh — its verification commands were carved into
`.agents/repo-guidance.md`. The CURRENT canonical set is `AGENTS.md`,
`.agents/repo-guidance.md`, `.agents/state.md`, and `.agents/decisions.md`;
the Decision text below predates the refresh and still names repo-map)

Decision:
`AGENTS.md`, `.agents/state.md`, `.agents/decisions.md`, and
`.agents/repo-map.json` are the canonical agent guidance and memory files. The
`.review/` files are retained as historical review evidence, not updated as
current state.

Reason:
The repo needs one discoverable current-state entry point and one decision log
so future work does not reconstruct state from historical review documents.

Supersedes:
Current-state use of `.review/deduped_action_list.md` and `.review/gpt_review.md`.

### 2026-06-20 - Bump version on code change, not on build

Status: Active

Decision:
Vela's version is bumped (via `scripts/bump.sh`) as part of a code change, not
at build time. `scripts/build.sh` builds the host platform's installable bundle
and must not change the version.

Reason:
A build is only meaningfully unique when the source is. Tying the version to
builds produces version churn with no code difference; tying it to code edits
makes each version correspond to a real change. The version is shown in the
window footer and the bundle filename.

Supersedes:
The prior "bump on every build" rule previously stated in the `scripts/bump.sh`
header comment.

### 2026-06-28 - Letterbox/IMAX black-bar cropping on ultrawide (OPEN)

Status: Resolved by the 2026-07-03 crop decision below; the confirmed facts,
constraints, and owner requirements recorded here remain valid evidence.

Problem:
On a screen wider than the content, video mastered narrower than its container
shows black on all four sides. Example: a 2:1 picture baked into a 16:9 container
with hardcoded top/bottom bars, played on a 2.37:1 (5120x2160) ultrawide. mpv
fills the height with the 16:9 frame and pillarboxes left/right, so the baked
top/bottom bars plus mpv's side bars produce a four-sided border. Vela passes mpv
no fullscreen/scaling/crop args and runs `--no-config` by default, so this is the
source geometry, not a Vela launch-arg bug. Confirmed on a Plex 4K HDR file
(3840x2160 container, true picture 3840x1920 / 2:1, 120px baked bars top and
bottom; correct crop `3840x1920+0+120`).

Confirmed facts and constraints (treat as durable evidence):
- Detection: stock mpv `cropdetect` (limit=24) and the `osd-dimensions` property
  both gave misleading readings on HDR/PQ content. `osd-dimensions` only reports
  mpv's own window padding, never bars baked into the frame. The reliable signal
  was per-decoded-row pixel brightness on an extracted frame.
- Safety: applying the `video-crop` property live over the mpv IPC socket
  (observed while paused) on the gpu-next / Vulkan / Wayland / HDR stack wedged
  mpv into uninterruptible (D-state) I/O twice; even SIGKILL could not reap it
  until the blocked I/O returned. Therefore any crop must be applied at mpv
  launch, not mid-stream. Dynamic per-scene re-cropping is considered unsafe on
  this setup unless a future spike proves a live mechanism (e.g. render-only
  `video-zoom`/`panscan`, which avoid a VO reconfigure) does not wedge.

Owner requirements gathered so far:
- Must work for any video.
- No dynamic per-scene cropping (rejected: it changes the zoom/bars per scene).
- Target model: a single static crop equal to the bounding box of the
  largest-picture scene (maximum picture / minimum bars across the whole file),
  applied once at launch. Narrower scenes keep their own bars; real picture is
  never clipped. For uniform-aspect files this reduces to the obvious crop.
- That model needs whole-file lookahead (a pre-scan) or a per-file cache, because
  frame 0 cannot know which scene is widest.

Open question (undecided):
Owner rejected all three first-time scan-timing strategies offered
(fast sampled scan + background refine; first play uncropped + cache for next;
block until full scan). The live choices are: (a) a no-scan manual per-file crop
(user sets the aspect/crop, Vela remembers it and applies it at the next launch);
(b) drop the feature; (c) another approach not yet framed. Resolve before any
design or code.

Supersedes:
None.

### 2026-07-03 - Letterbox crop: detect during first playback, correct once

Status: Active (direction decided; render-zoom spike gates the mechanism;
design plan approval still required before code)

Decision:
First-ever play of a file launches immediately, uncropped. A sampled
bar-detection scan runs concurrently against the same stream; when it
completes, Vela applies exactly one correction to the running player and
caches the crop per file. Every later launch of that file applies the cached
crop in mpv's launch arguments (static, never changes mid-play). The
correction mechanism is gated on a safety spike: prove whether render-only
`video-zoom`/`panscan`/`video-align-y` adjustments avoid the VO-reconfigure
D-state wedge on the gpu-next/Vulkan/Wayland/HDR stack. If the spike passes,
the one-time correction happens in place via render-space properties; if it
fails, Vela relaunches mpv at the current position with the crop in launch
arguments (a fresh launch avoids the live `video-crop` path entirely).

Non-goals: no per-scene dynamic cropping (still rejected), no detail-page or
library prefetch scanning, no external metadata (IMDb/TMDb aspect-ratio)
lookup, no manual crop UI unless the automatic path proves insufficient.

Reason:
Owner requirements: works for any video; never clip real picture (crop equals
the bounding box of the largest-picture scene); nothing may repeatedly change
during playback. All scan timings that delay or degrade launch were rejected;
the owner explicitly chose one early correction on first play (option "3/5")
over a manual per-file crop and over dropping the feature. Live `video-crop`
over IPC is unsafe on this stack (see the 2026-06-28 entry).

Supersedes:
The open question in the 2026-06-28 entry, and that entry's "single static
crop applied once at launch" target model for the first-ever play of a file
(later plays still match it via the cache).

### 2026-07-03 - Keep human titles in mpv; move Plex stream auth out of the URL

Status: Active

Decision:
Commit `d35cfe3` (mpv `--force-media-title`/`--title` driven by the human
title) is kept; it is no longer WIP. The remaining token surface is closed
rather than accepted: Plex stream URLs handed to mpv no longer carry
`X-Plex-Token` as a query parameter. The token is supplied as the
`X-Plex-Token` HTTP header via a per-launch mpv include file with owner-only
permissions — never on the mpv command line, which would expose it in the
process argument list. Jellyfin/Emby stream-URL parity is a recorded
follow-up, not part of this change.

Reason:
Window titles leak passively off the machine (taskbars, window lists,
screen-share pickers), which justifies keeping d35cfe3. mpv renders `${path}`
in more surfaces than the title (the `Shift+I` stats overlay's `File:` line,
playlist display), so suppressing the title alone leaves residuals; removing
the token from the URL cleans every mpv surface at once and extends the
existing "backend-only Plex calls use header auth where practical" stance
(2026-05-23) to the stream itself.

Supersedes:
Refines the 2026-05-23 "Accept token-bearing media URLs as local-only
exposure" entry: Plex stream URLs are no longer token-bearing once this
lands; poster/photo-transcode URLs and SMB mount-argument exposure remain
accepted local-only exposures.

### 2026-07-04 - macOS SSH: keep sshfs, add in-UI setup guidance; live testing parked

Status: Closed 2026-07-09 — code removed. SSH/SFTP sources (and the sshfs
guidance UI this decision mandated) were deleted in drop-local-sources
slice 1 (0.1.33, `6855df5`; decision 2026-07-08 "Vela is a multi-server
client").

Decision:
SSH/SFTP sources keep the `sshfs` dependency on macOS (no in-app SFTP client
for now). Vela's add-SSH UI must carry platform-aware help: state the sshfs
requirement up front rather than at add-failure time, and on macOS describe
the actual working route — the `macfuse` cask plus a macFUSE-compatible sshfs
build (e.g. `gromgit/fuse/sshfs-mac`), including system-extension approval
and, on Apple Silicon, the Recovery reduced-security step — with a caution
that these builds can be unstable. macOS SSH live smoke testing is parked:
the brew macFUSE/sshfs-mac builds segfault or act oddly on the owner's
machine (2026-07-04), so Vela documents that stack rather than owning it.

Reason:
Homebrew core's `sshfs` cannot install on macOS (Linux-only libfuse
dependency), so the app's bare "Install sshfs, then try again" error sends
macOS users into a dead end the owner hit verbatim; the working tap route
then proved unstable on the owner's machine and carries real friction
(closed-source kext, extension approval, Recovery step, restart). An in-app
SFTP client is more scope than the feature currently justifies. Honest
in-UI guidance is the supportable stance.

Supersedes:
The open product question in `ISSUES.md` ("Open - Owner-Reported
(2026-07-04)", SSH entry) about whether macOS SSH should depend on macFUSE
or switch to an in-app SFTP client. The Linux sshfs stance from 2026-05-23
is unchanged.

### 2026-07-04 - UI design language: steer toward the Infuse reference

Status: Active

Decision:
Vela's browsing UI moves toward the design language of the owner-supplied
Infuse screenshot at `reference_screens/infuse-home-reference.png`
(committed with this entry) — a direction, explicitly not a pixel clone.
Concrete direction (refined by the owner the same day):
- Continue Watching is a single hero carousel, not a card row: one large
  centered 16:9 card showing the most recently watched item — scene still
  for episodes, backdrop art for movies — with the progress bar and a
  title + S·E/episode-name caption. Prev/next arrows float overlaid on the
  hero image's left/right edges (not separate side controls) and swap the
  hero through the other recent items.
- Other resume-style content (e.g. On Deck) uses the same landscape artwork
  rules; whether it folds into the hero rotation or stays a row is settled
  in the artwork plan.
- Catalog rows and library grids are uniform 2:3 posters; episodic entries
  show series artwork there, not episode stills.
- Navigation trends toward a sidebar structure: Home, a consolidated
  Library, and per-connection Files entries (matches the library/All-view
  rework direction).
Explicitly excluded from the reference: the Favorites tile row (superfluous
per owner). Specifics land only through approved plans.

Reason:
Reviewing the artwork plan's open question (in-progress movies: poster or
content view?), the owner supplied Infuse as the reference and asked that
Vela get closer to its design language (2026-07-04). The split policy keeps
every row internally uniform — the original complaint — while resume rows
read as scenes in progress.

Supersedes:
The "poster-uniform hub rows" primary proposal in
`.agents/plans/row-artwork-consistency.md` (updated to the split policy) and
that plan's poster-vs-landscape open point. Informs, not replaces, the nav
phases of `.agents/plans/library-all-view-rework.md`.

### 2026-07-04 - Hero is a cover-flow fed by Vela's own recency

Status: Active (amended 2026-07-09: the "local/SMB plays appear" clause
below is historical — those sources were removed 2026-07-08. The
source-agnostic recents mechanism itself is unchanged and now serves
server items only)

Decision:
The Continue Watching hero becomes a cover-flow (owner reference: foobar2000
album wall): the current item front-and-center capped at ~30% of the window
height, older items fanned behind-left, newer behind-right, side cards and
always-visible arrows both navigate — hover-revealed controls are dropped
(they read as no controls at all). Its content follows the owner's semantic
"recently played and not finished = Continue Watching": Vela records its own
recents at play time (item snapshot; final position stamped at mpv exit) and
the hero shows recents merged with the server continue-watching hubs, newest
first, deduped. Entries past the watched threshold (config
`watched_threshold_percent`, default 95%) drop out as finished. This makes
the hero source-agnostic (local/SMB plays appear) and independent of Plex's
server-side ~60s resume threshold. Home renders ONE consolidated hero;
per-source scoping filters it.

Reason:
Owner playtest 2026-07-04 (v0.1.7): played a video for a few seconds,
stopped — the hero never changed and showed no controls. Two real causes:
Plex never registered the short play (server threshold), and with a
one-item hub the hover-revealed arrows rendered nothing. The owner's stated
expectation is recency semantics, which only a client-side record satisfies.

Supersedes:
The hero-carousel shape in the 2026-07-04 design-language decision (single
centered card, hover-revealed overlay arrows). The split artwork policy and
the rest of that decision stand.

### 2026-07-04 - Linux SMB goes native: in-process client + loopback stream proxy

Status: Closed 2026-07-09 — code removed. The native SMB client, loopback
proxy, and `velasmb:` scheme were deleted in drop-local-sources slice 1
(0.1.33, `6855df5`; decision 2026-07-08 "Vela is a multi-server client").
Kept as history; note the implementation used `pavao-sys`, not the `pavao`
crate named below (deviation recorded in
`.agents/plans/smb-native-client.md`).

Decision:
On Linux, Vela speaks SMB itself: browsing/listing/search through an
in-process libsmbclient-backed client (`pavao` crate), and playback through a
localhost-only HTTP Range proxy that translates mpv byte-range requests into
SMB reads. The GVfs/KIO-FUSE mount-resolution path, `gio mount` nudge, and
boot remount are removed on Linux. macOS (`mount_smbfs`) and Windows
(`net use`) keep their OS-mount flows; taking those native is a separate
future decision. Plan: `.agents/plans/smb-native-client.md`.

Reason:
Owner rejected the mount dependency outright (2026-07-04, hit on Arch/KDE
with no gvfs and no active KIO mapping): "if Vela cannot make the connection
itself without the underlying OS mount, it's worthless." mpv on the owner's
system has no smb:// protocol support, so playback requires the proxy, not
just native browsing.

Supersedes:
The *mechanism* of the 2026-05-23 "Keep Linux SMB user-space only by
default" entry (resolving desktop-session GVfs/KIO-FUSE mounts). Its
*constraint* stands: no root, no privileged CIFS mounts. The SSH/sshfs
stance in that entry is unchanged.

### 2026-07-04 - Owner delegation: progress must not block on the owner

Status: Active as a working principle (amended 2026-07-09: of the named
approvals below, the SMB share-root plan is CLOSED-obsolete and the E2E
harness's "live SMB probe" leg died with the 2026-07-08 local/SMB/SSH
removal — the harness itself landed and was re-homed to mock servers;
continue-watching-curation landed 2026-07-04. The delegation rule and its
owner-in-the-loop boundaries remain in force)

Decision:
The owner directed (2026-07-04, verbatim intent: "I need progress to pick
up. however we can do that. I can't be the delay.") that queued work whose
direction he has already chosen proceeds without a further per-plan
approval round-trip. Concretely this approves for implementation:
- `.agents/plans/smb-share-root-autoadd.md` (direction chosen 2026-07-04),
- `.agents/plans/continue-watching-curation.md` (choices locked 2026-07-04),
- building an automated end-to-end test harness (WebDriver/tauri-driver UI
  automation plus mpv-IPC playback checks plus the live SMB probe) so
  routine playtesting no longer requires the owner; the harness design
  still lands as a written plan in `.agents/plans/` before its code.
The owner stays in the loop only for what physically requires him: visual
judgments (HDR passthrough, artwork look), release/version calls,
credentials, and destructive or outward-facing actions. This is NOT a
blanket approval for new feature directions the owner has not chosen;
"no code change without an approved plan" still stands — this entry is the
approval for the plans it names, and future plans with owner-locked
choices may cite it instead of waiting when the owner is unavailable.

Reason:
The owner is the current throughput bottleneck and explicitly asked not to
be. All three named items had their product choices made by the owner
before this entry; only the approval formality was pending.

Supersedes:
Nothing structural. Narrows the per-plan approval wait for the named items
and for future owner-locked plans.

### 2026-07-04 - On Deck folds into the Continue Watching flow

Status: Active

Decision:
Plex On Deck items are part of the hero cover-flow, interleaved with
recents and continue-watching items by last watch activity (Plex
`lastViewedAt`; Vela recents' `ended_at_ms`), newest first; items with no
timestamp follow the timestamped ones in feed order. Vela builds its own
On Deck hub from `/library/onDeck` (synthetic id `vela.ondeck`) because
the `/hubs` On Deck hub is server-controlled and often absent. There is
no separate On Deck row. Jellyfin `/Shows/NextUp` and Emby equivalents,
and Jellyfin/Emby last-watched timestamps, are recorded follow-ups.

Reason:
Owner choices locked 2026-07-04 (plan
`.agents/plans/continue-watching-curation.md`): fold in, no row,
interleave by recency. On the owner's server, `/hubs` returns no On Deck
hub, so an in-progress movie was invisible in the flow.

Supersedes:
The "On Deck ... uses the same landscape artwork rules" 16:9-row
treatment in the 2026-07-04 "UI design language" decision; the hero
cover-flow decision's merge ordering (was: recents then hub order — now
recency-interleaved across both feeds).

### 2026-07-05 - Letterbox crop feature DROPPED

Status: Active

Decision:
Vela ships no letterbox/black-bar cropping feature. Owner ruling
(2026-07-05): "this is mpv's problem, not Vela's." The draft plan
`.agents/plans/letterbox-crop.md` is deleted; no spike, no design, no
code. Users who want bar cropping can use mpv's own facilities via the
existing `mpv_extra_args` config passthrough.

Reason:
The owner does not recall choosing the 2026-07-03 direction and, on
re-review, rejects the feature outright as out of Vela's scope. The
scope boundary is durable: Vela launches and controls mpv but does not
re-implement video-geometry processing.

Supersedes:
The 2026-07-03 "detect during first playback, correct once" decision
(entirely) and the 2026-06-28 open-question entry's target model. The
2026-06-28 entry's CONFIRMED FACTS remain valid durable evidence for
anyone touching mpv on this stack: live `video-crop` over IPC can wedge
mpv into D-state on gpu-next/Vulkan/Wayland/HDR, and cropdetect /
osd-dimensions readings are unreliable on HDR/PQ content.

### 2026-07-05 - Ship mpv's autocrop.lua behind an opt-in toggle

Status: Active

Decision:
Vela bundles mpv's unmodified `autocrop.lua` (GPLv2+, provenance recorded) as a
resource and adds an off-by-default, three-state Settings control
(Off / Manual / Automatic) that injects mpv `--script` launch args. Off injects
nothing. Manual appends `--script=<bundled>` plus
`--script-opts-append=autocrop-auto=no`, so cropping fires only on an explicit
in-player `Shift+C`. Automatic appends `--script=<bundled>` with the script's own
`auto=true` (crop at every playback start). Vela writes no crop logic; all
geometry processing remains mpv's. The Automatic mode auto-fires the recorded
live-`video-crop` D-state hang path and is therefore an explicit, non-default,
owner-chosen opt-in guarded by a prominent UI warning; Off/Manual carry the
`auto=no` code guard. Plan (converged, codex reviewloop r1-r4):
`.agents/plans/mpv-autocrop-bundle.md`.

Reason:
Owner reversed the 2026-07-05 drop after confirming the script works via manual
`--script=`/`Shift+C`, and wanted it distributable without users hand-managing the
file (2026-07-05: "add it, then approved"), with both automatic and manual modes
user-selectable. Bundling + a toggle is an extension of the prior decision's
endorsed `mpv_extra_args` passthrough, not a re-implementation of crop logic.

Supersedes:
The 2026-07-05 "Letterbox crop feature DROPPED" decision for this narrow
bundled-script case — specifically its "ships no crop feature", "no design/no
code", and "existing `mpv_extra_args` passthrough only" clauses. What REMAINS in
force from that entry (not superseded): the scope boundary that "Vela launches and
controls mpv but does not re-implement video-geometry processing" (this ships
mpv's own script, writes no geometry code), and the 2026-06-28 confirmed facts
(live `video-crop` D-state wedge; unreliable cropdetect on HDR/PQ) — which are the
reason for the `auto=no` guard on Off/Manual and the warning on Automatic.


### 2026-07-08 - Plex-first item detail: uniform nav flip, non-Plex detail backends deferred

Status: Active (amended 2026-07-09: the LOCAL `item_detail` deferral below
is DEAD, not deferred — local sources were removed by the same-day
"multi-server client" decision; JF/Emby is the only backend still deferred
on an owner go)

Decision:
For the item-detail-view feature (`.agents/plans/item-detail-view.md`), sources
other than Plex are deprioritized: the Jellyfin/Emby and local `item_detail`
backends (the plan's original slices 2-3) are DEFERRED until the owner picks
them back up; do not start them without an explicit owner go. Navigation still
flips uniformly for every source: in library views a movie click opens the info
page, a show drills seasons then episodes, an episode opens the shared info
page; the Continue Watching carousel keeps click-to-play everywhere. Non-Plex
items open the same detail pages rendered sparse from listing data (`ItemDto`);
a failed `get_item_detail` fetch (e.g. a deferred backend's graceful `Err`
default) falls back silently to listing data and never surfaces an error page.

Owner wording (2026-07-08): "sources other than plex are deprioritized. get
this perfect with plex, then we'll worry about the others." and "plex items
only go to detail page from library views, not from continue watching
carousel. other sources should behave the same way."

Reason:
The owner is primarily a Plex user (consistent with the 2026-07-06 Plex-first
reprioritization of the SMB/SSH work) and wants the detail surface polished for
Plex before effort goes to other backends, without forking navigation semantics
per source.

Supersedes:
The backend-coverage half of the plan's "no half-built state" ruling
(2026-07-06): the navigation flip no longer waits for JF/Emby/local
`item_detail`; it waits for a polished Plex surface plus a clean (never
broken/empty/erroring) sparse page on other sources. Unchanged: the routing
spec (CW carousel plays; library views drill), build-behind-the-nav with the
flip landing last, and the per-slice commit + reviewloop discipline.


### 2026-07-08 - Vela is a multi-server client: local/SMB/SSH playback dropped

Status: Active (slice 1 — turn-off-and-delete — landed 2026-07-08, 0.1.33
`6855df5`, owner-playtested; slice 3 docs sweep landed 2026-07-09; only
slice 2, the E2E re-home, remains — `.agents/plans/drop-local-sources.md`)

Decision:
Vela will not play local files at all. The local-family sources (local
folders, SMB shares, SSH/SFTP mounts) are to be REMOVED: sources are media
servers only (Plex now; Jellyfin/Emby remain supported because the owner
plans to eventually migrate from Plex to one of them). No file browser, no
"open with Vela" plumbing, no local library. The removal is a separate
planned track (deletion plan to be drafted and reviewed before any code
change).

Owner wording (2026-07-08): "no need for vela to play local files at all.
multiple servers stay. I will eventually migrate from plex to emby or jf."
Context: the owner rejected every library/browse/direct-play framing for
files ("zero value to accessing the same file via ssh, smb, plex, emby, and
jellyfin"; "I already have file explorer, finder, dolphin").

Watch-state transfer for the eventual migration (owner-refined 2026-07-08):
the GOAL is a one-shot direct Plex -> Jellyfin/Emby copy built into Vela
(match by normalized provider ids; copy played + resume position),
contingent on it proving simple when planned ("if it's simple to do it
directly, that will be the goal"). Trakt relay tooling is NOT the plan —
the owner's Trakt use lives in Infuse, not in Plex, so PlexTraktSync-style
relays don't fit; if the direct copy turns out non-simple, an alternative
gets found then. No continuous sync daemon in Vela either way. Assess and
plan at migration time; nothing is built now.

Reason:
The local family was a pseudo-library: kind detection, filename parsing,
sidecar/online metadata scraping, listing caches, and title+year dedup
produced bare cards and a long defect tail (Bug 4, metadata revalidation,
reload-on-open) for a use case the owner does not have. The owner is a
server-library user; playback quality work (mpv/HDR) applies to server
streams the same way.

Supersedes:
- The 2026-07-06 deferral of Bug 4 (share-root classification), the
  metadata "Recently added" rail, and the SMB metadata-revalidation plan
  (`.agents/plans/local-metadata-revalidation.md`): all three CLOSE as
  obsolete rather than parked - their subject matter is being removed.
- The local slice (original slice 3) of `.agents/plans/item-detail-view.md`:
  dead, not deferred.
- The 2026-05-23 local-media-roots narrowing decision and the 2026-07-04
  SMB native-client decision remain historically accurate for the code
  while it exists, but the code they govern is slated for removal; they
  close when the removal lands.
NOT superseded: multi-server support, the merged All view and its
dedup/backing machinery (server-to-server overlap becomes real during the
eventual Plex -> JF/Emby migration), mpv delegation, and the token-handling
stance.

### 2026-07-10 - Watched-state edits curate Continue Watching in one op

Status: Active

Decision:
Mark watched and Mark unwatched (any surface: grid, carousel, search,
person grid) both flip the server state AND remove the item from
Continue Watching in the same operation - the recents entry is dropped
and the item's identity set is tombstoned, curate-first with a rollback
if the server edit fails. "Remove from Continue Watching" stays the
dismiss-only op: it never touches watched state or progress. Any play of
an item clears its tombstone, on every play path (direct plays via
record_recent, queue/auto-advance via play_by_key untombstone).

Reason:
Owner reports 2026-07-08 ("two ops to get what I want") and 2026-07-10
(watched status could not be changed from anywhere while an item sat in
Continue Watching). The hero carousel merges Vela's local recents
snapshot - frozen at playback time and deliberately winning the dedup -
with the server continue/On Deck hubs; only tombstones suppress an item
across all feeds. Without curation on both edit directions, the stale
local snapshot masked every server-side watched-state change until the
item was manually removed.

Detail and residual-race dispositions:
`.agents/plans/continue-watching-watch-state.md` (design, accepted
edges, and the 6-round plan-review trail incl. one finding contested and
routed to the owner).

Supersedes:
The implicit prior semantic (mark-watched dropped recents only,
mark-unwatched deliberately left the entry; commands.rs set_watched
comment before 02504be).

## 2026-07-14 - Failures report on the surface that owns them, not on one shared banner

Status: APPROVED (owner, 2026-07-14). Implementation plan:
`.agents/plans/per-surface-status.md`.

Decision:
Every failure is reported on the surface it belongs to, and is cleared by
that surface. The top error banner keeps only VIEW-scoped failures (the
listing, the refresh, the search). The play queue, the mpv setup bar and the
open detail page each report their own. This extends the r15 ruling that gave
the library scan its own status line, to every writer that shares the banner.

A failed WATCH-STATE EDIT (mark watched / unwatched / remove from Continue
Watching) also gets its own action line, in the owner's words "its own line" -
it is an action the user took, not a fact about the grid. That writer caused
most of the loop's defects, and its own line is what removes the machinery
(the root-identity gate) built to keep it from fighting the view's banner.

Reason:
The shared banner is written by surfaces with four different lifetimes, and
the code had no way to say which failure belonged to which. Every clear was
therefore either too wide (silently erasing a failure the user still needed)
or too narrow (stranding a diagnostic over a view that no longer exists). The
library-refresh-scan code-review loop found EIGHT consecutive rounds of this,
each one a new door into the same silent loss, several opened by the fix for
the previous one: r18 (publish), r19 (ordering), r20 (retract), r21 (dedup),
r22 (setError), r23 (a `linking` flag), r24 (setError(null) again). Patching
the shared banner has a demonstrated defect rate of one new door per round,
and three of the four surfaces cannot be guarded by the E2E harness at all.

Evidence:
`.agents/plans/library-refresh-scan.md` `## Code review log`, rounds r17-r24
(two independent reviewers per round; they converged on the same top finding,
without seeing each other, in six straight rounds).

Supersedes:
The `owner`-enum-on-a-shared-part model landed in `da99a46` as the interim fix.
That model is coherent and stays until this plan lands - it is the thing this
plan replaces, not a thing to build on.

## 2026-07-14 - There is no play queue; playlists are the only sequence

Status: APPROVED (owner, 2026-07-14). Implementation plan:
`.agents/plans/playlists.md`, which owns the full model and the slices.

Decision:
VELA HAS NO PLAY QUEUE. "Add to queue", "Play Next", the queue chip and the
queue drawer are DELETED. Playback context is a single item, or a named
playlist. Do not reintroduce an ephemeral queue in any form without an explicit
owner decision.

The owner's reasoning, recorded because it is the justification for deleting
shipped code: an ephemeral queue is a MUSIC idiom, not a video one. The only
preset video sequence worth having is a show binge - and there the sequence IS
the show's own episode order, which Continue Playing already walks. Anything
larger (a movie series; a meta-series like "all Star Trek shows in order") is a
real named playlist. Infuse has no Up Next queue for exactly this reason: its
verbs are play, or add to a named playlist. That is the model people expect.

NAMED PLAYLISTS are durable objects in a Playlists sidebar entry. PLAYING ONE
NEVER MUTATES IT - a cursor walks the list; the list does not change. Vela's own
playlists may mix items from DIFFERENT servers in one list, the thing no single
server's playlist API can represent, and the reason this is a Vela-native
feature. The servers' own playlists appear alongside them, READ-ONLY.

THE PLAY VERBS ARE: Play (item with no resume position); Resume AND Play from
Beginning, as two explicit choices (item in progress); Add to Playlist ->.
Everywhere playback can be started.

THERE IS NO RESUME PROMPT AND NO COUNTDOWN. It was only ever wanted for an
in-progress item reached by AUTO-ADVANCE, and mpv owns the screen by then, so
there is nowhere to draw it. Auto-advance onto an in-progress item resumes
silently. This is a direct consequence of the external-video decision below; if
embedded video were ever adopted, revisit it.

THE CONTINUE WATCHING CAROUSEL IS UNCHANGED - recents union server hubs, as
today. Playlists never appear in it. What DOES change is that plays finally
register in it (see Reason).

CONTINUE PLAYING is a three-mode setting consulted when a playlist ends or a
single item finishes: `off` stops; `on` keeps walking down Continue Watching;
`only-tv` plays the next episode in order, rolls into the next season, and stops
when the show runs out. Default `only-tv`. "Next episode" means strictly the
next in order, watched or not, so a deliberate rewatch keeps rolling. THIS IS
THE BINGE MECHANISM, and it is what replaces the queue for the only preset video
sequence the owner wants.

PLAYLISTS ARE STORED IN THEIR OWN JSON FILE, not in `config.json` and not in a
database. The criterion for splitting a store out of `config.json`: the data
grows without a bound the user controls, AND losing it would be far less bad
than failing to load the config. Playlists meet both. Recents (capped at 20),
Continue Watching tombstones (capped at 200) and the per-library sort map meet
neither, and stay in `config.json`.

Reason:
The owner's complaint was that the queue does not survive a restart. Tracing it
found a second, INDEPENDENT defect the queue was hiding: the carousel does not
reflect anything played through the dispatcher AT ALL, because `play_by_key`
records no recent (`commands.rs:2365` says so outright), so Vela's half of the
hero merge stays empty and only the server's hub half moves. That bug survives
the queue's deletion and is the plan's S2.

The `on` mode must walk the SAME Continue Watching list the carousel renders,
never a fresh server query. A second source of truth for "what plays next" would
diverge from the first, which is precisely the failure class the per-surface
status decision (above) was created to kill.

SQLite was considered and rejected: it is a new native dependency touching every
packaging target, and playlists need none of what a database provides (indexed
queries, partial reads, concurrent writers, transactions). The one store that
would have justified it - an unbounded metadata cache - no longer exists; it
died with the local-source removal (2026-07-08).

Supersedes:
- The play queue itself: `AppState.queue` / `queue_index` (`lib.rs:61`), the six
  `queue_*` commands (`commands.rs:2396-2477`), the chip and drawer, and
  `play_item`'s queue-clearing behavior (`commands.rs:2380`). Also
  per-surface-status slice 2 (`67358fd`), which gave the queue drawer and chip
  their own status line - it goes with the queue, as does step 2 of the 0.1.48
  playtest ask.
- An earlier SAME-DAY draft of this decision (committed `9426f75`, never
  implemented) recorded an "Up Next" ephemeral consumption queue persisted
  alongside named playlists, with a carousel melding the two. The owner rejected
  that model on the same day: it is a music idiom, and a scratch list that exists
  only to protect saved playlists from playback edits is unnecessary once the
  queue verbs it served are gone. Git history holds the original.

## 2026-07-14 - Video stays external; embedding mpv is a spike, not a plan

Status: APPROVED (owner, 2026-07-14). Reaffirms and hardens the 2026-05-23
decision "Use external mpv playback for HDR" with the technical findings that
answer the question directly.

Decision:
Vela does not embed video in the webview. The question - asked by the owner at
the start of the project and again on 2026-07-14 - is now ANSWERED rather than
merely deferred: embedding is not planned, and if it is ever revisited it is a
SPIKE, not a plan. The first question any such spike must answer is "does HDR
passthrough survive?", because a No there kills the idea outright.

No feature may be designed as depending on embedded video. In particular, the
in-app resume prompt ("Continue from <time> / Start from beginning") does NOT
need it: the prompt fires when the user clicks Play, while the Vela window is
still frontmost and mpv has not been spawned.

Reason:
Three routes exist, and each fails on the platform Vela targets first:

- mpv's `--wid` foreign-window embedding is X11/Windows/macOS only. WAYLAND HAS
  NO PROTOCOL for embedding another process's surface into your window.
- The "float a borderless mpv window over a div and track its rect" hack needs
  absolute window positioning, which Wayland deliberately denies clients.
- Linking libmpv and driving its render API against a GL surface inside Vela's
  own window DOES work in principle (it is what native GTK players do), but it
  must be built separately for Linux, macOS (where Apple deprecated OpenGL) and
  Windows - by a wide margin the largest engineering effort in this repo - and
  it puts the video behind a toplevel that is not an HDR surface. The likely
  result is tone-mapping down to SDR, which forfeits the entire reason mpv is
  external.

That last risk is stated as a RISK, not a certainty: the Wayland limits are
firm, the HDR ceiling is not fully established. That uncertainty is itself the
finding - it means embedding cannot be planned, only prototyped.

Supersedes:
Nothing. Extends the 2026-05-23 external-mpv decision from a preference into a
researched position, and closes the owner's standing open question.

## 2026-07-14 - Code review uses external reviewers; an author never adjudicates their own decline

Status: APPROVED (owner, 2026-07-14), AMENDED by the owner 2026-07-15.
STANDING - applies to every review loop, not just the one that produced it.
Carved out of `.agents/state.md` on 2026-07-14 so it survives that entry's
rotation to the archive.

Decision:
Reviewers must be independent of the author harness/model. A Codex author may
not count a Codex CLI run as code review; that is self-review. Use Claude or
Grok for Codex-authored code. The default remains two external reviewers on the
same pinned diff, neither seeing the other's findings, unless the owner gives a
more specific review instruction for a task. For the dependency-LTS refresh,
the owner's specific instruction is Grok reviewloop for each code slice, with
no round cap; Claude is an eligible external fallback or adjudicator. The
author writes the fixes and runs every guard, red-proof and E2E run.

AN AUTHOR MAY NEVER ADJUDICATE THEIR OWN DECLINE. A declined finding goes to the
reviewer that did NOT raise it. Reviewer-vs-reviewer disagreement goes to the
owner - but only when the two positions genuinely cannot both hold; if both are
satisfiable at once, fix both rather than escalating (r23 precedent).

Reason:
Author self-adjudication was tested twice and failed twice: r8-4 and r12-1 were
both declined by the author and both OVERTURNED on independent adjudication.
On 2026-07-15 the owner identified that a Codex author dispatching Codex CLI
was still reviewing its own work; those runs no longer count as independent
review.

The two-reviewer requirement is not belt-and-braces. Across the r17-r24 loop the
two reviewers converged, independently, on the same top finding in FOUR straight
rounds - and in each of those rounds the finding was in the PREVIOUS round's fix.
A single reviewer, or the author alone, ships every one of them.

Evidence:
`.agents/plans/library-refresh-scan.md` `## Code review log` (r1-r24), and the
guard-discipline practices carved into `.agents/repo-guidance.md` at the same
time.

Supersedes:
The 2026-07-15 amendment replaces the original named `codex` + `grok`
reviewer pair with author-external reviewer selection. It preserves the guard,
decline-adjudication, disagreement, and pinned-diff rules.

## 2026-07-15 - Failed watch-state edit errors auto-dismiss after eight seconds

Status: APPROVED (owner, 2026-07-15). Implementation plan:
`.agents/plans/edit-error-auto-dismiss.md`.

Decision:
A failed watch-state edit appears on the edit's own line, follows navigation,
and auto-dismisses eight seconds after publication. A newer edit or source-list
change clears it immediately. Expiry is attempt-owned so an older timer cannot
clear a newer failure. Scan failures retain their next-scan lifetime; there is
no manual dismiss control.

Reason:
The owner confirmed the destructive stopped-Plex path was fixed in 0.1.49, but
the correctly separated red action line then remained indefinitely and looked
permanently active after it had been read. Eight seconds preserves a visible,
accessible failure without requiring another edit merely to clear presentation.

Supersedes:
Only the watch-state edit lifetime detail in
`.agents/plans/per-surface-status.md` and
`.agents/plans/failed-watch-edit-recovery.md`. The 2026-07-14 own-surface
decision, recovery behavior, and every other surface lifetime remain unchanged.

## 2026-07-15 - Dependency baseline is Node 26 plus current mutually compatible stable releases

Status: APPROVED (owner, 2026-07-15). Approved implementation plan:
`.agents/plans/dependency-lts-refresh.md`.

Decision:
Vela adopts Node 26, the immediate next LTS line, before its October 2026 LTS
promotion. The repo, CI, release workflow, Node type declarations, npm version,
and Linux E2E venue move together to one pinned Node 26/npm 12 baseline. The
owner explicitly chose 26 over the recommended current Node 24 LTS and
authorized changing Node/npm on the E2E VM; that grant does not extend to any
other VM or media-server software.

For ecosystems with no LTS concept, dependency refreshes use the newest stable
mutually compatible set: no prereleases, forced peer graphs, or independently
latest version outside a direct package's declared compatibility range. Thus
TypeScript remains on current compatible 6 while SvelteKit excludes 7; Rust
stays rolling stable while Vela preserves and tests its declared 1.89 MSRV.

Known npm vulnerabilities fail CI under the existing fail-closed security
rule. Current SvelteKit still requests vulnerable `cookie ^0.6.0`; because Vela
is a static SPA with no server-cookie use, the owner approved a narrow, tested
override to the closest patched 0.7 line plus a failing npm-audit gate. There is
no blanket suppression and no `npm audit fix --force` downgrade.

Reason:
CI and release still use EOL Node 20 and JavaScript actions whose internal
runtime is Node 20. The direct npm graph spans incompatible toolchain
generations, Rust has three unadopted current majors, and the npm lock contains
a high Vite advisory plus the SvelteKit cookie advisory. One explicit baseline
and fail-closed audits make "current" reproducible rather than host-dependent.

Supersedes:
The Node 20 selections in CI/release and the accidental `@types/node` 25
baseline. Extends the 2026-07-14 known-vulnerabilities-fail decision from Cargo
to npm; it does not weaken Cargo audit or authorize raising the Linux release
glibc floor.
