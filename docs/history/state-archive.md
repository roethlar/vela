# State Archive

Entries rotated verbatim out of `.agents/state.md` `## Now` when they stopped
being live (handoff pruning rule). Newest rotation first; each block keeps its
original wording and internal chronology.

## Rotated 2026-07-25 (catchup sweep — nine landed entries, v1.0.20)

Context for readers: the 1.0.0 release entries and the whole
config-integrity/recovery track were landed, verified, guard-proven, and
carried no live decision anymore. They rotate here verbatim in their original
`## Now` order. One exception to verbatim: the guard-pass entry's trailing
push-status sentence was deleted rather than copied, per the 2026-07-11 ruling
that push state is never recorded in these files.

- **Vela 1.0.0 is published.** Annotated tag `v1.0.0` targets
  `06df6812d7fe81185213778669fcaa87680ac83b`; the public Latest release is
  `https://github.com/roethlar/vela/releases/tag/v1.0.0`. It contains the
  universal macOS DMG, Windows NSIS and MSI installers, Linux AppImage/deb/rpm,
  Arch package, and the verified checksum manifest.
- The release closed the native Bash 3 wrapper, fail-closed artifact inventory,
  Arch packaging, real-Plex completion/refresh, 1.0 docs/graphics, cross-platform
  package, and Windows install-over gates. Exact commits, guard red proofs,
  live-state restoration, workflow evidence, artifact hashes, and the GitHub
  permission recovery are canonical in
  `.agents/plans/v1-release-readiness.md`.
- The tag's first release jobs exposed missing GitHub release-write permission
  before publication. The future-tag fix is `9f97355`; repository workflow
  defaults remain read-only. The 1.0 release itself was created and populated
  with `gh` from the successful exact-tag-commit rehearsal, then downloaded
  and checksum-verified before publication.
- The app-wide fail-closed settings prerequisite is COMPLETE at
  `.agents/plans/config-integrity-recovery.md`. It specifies independent
  strict boundaries and targeted byte-exact recovery for settings and active
  server connections. Active connection records and plaintext tokens move to
  private `connections.json`; valid connections survive a settings reset
  without reauthorization. The owner rejected an OS credential vault and
  app-managed pretend encryption: owner-account file/backup permissions,
  redacted runtime handling, private request headers, and removal of Plex token
  URLs/query strings are the security boundary. Unknown fields invalidate only
  their whole owning file; documented legacy rollback fields and non-settings
  media payloads remain compatible. Slice 1 is implemented and canonically
  verified at version 1.0.1: active connections now live in private
  `connections.json`, startup and runtime fail closed behind the two-file gate,
  a valid combined 1.0.0 config splits only after an exact verified backup, and
  invalid combined settings are not mined. Native Windows ACL validation and
  the checksum-matched Linux real-app suite passed. Slice 1 landed as `016a958`;
  its mandatory post-commit guard regressions were injected, failed for their
  intended reasons, restored, and rerun green. A vacuous source-write static
  guard found during that pass was strengthened and independently red-proven.
- Slice 2 is implemented and canonically verified at version 1.0.2. Invalid
  settings and connections now offer real Rename/Reconnect and Exit buttons;
  recovery uses an exact private no-replace rename and targeted validated
  default while leaving the other file and playlists unchanged. Damaged legacy
  combined settings yield no connection data and require reconnection. A
  private strict recovery record keeps crashes after the user's click blocked
  across restart and resumes only an exact unambiguous transaction state.
  Checksum-matched Linux real-app coverage passed 35/35, including click, Space,
  Exit no-write, restart, preserved-connection, reconnect, and crash-resume
  cases; native Windows no-replace, ACL, recovery, and resume tests passed.
  Slice 2 landed as `0c9b48f`. Nine behavior guards were independently
  red-proven and restored. A vacuous busy-disabled button check found during
  that pass was strengthened and then failed for the intended regression.
- Slice 2A is implemented, canonically verified, committed, and independently
  red-proven at version 1.0.3. Settings and connections independently retain
  the three newest private, distinct, strictly valid prior versions. A
  damaged-file screen shows all available versions newest first as real dated
  buttons while retaining fresh-file recovery and Exit. Rollback is bound to
  the selected whole file/version, preserves the exact damaged current file
  first, and leaves the other durable file and playlists untouched.
  Checksum-identical native Windows tests and the rebuilt Linux real-app suite
  passed. Production landed as `b09b610`; the guard pass found and strengthened
  three insufficient tests in `ee79573`, `b8d2860`, and `ac65b0f`, then proved
  their exact regressions red and restored green.
- Slice 3 production is implemented, independently reviewed, canonically
  verified, and committed at version 1.0.4 as `21ecbe8`. Plex artwork,
  progress, timeline, and playback now keep credentials in backend/header
  paths; legacy persisted Plex artwork is converted or removed; provider Part
  keys containing the active credential fail closed; and mpv's private
  per-launch include is cleaned on partial write, replacement, confirmed exit,
  and app exit. Both independent review passes returned findings (two HIGH,
  five MEDIUM, five LOW total), every finding was admitted and resolved, and no
  clean verdict is claimed. Canonical local verification passed with 51 Node
  tests and 259 Rust tests; checksum-identical native Windows passed 255/255
  after one nonreproducible transient history-test failure, and the rebuilt
  Linux real app passed 37/37 E2E scenarios.
- Slice 3's post-commit guard pass is complete (2026-07-24). Beyond the
  restored regressions that proved progress and timeline header auth; settings
  and playlist legacy-artwork sanitation; embedded provider-Part credential
  refusal; frontend protocol conversion and Windows CSP; artwork dimension,
  MIME, traversal, query, header-auth, redirect, declared-size, and streamed
  size bounds; mpv ACL-before-write, partial-write cleanup, process-query
  retention/reaping, replacement ordering, and exit-queue cleanup; discovery
  body nonreflection; and exact/embedded mock-log redaction, the three
  real-app multiplex behaviors were red-proven separately on the Linux E2E
  venue: a transcode query token failed the Plex mock contract, and
  query-token progress and query-token timeline each failed the
  source-token-header assertion. Every regression was restored from its
  committed state and reran green; one restored-green run needed an immediate
  rerun after a transient Settings-dialog timeout on the identical binary.
  Closeout verification passed with the exact Node/npm toolchain, 51 Node
  tests, 259 Rust tests, and the rebuilt Linux real app at 37/37 E2E
  scenarios. The full evidence paragraph is canonical in
  `.agents/plans/config-integrity-recovery.md`; the docs-only closeout
  (evidence plus this state entry) landed as `8b550d6`.
- The required plan `openreview` ran over exact range `7a4b5b0..bf3730a` with
  Claude Code 2.1.218 / `claude-opus-4-8` at max and admitted one MEDIUM finding,
  `cir-1`. The owner resolved it on 2026-07-23: damaged settings are renamed
  whole and replaced or Vela exits; damaged connections are renamed and enter
  reconnection or Vela exits; a damaged legacy combined config is not mined for
  connection records and therefore also requires reconnection. Plan revision 4
  records the repair. The owner declined a follow-up Claude review and
  explicitly activated implementation on 2026-07-23; no clean follow-up verdict
  is claimed.

## Rotated 2026-07-17 (drift pass — seven landed entries, v0.1.57)

Context for readers: seven `## Now` entries were landed, externally accepted,
and (where applicable) owner-confirmed, with no live decision depending on
their detail anymore. They rotate here verbatim, in their original `## Now`
order.

- **OLED BLACK THEME COMPLETE at 0.1.57.** The owner-approved
  direct slice at `6029dbf` adds a selectable literal-black canvas, removes its
  gradient/grain, and lowers chrome luminance without dimming media. Five source
  regressions and the real-app carousel-opacity regression were separately
  proven red and restored green. Local frontend/Rust gates and a fresh focused
  Linux run pass; its OLED Home screenshot was inspected. Claude Fable 5
  accepted exact reviewed head `6e0144d` with an independent production-
  mutation proof and no material finding. The exact implementation and version
  surfaces were checksum-matched on Linux; a fresh `vela v0.1.57` binary built
  and the complete real-app suite passed 27/27. Exact evidence:
  `.agents/review/findings/oled-1.md`.

- **UI EMBELLISHMENTS SLICE 3 COMPLETE at 0.1.56 (owner go 2026-07-16).**
  The approved motion and designed-empty-state implementation lands through
  `841f749`. Every focused source family was separately mutation-proven, three
  initially vacuous guards were repaired and re-proven, normal and verified
  reduced-motion focused Linux runs pass, final dark/light screenshots were
  inspected, and the complete fresh-binary real-app suite passes 27/27. Primary
  Claude Fable 5 accepted exact reviewed head `6075f52` with an independent
  red/restored-green guard proof and no material finding. The owner then removed
  Agy and the default secondary-review gate. Final local frontend/Rust gates and
  the fresh `vela v0.1.56` Linux real-app suite pass 27/27. No owner playtest is
  required. Exact evidence:
  `.agents/review/findings/ui-s3.md`.

- **UI EMBELLISHMENTS SLICE 1 COMPLETE at 0.1.53 (owner go 2026-07-16).** The
  theme-correct foundation in `.agents/plans/ui-embellishments.md` now owns
  semantic theme states, six shared visual primitives, and typed SVG icons.
  Implementation landed through `c1c4db4`; version `0ce3629`. Canonical
  frontend checks, separately red-proven source guards, normal and
  reduced-motion focused Linux runs, final Linux E2E 25/25, and dark/light
  screenshot inspection are green. Primary Claude Fable 5 and independent Grok
  4.5 each accepted the exact code range with separate red/green guard proofs
  and no material findings. The owner is unavailable to playtest this track;
  no playtest is required or pending. Slices 2 and 3 are complete below and
  above, respectively.
  Exact review evidence: `.agents/review/findings/ui-s1.md`.

- **UI EMBELLISHMENTS SLICE 2 COMPLETE at 0.1.55 (owner go 2026-07-16).** One
  cache/source-safe reveal action now owns nine media-art surfaces over fixed
  failure underlays; all ten runtime images decode asynchronously and the Plex
  QR remains the sole no-fade exception. Implementation `830cabd`, real-app
  selector repair `c22a07e`, version `49c0dc9`. Focused guards were separately
  proven red, canonical frontend plus complete Rust verification passed, normal
  and reduced-motion Linux runs passed, the full real-app suite passed 26/26,
  and six dark/light screenshots were inspected. Primary Claude Fable 5 and the
  owner-authorized independent Agy/Gemini substitute accepted the exact range
  with separate guard proofs and no findings. No owner playtest is required or
  pending. Slice 3 is complete above. Exact
  evidence:
  `.agents/review/findings/ui-s2.md`.

- **CAROUSEL DUPLICATE PLAY ACTION CORRECTION COMPLETE at 0.1.54 (owner go
  2026-07-16).** Playlist S1 added a primary Play/Resume row beneath the
  Continue Watching cover-flow even though the centered card already performs
  that action. Implementation `507fb2f` removes the complete row, retains card
  click/keyboard semantics, and keeps Play from Beginning in the context menu.
  The exact guard failed with the row restored and passed after restoration;
  affected Linux scenarios and the full 25/25 suite are green. External review
  converged clean with independent Claude and Grok guard proofs. Final 0.1.54
  focused Linux build and screenshot inspection confirm the row is absent. No
  owner playtest is required or pending. Evidence:
  `.agents/review/findings/car-1.md`.

- **FAILED EDIT-ERROR AUTO-DISMISS: OWNER-CONFIRMED ON 0.1.50.**
  Implemented at `01e30cf` from approved plan
  `.agents/plans/edit-error-auto-dismiss.md`. Failed watch-state edit errors
  retain their own line, follow navigation, clear after eight seconds, and
  still clear immediately for a newer edit/source change; captured attempts
  prevent a queued old timer from erasing a newer failure. Four distinct
  regressions were proven red, the restored Linux suite is green, and both live
  server paths passed. Grok independently proved the stale-timer ownership
  guard, restored the head green, and accepted with no findings. Closed review
  record: `.agents/review/findings/eet-1.md`. The owner built 0.1.50 and
  confirmed the exact stopped-Plex timing path: the grid/title stayed present,
  the named red edit line disappeared after about eight seconds, and the item
  remained unwatched/actionable after Plex restart plus Refresh.

