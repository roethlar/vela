# Review status

Workflow: see `.agents/playbooks/reviewloop.md`. Reviewer harnesses are
loop-specific. Under the standing 2026-07-15 owner amendment, Codex-authored
changes require Grok or Claude; the historical Codex loops below remain
evidence but are not precedent for self-review.
Per-finding detail: see `.agents/review/findings/<id>.md`.
Closed prior loops: `.agents/review/2026-07-04-feature-batch-closed.md`
(rev-1..rev-6) and `.agents/review/2026-07-04-smb-native-closed.md`
(smb-1..smb-6).

Loop `pl-s5` OPEN: implementation and coder guard proofs are complete; two
independent Grok reviews are pending against the pinned Slice 5 range. Detail:
`.agents/review/findings/pl-s5.md`.

Loop `pl-s4` CLOSED 2026-07-16: **accepted clean by two independent external
reviewer sessions at r1, including the full-suite integration repair** (Grok
0.2.101 / `grok-4.5`, each `guard_confirmed:true`). Implementation base
`97acab1`, head `963ef73`; integration-guard base `963ef73`, head `4090d73`.
Slice 4 adds read-only Plex/Jellyfin/experimental-Emby server playlists,
per-source availability, and exact-session fresh-read sequence playback with
no server or local playlist writes. The coder red-proved every claimed source,
UI, sequence, and write boundary; restored verification passed 123 Rust tests
and Linux real-app E2E 21/21. Four independent reviewer proofs across the two
exact ranges returned no comments. Detail:
`.agents/review/findings/pl-s4.md`.

Loop `pl-s3` CLOSED 2026-07-15: **accepted clean by two independent external
reviewer sessions at r1** (Grok 0.2.101 / `grok-4.5`, each
`guard_confirmed:true`; base `cdfc91a`, reviewed head `304f493`). Slice 3 adds
durable cross-source Vela playlists, retained unavailable entries, owned UI
status, and exact-session, fresh-read sequence advancement. The coder
separately red-proved the storage, CRUD, session, UI, and playback-sequence
behaviors; restored verification passed 118 Rust tests and Linux real-app E2E
20/20. Both reviewers independently produced exact focused red/green proofs
and returned no comments. Detail: `.agents/review/findings/pl-s3.md`.

Loop `pl-s2` CLOSED 2026-07-15: **accepted clean by two independent external
reviewer sessions at r1** (Grok 0.2.101 / `grok-4.5`, each
`guard_confirmed:true`; base `4e4eec0`, reviewed head `c6bc5c1`). Slice 2 makes
the shared backend path the sole successful-play recent writer, preserves
failed-launch curation, and orders start recording before the matching end
stamp. The coder separately red-proved every claimed behavior; the reviewers
independently proved the failed-launch side-effect and start/end-boundary
guards and returned no comments. Detail: `.agents/review/findings/pl-s2.md`.

Loop `pl-s1` CLOSED 2026-07-15: **accepted clean by two independent external
reviewer sessions at r1** (Grok 0.2.101 / `grok-4.5`, each
`guard_confirmed:true`; base `7f8a2c2`, reviewed head `ec5d613`). Slice 1
deletes the ephemeral queue and adds explicit Play / Resume / Play from
Beginning behavior across playback surfaces while retaining neutral EOF
plumbing for durable playlists. The coder red-proved seven distinct production
regressions and restored Linux E2E 18/18; both reviewers independently
red/green-proved the forced-beginning backend guard and returned no comments.
One Claude Fable 5 attempt plus its allowed retry failed closed and do not count.
Detail: `.agents/review/findings/pl-s1.md`.

Loop `dlr-s8-final` CLOSED 2026-07-15: **accepted clean at r1** (Claude Code
2.1.210, `claude-fable-5`, `guard_confirmed:true`; base `33163c5`, reviewed
head `ebcd81a`). Scope was the complete Slice 8 integration/version range:
exact Node/npm enforcement, matching WebKit driver fixture, 0.1.51 surfaces,
live Jellyfin readiness, fresh package collection, and durable verification
evidence. Claude independently red-proved the Node pin, restored it green,
ran the artifact guard, verified every version surface, left a clean worktree,
and returned no comments. The dependency-refresh plan is complete. Detail:
`.agents/plans/dependency-lts-refresh.md`.

Loop `dlr-s8-4` CLOSED 2026-07-15: **accepted clean at r1** (Claude Code
2.1.210, `claude-fable-5`, `guard_confirmed:true`; base `58279a0`, reviewed
head `f7bc344`). The final 0.1.51 Linux package run exposed stale 0.1.39 and
0.1.50 artifacts being recopied into `dist/`. `bff2905` validates exact Linux
bundle selections, clears only selected generated targets, and adds a
canonical hermetic regression. Author and reviewer independently reproduced
the five-file red result and restored the two-file green result; the real
0.1.51 deb/rpm build also passed. Claude returned no comments. Detail:
`.agents/review/findings/dlr-s8-4.md`.

Loop `dlr-s8-3` CLOSED 2026-07-15: **accepted clean at r1** (Claude Code
2.1.210, `claude-fable-5`, `guard_confirmed:true`; base `39b233a`, reviewed
head `32b8e83`). The matching WebKit driver exposed a live Jellyfin harness
race where static Home satisfied the library-ready wait before an actual
server library rendered. The fix and deterministic guard landed at `5532e93`
and `8366b4f`; author and reviewer independently proved the old predicate red,
the restored guard green, and the complete live Jellyfin scenario passed at
0.1.51. Claude returned no comments. Detail:
`.agents/review/findings/dlr-s8-3.md`.

Loop `dlr-s8-2` CLOSED 2026-07-15: **accepted clean at r1** (Grok 0.2.101,
`guard_confirmed:true`; base `76c844c`, reviewed head `f3e5601`). The Slice 8
audit found an official Ubuntu WebKitWebDriver 2.52.3 package exactly matching
the VM's WebKitGTK, superseding the skewed Debian 2.50.6/ICU72 fixture.
`ec7c43e` pins both architecture packages, stamps cache identity, and removes
the old ICU shim. Author and reviewer checksum/cache guards passed, the ARM64
session/IPC/UI probe and full E2E 18/18 passed, and Grok returned no comments.
Detail: `.agents/review/findings/dlr-s8-2.md`.

