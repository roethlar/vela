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

Status: Active

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

Status: Active

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

Status: Partially superseded (2026-07-04). The GVfs/KIO-FUSE resolution
mechanism is replaced by the native in-process client — see "Linux SMB
goes native" (2026-07-04). Still active from this entry: the no-root /
no-privileged-CIFS constraint and the entire SSH/sshfs stance.

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

Status: Active

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

Status: Active

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

Status: Active

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

Status: Active

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

Status: Active

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

Status: Active

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

Status: Active (decided; removal not yet implemented or planned in detail)

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