- **FAILED-WATCH-EDIT RECOVERY: OWNER-CONFIRMED ON 0.1.49.** Implemented at
  `b5c170a`; Grok accepted r1.
  The owner playtest failed 2026-07-14 on 0.1.48; follow-up plan
  `.agents/plans/failed-watch-edit-recovery.md` is IMPLEMENTED. The stopped-Plex
  test showed a view failure and a named edit failure on separate lines, with no
  raw URL. Recovery failed: the whole loaded Movies grid disappeared, and
  **12 Years a Slave** remained absent after Plex returned even though Plex Web
  showed it present and unwatched. Read-only diagnosis at `310c2ca` confirmed
  Plex returns that exact item at index 5 of Vela's first-page query and Vela
  has no tombstone for it. The frontend's failed-edit catch unnecessarily
  re-enters the browse listing; backend rollback affects only Home
  recents/tombstones. `b5c170a` replaces that browse reload with a Home-only
  repair and strengthens the count-only guards to exact identity. The four
  planned hermetic regressions were each proven red, the restored scenario is
  green, the full Linux suite is 18/18, and the exact live Plex path passed.
  Grok independently red-proved both the old broad recovery and the exact-
  identity guard, restored the head green, and accepted with no findings.
  The owner repeated the exact stopped-Plex path on installed 0.1.49 and
  confirmed the grid/title remain present and the item remains unwatched. The
  follow-up for the indefinite red edit line is implemented and Grok-accepted
  at `01e30cf` under `.agents/plans/edit-error-auto-dismiss.md`. Closed recovery
  review record:
  `.agents/review/findings/fwer-1.md`. Original plan
  `.agents/plans/per-surface-status.md`; decision `.agents/decisions.md`
  (2026-07-14). Every failure now reports on the surface it belongs to: the
  view's banner keeps listing/refresh/search failures, and the watch-state edit
  (`fee7f0e`), the queue (`67358fd`), the mpv bar (`0f41c7b`) and the detail
  page (`40dfc40`) each report their own. Slice 5 (`282702b`) then DELETED the
  whole refereeing apparatus — the `owner` field, `ErrorOwner`, `clearOwned`,
  per-surface clearing, the scope merge — net -67 lines. As of `282702b`: e2e
  18/18, cargo test 95, clippy/svelte-check/build clean.
  - **What it bought:** the defect class that ran for EIGHT review rounds
    (r17-r24, each fix opening the next door, always the same loss — a failure
    the user needed, silently gone) is structurally gone, because the fight over
    one surface is gone. Slice 1 alone collapsed SIX e2e cases, three of which
    had asserted that a failed edit must be SUPPRESSED when the user navigated
    away — never right, just the price of sharing.
  - **The queue and its status slice (`67358fd`) were deleted in playlist S1**
    (`ec5d613`). The rewritten `surfaces` scenario still red-proves the detail
    and edit lines that remain; exact proof and external review:
    `.agents/review/findings/pl-s1.md`.
  - The library-refresh-scan review loop is CLOSED at r24. Its evidence rotated
    to `docs/history/state-archive.md` (2026-07-14); its two standing rules were
    carved out first — the **review protocol** to `.agents/decisions.md` and
    **guard discipline** to `.agents/repo-guidance.md`. Do not reopen the loop
    against the shared banner.

## Rotated 2026-07-14 (drift pass — five completed tracks, v0.1.48)

Context for readers: five tracks in `## Now` were COMPLETE and owner-verified,
and were carrying a great deal of detail that no live decision depends on
anymore. They rotate here verbatim.

TWO things were CARVED OUT of the library-refresh-scan block below before it was
rotated, because they are standing rules and would have died with it:

- The **review protocol** (two independent reviewers; an author never
  adjudicates their own decline) is now `.agents/decisions.md`, 2026-07-14.
- The **guard-discipline practices** (red-proof every guard; prove each claimed
  behavior separately; the newest fix is the most dangerous code; a self-audit is
  not a check; read the failure path before calling something unguardable) are now
  `.agents/repo-guidance.md`, "Guard discipline".

Read those two first. The block below is the evidence they rest on, not the rule.

---

- **LIBRARY-REFRESH-SCAN: COMPLETE — owner playtest VERIFIED on REAL PLEX
  2026-07-14 (0.1.45).** Refresh button + per-library server scan trigger. Plan:
  `.agents/plans/library-refresh-scan.md`. As of `22dad8b`: E2E 17/17 on the VM;
  `cargo test` 95; clippy `-D warnings`, svelte-check, npm build all clean.
  - **Owner playtest (all six green, real Plex):** refresh while in a library;
    library RENAMED then refresh (sidebar + breadcrumb both update); library
    DELETED while standing in it then refresh (reconciled to Home); right-click →
    Scan Library (Plex really scans — **the Plex scan path had never touched a real
    server before this**); general use with no ~5s stalls (confirms the r16-2
    `/identity` fix); footer 0.1.45. Detail in the plan's `## Owner playtest`.
  - **Where the trail lives:** the plan's `## Code review log` — every round, every
    finding, every fix commit, the declines (r10-1 UPHELD; r8-4 and r12-1 OVERTURNED on
    independent adjudication), the guard gaps, and the process disclosures. Do not
    reconstruct any of it from chat.
  - **r19 LANDED (2026-07-14).** codex 1 HIGH + 3 MEDIUM, grok 1 MEDIUM — and BOTH
    reviewers, independently, found the same defect in the r18 `setWatched` fix. Every
    r19 finding was in code written during r17/r18; nothing in the original slices was
    faulted. Fixes: `563b2fb` (HIGH — an identity probe's answer was written onto the
    server that REPLACED the one it described, pinning rediscovery to a lie: the third
    distinct route to the wrong-server scan in this plan, and the second one opened by
    a fix for the previous one) and `91045cb` (a failed edit stomped the banner
    explaining its own empty grid, and could paint itself on a root the user had left).
  - **r20 LANDED (2026-07-14).** codex 3 MEDIUM + 1 LOW, grok 2 MEDIUM — converging
    independently, for the third round running, on the same defects. Both were in the
    r19 fixes, and the first was **r19's own bug returning through a door r19 opened**:
    the combined banner inherited the listing's generation tag, so the refresh's
    RETRACT erased the user's failed edit (`64972ac` — the banner is now a list of
    OWNED parts, and a retract takes only what it superseded). The second: the r19
    currency gate was wrong in BOTH directions — the Plex link screen replaces the whole
    view while bumping no load generation, and re-entering the library you are standing
    in bumps both counters while going nowhere (`39ead92` — `rootSig()` asks the view
    what root it is on, instead of inferring it from counters).
  - **r21 + r22 LANDED (2026-07-14).** r21: codex 5 MEDIUM + 3 LOW, grok 1 HIGH + 3
    MEDIUM + 1 LOW. r22: codex 3 MEDIUM, grok 1 MEDIUM. Both rounds' top finding was
    found independently by BOTH reviewers, and both were in the PREVIOUS round's fixes.
    Nine fix commits, `d7cb3ef`..`74fc3ad`, each with a red-proven guard where the
    harness can reach it. Detail in the plan's `## Code review log`.
  - **r23 LANDED (2026-07-14).** codex 4 MEDIUM + 1 LOW, grok 3 MEDIUM + 1 LOW — and all
    three top findings were in ONE commit: the `linking` flag invented two commits
    earlier, which shipped three defects of its own (it dropped an edit made on a grid the
    user never left; it stuck true forever after any source change abandoned a link; it
    could not retract a banner already on screen). Replaced by a SCOPE on each banner part
    (`0a79013`) — which is what the model needed from the start. Also `78f7ba0` (skipping
    the repaint was skipping a needed heal — Continue Watching kept showing a rolled-back
    item as gone) and `f61bc71` (the delivery witness overstated what it saw).
  - **First reviewer-vs-reviewer disagreement of the loop (r23):** grok blessed the r22-2
    early return, codex found it skipped a heal. Both positions were satisfiable at once,
    so it was FIXED, not escalated. Escalate to the owner only when they genuinely cannot
    both hold.
  - **r24 LANDED (2026-07-14).** codex 6 MEDIUM, grok 3 MEDIUM. Every finding was in an
    r23 fix or an r23 guard. The r23 scope was defeated on the very next navigation
    (`setError(null)` — which every load start calls — still wiped app-scoped parts): the
    SEVENTH door into the same silent loss, opened by the fix for the sixth. Banner parts
    now name the SURFACE that owns them (`da99a46`), and the heal retracts what it repairs
    and stays off Welcome (in `da99a46`; guards `49a2141`).
  - **THE LOOP IS CLOSED AT r24, and its conclusion is now an approved plan.** The findings
    had migrated out of this feature into a pre-existing design weakness: one shared error
    banner carrying failures from four surfaces with four different lifetimes. The owner was
    asked and chose the durable fix: **per-surface status**
    (`.agents/plans/per-surface-status.md`, decision 2026-07-14). The r17-r24 log is the
    evidence for it — read it before touching the banner again.
  - **A THIRD process violation, disclosed, NOT rewritten:** `da99a46` is MIS-DESCRIBED —
    its message covers only the banner-owner model, but it also carries two production
    fixes to the heal. Root cause, for the third time: staging a whole path instead of the
    hunks just written.
  - **Why it ran so long (r17-r22 evidence):** in this subsystem the author's
    FIXES carry defects at the same rate as the original code. EIGHT rounds running, the
    newest fix has carried a defect of the same CLASS it was fixing, through another
    door — and the class never changes: **a failure the user needs is silently lost.**
    It has now been reached through the publish door, the ordering door, the retract
    door, the dedup door and the setError door, each opened by the fix for the last
    (plus, twice, a wrong-server scan reintroduced by the fix for the previous
    wrong-server scan). The two reviewers have converged, independently, on the same top
    finding in FOUR straight rounds. A single reviewer — or the author alone — ships
    every one of them.
  - **OPEN, recorded, not fixed:** r13-2 (reads carry no binding — owner-DEFERRED to a
    follow-up plan, do not re-raise in review); the tall-viewport request storm is
    untestable at the harness viewport; no guard on scan invalidation when a source is
    removed (r16-3).
  - **FIXES THE HARNESS CANNOT GUARD** (fixed, verified only by inspection — say so
    rather than implying coverage): `sameSection` and `rootSig`'s section BINDING (the
    E2E mock is Jellyfin — GUID ids, never rebinds); the drilled-below-a-search repaint
    gate (the mock resolves ParentId only against VIEWS, so no scenario can drill); and
    everything on the Plex link/pin screen (`link_begin` needs plex.tv). Each is a
    standing hole a future regression could walk through unseen.
  - **Two process violations, disclosed, NOT rewritten** (rewrite needs owner go):
    `4cb6b2a` batches three findings; `878c92e` is MIS-DESCRIBED — a `git add -A`
    swept two production fixes and an E2E case into a commit whose message covers
    only a test fix. Root cause both times: `git add -A` instead of naming paths.
- **DLS (drop-local-sources): slice 1 LANDED + owner-playtested 2026-07-08**
  (0.1.33, `6855df5`; loop `dls-s1` clean r1; full detail rotated to
  `docs/history/state-archive.md`). **Slice 3 (docs sweep) LANDED
  2026-07-09, loop `dls-s3` accepted at r5** (`861442f` + four fix commits
  through `ec6a4b9`; trail in `.agents/review/index.md`) — README/ISSUES/
  repo-guidance de-localed, obsolete plans bannered, decision statuses
  closed/amended, config round-trip guard extended to legacy SMB
  credentials (guard-proven); repo-map refresh moot (file retired
  2026-07-08). **Slice 2 (E2E re-home) LANDED 2026-07-09, loop `dls-s2`
  accepted clean r1** (`80dd8e6` app fix + `b223951` suite + `b41703a`
  bump 0.1.40): all scenarios mock-served and nav-flip-aware, suite 10/10
  on the owner's Linux VM. The re-home banked a real regression fix —
  **context-menu Play threw since the nav flip** (Svelte 5 `{@const}` read
  after `closeMenu()`; fixed `80dd8e6`, guard = queue/curation scenarios
  red→green). **THE DLS PLAN IS COMPLETE.** Owner spot-check ask for the
  next build: right-click → Play on a library card now works (0.1.40).