Loop `dlr-s8-1` CLOSED 2026-07-15: **accepted clean at r1** (Grok 0.2.101,
`guard_confirmed:true`; base `33163c5`, reviewed head `0934628`). The Slice 8
integration audit found that local Bash and PowerShell package scripts could
install/build without the pinned Node/npm pair. `4cba5db` adds one assertion
derived from `.node-version` plus `packageManager` and reuses it in local,
CI, and release paths. The author and Grok independently proved both npm and
Node mismatch legs red, restored green, and Grok returned no comments. Detail:
`.agents/review/findings/dlr-s8-1.md`.

Loop `dlr-s7` CLOSED 2026-07-15: **accepted clean at r1** (Grok 0.2.101,
`guard_confirmed:true`; base `8a563c9`, code head `1d619fd`). Scope was
dependency-refresh Slice 7: reqwest 0.13.4 with explicit query support and
native TLS without ALPN, plus the duplicate-family lock consolidation. Grok
independently removed only `query`, observed compiler failures at Vela's real
query call sites, restored the disposable worktree to a clean locked build,
and returned no comments. Full local, Linux E2E/live-server, and native package
verification is recorded in `.agents/plans/dependency-lts-refresh.md`.

Loop `dlr-s6` CLOSED 2026-07-15: **accepted clean by both external reviewers
at r1** (Grok 0.2.101 and Claude Code 2.1.210 / `claude-fable-5`, each
`guard_confirmed:true`; base `c8b9159`, code head `3a002fa`). Scope was
dependency-refresh Slice 6: serde XML 0.8 Plex mappings, strengthened fixtures,
live Plex browse/detail/episode/play/watch/scan coverage, and handled signal
cleanup. Each reviewer independently injected an optional attribute regression,
observed the exact targeted failure, restored its disposable worktree green,
and returned no comments. Full verification and the harness-only first-live-run
correction are recorded in `.agents/plans/dependency-lts-refresh.md`.

Loop `eet-1` CLOSED 2026-07-15: **accepted clean at r1** (Grok 0.2.101,
`guard_confirmed:true`; base `26a48ca`, code head `01e30cf`). The
owner-approved slice adds attempt-owned eight-second edit-error expiry and
deterministic exact-timer / stale-callback guards. Four coder regressions and
Grok's independent ownership regression were proven red; targeted, full Linux,
and both live-server paths are green. The owner subsequently confirmed the
exact stopped-Plex timing path on installed 0.1.50. Detail:
`.agents/review/findings/eet-1.md`.

Loop `fwer-1` CLOSED 2026-07-15: **accepted clean at r1** (Grok 0.2.101,
`guard_confirmed:true`; base `012a031`, code head `b5c170a`). Scope was the
failed-watch-edit recovery follow-up: failed browse/search/person/drill/detail
edits keep exact loaded identity and make no listing request; Home alone heals
its transient curation after backend rollback. Grok independently proved the
old broad recovery and a same-cardinality substitution red, then restored the
head and passed `pagefail`. Coder verification: four distinct red proofs,
local checks/build/Rust gates clean, Linux E2E 18/18, and the exact opt-in live
Plex outage/restart path passed. Detail: `.agents/review/findings/fwer-1.md`.

Loop `lrs-code` OPEN (library-refresh-scan IMPLEMENTATION review; batch
adaptation, no per-finding branches). Base `63560a6` (plan APPROVED), head
`ca84f5b`. **r1 reopened 4, all fixed (`e9edac8`); r2 (codex-cli 0.144.1,
2026-07-13) reopened 8 MEDIUM — 7 admitted, 1 split (half admitted, half
declined).** One behavior defect (lrs-1: the empty-Home redirect's
`navEpoch` bump suppresses the refresh error aggregate); the other seven are
vacuous-guard findings against the plan's own required tests. Full trail and
the DECLINED reason: the plan's `## Code review log`
(`.agents/plans/library-refresh-scan.md`) — not restated here. Fixes land one
per commit, guard-proven on the Linux VM; r3 re-reviews. E2E suite 16/16 at
`ca84f5b` (VM); local CI green.

Item-detail-view PLAN-review loop CLOSED 2026-07-06: **accepted at r3** (base=head
`410fa4e`; three codex rounds, six findings idv-1..6 — full trail in the plan's
`.agents/plans/item-detail-view.md` `## Review log`, not restated here). A healthy
converging loop (r1 found 4, r2 found 3 incl. one correcting an r1 error, r3 clean).
Plan APPROVED for implementation; slices land 1-5, each its own commit + reviewloop.

Loop idv-s1 CLOSED 2026-07-06: **accepted clean, no findings** (codex read-only,
`guard_confirmed:false` — coder guard-proved red/green). Scope was **item-detail-view
slice 1** (base `fd0c414`, head `a2abcb7`): backend-only `DetailDto` + `item_detail`
trait method (graceful default) + `get_item_detail` command + Plex `PlexDetail` serde
parse over `/library/metadata/{rk}` + `to_detail` mapper (cast headshots via the
tokened poster path — accepted exposure class). Nav unwired (flip is slice 5). cargo
test 133 green, clippy -D warnings clean. Same no-branches adaptation. REMAINING for
the feature: slices 2 (JF/Emby), 3 (local), 4 (info components), 5 (nav flip).
(Superseded 2026-07-08 by the owner amendment — see the idv-s2 loop below.)

Loop br CLOSED 2026-07-10 (owner-invoked `playbook reviewloop codex`):
batch pass over the unpushed range `a39be7f..2f33185` (19 commits) returned
3 candidates, **3 admitted, 0 declined, all three fixed and accepted**
(br-1 `8c596d0`, br-2 `36dec5d`, br-3 `88ab605` — docs in
`.agents/review/findings/`). All review-hardening/tooling, no app defects
in the batch: resume's recents-fallback guard restored via the mock's new
`minResumeTicks` (red/vacuous-pass/green proven on the Linux VM — the
severed backend fallback fails the new shape at "0.125s vs 7500ms" while
the old shape passed over the same broken build); the mock search branch
now enforces the client query contract (eh-12 class; Series-narrowing red
proven); bump.sh keeps package-lock's root versions in sync (scratch-
worktree proven). Final full suite 10/10 at the committed state. No
version bump: tests/mock/tooling only, shipped binary unchanged.