- **ITEM-DETAIL TRACK: COMPLETE and owner-verified through 0.1.41
  (2026-07-10)** — nav flip (`74ff385`), episode navigation polish
  (`f1e36d3`+`cc9f060`), detail crumb trail (`496218e`), context-menu Play
  un-broken (`80dd8e6`, owner-verified 2026-07-09), hero episode Info →
  season page (`d7b938f`+`18c5bcd`, loop `idv-s6` accepted r2;
  owner-verified 2026-07-10 "info goes to series view"). Loops
  idv-s3/s4/s5/s6 in `.agents/review/index.md`; older history rotated to
  the archive and `.agents/plans/item-detail-view.md`. No open defect;
  further Plex polish only on the next owner report. JF/Emby `item_detail`
  stays deferred on an explicit owner go. No automated frontend guard for
  these flows (no JS runner; the mock E2E servers carry no episodes) —
  owner playtests are the behavioral check.
- **PERSON BROWSE (clickable actor/director/writer → filtered grid):
  COMPLETE — owner playtest VERIFIED 2026-07-09 ("works well") on 0.1.39.**
  Plan `.agents/plans/person-browse.md`; slice 1 backend `35fcc67` (loop
  `pb-s1` clean r1), slice 2 frontend `b290b31` (loop `pb-s2` clean r1),
  bumps `62fd927`/`8204a77`. No open defect; JF/Emby person browse stays
  deferred on an explicit owner go (same bar as JF/Emby `item_detail`).
- **CW WATCH-STATE: COMPLETE — owner playtest VERIFIED 2026-07-10
  ("carousel fix verified") on 0.1.42** (fix `02504be`; plan
  `.agents/plans/continue-watching-watch-state.md`, retained as design
  record). Mark watched/unwatched are one-op curations (recents drop +
  identity tombstone, curate-first with rollback; edits serialized;
  every play path clears tombstones); "Remove from Continue Watching"
  stays the keep-progress dismiss. Decision in `.agents/decisions.md`
  (2026-07-10). Resolved BOTH the 2026-07-10 masking defect and the
  2026-07-08 two-op curation annoyance. Guard: `watchcurate` E2E,
  red→green proven on the VM; full suite 11/11; local CI set green.
  Codex plan-review loop closed at r6 — the one CONTESTED r6 finding
  still awaits owner adjudication (item in ## Next).

## Rotated 2026-07-09 (item-detail complete + person browse code-complete, v0.1.39)

Context for readers: the item-detail track (nav flip + episode navigation +
crumb trail) completed and was owner-verified through 0.1.36; the DLS slice-1
landed detail and its playtest record rotate now that only slices 2-3 remain
live; the 2026-07-05 RED-CI re-triage item CLOSED 2026-07-09 — the owner
pushed both remotes to `926162c` and GitHub CI ran GREEN on it (and on
`2b7e769`), so the old failures no longer reproduce on current code.

- **DLS slice 1 LANDED 2026-07-08 (0.1.33)** — the turn-off-and-delete commit
  `6855df5` (22 files, +297/−8087): ten Rust modules deleted (source/{local,
  vfs,smb_vfs,metadata,listing_cache}.rs, smb.rs, smb_client.rs, sshfs.rs,
  stream_proxy.rs, ui_events.rs), 15 local-family commands + lib.rs
  registrations + startup remount/refresh paths gone, velasmb scheme + CSP
  token gone, proxy plumbing out of play_by_key/playback.rs, server-only
  `kind_rank`/`detail_rank` (plex 0, jf/emby 1; they now coincide, so
  `detail_key` only appears under a per-title play override), Settings
  local/SMB/SSH forms + Connected mount rows + Folders tab removed, packaging
  deps (smbclient/sshfs/libsmbclient/pavao-sys) dropped, packaged descriptions
  de-SMB'd (`97d4467` got the crate description). **Compat rails, guard-proven
  red/green:** config's `local_folders`/`smb_mounts`/`ssh_mounts` are inert —
  parsed, ignored, PRESERVED on save (legacy migrator deleted; SmbMount
  `kind`/`local_folder_id` now round-trip) — and recents from dead sources are
  filtered at read time (`filter_live_recents`), never stripped from config.
  Reviewloop `dls-s1` accepted CLEAN r1; an ultracode 13-agent audit found 8
  inert-only findings (4 hardened anyway). Trail `c11a458`; bump `e66bf7c`.
- **ITEM-DETAIL TRACK (Plex-first, owner amendment 2026-07-08):** slice 1
  (backend DetailDto + Plex parse, 0.1.31), amended slice 2 (info surfaces
  + `detail_key` routing, 0.1.32, loop `idv-s2` accepted r2), and **amended
  slice 3 — the uniform nav flip — LANDED 2026-07-08** (`74ff385`, 0.1.34;
  loop `idv-s3` accepted clean r1, trail `.agents/review/index.md`):
  library/home-rail clicks open the detail surface for every source
  (movie/video → info page; season/episode → shared episode page; show
  keeps the seasons drill through `detail_key`), the CW cover-flow center
  click plays directly, the context-menu "Info" entry is ungated
  (`devDetail` flag removed), and the poster-card hover play overlay is
  gone. JF/Emby `item_detail` stays deferred (local permanently, per the
  removal). **0.1.34 PLAYTEST (owner, 2026-07-08): "otherwise, successful
  test"** — one defect: a home-rail episode click opened a single-episode
  page with no season/show context. **Fixed in polish round idv-s4
  (0.1.35)** — `f1e36d3` + r1 fix `cc9f060`, loop accepted r2: episodes
  carry namespaced parent/grandparent keys (Plex + JF/Emby), episode clicks
  open the shared season page with the episode selected however arrived at,
  and the season page heading links to the show (seasons drill) and, in
  single-episode mode, to the full season page. The 0.1.35 playtest
  verified all of that and surfaced one more inconsistency — the detail
  pages dropped the browse crumb trail (no direct back to TV Shows from a
  season page) — **fixed in polish round idv-s5 (0.1.36, `496218e`, loop
  accepted clean r1)**: detail pages carry the standard crumb bar
  (ancestors clickable; detail page as current crumb; just Back over
  Home). **0.1.36 PLAYTEST VERIFIED (owner, 2026-07-09)** — the whole
  flipped-nav surface is owner-verified: episode→season routing, heading
  links, and the detail crumb trail.
- **DLS slice 1 PLAYTEST SUCCESSFUL (owner, 2026-07-08, 0.1.33 Windows NSIS
  build):** Plex-only sidebar, no dead hero cards, playback unchanged. The
  item-detail Info pages were NOT exercised — release builds cannot show the
  dev-gated Info entry (no `devtools` feature, so no console for the
  localStorage flag); the owner knows clicking tiles still plays by design
  until the nav flip.
- Owner styling ruling 2026-07-09 (encoded at the `.watchedbadge` CSS
  comment, landed `fcb3e22` without a reviewloop — one-rule CSS deletion,
  disproportionate): watched items are NOT dimmed; the checkmark badge is
  the only indicator. Owner confirmed in the 0.1.37 build.
- QUEUED (owner-parked 2026-07-05 — "after current work"): GitHub CI was RED
  on the last PUSHED commit `05f9594` (`cargo audit` advisory noise + an
  untriaged `cargo check --locked` failure on the runner). Stale-risk: local
  code has since changed enormously (unpushed); re-triage only after the next
  owner push gives CI something current to run. [CLOSED 2026-07-09: CI GREEN
  on the newly pushed `926162c` and `2b7e769`.]

## Rotated 2026-07-08 (post drop-local-sources slice 1, v0.1.33)

Context for readers: everything below predates or was superseded by the
2026-07-08 `Vela is a multi-server client` decision (local/SMB/SSH sources
REMOVED; see `.agents/decisions.md`). The SMB/SSH playtest-fix items and
their pending owner playtests are OBSOLETE - that code was deleted in
drop-local-sources slice 1 (`6855df5`). Kept for provenance: the review
trails and finding docs they reference remain valid history.

- LANDED 2026-07-05 — mpv autocrop bundle (`.agents/plans/mpv-autocrop-bundle.md`,
  owner-approved; decision in `.agents/decisions.md` 2026-07-05 "Ship mpv's
  autocrop.lua behind an opt-in toggle", which REVERSES the same-day crop-drop for
  this narrow bundled-script case only). Owner reopened the dropped crop feature
  (option "C") to ship mpv's own `autocrop.lua` + a Settings control. Three
  commits, each `reviewloop codex`-accepted (plan itself converged r1-r4):
  - slice 1 `95d4b1a`: vendored `autocrop.lua` (mpv `efb70d7f`) + LICENSE.GPL +
    PROVENANCE under `src-tauri/resources/mpv-scripts/`; `tauri.conf.json` resources
    map + `bundle.license = "MIT AND GPL-2.0-or-later"`; PKGBUILD installs to
    `/usr/lib/Vela/mpv-scripts/` + `license=('MIT' 'GPL2')`. Verified: deb AND Arch
    packages ship the files at the resolver path (`.PKGINFO` = MIT+GPL2).
  - slice 2 `c66e680`: tri-state `mpv_autocrop` config (off/manual/auto); resolver
    via `AppHandle::path().resolve(mpv-scripts/autocrop.lua, Resource)`; mode-branched
    `--script` injection in `play()` (manual adds `autocrop-auto=no`; auto = crop on
    start). Guard-proven arg tests.
  - slice 3 `be292e0`: Settings → Advanced mpv three-state selector
    (Off/Manual/Automatic); Automatic carries the D-state/HDR hang warning.
  Off by default; **Automatic auto-fires the live `video-crop` D-state path** (owner
  chose to have it available, disclosed via UI). Version bumped to 0.1.22
  (`f237705`); handoff `607143d`. REMAINING: owner playtest only (does Shift+C /
  Automatic crop cleanly on the real HDR stack). NOTE: rpm `License` tag not locally
  verifiable (no rpm tooling on the Arch dev host).
- CURRENT WORK (2026-07-05) — implementing
  `.agents/plans/smb-ssh-playtest-fixes.md`, **owner-approved** after a 3-round
  codex plan-reviewloop (accepted at r3; trail in the plan's Review log). Five
  bugs from the owner's 0.1.21 NAS playtest (SMB share + SSH folder, same host);
  diagnoses done this session, SMB/SSH seek mechanisms adversarially verified.
  **Bug 1 sub-slice 1 LANDED 2026-07-05** (code `941b933`; three
  reviewloop-codex fixups `08fef74`/`79f3979`/`401fd1b`; version bump 0.1.23
  `3e9256d`; all UNPUSHED — owner pushes manually). It: (a) moved the share-root
  `list_dir("")` out of `SmbConnection::connect` (per-seek/stream/browse hot path)
  into `verify_mount` (add-time only) — connect verifies lazily on first op; and
  (b) caches the entity length per proxy token so a seek skips the redundant
  `stat`, via `SmbConnection::open_read_with_len` + `open_raw`. The per-token cache
  is scoped to one playback (cleared on token reuse) and generation-guarded against
  stale writers. `reviewloop codex` converged at r4 (accepted clean) after three
  reopens, each a real distinct defect, all guard-proven — trail
  `.agents/review/index.md` + `findings/sspf-1..3.md`. Hermetic guards in
  `stream_proxy.rs` tests (seek reuses cached len; reuse clears stale len; stale-gen
  store rejected).
  **Bug 1 sub-slice 2 LANDED 2026-07-05** (code `d45ffe3`; reviewloop-codex fixup
  `8f41b90`; version bump 0.1.24 `92b1984`; UNPUSHED). Per-response write deadline
  on the proxy socket in `serve_target` (`stream_proxy.rs`
  `DEFAULT_WRITE_TIMEOUT_MS`, atomic, test-configurable) so a non-draining client
  can't pin a thread + a bounded point for sub-slice 3's cooperative cancel. codex
  reopened once (sspf-4): a short deadline breaks a normal long mpv pause because
  ffmpeg HTTP reconnect is off by default → premature EOF on resume. Fixed by
  enabling ffmpeg reconnect for the loopback proxy stream on the mpv side
  (`playback::proxy_reconnect_args`, `--stream-lavf-o-append=reconnect=1/…`,
  scoped to `http://127.0.0.1:`, asserted after user args) AND raising the deadline
  default to 300s so it's a backstop, not a pause-killer. Trail `findings/sspf-4.md`.
  Owner playtest still owes the end-to-end check (pause past the deadline resumes
  seamlessly — needs mpv + a live drop).
  **Bug 1 sub-slice 3 LANDED 2026-07-06** (code `05ed86b`; five reviewloop-codex
  fixups `c7211e6`/`5a64172`/`dec0121`/`ada9f65`/`ab3f74c`; trail `14b0102`; version
  bump 0.1.25 `503593a`; UNPUSHED). Per-token SMB session reuse — the real seek fix:
  the loopback proxy caches the live `SmbConnection` per token (created once, reused
  by every seek — each connection opens its OWN file handle) instead of rebuilding a
  libsmbclient session per seek. A session is stored only for the current, still-live
  play (`generation` = which play, bumped on replay; `active` = is it live; invariant
  `active==false ⇒ session==None`), freed once at playback-end (generation-guarded
  compare-and-remove) — or by the play path if `play()` fails — always OFF the
  registry lock AND off async workers; evicted entries drop off-lock too. A per-token
  `serve_epoch` cooperatively cancels a superseded in-flight serve (GET bumps, HEAD
  doesn't). `ctx_lifecycle_lock` UNCHANGED (per-seek context churn removed instead).
  `reviewloop codex` converged r1-r5 (r1-r4 reopened, r5 accepted clean) — five real
  distinct defects, all guard-proven (trail `.agents/review/index.md` +
  `findings/sspf-5..9.md`); the fixes built toward a correct session-lifecycle model
  (a healthy converging loop, not a stall). **Bug 1 (SMB seek) is now COMPLETE** —
  all three sub-slices landed. REMAINING: owner NAS playtest — confirm the felt
  seek-freeze is gone on the real share.
  **Bug 3 (source-click dead-end) LANDED 2026-07-06** (code `b9cca81`;
  reviewloop-codex fixup `6837157`; trail `f73cdaa`; version bump 0.1.26 `01c54ec`;
  UNPUSHED). A scoped source whose per-source Home settles empty (no hubs AND no
  hero/recents) but has browsable sections now lands on its content (opens the first
  section) instead of the "Nothing on your home screen yet" dead-end; a server source
  that returns Home hubs keeps its Home unchanged. Implemented as a reactive
  `!loading`-gated `$effect` in `+page.svelte` (NOT a tail of `selectSource`), so it
  covers source click, Home, AND Back uniformly. `reviewloop codex` converged r1-r2:
  r1 reopened sspf-10 (HIGH — the first cut lived only in `selectSource`, so Home/Back
  still dead-ended and re-clicking the source early-returned = trap) + sspf-11
  (MEDIUM — reading `hubs`/`heroItems` after `await loadEverything()` could read a
  superseded load → force-browse a slow server source); both fixed by the
  reactive-effect approach; r2 accepted clean. Guard
  `tests/e2e/scenarios/sourcedeadend.mjs` (both directions + Home-button leg),
  guard-proven red/green — ran HEADED (Xvfb absent on this host; owner-approved
  2026-07-06). REMAINING: owner playtest — clicking the SMB/SSH source on the real NAS
  lands on content.
  **Bug 5 P1 (Connected-tab triplication + erroring Remove) LANDED 2026-07-06**
  (code `9c3597a`+`9379ec5`; reviewloop-codex fixup `0a64cd0`; trail `eb6a85a`;
  version bump 0.1.27 `d88a277`; UNPUSHED). `9c3597a`: the Connected registered-source
  loop now excludes the whole local family (`LOCAL_FAMILY_KINDS = ["local","smb","ssh"]`
  const mirroring the backend), dropping the leaked smb/ssh source row whose Remove
  called `remove_source` and errored (a dead-end) + the triplication. `9379ec5`:
  `remove_smb_folder_in_config` (pure, unit-tested) refuses to remove a mount's last
  folder (a zombie zero-folder invisible share), and the UI `removeSmbFolder` cascades
  a last-folder Remove to a full `unmount_smb` (clean, no error). `reviewloop codex`
  converged r1-r2: r1 reopened sspf-12 (MEDIUM — the frontend filter+cascade shipped
  with no automated guard; codex showed a hermetic guard IS feasible via a native
  mountless SMB seed), fixed by `tests/e2e/scenarios/connectedtab.mjs`; r2 accepted.
  Full CI green (cargo test 102, clippy -D warnings clean). REMAINING: owner playtest
  — one row per SMB/SSH mount on the real NAS, no erroring Remove.
  **Bug 2 (SSH seek) LANDED 2026-07-06** (code `2174d2e`+`0bbff29`; reviewloop-codex
  fixup `314d76c`; trail `d11ce50`; version bump 0.1.28 `49072f8`; UNPUSHED). SSH uses
  the raw sshfs mount (NOT the SMB proxy); its single default SFTP channel
  head-of-line-blocks a seek's read behind the readahead backlog → stall on a latency
  link. Fix: `-o max_conns=4` (parallel SFTP channels) added to the sshfs options,
  now via `sshfs_options_for(target_os)` — **Linux-only** (macOS sshfs-mac 2.10
  rejects max_conns; codex sspf-13 caught the unconditional version as a HIGH macOS
  regression). Owner chose the FUNCTIONAL hermetic guard (loopback sshd+sshfs mounts
  with the option set + reads correctly; gated on sshd/sshfs/ssh-keygen + /dev/fuse,
  skips gracefully) over a latency-repro — localhost has ~0 latency, so the stall
  can't reproduce hermetically. `reviewloop codex` converged r1-r2 (r1 sspf-13, r2
  accepted). Full CI green (cargo test 105, clippy -D warnings clean). REMAINING:
  owner NAS playtest — SSH seek no longer hangs (the authoritative stall-fix check).
  **OWNER REORDER 2026-07-06: Bug 5 P2 done first, THEN Bug 4, THEN metadata rail.**
  The owner chose to do the small Bug 5 P2 (naming/rename) before the larger Bug 4,
  deviating from the plan's "Slice order & commits" (which had Bug 4 before Bug 5 P2).
  Reason: bank the small win and let the three landed P1 fixes (0.1.26–0.1.28) be
  playtested before the large P2 metadata work lands on top.
  **Bug 5 P2 (source naming + rename) LANDED 2026-07-06** (code `c83a1be`+`55a6852`;
  reviewloop-codex fixup `5053d2b`; trail `b7652d0`; version bump 0.1.29 `fce064b`;
  UNPUSHED). **Bug 5 is now COMPLETE** (P1 0.1.27 + P2 here). `c83a1be`: an added
  SMB share / SSH folder now gets a friendly default label — bare share (SMB) / last
  remote-path segment (SSH), disambiguated against existing local-family labels
  (qualify with server/host, then numeric suffix; case-insensitive) via pure
  `unique_mount_name`/`last_path_segment` — instead of the URL-shaped
  `server/share` / `host:remote_path`; plus an optional **Name** field in both add
  forms (passed through the existing `name` param — NO schema change). `55a6852`:
  `rename_smb_mount`/`rename_ssh_mount` commands (pure `rename_*_mount_in_config`
  helpers; propagate the new label to the name copies seeded at add time — the SMB
  share-root folder `path==""` and the SSH-fed local folder — only when that copy
  still equals the OLD mount name, so a user-renamed folder is left alone) + an
  inline rename affordance in the Connected tab. `reviewloop codex` converged r1→r2:
  r1 reopened sspf-14 (MEDIUM — rename **Save** stayed enabled on a blank field, so
  the click surfaced the "A name is required." error, a Bug-5-UX-ruling-forbidden
  error click); fixed `5053d2b` (Save disabled on blank + silent Enter no-op; backend
  still rejects empty defensively). r2 accepted clean. Full CI green (cargo test 118,
  clippy -D warnings clean, npm check + build clean); pure helpers guard-proven
  red/green by the coder (codex ran read-only → `guard_confirmed:false`, its value
  was the code review that surfaced sspf-14). Trail `.agents/review/index.md` +
  `findings/sspf-14.md`. REMAINING: owner playtest — friendly default labels appear
  and rename works on the real NAS.
  **OWNER REPRIORITIZATION 2026-07-06: SMB/SSH work is DEFERRED — go Plex-first.**
  The owner is primarily a Plex user, is wary of Vela becoming Kodi-like, and chose
  to defer *all* remaining SMB/SSH work: **Bug 4 (share-root classification), the
  metadata "Recently added" rail, AND the SMB metadata-revalidation fix** (plan
  `.agents/plans/local-metadata-revalidation.md`, diagnosed but parked — SMB
  "reloading every open" = no-TTL stale-while-revalidate re-walk over the network).
  Owner: "if I care about that more later, I'll pick it up." Do NOT start these
  without an explicit owner go. Bug 4 technical detail is preserved below for
  whenever it resumes.
  **LIBRARY SORTING LANDED 2026-07-06 (0.1.30)** — first of the two Plex-first
  tracks is DONE. Plan `.agents/plans/library-sorting.md` (LANDED); reviewloop
  `sort-1` converged r1-r3 (trail `.agents/review/index.md`). Slices `c368270`
  (local honors release-date/last-played), `9a47d43` (`added_at_ms` + `Vfs::
  modified_ms`; date-added), `21552c9` (merged "All" view honors added/last-played/
  release-date); review fixups `c904c66` (cache SCHEMA 1→2 + `same_items`; merged
  dedup timestamp adoption) + `19b2735` (Plex shows carry addedAt/lastViewedAt via
  `PlexDir`); trail `4fca2ef`; version bump 0.1.30 `f3d8192`; UNPUSHED. Effective
  set delivered per-source AND merged: date added, date last played, title, release
  date. **Folder DROPPED** (owner: podcast/audio need, video-only Vela doesn't need
  it). DEFERRED (low value, Plex-first): JF + local last-played/added population
  (Plex sorts these server-side; only the merged ranking of JF/local items is
  affected; local last-played needs recents merged into library items). REMAINING:
  owner playtest — the sort dropdown on Plex libraries + the merged All view.
  **ACTIVE WORK (2026-07-06): the second track — Plex item detail / info view**
  (`.agents/plans/item-detail-view.md`), **APPROVED and implementing.** Owner go
  ("continue" on the prior handoff) + the repo-convention **codex plan-review loop
  CLOSED accepted at r3** (base=head `410fa4e`; three rounds, six findings idv-1..6
  all resolved — full trail in the plan's `## Review log`; trail commits `775e1ce`
  → `0df45b7` → `b6fab67`, UNPUSHED). The "richer client" surface. **Binding owner
  UX ruling:** CW carousel click = play; movie click → info page (Play on poster);
  show → seasons → episodes; episode → ONE shared info page updating per selected
  episode; info page = full-screen route. **Binding "no half-built state":** build
  all backends (Plex+JF+local) behind the current nav and **flip navigation last**
  (slice 5) — never a Plex-only stub. Five slices, each its own commit + `reviewloop
  codex` + version bump. **SLICE 1 LANDED 2026-07-06 (0.1.31)** — backend-only
  `DetailDto` + `item_detail` trait method (graceful `Err` default) + `get_item_detail`
  command + Plex `PlexDetail` serde parse over `/library/metadata/{rk}` + `to_detail`
  mapper (cast headshots via the tokened poster path). Code `b32821e`+`a2abcb7` (fixup:
  `DetailContainer` also captures `<Metadata>`); reviewloop `idv-s1` accepted CLEAN
  (base `fd0c414`, head `a2abcb7`); trail `2590508`; bump `2feac75`; all UNPUSHED. Nav
  unwired (flip is slice 5). cargo test 133 green, clippy clean; 3 guard-proven tests.
  **OWNER AMENDMENT 2026-07-08 (Plex-first)** — supersedes the "never a Plex-only
  stub" half of the binding above (see the plan's amendment section +
  `.agents/decisions.md` 2026-07-08): JF/Emby + local `item_detail` (original
  slices 2-3) DEFERRED (no start without owner go); nav flip UNIFORM (library
  views → detail for every source, CW carousel click = play; non-Plex opens the
  same pages sparse from listing `ItemDto`, detail `Err` → silent fallback, never
  an error page). **NOW: amended slice 2** (was slice 4, Plex + sparse fallback):
  info components + `detail_key`/server-preferred rank in `rank_backings` +
  merged-show drill via `detail_key` + episode paging/selection-generation guard.
  Then amended slice 3 (was 5): the uniform nav flip.
  **AMENDED SLICE 2 LANDED 2026-07-08 (0.1.32)** — code `7085fdf`; reviewloop-codex
  fixup `0ecd819`; trail `76415ea`; bump `c7aaeba`; all UNPUSHED. Backend:
  `detail_key` on merged `ItemDto` computed in `rank_backings` via `detail_rank`
  (reverse of `kind_rank`; folds away when redundant/unmerged; guard-proven
  red/green `merge_tests::*detail_key*`). Frontend: `src/lib/ItemDetail.svelte`
  (movie/video full-screen info page), `src/lib/SeasonDetail.svelte` (shared
  episode page — loads ALL `get_children` pages (idv-4a); per-episode detail
  cache keyed by the CURRENT selection so stale paint is structurally impossible
  (idv-4b)); both render listing data instantly and enrich from `get_item_detail`,
  Err → silent sparse render (the NORMAL path for deferred JF/Emby/local);
  shared `Item`/`Detail` types in `src/lib/types.ts`; merged shows drill through
  `detailKey ?? ratingKey` in `open()` (idv-5). Entry: context-menu **Info**
  (dev builds, or localStorage `vela.devDetail`="1"; no entry on shows — they
  keep the seasons drill); the detail view LAYERS over home/browse state (Back/
  Esc returns exactly; cleared on any nav). Loop `idv-s2` r1 reopened idv-s2-1
  (LOW — episode Info inside an open season page re-listed seasons as episodes;
  fixed via `seasonKeyFor`: trust only the open page's own key or a crumb whose
  grid contains the episode, else single-episode mode), r2 accepted clean
  (trail `.agents/review/index.md` + `findings/idv-s2-1.md`). svelte-check 0/0,
  npm build, cargo test 111 (Windows host), clippy baseline-clean. REMAINING:
  owner playtest (dev build, or set the localStorage flag in a release build:
  Info on a Plex movie → rich page; Info on a season → shared episode page;
  Info on a JF/local item → clean sparse page, no error); then slice 3.
  Load-bearing
  facts the plan-review pinned (don't relearn — all in the plan): Plex listing IS
  serde-parsed (`get_items`/`ItemsContainer`, `plex_library.rs:669`) so Media/Part
  are already populated — slice 1 adds a NEW `PlexDetail` serde struct, not a
  hand-rolled parser; merged cards need an explicit `detail_key` computed in
  `rank_backings` via a SERVER-preferred rank (reverse of `kind_rank`) — routing both
  movie detail AND the merged-show `get_children` drill (`commands.rs:3262`);
  local-family detail runs on the blocking pool; the episode page pages the full
  `get_children` list + a per-selection generation guard. One non-blocking owner
  open-decision remains (merged-show episode playback source, idv-5 — default =
  server-rich; needed only by slice 4/5).
  **DEFERRED (owner 2026-07-06) — Bug 4 (LARGER) — share/mount root shows bare metadata-less cards,
  starves the merged view (P2, the metadata unlock).** The share-root auto-add
  registers the whole share as ONE flat kind-auto folder; a NAS root of category dirs
  (Movies/, TV/…) is mis-classified into one flat section of bare cards (no
  title/year/poster), which can't dedup against server copies in the merged All view.
  Fix (see plan Bug 4 for the full design + the THREE codex-pinned constraints): make
  kind-auto roots category-aware — expand each into per-category effective
  `LocalFolder` roots so `sections()`/`items()` see normal configured folders (the
  `items()` guard at `local.rs:744` rejects non-configured section keys — do NOT
  loosen it); key the detected-kind cache by **source/mount id + path** with a schema
  bump (raw-path keys collide across mounts and a stale root kind preserves the flat
  classification); and run the expansion OFF-LOCK on the blocking pool (NOT under
  `config::update`/`source_lock` — the lock-across-blocking invariant slice 7
  `e7c5231` honored). This is the metadata unlock (`metadata.rs` already resolves
  .nfo/artwork sidecars over the VFS + keyless iTunes/TVmaze once items parse at the
  right level). DESIGN FORK to settle first: lazy effective-root cache inside
  `LocalSource` (expand under `run_blocking` in sections()/items()) vs a
  config-snapshot expand-then-swap.
  **DEFERRED with Bug 4 — the metadata-gated local/SMB/SSH "Recently added" rail**
  (depends on Bug 4 — only items with resolved metadata, never blank filename cards).
  STANDING INSTRUCTION: `reviewloop codex` on every slice; bump version per landed
  code slice (routine). Reviewloop mechanics (proven across the SMB/SSH, sorting, and
  item-detail loops — unchanged): codex incantation `codex exec --json -s read-only
  --output-schema <schema> "<prompt>" </dev/null` (final verdict in the last
  `item.completed` agent_message; schema =
  `{verdict,guard_confirmed,reviewed_sha,base_sha,comments}`, fail-closed — recreate the
  schema file each session, it's a scratch artifact); pin base = pre-slice SHA, head =
  the fixup commit each round; per slice: code commit(s) → trail commit (`review(...)`)
  + version bump (`scripts/bump.sh`) + `handoff:` state commit. For the item-detail
  work the PLAN itself also went through the loop first (accepted r3) — a plan is
  `guard_confirmed:false` (design doc, no executable guard), findings live in the plan's
  `## Review log`, not `findings/`. codex reads read-only so `guard_confirmed:false`
  always — the CODER guard-proves each code slice red/green (revert → FAIL → restore →
  PASS) and records it. Findings are only real when they predict an observable failure;
  a clean accept (0 comments) is a valid outcome (slice 1 `idv-s1` accepted clean).
  E2E/test techniques earned this session (reuse them):
  - **E2E must run HEADED on this host** — Xvfb is NOT installed, so the harness's
    default headless mode fails; run `VELA_E2E_HEADED=1 npm run e2e -- <scenario>`
    (owner-approved 2026-07-06; it pops a window over the desktop). Frontend slices
    rebuild the debug binary to embed the change, so guard-proof = revert + rebuild +
    run RED, restore + rebuild + run GREEN.
  - **Hermetic Connected-tab E2E**: seed a NATIVE SMB mount (`mountpoint:""`) in
    `config.json` — renders from config (get_sources + list_smb_mounts) with NO
    connection (`tests/e2e/scenarios/connectedtab.mjs`).
  - **Hermetic sshfs test**: a non-root loopback sshd (StrictModes no, UsePAM no,
    key-only) + sshfs mount + read, gated on sshd/sshfs/ssh-keygen + /dev/fuse
    (`src/sshfs.rs` `max_conns_option_set_mounts_and_reads_over_loopback_sshd`).
  - **Testable platform gates**: split on an explicit OS string (`sshfs_options_for(os)`
    via `std::env::consts::OS`) instead of `#[cfg]`, so both branches unit-test from
    any host.
  Load-bearing gotchas from the plan-review a
  fresh session must not relearn:
  - SMB seek (slice 1): every mpv seek rebuilds a full libsmbclient session; fix =
    per-token session reuse (own file handle per stream, generation-owned
    compare-and-remove cleanup, cooperative cancel bounded by `OP_TIMEOUT_MS`).
    Do NOT release `ctx_lifecycle_lock` around `smbc_free_context` (codex blocker
    — lifecycle race); remove per-seek context churn instead.
  - SSH seek is DISTINCT (raw sshfs path, NOT the proxy): fix = sshfs mount
    options (max_conns/cache) + a required hermetic loopback sshd+sshfs test.
  - Bug 4 (share-root) expansion must run OFF-LOCK on the blocking pool (never
    `detect_kind`/`read_dir` under `config::update`/`source_lock`); key the kind
    cache by mount id + path with a schema bump.
  - Source-click routing keys on the empty-Home STATE (not "any source") so
    server-source Home hubs aren't regressed.
  - METADATA (owner-gated the whole thing on this): no product shift.
    `metadata.rs` already resolves `.nfo`+artwork sidecar OVER THE VFS (so SMB/SSH
    work) + keyless iTunes/TVmaze online; the blank filename cards ARE Bug 4's
    mis-classification, and Bug 4 is the metadata unlock. The local/SMB/SSH "Recently
    added" Home rail (last slice) is metadata-gated (only items with resolved
    metadata; never blank filename cards).
- E2E slice 11 landed 2026-07-05 (`7c899be`) + review loop e2e-10 CLOSED
  (eh-15 verified, fix `6db391c`): the markwatched scenario now round-trips
  mark-unwatched (DELETE PlayedItems, server flip, badge clears). eh-15
  then hardened BOTH badge legs — each assertion gates on a later
  `/Users/{u}/Items` server refetch and asserts a PRESENT card, so a
  refetch that drops the card or serves stale state can no longer pass on
  the optimistic mutation or the empty-grid refresh gap (the old
  `!card?...watched` unwatch wait was vacuously true while the card was
  missing). Guard-proven with a drop-after-unwatch mock (pre-fix scenario
  GREEN on the dropped card, fixed scenario RED) and codex-accepted; a
  3-lens adversarial pre-review (all `refuted:false`) confirmed the
  optimistic *watched* card never paints (batched Svelte flush), so the
  unwatch leg was the load-bearing hole. Suite: 9 scenarios. The
  locally-testable backlog stays EXHAUSTED; SMB + live scenarios remain
  owner-cred-gated (see Next).
- E2E slice 10 landed 2026-07-05 (`5742789`) + review loop e2e-9 CLOSED
  (eh-14 verified): the merged All view scenario machine-verifies the
  library-rework owner checks — mock JF + local folder dedup to ONE
  "2 sources" card; "Play from Local" plays the file path while "Play
  from Mock JF" plays the mock stream; the per-title override persists
  under the exact canonical key (`title:mockmovie|2020`) and flips with
  the choice. Suite: 9 scenarios. Unblocked follow-on remaining:
  mark-unwatched (small).
- E2E slice 9 landed 2026-07-05 (`ccc6270`) + review loop e2e-8 CLOSED
  (eh-13 verified after the loop's first reopen→fix→accept round-trip):
  the watch-state scenario machine-verifies the 2026-07-04 owner-reported
  "stale until restart" fix end-to-end — mpv plays THROUGH the mock's
  Range-capable HTTP stream, Start/Stopped check-ins carry correct
  ItemId/MediaSourceId/PositionTicks, and the card gains '% watched'
  without restart. Suite: 8 scenarios. Remaining unblocked follow-ons:
  mark-unwatched; merged All view (mock JF + local folder = two sources).
- E2E slice 8 landed 2026-07-05 (`c706228`) + review loop e2e-7 CLOSED
  (eh-12 verified): NEW HARNESS LEG — a hermetic mock Jellyfin server
  (`tests/e2e/mockjf.mjs`, stateful, fail-closed on the client's Items
  query contract) runs inside the runner; a seeded `sources` entry
  restores it at boot with no auth. The mark-watched scenario asserts
  both the PlayedItems POST and the watched badge surviving the refetch.
  Suite: 7 scenarios. UNBLOCKED FOLLOW-ONS this leg enables without owner
  creds: watch-state-refresh-after-playback scenario (mock + local play),
  mark-unwatched, and a merged All view scenario (mock JF + local folder
  = two sources).
- Slice 7 landed 2026-07-05 (`e7c5231`, loop app-1 CLOSED clean):
  `resolve_stream`'s VFS checks (canonicalize/is_file — network-priced on
  native SMB) moved onto the blocking pool per the async-worker invariant;
  full Kimi-P0 audit against current code came back all-resolved and
  repo-map's stale deferred-issue note was corrected.
- E2E slice 6 landed 2026-07-05 (`fc902f4`) + review loop e2e-6 CLOSED
  (clean pass, no findings): search scenario — short-query validation
  error, hit/miss result filtering, play-from-results over mpv IPC.
  Suite: 6 scenarios. The harness's locally-testable backlog is now
  EXHAUSTED — remaining scenarios (SMB, merged All view, server-gated
  mark-watched/watch-state/Plex-removal) are blocked on owner-provided
  creds/test-server env (see Next).
- E2E slice 5 landed 2026-07-05 (`9274ac2`) + review loop e2e-5 CLOSED
  (eh-11 verified): the queue scenario proves backend auto-advance live —
  clip A to natural EOF over IPC, "Play next"-queued clip B spawns in a
  fresh mpv session with no UI interaction. eh-11 hardened it against the
  EOF-races-UI-window flake (A paused across the menu/screenshot window).
  Suite is now 5 scenarios: smoke, playback, curation, resume, queue.
- E2E slice 4 landed 2026-07-05 (`2f5bba8`) + review loop e2e-4 CLOSED:
  the resume scenario caught eh-10 (`4527613`), a HIGH app bug —
  **Continue Watching restarted local/SMB/SSH items from 0:00** (local
  provider resolves resume_ms 0; play_by_key ignored Vela's own stamp).
  play_by_key now falls back to `recents::resume_stamp_ms` when the
  provider resolves 0; server positions still win. Queue auto-advance
  inherits the fallback (routes through play_by_key). Codex batch pass on
  the slice itself: clean, no findings.
- E2E slice 3 landed 2026-07-05 (`ee01101`) + review loop e2e-3 CLOSED
  (eh-8, eh-9 verified): the curation scenario drives
  remove-from-continue via the hero's real context menu, proves tombstone
  APPLICATION across a true app restart (session recycle; recents entry
  reinserted while the app is down so only the tombstone can keep the
  hero empty; environ-scoped pid guard immune to the owner's own Vela),
  and replay-restore. This live-verifies the cw-3 semantics that were on
  the owner-check list. Mark-watched stays server-gated (local items have
  `played: null` — no menu entry).
- Review loop e2e-2 (2026-07-05) CLOSED: 3 findings verified — eh-5 (the
  app bug below), eh-6 (scenario raced the seeded source render), eh-7
  (the guard couldn't tell IPC quit from natural EOF; now a socket
  connectability probe + [3000,8000] stamp bound). Trail:
  `.agents/review/index.md` + `findings/eh-*.md`.
- E2E slice 2 landed 2026-07-05 (`d2be263`): mpv-IPC playback scenario —
  seeded local source (ffmpeg clip), real poster-click play, mpv socket
  probe (path/seek/quit), recents `viewOffsetMs` stamp, hero assertion.
  It immediately caught a REAL app bug (eh-5, fixed in `b4b4ebb`): the
  Continue Watching hero was hub-gated, so local-only setups never saw
  it despite the 2026-07-04 recents-fed-hero decision. Harness knowledge
  gained: `mpv_extra_args` parses one option per LINE; recents items
  serialize camelCase (`viewOffsetMs`); local folder `kind` is singular
  (`movie`/`show`).
- Review loop e2e-1 (2026-07-05) CLOSED: slice-1 review found 2 defects
  (codex) + 2 coder-filed during live diagnosis — signal-orphaned process
  groups; silent false-green on typo'd scenario filters; unbounded
  requests hiding stalls for 300s; and the big one, eh-4: on the live
  Wayland desktop, screenshots hang whenever the test window opens
  unfocused (no frame callbacks). The harness now runs HEADLESS on a
  managed Xvfb display by default (`VELA_E2E_HEADED=1` to watch live;
  `VELA_E2E_DEBUG=1` for per-call timing) — runs are deterministic and
  never pop windows over the owner's work. All 4 fixes verified
  (`.agents/review/index.md`, `findings/eh-*.md`).
- E2E harness slice 1 landed 2026-07-05 (plan `.agents/plans/e2e-harness.md`,
  approved via the 2026-07-04 delegation): `npm run e2e` drives the real
  debug binary via `tauri-driver` + a **vendored Debian WebKitWebDriver
  2.50.6** — verified fact: no distro ships a driver for webkit2gtk 2.52
  (Arch/Fedora/openSUSE drop it; Debian tops out at 2.50.6); the version
  skew was probe-validated live and the deviation (plus the no-WDIO,
  zero-dep WebDriver client) is recorded in the plan. Throwaway
  `XDG_CONFIG_HOME` per scenario; screenshots + driver logs in
  `tests/e2e/artifacts/`; smoke scenario green and red-proven (broken
  assertion → exit 1). Owner standing instruction 2026-07-05: run
  `playbook reviewloop codex` on EVERY slice.
- Review loop cw-1..cw-3 (2026-07-05) CLOSED: codex batch pass over
  `ec94715..a055556` found 3 real defects (merged-key miss in curation
  actions; registry lock across network await; failed play clearing
  tombstones), all fixed as single commits on `main` and independently
  verified. Trail: `.agents/review/index.md` + `findings/cw-*.md`.
- OWNER DELEGATION 2026-07-04 (decision recorded): progress must not block
  on the owner. The two locked-choice plans were approved via that
  decision and are now IMPLEMENTED, one commit per slice, all
  guard-proven, full suite green (78 Rust tests; svelte-check + build
  clean):
  - SMB share-root auto-add (`f05919e`): adding a share auto-selects its
    root as a library folder — the zero-folder invisible-share trap is
    closed. Existing zero-folder shares (owner's `zoey`) are NOT
    migrated by design; re-add the share or add a folder once.
  - Continue Watching curation (`d2ea1a7`, `cf5af95`, `d259213`): On
    Deck folds into the hero cover-flow via Vela's own
    `/library/onDeck` fetch (synthetic hub `vela.ondeck`), everything
    interleaved by recency (`lastWatchedAtMs`); mark-watched drops the
    recents entry and re-fetches; "Remove from Continue Watching"
    (hero context menu) tombstones the key (`hidden_from_continue`,
    FIFO cap 200, cleared on replay) + best-effort Plex server-side
    removal. Implementation notes + deviations in the plan file.
  - Open verification residual: the Plex removal route's real-item
    effect is unverified (permissions layer correctly refused a live
    mutating probe); route existence IS live-verified. Non-fatal by
    design. Also: hero merge ORDERING is frontend-only and has no unit
    test (no JS runner in repo) — E2E harness covers it later.
  - E2E harness plan drafted (`.agents/plans/e2e-harness.md`,
    approved-in-principle via the delegation decision): tauri-driver +
    WebdriverIO UI driving, mpv-IPC playback probes, env-gated live
    smoke, throwaway config dir, creds via env only. NOT yet built.
  - Versioning clarified by the owner 2026-07-05: bumping is routine —
    just run `scripts/bump.sh` when code lands (per the 2026-06-20
    decision); it is NOT an owner gate. Bumped to 0.1.10.
- MERGED to `main` 2026-07-04 (owner go; merge commit `e9f6029`; the
  `smb-native` branch is deleted — owner direction: no branches without
  his explicit word in future). `.agents/plans/smb-native-client.md` is
  IMPLEMENTED. All six slices codex-verified
  (`.agents/review/index.md`; smb-2/3/4/5 took 2-3 rounds each — every
  reopen was a real finding, recorded in the finding docs). Linux SMB is
  now native and mountless: libsmbclient in-process (via pavao-sys, one
  context per connection), share browsing/listing/search through the
  local-family Vfs provider, playback via a loopback HTTP Range proxy
  (127.0.0.1, tokenized), sidecar posters via the stable `velasmb:`
  scheme, .nfo enrichment over the wire. gvfs/kio machinery deleted;
  macOS/Windows keep OS mounts. PKGBUILD depends on `smbclient`; deb/rpm
  on `libsmbclient`.
  NEXT: (1) owner playtest on the real NAS — add share 10.1.10.206/media
  with credentials in Settings (native, no mount), browse, add a folder,
  play/seek/resume, check hero recents and posters; (2) version bump
  (not done on the branch — owner's release call). Env-gated live probe:
  `VELA_SMB_LIVE=server/share cargo test --lib live_probe -- --nocapture`.
- Vela is a Tauri 2 + SvelteKit + Rust desktop media client for Plex,
  Jellyfin, Emby, local folders, SMB shares, and SSH/SFTP mounts. It plays
  media through the system `mpv` binary for HDR passthrough.
- Version 0.1.31 (bumped 2026-07-06, `2feac75`, for item-detail-view slice 1).
  The owner pushes manually (push policy:
  ask, `.agents/push-policy.md`); treat remote positions as owner-managed.
- 2026-07-04 landed a large batch, all owner-approved and verified:
  - All five approved plans implemented (see `.agents/plans/*`): post-playback
    watch-state refresh (`playback-ended` event); platform-aware sshfs
    guidance in the add-SSH UI; each SMB/SSH mount registered as its own
    named source; split artwork policy (16:9 resume surfaces, 2:3 catalog
    rows with series posters); library rework phases A-D (persistent listing
    cache for local/SMB, consolidated type-based All nav, cross-source dedup
    via provider ids with backing lists, kind-ranked playback with per-title
    override persisted in `merged_overrides`).
  - The batch then passed a cross-harness review loop (playbook
    `reviewloop`, reviewer codex): 5 findings fixed, guard-proven,
    independently verified, merged. Durable trail:
    `.agents/review/index.md` + `findings/`. Notable outcome: the merged
    All view pages from an immutable `MergedSnapshot` in `AppState`
    (stateless merged pagination was proven unsound in review).
  - Post-review owner-directed UI changes (decisions recorded 2026-07-04 in
    `.agents/decisions.md`): the Continue Watching hero is a cover-flow
    (~30% window height, older items fanned behind-left, newer behind-right,
    always-visible arrows) fed by Vela's OWN recents — semantic: "recently
    played and not finished = Continue Watching", any source, any duration
    (`src-tauri/src/recents.rs`; snapshot at play, position stamped at mpv
    exit via `EndNotify(u64)`, finished entries dropped at
    `watched_threshold_percent`, default 95%). Library nav moved to a left
    sidebar (Home / Library / Sources groups, Infuse reference
    `reference_screens/infuse-home-reference.png`).
- Token/credential stance: poster URLs (all backends) and Jellyfin/Emby
  stream URLs are accepted local-only exposures; SMB mount arguments remain
  one only on macOS/Windows (Linux credentials never leave the process —
  libsmbclient auth callback); Plex stream auth rides as an `X-Plex-Token` header via an
  owner-only mpv include file. Add nothing new that logs or displays
  token-bearing URLs. Recents snapshots in `config.json` carry poster URLs —
  same exposure class as the config's stored tokens.
- macOS SSH live testing is parked (brew macFUSE/sshfs-mac unstable on the
  owner's machine); the shipped in-UI guidance is the decided handling.
- Known accepted v1 gaps: backend queue auto-advance plays are not
  snapshotted into recents; local-source series artwork deferred (portrait
  cards fall back to episode still/no-art); a merged card's progress bar can
  reflect server state while the ranked play target is a local copy (the
  per-title override is the escape hatch).
- `scripts/build.sh` takes ~2.5 min cold on macOS; the session's `!`
  foreground runner kills at 2 min mid-DMG (leaves a mounted staging volume
  and no final dmg) — run builds via the agent (no cap) or a real terminal.
  The `.app` exists only transiently during bundling; the DMG is the
  artifact.


## Rotated 2026-07-10 (cw-watch-state owner-verified)

- QUEUED LAST (owner, 2026-07-08, from the 0.1.33 playtest — "add this to the
  bottom of the queue"): **Continue Watching carousel needs a one-op
  curation.** Owner-reported annoyance: "if I mark a video in the carousel as
  unwatched, it stays in the carousel. if I remove it from continue watching,
  the watched status remains. so I have to do two ops to get what I want."
  RESOLUTION: folded into `.agents/plans/continue-watching-watch-state.md`
  and implemented 2026-07-10 (`02504be`) — mark-unwatched is a one-op full
  reset that also leaves the carousel; owner-verified on 0.1.42.


## Rotated 2026-07-20 (Vela 1.0 release closeout)

# Agent State

This file is the first place future agents should read for current repo state.
Keep it short and update it when important repo facts change. Landed or
superseded entries rotate verbatim to `docs/history/state-archive.md`.

Machine-specific facts (host paths, tool quirks, the E2E venue) live in
`.agents/machines.md`, never here — this file stays portable.

## Now

- **PLAYBACK-SOURCE POLICY COMPLETE ON FEATURE BRANCH AT 0.1.62.** The
  Multi-Plex core remains complete through `5e63462`, with accepted review
  evidence through `3a1dd8b`. The approved playback plan implements Prefer
  Best, Prefer Compatible, Prefer Fastest Source, and Ask Every Time; exact
  per-title Play Version behavior; display-aware selection; sequence-scoped Ask
  affinity; merged hierarchy continuation; title-level watched fan-out; and the
  offline server-playlist boundary. Slice implementations are `c7ac901` plus
  `cadbbb0`, `7d9a00e`, `7720d2a` plus `a749974`/`b4b702b`, `3391986`, and
  integration `62133b3` plus `c07abc8`.

  Windows-native validation exposed and closed four portability findings in
  one commit each: path normalization `35a54de`, a Windows-only skip for the
  POSIX packaging harness `1cd3e0e`, Unix-helper cfg cleanup `ed4c745`, and
  warning-clean DisplayConfig initialization `54c2f09`. Every guard was
  independently regressed on Windows, failed for the intended reason, restored,
  and reran green. Version 0.1.62 is `1a2bef5`.

  Exact implementation head `1a2bef5` passes the complete macOS canonical set;
  exact tracked Linux bytes pass toolchain/npm audit/frontend/stable/clippy,
  fresh-build real-app E2E, and arm64 deb/rpm packaging; and an independently
  hash-matched Windows archive passes frontend, Rust 1.89/stable, warning-free
  clippy, native Rust tests, and NSIS packaging. With explicit owner approval,
  the 0.1.62 installer replaced 0.1.37 on `netwatch-01` and verified the installed
  registration/executable at 0.1.62; the owner subsequently confirmed Windows
  HDR works. The owner explicitly ended further Fable reviews after the clean
  one-pass plan review; none were run during the implementation or Windows
  closeout.
  Durable evidence: `.agents/plans/multi-plex.md` and
  `.agents/plans/playback-source-policy.md`.

- **`chr-1` MERGED to `main` — accepted by owner-directed in-session Claude
  review.**
  The clean-EOF dispatcher now emits its authoritative Home refresh after the
  best-effort server played-state attempt while releasing playlist/Continue
  Playing work first. Implementation `6ec2ba6`; exact-head evidence/docs
  `fe8eebe`; version 0.1.60; local canonical gates and fresh Linux real-app E2E
  29/29 passed, with five independent production mutation proofs restored
  exactly. After the Claude Code 2.1.214 MCP reviewer transport was proven
  unable to inherit launch-granted permissions, the owner waived headless
  dispatch and directed an interactive in-session Claude Code 2.1.215 review;
  it accepted `6ec2ba6` at head `5a7cabf` with no material issues (same-vendor
  caveat recorded). Verdict and evidence: `.agents/review/findings/chr-1.md`.
  The governance defect and its prevention are filed as
  `https://github.com/roethlar/AgentGovernanceBootstrap/issues/6`. The owner
  approved the merge on 2026-07-19; `main` fast-forwarded to `5248fe6`.

- **`pws-1` IMPLEMENTED AND EXTERNALLY ACCEPTED.** Exact automatic successors
  retain the completed mpv process's actual fullscreen and maximized state;
  manual starts retain configured defaults. Claude Code 2.1.214 /
  `claude-opus-4-8` / high accepted exact head `8d4c7bc` with an independent
  red/restored-green stale-session proof and no comments. Exact status lives in
  `.agents/review/findings/pws-1.md`.

- **Version 0.1.60 on `main`** (`package.json`, both lockfiles,
  `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, and the Arch PKGBUILD
  agree, as of implementation `6ec2ba6`, merged at `5248fe6`).

- **v1.0 README POLISH COMPLETE; OWNER-REPORTED ISSUE QUEUE IS NEXT.** The
  user-first README rewrite landed at `d4d9e95`, correcting first-run, server-
  parity, multi-Plex, privacy, build, and support claims while surfacing the
  current product features. Functional and UI implementation through OLED
  Black is complete. The owner ran
  `./scripts/build.sh` on 2026-07-17 and produced the unsigned universal macOS
  0.1.57 DMG successfully; GitHub CI passed at `1300f00`, and the fresh Linux
  real-app suite passed 27/27. On 2026-07-18 the owner directed autonomous work
  through the open queue before returning to release-track graphics. The
  deferred real-Plex completion/error smoke remains the final pre-publication
  gate.

- **LIVE E2E (`npm run e2e:live`) — landed 2026-07-14, owner-approved.** Drives
  the app against the owner's REAL Plex and REAL Jellyfin from the Linux VM.
  Opt-in; NEVER part of the gating suite (non-hermetic). Venue, access grants and
  restore-on-exit rules: `.agents/machines.md`.
  - **Why:** the owner's manual playtests found FOUR defects in two sessions that
    18 mock scenarios and 24 rounds of two-reviewer review all missed. Real
    servers say things mocks do not.
  - **What it closed:** the Plex scan path had NEVER been exercised — the mock is
    Jellyfin (GUID ids, never rebinds), so a real Plex section key (a
    server-LOCAL number) had never appeared in a test. `live-plex` now browses
    real libraries and scans one, red-proven.
  - **STILL OPEN, and unclosable here:** a Plex REBIND needs a SECOND Plex
    server, which does not exist. `sameSection` and the section-binding
    comparison remain inspection-only. (The release question this raises is
    answered in `## Next`.)

- **CI: a known vulnerability FAILS the build** (owner ruling 2026-07-14, "known
  vulnerabilities should fail so we can keep current"). The `audit` job used to
  be `continue-on-error: true` and had been swallowing two RUSTSEC DoS advisories
  in `quick-xml` — the crate that parses every Plex response off the network —
  into green runs (fixed `5cb467f`, gate flipped `2ba5f95`). Advisories with no
  upstream fix get an explicit `--ignore RUSTSEC-XXXX-NNNN` with a reason, never
  a blanket re-disable. The remaining entries are WARNINGS (unmaintained GTK 0.18
  crates via Tauri 2, one unsoundness note); `cargo audit` does not fail on those.
  - CI runs on the `github` remote only, not the gitea `origin` — see
    `.agents/repo-guidance.md` (Verification). Run the checks locally; do not
    assume CI covered them.

- **PRODUCT DIRECTION (2026-07-08, owner): Vela is a multi-server client.**
  Local/SMB/SSH sources are REMOVED (decision `.agents/decisions.md` 2026-07-08;
  plan `.agents/plans/drop-local-sources.md`, COMPLETE). The owner is Plex-first
  today and will eventually migrate to Jellyfin or Emby; the watch-state
  migration goal is a one-shot direct Plex→JF/Emby copy in Vela at migration time
  if simple (no Trakt relay). Nothing of that tool is built yet.

- **Completed and owner-verified work rotates to
  `docs/history/state-archive.md`** (rotations 2026-07-14 and 2026-07-17; do
  not reconstruct from chat). The archive owns that list; plans remain in
  `.agents/plans/` as design records.

## Next

- **Reviewer-transport cleanup COMPLETE (2026-07-19):** the `chr-1` temporary
  local artifacts recorded under the macOS host in `.agents/machines.md` are
  cleaned with owner approval; the disposable worktree was confirmed clean at
  `fe8eebe` and removed. Do not retry Claude MCP review on Claude Code
  2.1.214: the exact child-tool capability is proven unavailable. `chr-1`
  merged to `main` at `5248fe6` on 2026-07-19; no chr-1 gate remains besides
  the deferred real-Plex smoke.

- **NEXT: work the open issue queue one item at a time.** The
  owner reports from 2026-07-18 are code-traced at the top of `ISSUES.md`; the
  older macOS `build.sh --native` failure follows them. Each code item gets its
  own durable plan, guard proof, commit, and Claude `codereview` before the
  next begins. Outbound actions remain governed by `.agents/push-policy.md`.

- **CLEAN-EOF CAROUSEL REGRESSION — IMPLEMENTED; REVIEW AND OWNER PLAYTEST OPEN.** The owner's 0.1.51
  macOS playtest found that a naturally completed episode remains the only
  Continue Watching card until it is manually marked watched. Diagnosis found
  a pre-existing missing explicit server played mutation plus Slice 5's
  end-refresh/automatic-start repaint race. Draft plan:
  `.agents/plans/clean-eof-carousel.md`. Claude r1 and r2 reopened concrete test
  design issues, which are revised at `43957b1`; r3 then failed without a
  verdict because the local bridge refused connection and its one direct retry
  hit the Claude session limit. The next owner-authorized attempt was discarded
  before verdict when the owner replaced Claude's standing review prompt with a
  neutral best-way-to-achieve-the-goal question (`85e7cd0`). Goal-only r5 then
  accepted `0dd5001` with no material finding and two compatible hardening
  comments. Goal-only r6 accepted the clarified `f57d6a4` with no material
  finding, and goal-only r7 accepted the final `a13942b` plan with no material
  finding. The owner approved the plan on 2026-07-16. Code/test slice
  `8894ca6` is implemented; all seven planned mutations failed for the right
  reason and restored green, the canonical local gates and fresh-build Linux
  E2E 24/24 pass, and two independent Grok secondary reviews accepted after
  separately proving the newer-session and tombstone guards. Version 0.1.52
  landed at `52e1a67`, and its unsigned universal macOS DMG is checksum-valid
  with both architectures. The refreshed primary Claude plan `openreview`
  admitted one LOW lock-scope finding. Primary Claude `codereview` then
  independently red-proved the newer-session and tombstone guard families,
  restored all 140 Rust tests green, and accepted the implementation; exact
  status lives in `.agents/review/index.md`. The owner's final real-Plex
  completion playtest remains unresolved; verification and artifact details
  live in the plan.

- **OWNER-GATED FOLLOW-UP: publish dependency refresh 0.1.51.** Implementation
  is COMPLETE. The
  owner selected Node 26 over current Node 24 LTS, approved a scoped
  patched-cookie override plus fail-closed npm audit, and authorized aligning
  only Node/npm on the existing Linux E2E VM. Slice 1 landed at `7fef89a`; its
  Windows assertion review fix landed at `adc0104`, and Grok accepted the
  corrected full slice at r2. Node 26.5/npm 12.0.1 now drive the repo and E2E
  VM; the VM's Ubuntu packages remain untouched. Slice 2 then landed at
  `986fa2e`, changing
  only the Ubuntu appindicator prerequisite to the current Ayatana package;
  Grok accepted it with no comments. Slice 3 landed at `28159ea`: the compatible
  Vite 8 / Svelte / TypeScript 6 / Tauri JS graph, scoped patched-cookie
  override, explicit optional-hook denial, and fail-closed npm audit gates.
  Local canonical checks, the security red proof, Vite HMR protocol, Tauri
  build, and Linux real-app E2E 18/18 passed; Grok accepted the pinned diff with
  no comments. Per the owner's 2026-07-15 correction, Codex-authored code is
  reviewed externally by Grok or Claude; Codex CLI self-review does not count.
  Slice 4 landed at `770bfba`: the compatible Cargo graph is current, the Rust
  1.89 floor is now checked in CI, audit is at zero vulnerabilities, and local
  plus Linux real-app verification passed. Grok accepted the pinned diff with
  no comments. Slice 5 landed at `8559c59`: `directories` 6 preserves the exact
  macOS/Linux/XDG config paths, audit and all compiler gates pass, Linux E2E is
  18/18, and Grok accepted the pinned diff with no comments. Slice 6 landed at
  `fa3d04f` plus live-coverage commits `69a8f83`/`3a002fa`: every Plex scalar
  mapping now uses serde XML 0.8 attribute syntax while repeated child elements
  remain children. Six mapping regressions were proven red; Rust, audit,
  frontend, Linux E2E 18/18, signal cleanup, and live Plex
  browse/detail/episode/play/watch/scan/offline/restart all passed. Independent
  post-run checks found the watch fixture clean, credentials removed, and both
  Plex services active. Grok and Claude Fable 5 each independently red-proved
  a guard and accepted the exact Slice 6 diff with no comments. Slice 7 landed
  at `1d619fd`: reqwest 0.13.4 retains native TLS without ALPN, the direct 0.12
  duplicate is gone, and the required local, Linux E2E/live-server, Linux
  package, and macOS universal package checks passed. Grok independently
  red-proved the explicit `query` feature and accepted the pinned diff with no
  comments. Slice 8 then closed four integration findings: `4cba5db` enforces
  the pinned Node/npm pair in local, CI, and release install paths; `ec7c43e`
  replaces the old WebKit driver/ICU fixture with Ubuntu's exact 2.52.3 match;
  `5532e93`/`8366b4f` make the live Jellyfin library wait deterministic; and
  `bff2905` prevents stale installers from being recopied into `dist/`.
  Their guards, real integrations, and external Grok/Claude reviews passed.
  `dc73627` bumps Vela once to 0.1.51. The final local canonical suite, Linux
  E2E 18/18, live server checks, current-only Linux deb/rpm packages, and
  checksum-valid macOS universal DMG are green. Claude Fable 5 independently
  accepted the complete pinned Slice 8 integration/version range with its
  guard confirmed and no comments. No implementation gate remains; publishing
  or triggering GitHub-hosted Windows/CI proof requires a separate owner go.
  Exact evidence lives in
  `.agents/plans/dependency-lts-refresh.md`.

- **PLAYLIST IMPLEMENTATION COMPLETE: `.agents/plans/playlists.md`** (all five
  approved slices landed, were red-proven, passed canonical verification, and
  were externally accepted by 2026-07-16).
  Product model and the two durable
  rulings: `.agents/decisions.md`
  (2026-07-14 — no play queue; video stays external). Five slices.
  - **THE PLAY QUEUE IS DELETED** (owner ruling; `ec5d613`, two independent
    Grok acceptances at r1). Ephemeral queues are a
    music idiom; the only preset video sequence worth having is a show binge, and
    there the sequence IS the show's episode order — which Continue Playing walks.
    Anything larger is a named playlist. Infuse's model, and the owner's. S1
    deleted the chip, the drawer, the six `queue_*` commands, and
    per-surface-status slice 2 (`67358fd`) with them; it also added explicit
    Resume / Play from Beginning behavior. Exact verification and fail-closed
    review trail: `.agents/review/findings/pl-s1.md`. **This is why the queue step
    of the playtest above is gone: never ask the owner to test a surface that is
    being removed.**
  - **S2 (every successful play records a recent) is complete** (`c6bc5c1`, two
    independent Grok acceptances at r1). The shared backend path is now the sole
    play-start writer; failed launches create no recent and preserve tombstones,
    and the matching end callback cannot overtake its start record. Exact guard
    and review trail: `.agents/review/findings/pl-s2.md`.
  - **S3 (durable Vela playlists and session-safe sequence playback) is
    complete** (`304f493`, two independent Grok acceptances at r1). The
    fail-closed JSON store, stable entries, editor, cross-source routing,
    retained unavailable entries, and exact-session/fresh-anchor advancement
    were independently red-proved; restored Rust passed 118 tests and Linux
    real-app E2E passed 20/20. Removed sources are pre-marked unavailable;
    configured-but-offline sources are skipped at route time. Exact guard and
    review trail: `.agents/review/findings/pl-s3.md`.
  - **S4 (read-only server playlists) is complete** (`963ef73`; integration
    guard repair `4090d73`; independent Grok acceptances at r1). Plex and the
    shared Jellyfin/experimental-Emby implementation discover video playlists,
    retain per-source unavailable groups, preserve exact server order, and
    re-fetch on exact-session sequence advancement. The separate detail surface
    has no edit affordance, and playback writes neither the server playlist nor
    `playlists.json`. Source/unit guards passed 123 Rust tests; every UI,
    sequence, isolation, and no-write leg was red-proven; restored Linux
    real-app E2E passed 21/21. Exact trail:
    `.agents/review/findings/pl-s4.md`.
  - **S5 (Continue Playing) is complete** (`6938c0f`; guard repairs
    `18ae3d4`/`0da06b7`; two independent Grok acceptances at r1). The persisted
    `off` / `on` / `only-tv` policy defaults to `only-tv`; continuation requires
    the exact cleanly-ended session; `on` walks the literal rendered Continue
    Watching list with a per-run no-repeat guard; and `only-tv` walks watched or
    unwatched episodes across seasons while respecting the Specials boundary.
    Restored local gates passed 132 Rust tests and zero npm/RustSec
    vulnerabilities; the exact 13-file VM sync and fresh-build Linux real-app
    E2E passed 24/24. Exact trail: `.agents/review/findings/pl-s5.md`.
    No playlist implementation gate remains. The owner deferred the final live
    Plex smoke to release preparation; Emby remains explicitly experimental.
  - **The design churned hard before settling** (an "Up Next" consumption queue, a
    melded carousel, a 5-second resume countdown — all proposed, all rejected by
    the owner the same day). The decisions entry records what was rejected and why;
    do not resurrect any of it from git history without reading that first.

- **RELEASE (owner asked 2026-07-14): a second Plex server is NOT needed.** The
  dangerous half of the rebind path IS guarded — `src-tauri/src/source/plex.rs`
  spins up TWO mock Plex servers (machine-A / machine-B), both serving a section
  "2" with DIFFERENT libraries behind it, and drives the real rebind scenarios.
  What has no end-to-end guard is the FRONTEND `sameSection` comparison, because
  the E2E mock is Jellyfin. **A rebind cannot happen at all on a single-server
  account** (it needs 2+ Plex servers on one account AND the saved one becoming
  unidentifiable), so it is inert for this owner. Ship without it and keep it
  recorded. Closing it would need a TLS-capable mock Plex in the harness (the app
  only restores an `https` Plex server, so it needs a trusted cert) or a second
  real instance.

- **AUTOCROP-RESUME: OWNER-CONFIRMED 2026-07-15** (fix
  `c2962a8` on 0.1.43; plan `.agents/plans/autocrop-resume.md`, loop closed
  accepted r3). Root cause probe-CONFIRMED: the stock script's positional
  auto_delay makes resumed plays detect immediately at file-loaded, before hwdec
  engages, so its hwdec guard misfires and cropdetect gathers nothing (fresh plays
  only worked because the delay deferred detection past hwdec init). Fix per owner
  fork ruling: stock `autocrop.lua` stays byte-identical upstream; a new
  Vela-owned `vela-autocrop.lua` shim owns the auto trigger. Guards: mac probe
  red→green recorded in the plan; the `autocrop` E2E (sed-red proven).
  The owner playtested the shipped behavior and confirmed autocrop on 2026-07-15.

- **SHOW-SORT + PER-LIBRARY PERSISTENCE: OWNER-CONFIRMED 2026-07-15**
  (`9cd3323` on 0.1.44; plan `.agents/plans/show-last-episode-sort.md`). Show
  libraries get "Last episode added" (Plex `episode.addedAt` LIVE-VERIFIED against
  the owner's server; JF/Emby `DateLastContentAdded`; show-only, excluded from the
  merged view), and every library's sort now persists across restarts
  (`section_sorts` config map). Guards: 3 new unit tests + the `sortpersist`
  restart E2E, all proven red→green. **Owner playtest ask (real Plex):** show
  library → "Last episode added" → a show with a fresh episode tops the list; movie
  libraries don't offer that option; set different sorts on two libraries, restart
  → each reopens on its own sort. The owner confirmed the playtest on 2026-07-15.

- **QUEUED WATCH-EDIT RACE ACCEPTED FOR v1.0 (owner, 2026-07-15):** the
  contested r6 residual race is rare, bounded and self-healing; closing it is not
  a quick fix because it needs persisted per-entry epochs and compare-and-swap
  curation. Ship it as a known potential issue in the v1.0 release notes. Detail:
  `.agents/plans/continue-watching-watch-state.md` Review log r6 + Accepted edges.

- **DRIFT FIXED (owner go, this session):** `src-tauri/src/source/mod.rs:63`
  comment referenced a "listing-cache" that died with the local-source removal
  (2026-07-08). Owner gave the go ("1 go"); comment now reads "recents
  persistence round-trip" only — verified `Deserialize` on `ItemDto` survives
  solely for `recents.rs` config embedding. (`ISSUES.md`'s companion drift was
  fixed earlier, 2026-07-14 pass.)

- **v1.0.0 RELEASE TRACK (owner, 2026-07-10, refined 2026-07-15 — ordered LAST,
  behind the functional
  work above; "queue first, v1 polish goes to the bottom", where "queue" means the
  work queue, not the play queue):** (1) UI embellishments — Slice 1 is complete
  at 0.1.53, Slice 2 at 0.1.55, and Slice 3 at 0.1.56; item 1 is complete
  (`.agents/plans/ui-embellishments.md`; vibrancy CUT — Linux/Wayland first;
  motion SUBTLE, binding); (2) docs polish — complete at `d4d9e95`; (3)
  graphics + screenshots for socials, deferred until the newly prioritized open
  issue queue is empty; (4) harden the GitHub release
  build so missing platform artifacts fail closed, add the required Arch package
  for AUR publication, and ship unsigned binaries because the owner has no Apple
  or Windows developer credentials; (5) run the deferred final Plex/error smoke
  before publishing. Emby ships explicitly experimental until a real-server
  integration test exists.

- Migration-time (not now): plan the one-shot Plex→JF/Emby watch-state copy
  (provider-id matching; both APIs already integrated).

## Blockers

- **None for `chr-1`:** the branch is merged to `main` (`5248fe6`,
  owner-approved 2026-07-19). The final real-Plex smoke remains deferred to
  release preparation by owner choice.

## Verification

- Canonical commands live in `.agents/repo-guidance.md` (Verification). Do not
  copy them here — that file owns them.
- `npm run e2e` is Linux-only; the venue and its quirks are in
  `.agents/machines.md`. The scenario count is owned by `tests/e2e/scenarios/`,
  not by this file.

## Active Sources

- `AGENTS.md` + `.agents/repo-guidance.md` (governance refreshed 2026-07-08,
  toolkit `6f08a67`; verification commands and Guard discipline live there)
- `.agents/decisions.md`
- `.agents/machines.md` (host-specific facts; the E2E venue)
- `.agents/push-policy.md` (always ask — including the `vm` remote)
- `.agents/plans/failed-watch-edit-recovery.md` (IMPLEMENTED — Grok accepted r1;
  owner playtest confirmed)
- `.agents/plans/edit-error-auto-dismiss.md` (IMPLEMENTED — Grok accepted r1;
  owner confirmed 0.1.50)
- `.agents/plans/dependency-lts-refresh.md` (COMPLETE — final canonical,
  native-package, live-server, and external integration review green)
- `.agents/plans/playlists.md` (COMPLETE — five slices; externally accepted)
- `.agents/plans/clean-eof-carousel.md` (IMPLEMENTED — 0.1.52 universal DMG
  built; owner real-Plex completion playtest outstanding)
- `.agents/plans/per-surface-status.md` (COMPLETE — owner playtest outstanding)
- `.agents/plans/autocrop-resume.md` (IMPLEMENTED — owner-confirmed)
- `.agents/plans/show-last-episode-sort.md` (LANDED — owner-confirmed)
- `.agents/plans/ui-embellishments.md` (COMPLETE at 0.1.56 — v1.0.0 item 1)
- `.agents/plans/multi-plex.md` (CORE COMPLETE — playback policy moved to the
  plan below)
- `.agents/plans/playback-source-policy.md` (COMPLETE — one Fable max-effort
  openreview clean over `ad27cf0..13405dc`; COMPLETE at version 0.1.62: Slice 1
  at
  `c7ac901`/`cadbbb0`, Slice 2 at `7d9a00e`, Slice 3 at
  `7720d2a`/`a749974`/`b4b702b`, Slice 4 at `3391986`, and final integration
  coverage at `62133b3`/`c07abc8`; macOS/Linux/Windows native validation and
  packages complete, with further Fable review withdrawn by the owner)
- `.agents/plans/library-refresh-scan.md` (COMPLETE + owner-playtested; the
  r1-r24 two-reviewer log is its `## Code review log` — the standing rules it
  produced now live in decisions.md and repo-guidance.md)
- `.agents/plans/continue-watching-watch-state.md` (COMPLETE — owner accepted
  the r6 residual race for v1.0 release-note disclosure)
- `.agents/plans/drop-local-sources.md`, `.agents/plans/item-detail-view.md`,
  `.agents/plans/person-browse.md` (all COMPLETE — design records)
- `.agents/review/index.md` (durable review trails)
- `docs/history/state-archive.md` (rotated state entries)
- `README.md`, `ISSUES.md`

## Unrecorded Repo Memory

- None known.