Loop idv-s6 CLOSED 2026-07-09: **accepted at r2** (codex read-only,
`guard_confirmed:true` both rounds). Scope was the **owner-playtest polish
round: hero episode Info routing** (base `c2ab703`, head `18c5bcd`;
`d7b938f` fix + `4552a66` bump 0.1.41 + `18c5bcd` r1 fix). Owner-reported
on 0.1.40: context-menu Info on a hero (Continue Watching) series episode
opened the degraded single-episode page, not the season page. Root cause:
the hero's recents copy wins the hero dedup, and a snapshot recorded
before ItemDto carried parent keys (pre-0.1.35) never heals — re-plays
from the hero re-record the same key-less copy — so `seasonKeyFor()` had
nothing to route with. Fix in layers the snapshot's vintage can't defeat:
`DetailDto` + the `PlexDetail` parse now carry namespaced
`parentRatingKey`/`grandparentRatingKey` (guard:
`to_detail_maps_and_namespaces` asserts them — proven red/green by nulling
the `to_detail` mapping), and `openInfo`'s episode branch opens the
degraded view immediately then UPGRADES `detailView` to the season page
when the detail fetch resolves a parent key (deferred JF/Emby backends
keep the degraded page via the graceful `Err`). **r1 reopened 1 MEDIUM,
admitted — and it was load-bearing: the liveness guard compared the raw
pre-assignment object against the deep-`$state` PROXY (always false; the
upgrade would never have run).** Fixed `18c5bcd`: capture
`opened = detailView` AFTER assignment and compare proxy identity. r2
accepted clean. Verified: cargo test 67, clippy `-D warnings`,
svelte-check 0/0, build clean; e2e 10/10 on the Linux VM with `d7b938f`
applied (the VM was powered off before `18c5bcd` — frontend-only change,
and the suite has no episode coverage anyway). NO automated frontend guard
for the hero flow (recorded gap) — the owner playtest on 0.1.41 is the
behavioral check.

Loop dls-s2 CLOSED 2026-07-09: **accepted clean at r1, no findings** (codex
read-only, `guard_confirmed:false` — the Linux-only suite can't run from the
mac host; the coder's red/green run is the recorded proof). Scope was
**drop-local-sources slice 2 — the E2E re-home to mock servers** (base
`ea3c410`, head `b41703a`; commits `80dd8e6` app fix + `b223951` suite +
`b41703a` bump 0.1.40): `mockjf.mjs` generalized (multiple movies,
`searchTerm` branch, per-item PlaybackInfo/PlayedItems/streams; the eh-12
query contract and eh-13 Range semantics kept), every scenario mock-served
and nav-flip-aware (card → info page → Play; ctx-menu play for
queue/curation; hero click for resume), mergedview + sourcedeadend rebuilt
as TWO-mock-server scenarios (server↔server dedup/override; hub-kept vs
empty-home-auto-open legs), markwatched on the `.watchedbadge` markup,
smoke's Settings tabs updated, `connectedtab` deleted with its SMB subject
(sspf-12 zombie-share coverage recorded LOST), `fetch-driver.sh` arch-aware
(arm64 debs pinned — the validation host is aarch64). The re-home
immediately banked a REAL app bug, the class this harness exists to catch:
the context-menu Play entry threw at click time (`mi` is a Svelte 5
`{@const}` lazy read over `menu.item`; the inline handler ran `closeMenu()`
first) — no play, no visible error; broken since the nav flip made the menu
the grid's play affordance, unreachable by the old click-to-play suite. Fix
`80dd8e6` (`playFromCtx` takes the item before closing, like every sibling
entry). Guard-proven red/green on the real app: pre-fix full run 8/10 with
EXACTLY the two ctx-menu-play scenarios red; post-fix 10/10; final 10/10 at
the committed head on the owner's Linux VM (Ubuntu 25.10 aarch64, Xvfb).
svelte-check 0/0, npm build clean, post-bump `cargo check --locked` clean.
**drop-local-sources is COMPLETE — all three slices landed.**

Loop dls-s3 CLOSED 2026-07-09: **accepted at r5** (codex read-only; rounds
r1-r4 reopened with 6/4/5/4 comments, ALL admitted and fixed — fix commits
`70a7a6a`+`02a918b`, `a76175c`, `95f340e`, `ec6a4b9` on slice head
`861442f`; r5 clean, `guard_confirmed:true`). Scope was **drop-local-sources
slice 3 — the docs/guidance sweep** (base `96c5836`, head `ec6a4b9`;
docs-only except one test hardening): README/ISSUES/repo-guidance
de-localed (legacy-config preservation note added), six obsolete plans
bannered CLOSED (smb-native-client, smb-share-root-autoadd,
smb-source-labeling, ssh-macos-guidance, local-metadata-revalidation,
smb-ssh-playtest-fixes), library-all-view-rework bannered PARTIALLY
OBSOLETE (merged-view machinery survives server↔server), e2e-harness
bannered RE-HOMED, and decision-log Status closures/amendments (2026-05-23
×3, 2026-06-10 canonical set, 2026-07-04 ×4, 2026-07-08 ×2). The one code
change: the config round-trip guard now asserts legacy SMB
username/password survive save — guard-proven red/green (a
`skip_serializing` on password fails exactly the new assertion), making
repo-guidance's "credentials included … guarded" claim true. The r3/r4
widening was the plan's non-exhaustive slice-3 file list, not churn — the
same dead-local drift class in docs the plan didn't name. Verified: cargo
test 67 + clippy `-D warnings` clean on BOTH the mac host and the Linux VM;
svelte-check 0/0 + npm build clean (VM). No version bump (test-only Rust
change, shipped binary identical; the bump folds into slice 2's landing).
Note: `a76175c` also carries the slice-2 `connectedtab.mjs` deletion
(staged early by mistake; acknowledged in-loop). REMAINING for DLS:
slice 2 (E2E re-home) — code written, awaiting Linux-VM validation.

Loop pb-s2 CLOSED 2026-07-09: **accepted clean at r1, no findings** (codex
read-only, `guard_confirmed:false` — no JS runner; owner playtest is the
plan-recorded behavioral check). Scope was **person-browse slice 2 — the
frontend** (base `1fef0e8`, head `b290b31`; 3 files +157/−24): cast cards and
director/writer names become buttons when `personKey` is present (plain text
otherwise — non-Plex sparse pages inert); clicking runs the new `personView`
browse root ("With <Name>" / "Directed by <Name>" / "Written by <Name>"
crumb, one-shot `get_person_items`, newest first, normal `open()` routing on
results). The plan-review bindings verified in-code by codex: root switches
clear `personView`, child drills preserve it (searchTerm pattern),
`refreshWatchState` re-runs the person query only at the root level, `goCrumb`
routes person roots to the re-run. svelte-check caught two defects pre-commit
(missing `onPerson` destructure; `svelte:element` a11y) — fixed before the
reviewed commit. Verified: svelte-check 0/0, npm build clean; bump 0.1.39
`8204a77`. **The person-browse feature is CODE-COMPLETE; REMAINING: owner
playtest** (incl. the plan's refresh case: mark-watched from the person grid
must keep the grid populated), then the env-gated live-filter check note in
the plan stands satisfied by the playtest itself (the grid IS the live check).

Loop pb-s1 CLOSED 2026-07-09: **accepted clean at r1, no findings** (codex
read-only, `guard_confirmed:false` — coder ran the red/green proof). Scope was
**person-browse slice 1** (base `02cbf39`, head `35fcc67`; 8 files +232/−15;
plan `.agents/plans/person-browse.md`, owner go 2026-07-09): Plex tag-id
capture on Role/Director/Writer (string-typed, digits-validated at mapping so
malformed ids degrade to plain text), `CastMember.person_key` +
directors/writers → `PersonRef`, `MediaSource::person_items` graceful default
+ Plex impl (section enumeration with rediscover-once, explicit-type
person-filtered listing with full Container paging, newest-first sort),
`get_person_items` command + registration, frontend type/render
compile-through (no visible change — clicks land in slice 2). Guards proven
red/green: to_detail person-key namespacing (nulled mapping fails it), serde
id capture, pure `person_filter_query` (asserts no token in the query).
Verified: cargo test 67, clippy 4-warning baseline, svelte-check 0/0, build
clean; bump 0.1.38 `62fd927`. REMAINING: slice 2 (clickable credits + the
person browse view).

Loop idv-s5 CLOSED 2026-07-08: **accepted clean at r1, no findings** (codex
read-only, `guard_confirmed:false` — no JS unit runner). Scope was the
**owner-playtest polish round: detail-page breadcrumb consistency** (base
`2b7e769`, head `496218e`; frontend-only, 3 files +30/−44). Owner-reported on
0.1.35: once a season page opened, the trail collapsed to Back + show link —
no direct way back to TV Shows. The change: the detail branch renders the
standard `.crumbs` bar (Back closes the detail; each underlying browse crumb
clickable — closes the detail then `goCrumb(i)`; the detail page itself is
the non-clickable current crumb via `detailCrumbTitle`); over Home the bar is
just Back. ItemDetail/SeasonDetail drop their private Back buttons, `onBack`
prop, and dead CSS; the season heading keeps its show/season links (still the
only route to the show from rail/search entries). Verified on the Windows dev
host: svelte-check 0/0 (would flag unused CSS), npm build clean; post-bump
`cargo check --locked` clean (0.1.36, `89d5391`). NO automated frontend guard
(recorded gap) — owner re-playtest is the check. REMAINING: owner playtest
0.1.36, then further Plex polish.

Loop idv-s4 CLOSED 2026-07-08: **accepted at r2** (codex read-only,
`guard_confirmed:false` both rounds — coder ran the red/green proof). Scope
was the **owner-playtest polish round: episode info navigation** (base
`4c56aac`, head `cc9f060`; slice `f1e36d3` + r1 fix `cc9f060`). Owner-reported
in the 0.1.34 playtest ("otherwise, successful test"): a home-rail episode
click opened a single-episode page with no season/show context. The change:
`ItemDto` gains source-namespaced `parentRatingKey`/`grandparentRatingKey`
(Plex: both listing parsers + season Directory rows; JF/Emby: SeasonId/
SeriesId; serde default/skip-none so recents + cache round-trip), frontend
`seasonKeyFor` prefers the episode's own parent key — so an episode clicked
from a rail/search opens the shared season page with the episode selected —
and the season page heading gains navigation (show title → seasons drill;
season title → full season page, only when not already listing it). **r1
reopened 1 finding, admitted:** a season seed's `parentRatingKey` is the SHOW
key, so while `selected` was null (loading, or failed/empty load) the Season
heading linked `onSeason(showKey)` — get_children(show) would list seasons as
episodes (the idv-s2 routing-guard violation). Fixed `cc9f060`: the season
link derives from EPISODE parents only. **r2 accepted clean.** Guards proven
red/green: `episode_and_season_rows_carry_parent_keys` (both Plex parsers +
Directory→Video map), `to_item_namespaces_parent_and_grandparent_keys`
(Plex), `to_item_namespaces_season_and_series_keys` (JF/Emby) — reverting the
mappings fails exactly these three. Verified on the Windows dev host: cargo
test 65, clippy at the 4-warning baseline, svelte-check 0/0, npm build clean;
post-bump `cargo check --locked` clean (0.1.35, `5ec20ad`). Frontend link/
routing behavior itself has NO automated guard (recorded gap) — owner
re-playtest is the check.

Loop idv-s3 CLOSED 2026-07-08: **accepted clean at r1, no findings** (codex
read-only, `guard_confirmed:false` — no JS unit runner; E2E harness is
Linux-only and awaits the DLS slice 2 re-home). Scope was **item-detail-view
AMENDED slice 3 — the uniform nav flip** (base `fdf0790`, head `74ff385`;
frontend-only, `src/routes/+page.svelte` +27/−42): library/home-rail clicks
route to the detail surface (movie/video → item info page; season/episode →
shared episode page; show keeps the seasons drill through `detail_key`), the
Continue Watching cover-flow center click calls `play` directly (click-to-play
unchanged), the context-menu "Info" entry is ungated (`devDetail` flag +
localStorage opt-in removed), and the poster-card hover play overlay + its CSS
are dropped (card clicks no longer play; the hero keeps its own overlay).
Verified on the Windows dev host: svelte-check 0/0, npm run build clean;
post-bump `cargo check --locked` clean (0.1.34, `e388a60`). NO automated
frontend guard (same recorded gap as idv-s2) — the owner playtest is the
behavioral check, and the E2E re-home (DLS slice 2) must also update scenarios
that assumed click-to-play. REMAINING for the feature: owner playtest → Plex
polish rounds; JF/Emby `item_detail` stays deferred (owner go required).

Loop dls-s1 CLOSED 2026-07-08: **accepted clean at r1, no findings** (codex
read-only, `guard_confirmed:false` — coder guard-proved red/green). Scope was
**drop-local-sources slice 1** (base `3a62ac4`, head `6855df5`): the single
turn-off-and-delete commit — 22 files, +297/−8087; ten Rust modules deleted,
15 commands + registrations + startup remount paths gone, Settings local/SMB/
SSH surfaces gone, server-only rank ladders, packaging deps dropped. Compat
rails guard-proven red/green: `config.rs
inert_local_family_config_round_trips_unchanged` (inert fields + legacy
SmbMount shape survive save) and `commands.rs
recents_from_removed_sources_are_filtered_at_read_time` (no dead hero cards;
config preserved). Windows-host CI green (cargo test 62, clippy at the
4-warning cfg baseline, svelte-check 0/0, build). Additionally an ULTRACODE
multi-agent audit (5 sweep lenses + adversarial verification, 13 agents) ran
pre-commit: 8 findings, all refuted as inert; 4 hardened anyway (velasmb CSP
token, packaged SMB/local description strings, guard-test comment). REMAINING:
slice 2 (E2E re-home — Linux host), slice 3 (docs sweep), owner playtest.

Drop-local-sources PLAN-review loop CLOSED 2026-07-08: **accepted at r5**
(base=head per round: `ff6fb64`→`48a0883`→`4fcbb80`→`a46be0f`→`2533f09`; five
rounds, nine findings, all resolved — full trail in the plan's
`.agents/plans/drop-local-sources.md` `## Review log`, not restated here).
Scope: remove local/SMB/SSH sources entirely (owner decision 2026-07-08,
`.agents/decisions.md` "Vela is a multi-server client"). The loop hardened:
lib.rs startup restore/remount paths into slice 1; the full deletion inventory
(sshfs_status, is_local_family_id, proxy-session cleanup, pavao-sys/libc,
PKGBUILD sshfs optdepends, empty-state copy); one merged turn-off-and-delete
slice (dead_code clippy boundary); and the config preserve-on-save rail
(legacy migrator deleted + SmbMount legacy serde attrs round-trip). **Awaiting
owner approval before implementation.**

Loop idv-s2 CLOSED 2026-07-08: verified `[x]`, on `main`. Scope was **item-detail-view
AMENDED slice 2** (base `3acf581`, head `0ecd819`; owner amendment 2026-07-08 in the
plan: Plex-first, JF/Emby+local `item_detail` deferred, uniform nav flip later):
backend `detail_key` on merged `ItemDto` via a server-preferred detail rank in
`rank_backings` (idv-2/6; guard-proven red/green `merge_tests::*detail_key*`);
frontend `ItemDetail.svelte` (movie/video info page) + `SeasonDetail.svelte` (shared
episode page: full `get_children` paging per idv-4a, per-episode detail cache keyed
by selection per idv-4b) with silent sparse fallback from listing data on detail
`Err`; merged shows drill through `detail_key` (idv-5); dev-flagged context-menu
"Info" entry (localStorage `vela.devDetail`, or dev builds) — nav NOT flipped.
Two codex rounds: r1 reopened idv-s2-1 (LOW — episode Info inside an open season
page re-listed seasons as episodes; fixed `0ecd819`, `seasonKeyFor` trusts only a
list that owns the episode), r2 accepted clean. `guard_confirmed:false` both rounds
(read-only). Verified on the WINDOWS dev host: svelte-check 0/0, npm build, cargo
test 111 (Linux-gated tests excluded here), clippy clean vs the 13-warning Windows
dead-code baseline. NO automated frontend guard (no JS runner; E2E harness is
Linux-only) — recorded follow-up: an E2E detail scenario on the Linux host.
REMAINING for the feature: amended slice 3 (uniform nav flip), then polish;
JF/Emby + local `item_detail` deferred (owner).

Loop sort-1 CLOSED 2026-07-06: verified `[x]`, on `main`. Scope was **Library
sorting** (base `361c5b7`, head `19b2735`, three codex rounds: r1 reopened 3, r2
reopened 1, r3 accepted clean). The feature (owner-approved minimum set, folder
dropped): local `sort_and_page` now honors release-date/last-played/date-added
(slice `c368270`); `added_at_ms` on `ItemDto` + `Vfs::modified_ms` (mtime; Plex
`addedAt`; local walk) (`9a47d43`); the merged "All" view accepts addedAt /
lastViewedAt / originallyAvailableAt in `get_type_listing` + `merge_sort_page` +
frontend `TYPE_SORTS` (`21552c9`). codex findings, all real, all fixed + guard-proven
except the one accepted deferral:
- **r1 f1 (MEDIUM, DEFERRED):** local items don't overlay Vela recents onto
  `last_watched_at_ms`, so "Recently played" is a no-op on a local library. Accepted
  as a documented deferral (owner deprioritized local/SMB last-played; Plex-first).
  Follow-up: merge recents into local library items.
- **r1 f2 (MEDIUM, FIXED `c904c66`):** the persistent listing cache accepted schema-1
  entries (no `added_at_ms` → None) and `same_items` ignored `added_at_ms`, so
  "Recently added" sorted stale on first browse after upgrade and the background
  rewalk's real mtimes were seen as "no change" (no repaint). Fix: cache SCHEMA 1→2
  (discard + rewalk) + `added_at_ms` in `same_items`.
- **r1 f3 (MEDIUM, FIXED `c904c66`):** merged dedup adopted played/view_offset across
  backings but not the new timestamps, so a card fronted by a JF/local face sorted
  last despite a Plex backing carrying the value. Fix: adopt max `added_at_ms` /
  `last_watched_at_ms` across backings + preserve across the face swap; guard
  `dedup_adopts_timestamps_from_a_non_face_backing`.
- **r2 f1 (MEDIUM, FIXED `19b2735`):** Plex shows deserialize as `Directory` rows and
  `PlexDir` dropped `addedAt`/`lastViewedAt` (`From<PlexDir>` hard-coded None), so the
  merged TV view mis-ranked every Plex show. Fix: add both fields to `PlexDir` (serde)
  + carry through the conversion; guard `plexdir_carries_added_and_last_viewed_into_video`.
r3 accepted clean, no findings (`guard_confirmed:false` — codex read-only; the coder
guard-proved every pure sort + the two mapping fixes red/green). Full CI green (130
tests, clippy -D warnings clean, npm check + build clean). Same no-branches adaptation.
REMAINING: owner playtest — the sort dropdown (Title / Year / Recently added / Release
date / Recently played) on Plex libraries + the merged All view. NOTE: `folder` sort
DROPPED (owner: podcast/audio need, video-only Vela doesn't need it); JF + local
last-played population deferred (low value for a Plex-first owner).

Loop sspf-14 CLOSED 2026-07-06: verified `[x]`, on `main`. Scope was **Bug 5 P2 —
source naming + rename** (base `8e4f140`, head `5053d2b`, two codex rounds: r1
reopened, r2 accepted clean). Two code slices: `c83a1be` gives an added SMB
share / SSH folder a friendly default label (bare share / last remote-path
segment, disambiguated against existing local-family labels — pure
`unique_mount_name`/`last_path_segment`, unit-tested) instead of the URL-shaped
`server/share` / `host:remote_path`, plus an optional Name field in both add
forms (passed through the existing `name` param — no schema change). `55a6852`
adds `rename_smb_mount`/`rename_ssh_mount` (pure `rename_*_mount_in_config`
helpers, unit-tested; propagate the new label to the name copies seeded at add
time — the SMB share-root folder `path==""` and the SSH-fed local folder — only
when that copy still equals the OLD mount name) + an inline rename affordance in
the Connected tab. r1 sspf-14 (MEDIUM): the rename Save button stayed enabled on
a blank field, so clicking it (or Enter) surfaced the "A name is required." error
— a click terminating in an error-like state, which the Bug 5 UX ruling forbids.
Fix `5053d2b`: Save is disabled on a blank name (both rows) and `saveRename`
no-ops silently on empty (Enter can't error either); the backend still rejects an
empty name defensively. Full CI green (cargo test 118, clippy -D warnings clean,
npm run check + build clean); pure helpers guard-proven red/green by the coder
(codex ran read-only, so `guard_confirmed:false` — its value was the independent
code review that surfaced sspf-14). Same no-branches adaptation. **Bug 5 is now
COMPLETE** (P1 landed 0.1.27; P2 here). REMAINING: owner playtest — friendly
default labels + rename on the real NAS.

Loop sspf-13 CLOSED 2026-07-06: verified `[x]`, on `main`. Scope was **Bug 2 —
mpv hangs on seek over SSH** (base `61efc4e`, head `314d76c`, two codex rounds: r1
reopened, r2 accepted). SSH uses the raw sshfs mount (not the SMB proxy); its
single default SFTP channel head-of-line-blocks a seek's read behind the readahead
backlog → stalls on a latency link. Fix `2174d2e`: add `-o max_conns=4` (parallel
SFTP channels) to the sshfs options, with a unit guard + a hermetic loopback
sshd+sshfs mount/read test (owner chose the functional guard over latency-repro,
which localhost can't reproduce; the NAS playtest is the authoritative stall check).
`0bbff29` hardened the test's CI-portability skips. r1 sspf-13 (HIGH): `max_conns`
was added for ALL Unix, but macOS sshfs-mac (2.10) rejects it → a macOS SSH mount
would fail outright. Fix `314d76c` gates max_conns to Linux via
`sshfs_options_for(os)` (split on an explicit OS string so both branches are
testable from any host); guard-proven (making it unconditional fails the macOS
test). Same no-branches adaptation.

Loop sspf-12 CLOSED 2026-07-06: verified `[x]`, on `main`. Scope was **Bug 5 P1 —
Connected-tab triplication + erroring Remove** (base `ae9d2ff`, head `0a64cd0`,
two codex rounds: r1 reopened, r2 accepted). The slice: `9c3597a` excludes the
whole local family (`LOCAL_FAMILY_KINDS`) from the Connected registered-source loop
(drops the leaked smb/ssh source row + its erroring `remove_source` Remove);
`9379ec5` refuses to remove an SMB mount's last folder (a zombie zero-folder share)
with a guard-proven Rust test, and cascades a last-folder Remove to a full unmount
in the UI. r1 sspf-12 (MEDIUM): both frontend fixes shipped with no automated
guard, so the P1 dead-end could regress with CI green — and codex showed a hermetic
guard IS feasible (a **native** SMB mount, `mountpoint:""`, seeded in config renders
the Connected tab with no connection). Fix `0a64cd0` adds
`tests/e2e/scenarios/connectedtab.mjs` (asserts one SMB row, no leaked source row;
last-folder Remove cascades to unmount with no error), guard-proven headed by
reverting each frontend fix independently. Durable technique: the native mountless
SMB seed makes the Connected tab E2E-testable without SMB infra. Same no-branches
adaptation.

Loop sspf-10..11 CLOSED 2026-07-06: all verified `[x]`, fixes on `main`. Scope
was **Bug 3 — clicking a source dead-ends on empty Home** (frontend nav; code
`b9cca81`) — base `f8e6d81`, converged at head `6837157` after two codex rounds
(r1 reopened, r2 accepted clean). The b9cca81 slice put the empty-scoped-Home →
content auto-open at the tail of `selectSource()`; r1 found two real defects: sspf-10
(HIGH) — Home button and Back (`back()`→`goHome()` from a top-level section) still
dead-ended a scoped local source, and the selectSource early-return trapped the user
there; sspf-11 (MEDIUM) — reading `hubs`/`heroItems` right after `await
loadEverything()` could see a superseded Home load (concurrent `goHome()` bumps
`homeGen`), force-browsing a slow server source whose hubs hadn't arrived. Both
fixed in `6837157` by replacing the imperative check with a reactive `$effect`
that opens the first section when a scoped source's Home *settles* empty (no hubs
AND no hero/recents) with sections present, gated on `!loading` (covers source
click / Home / Back; never misfires mid-load or on a superseded load; keeps server
Home rails — the r1-finding-3 guarantee). Guard `tests/e2e/scenarios/sourcedeadend.mjs`
drives both directions plus the Home-button leg; guard-proven red/green (ran HEADED
— Xvfb absent on this host, owner-approved). sspf-11 is a superseded-load race,
covered by the `!loading` gate + analysis (the deterministic guards cover the
non-raced paths). Same no-branches adaptation.

Loop sspf-5..9 CLOSED 2026-07-06: all verified `[x]`, fixes on `main`. Scope was
**SMB seek Bug 1 sub-slice 3** (per-token SMB session reuse — the real seek fix;
code `05ed86b`) — base `21cd8909`, converged at head `ab3f74c` after **five codex
rounds** (r1-r4 reopened, r5 accepted). Each round banked a distinct, real,
guard-proven defect (a healthy converging loop, not a stall — the fixes built
toward a correct session-lifecycle model): r1 sspf-5 (a create after the play
released orphaned a session → generation-guarded commit, fix `c7211e6`) + sspf-6
(eviction freed a context under the registry lock → drop off-lock, fix `5a64172`);
r2 sspf-7 (sspf-5's release-bump left an ownerless generation a straggler could
store under → replaced with generation=which-play + active=is-it-live, fix
`dec0121`); r3 sspf-8 (a same-file replay keeps the session but play() installs the
owner only on success → release on play failure, fix `ada9f65`); r4 sspf-9 (that
on-failure release ran a blocking `smbc_free_context` on the async worker → moved
onto the blocking pool, fix `ab3f74c`). r5 **accepted** clean (guard_confirmed,
no comments) after a first attempt returned a fail-closed `invalid` (a codex
tooling/budget wrap-up, not a finding; re-prompted per the playbook). All
`Arc<SmbConnection>` drops verified off both the registry lock and async workers.
Same no-branches adaptation.

Loop sspf-4 CLOSED 2026-07-05: verified `[x]`, fix on `main`. Scope was **SMB
seek Bug 1 sub-slice 2** (write deadline on the proxy socket) — base `5c50044`,
head `8f41b90` after two codex rounds. r1 reopened sspf-4 (the 30s write
deadline broke a normal long mpv pause — ffmpeg reconnect is off by default, so
a mid-stream close hit premature EOF on resume); fix `8f41b90` enables ffmpeg
reconnect for the loopback proxy stream (`playback::proxy_reconnect_args`) and
raises the deadline default 30s→300s as a backstop; r2 **accepted** clean.
Same no-branches adaptation.

Loop sspf-1..sspf-3 CLOSED 2026-07-05: all verified `[x]`, fixes on `main`.
Scope was **SMB seek Bug 1 sub-slice 1** (`.agents/plans/smb-ssh-playtest-fixes.md`)
— base `adbeb867`, converged at head `401fd1bc` after four codex rounds (three
reopens, each a real distinct defect, all guard-proven). r1 reopened sspf-1
(token reuse serves a stale cached length → fix `08fef74`); r2 reopened sspf-2
(a late `store_len` repopulates a length a replay cleared, TOCTOU → per-token
generation guard, fix `79f3979`); r3 reopened sspf-3 (env-gated live probe
panics after connect went lazy → fix `401fd1b`); r4 **accepted** clean, no
comments. Each round banked a verifiable delta (healthy converging loop, not a
stall). Same no-branches adaptation.

Loop CLOSED 2026-07-05: cw-1..cw-3 all verified `[x]`, fixes on `main`.

Loop e2e-10 CLOSED 2026-07-05: eh-15 verified `[x]`. Scope was E2E slice
11 (mark-unwatched round-trip; base `d307494`, head `7c899be`); codex
admitted 1 guard-strength finding (eh-15) at intake, extended to both
badge legs. Fix `6db391c` gates each badge assertion on a later
`/Users/{u}/Items` refetch then asserts a present card; guard-proven with
a `drop-after-unwatch` mock (old scenario PASSES the dropped card, fixed
scenario FAILS), and accepted by codex (analytical guard-confirm). An
independent 3-lens adversarial pre-review (all `refuted:false`) refined
the rationale: the optimistic *watched* card never paints (batched Svelte
flush), so the load-bearing hole is the unwatch leg's missing-card wait.
Same no-branches adaptation.

Loop e2e-9 CLOSED 2026-07-05: eh-14 verified `[x]`. Scope was E2E slice
10 — base `7c7a394`, head `5742789` (merged All view scenario); codex
admitted 1 guard-strength finding, fixed and verified. Same no-branches
adaptation.

Loop e2e-8 CLOSED 2026-07-05: eh-13 verified `[x]` after the loop's first
reopen→fix→accept round-trip (reviewer caught a reversed-range crash path
the first fix missed). Scope was E2E slice 9 — base `d3a79de`, head
`ccc6270` (watch-state scenario + mock stream/check-in routes). Same
no-branches adaptation.

Loop e2e-7 CLOSED 2026-07-05: eh-12 verified `[x]`. Scope was E2E slice 8
— base `4ffc272`, head `c706228` (mock-Jellyfin leg + mark-watched
scenario + cleanup hook + plan extension); codex admitted 1 mock-fidelity
finding, fixed and verified. Same no-branches adaptation.

Loop docs-2 CLOSED 2026-07-05: clean pass, no findings. Scope was the
artifact-manifest refresh — base `c1f2b65`, head `7e08272` (docs only).

Loop docs-1 CLOSED 2026-07-05: clean pass, no findings. Scope was the
README test-workflow section — base `b6063e8`, head `36b0a6f` (docs only;
every documented command was live-verified the same day).

Loop app-1 CLOSED 2026-07-05: clean pass, no findings. Scope was slice 7
— base `24de4ee`, head `e7c5231` (resolve_stream onto the blocking pool +
repo-map P0-audit note). Same no-branches adaptation.

Loop e2e-6 CLOSED 2026-07-05: clean pass, no findings. Scope was E2E
slice 6 — base `ee757e2`, head `fc902f4` (search scenario + driver
type()). Same no-branches adaptation.

Loop e2e-5 CLOSED 2026-07-05: eh-11 verified `[x]`. Scope was E2E slice 5
— base `ec69de0`, head `9274ac2` (queue auto-advance scenario + shared
seedLocalMedia helper); codex admitted 1 flakiness finding, fixed and
verified. Same no-branches adaptation.

Loop e2e-4 CLOSED 2026-07-05: eh-10 verified `[x]`. Scope was E2E slice 4
+ the app fix it surfaced — base `e91cbcf`, head `2f5bba8` (`4527613`
eh-10 local-resume fix, coder-filed with the resume scenario as guard;
`2f5bba8` helpers + resume scenario). The codex batch pass over the slice
itself returned NO material issue — recorded as a clean pass. Same
no-branches adaptation.

Loop e2e-3 CLOSED 2026-07-05: eh-8..eh-9 verified `[x]`. Scope was E2E
slice 3 — base `ca0e9da`, head `ee01101` (curation scenario + ctx.restart
in the runner); codex admitted 2 guard-strength findings, both fixed and
verified. Same no-branches adaptation.

Review pass 2026-07-05 (codex, read-only, base `ca0e9da` head `ee01101`,
loop e2e-3): 2 candidates, 2 admitted (eh-8, eh-9), 0 declined.

Loop e2e-2 CLOSED 2026-07-05: eh-5..eh-7 all verified `[x]`, fixes on
`main`. Scope was E2E slice 2 + the app fix it surfaced — base `8ebbde1`,
head `d2be263` (`b4b4ebb` eh-5 hero fix, coder-filed with the playback
scenario as its guard; codex batch pass admitted eh-6 flaky-race and eh-7
quit-vs-EOF false-green, both fixed and verified). Same no-branches
adaptation.

Review pass 2026-07-05 (codex, read-only, base `8ebbde1` head `d2be263`,
loop e2e-2): 2 candidates, 2 admitted (eh-6, eh-7), 0 declined.

Loop e2e-1 CLOSED 2026-07-05: eh-1..eh-4 all verified `[x]`, fixes on
`main`. Scope was E2E harness slice 1 (base `23f6857`, head `34d3412`);
codex admitted eh-1/eh-2, and live diagnosis during eh-1 verification
surfaced two coder-filed findings (eh-3 unbounded requests, eh-4
Wayland-focus screenshot hangs — the root cause of every observed hang),
both fixed and verified in the same loop. Same no-branches adaptation as
the cw loop: one finding ↔ one commit ↔ one verdict.

Prior loop (cw, CLOSED): scope was the 2026-07-04 delegation batch
`ec94715..a055556` — SMB share-root auto-add (`f05919e`) and Continue
Watching curation slices 1-3 (`d2ea1a7`, `cf5af95`, `d259213`). Review
dispatches pinned (base = ec94715, head = a055556) for the batch pass, and
(base = pre-fix main head, head = fix commit) per finding.

## Legend
- `[ ]` Admitted, open (not yet started)
- `[~]` In progress / pending review
- `[x]` Verified
- `[!]` Contested — awaiting owner adjudication
- `[-]` Declined at intake

## Findings

| ID | Severity | Impact (one line) | Status | Fix commit |
|----|----------|-------------------|--------|------------|
| dlr-s8-4 | MEDIUM | Package collection can republish stale installers | `[x]` | `bff2905` |
| dlr-s8-3 | MEDIUM | Live Jellyfin can fail on Home before a real library renders | `[x]` | `5532e93`, guard `8366b4f` |
| dlr-s8-2 | MEDIUM | E2E reuses a skewed WebKit driver despite an exact packaged match | `[x]` | `ec7c43e` |
| dlr-s8-1 | MEDIUM | Local package scripts can install/build with an unpinned Node/npm pair | `[x]` | `4cba5db` |
| fwer-1 | HIGH | Failed watch edit can blank/lose the loaded grid and manufacture a view failure | `[~]` | pending |
| dlr-s1-1 | MEDIUM | Windows release cannot run the direct npm.cmd version assertion | `[x]` | `adc0104` |
| lrs-1 | MEDIUM | Empty-Home redirect's navEpoch bump suppresses the refresh failure banner; user lands in a stale library silently | `[x]` | see log |
| lrs-2 | MEDIUM | Refresh case 5 passes with the content-leg navEpoch gate removed (loadMore reads live state) | `[x]` | see log |
| lrs-3 | MEDIUM | Refresh case 14 omits the plan's reverse-ordering phase: leg-failure generation ownership unguarded | `[x]` | see log |
| lrs-4 | MEDIUM | The r1 `loading` fix has no guard; a stranded skeleton blocks the empty-Home redirect undetected | `[x]` | see log |
| lrs-5 | MEDIUM | Scan stale-FAILURE gate unguarded; mock consumes failNextItemRefresh at respond time so the case can't be written | `[x]` | see log |
| lrs-6 | MEDIUM | scan_url never tested with a hostile id; raw interpolation would stay green on an admin-credentialed POST | `[x]` | see log |
| lrs-7 | MEDIUM | scan_query assertion is tautological; a destructive ReplaceAllMetadata=true refresh stays green | `[x]` | see log |
| lrs-8 | MEDIUM | Refresh case 13's deferral assertion runs before the fallback's Home fetch settles — vacuous | `[x]` | see log |
| br-1 | MEDIUM | Resume scenario green off the server offset — recents-fallback regressions invisible | `[x]` | `8c596d0` |
| br-2 | MEDIUM | Mock search ignores IncludeItemTypes/Recursive — search-contract regressions pass | `[x]` | `36dec5d` |
| br-3 | LOW | bump.sh leaves package-lock version stale; npm install dirties fresh checkouts | `[x]` | `88ab605` |
| cw-1 | MEDIUM | Merged items (local front, server watch key) survive mark-watched/remove in the hero | `[x]` | `5ce26db` |
| cw-2 | LOW | Registry lock held across Plex removal await stalls unrelated UI up to 15s | `[x]` | `07167f1` |
| cw-3 | LOW | Failed play clears a removal tombstone; item wrongly returns to hero | `[x]` | `f767ae4` |
| eh-1 | MEDIUM | Ctrl-C orphans the driver/app process group and blocks the next run on port 4444 | `[x]` | `25757ea` |
| eh-2 | MEDIUM | Mixed valid+unknown scenario filter exits 0 without running the unknown one | `[x]` | `404f86a` |
| eh-3 | MEDIUM | Unbounded driver requests turn any stall into an opaque 300s hang | `[x]` | `0945104` |
| eh-4 | HIGH | Screenshots hang whenever the test window opens unfocused on the live desktop | `[x]` | `cfe6ee4` |
| eh-5 | HIGH | Local-only setups never see the Continue Watching hero (hub-gated render path) | `[x]` | `b4b4ebb` |
| eh-6 | MEDIUM | Playback scenario races the seeded source render — flaky false-red | `[x]` | `4f5abd9` |
| eh-7 | MEDIUM | Quit-vs-EOF indistinguishable in the playback guard — false-green | `[x]` | `dd5cec9` |
| eh-8 | LOW | Curation restart leg passes without exercising tombstone application | `[x]` | `ebf8162` |
| eh-9 | LOW | PID restart guard: overlap false-green, foreign-Vela false-red | `[x]` | `4b24550` |
| eh-10 | HIGH | Continue Watching restarted local-family items from 0:00 | `[x]` | `4527613` |
| eh-11 | MEDIUM | Queue scenario: clip A's EOF races the UI window — flaky false-red | `[x]` | `2eabf26` |
| eh-12 | MEDIUM | Mock Jellyfin ignores the query contract — client regressions pass silently | `[x]` | `32c01e2` |
| eh-13 | MEDIUM | Mock stream Range edges crash the runner / send invalid 206s | `[x]` | `526f511`+`d5e1b04` |
| eh-14 | LOW | Merged-view override assertion accepts any key/value — wrong persist stays green | `[x]` | `2b8becb` |
| eh-15 | MEDIUM | Watched-badge waits satisfied by optimistic UI, not post-refetch state | `[x]` | `6db391c` |

Review pass 2026-07-05 (codex, read-only, base `ec94715` head `a055556`):
3 candidates, 3 admitted, 0 declined.

Review pass 2026-07-05 (codex, read-only, base `23f6857` head `34d3412`,
loop e2e-1): 2 candidates, 2 admitted, 0 declined; plus 2 coder-filed
findings admitted during the loop (eh-3, eh-4). All 4 verdicts: accepted,
guard_confirmed (codex, manual-check mode — no JS unit runner in repo).
