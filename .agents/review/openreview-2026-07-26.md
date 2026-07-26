# openreview 2026-07-26 — server transcoding, whole change

`Reviewer: codex / codex-cli 0.145.0 defaults (no model or effort specified —
`.agents/decisions.md` 2026-07-26) / — / —`

Range: `72e0f48f6c7ddeda603cea253951c4a93932e709..a8a9fec11be9364a2ee09ad2bcdad0c3ccd442ae`
(43 commits: server-transcoding slices 1-5, the tr-3..tr-9 fixes, and the marker
slice-4 work that landed alongside).

Envelope validated against the `openreview` verdict contract: `verdict:
findings`, `reviewed_sha` and `base_sha` both match the dispatched pins,
`capability_ok: true`, findings non-empty. Unprimed — the reviewer was given the
neutral question, the SHAs, isolation and side-effect bounds, and nothing about
the plan, the finding records, or any prior conclusion.

**Verdict: 7 findings — 1 HIGH, 6 MEDIUM.** All seven carry a `file:line`
citation and a predicted observable failure, so all seven pass intake triage as
ADMITTED. None is style-only; none duplicates another.

## or-1 (HIGH) — an Automatic replacement stops watching itself

`commands.rs` spawns the health sampler only when the resolved quality is
`automatic`, but `apply_step_down` relaunches with `quality_override:
Some(next.id)` — a concrete tier. The replacement play therefore resolves to a
tier, not to `automatic`, so no sampler is spawned for it.

**Consequence: Automatic can only ever take ONE step.** The second step, the
cap of 2, the cooldown-on-resume and the `steps_taken` threading are all
unreachable code. Playback that still cannot keep up after the first step stays
broken.

This is the same defect class this session already recorded twice in
`.agents/repo-guidance.md` — guards that drive the unit and never prove the
wiring. `AutomaticDetector::resuming(FILM, 1)` proves the detector honours a
carried count; the static guard proves the spawn condition exists. Neither can
see that the condition is false for every replacement.

## or-2 (MEDIUM) — a step-down discards playlist and continuation context

The replacement launch passes `playlist: None` and `run_kind: None`, so a
successful relaunch clears the cursor and run state. A step-down during a
playlist, a server playlist, or a TV continuation relaunches the current video
and then fails to advance when it ends.

## or-3 (MEDIUM) — a step-down cannot replace an Ask Every Time play

The replacement passes `explicit_source_id: None`, so under Ask Every Time with
duplicate copies the relaunch re-enters source selection and returns an
unobservable `SourceChoiceRequired`. The running play is never replaced and no
further verdict is emitted.

## or-4 (MEDIUM) — quiet-period samples are retained and fire at the boundary

`automatic.rs` pushes each sample into the window BEFORE the warm-up/cooldown
check, so samples that were supposedly ignored stay in the detection windows. A
startup, seek, or replacement burst can therefore trigger a step-down at the
exact moment the quiet period expires, even if playback has already recovered.

## or-5 (MEDIUM) — the first Automatic step can ask for MORE bandwidth

The ladder is filtered by resolution only. Stepping from Original on a 1080p
source selects the 20 Mbps tier even when the source itself is 10 Mbps, so a
starving link is "stepped down" to a higher bitrate target. Compounded by or-1,
it never then reaches a genuinely lower tier.

## Outcomes

| id | severity | status |
|------|----------|--------|
| or-1 | HIGH | FIXED `53dab6e` + `ef95144` (1.0.39-40) |
| or-2 | MEDIUM | FIXED `3f070f1` + `5dac833` (1.0.44, 1.0.46) |
| or-3 | MEDIUM | FIXED `9933b28` (1.0.45) |
| or-4 | MEDIUM | FIXED `a277f8a` (1.0.41) |
| or-5 | MEDIUM | FIXED `827ad8b` + `774e21d` (1.0.42-43) |
| or-6 | MEDIUM | FIXED `6d0f84e` + `ba48c17` (1.0.53-54) |
| or-7 | MEDIUM | FIXED `9485fe4` (1.0.47) |

Every fix was red-proven by injecting its regression separately. Three guards
written during this pass were VACUOUS on their first attempt and were caught
only by injection — the Automatic-mode gate's one-off clause (unguardable, so
the clause was deleted rather than kept), the inherited playlist cursor (matched
the computation without proving it reached the launch), and the `or-7` flag
mapping (re-derived the rule in the test instead of calling the production
helper). That is the same rate this session has seen throughout.

## or-6 (MEDIUM) — the quality menu is not pinned to the version that will play

`+page.svelte` sends `versionId: null` on every `quality_options` request. For an
item with several media versions the menu can describe one version while
playback policy selects another — omitting valid tiers, or offering one that
then degrades to Original.

**FIXED 2026-07-26 by option (1), after the objection to it turned out to be
wrong.** The deferral below claimed `select_playback_version` mutates state and
can enqueue a source-choice prompt. It does neither: it probes, ranks, and
returns, and its Ask-mode `Choice` is a RETURNED VALUE that the caller decides
what to do with. Reading the function instead of reasoning about it is what
settled this — the same lesson `.agents/repo-guidance.md` already records about
recording things as unguardable.

`quality_options` now takes the item and resolves the version the play path
would choose, using it only when the policy landed on the very copy the menu row
describes; anything ambiguous falls back to the source's default, which is the
old behaviour. Four regressions injected separately; one guard was vacuous
(matching the copy-comparison alone let `true || (...)` pass) and was tightened
to pin the whole condition.

The original deferral, kept because its option analysis is still the record of
what was considered:

**DEFERRED 2026-07-26 — this one needs an owner decision, not a patch.** The
finding is real and reproduced by reading the code: with `versionId: null` the
source falls back to its default media entry, while the play path uses
`selection.version_id` from `select_playback_version`. Slice 4's menu already
pins the COPY (each `Play Version` row asks about its own backing), so the gap
is narrower than it looks: it is multiple media VERSIONS inside one copy, which
is Plex `Media[]`.

Three ways to close it, and they are not equivalent:

1. **Have `quality_options` run the same selection the play will.** Strongest —
   menu and play agree by construction. But `select_playback_version` takes
   `state` and MUTATES it: in Ask Every Time it enqueues a pending
   source-choice. A menu opening must not enqueue a choice or pop a dialog, so
   this needs a side-effect-free variant of the selection, which does not exist
   today.
2. **Enumerate versions in the menu**, one row per version under each copy, and
   pass the chosen `version_id` through both the options request and the play.
   Honest and explicit, but it deepens an already three-level menu and needs a
   `version_id` parameter on the play command.
3. **Accept the mismatch** and have the menu say it describes the default
   version.

(1) is the right answer if a pure selection can be factored out; (2) is a
product change; (3) is a documentation change. Picking between them is a design
call with visible consequences, so it is left to the owner rather than decided
here. Nothing about it blocks the other six fixes.

## or-7 (MEDIUM) — the Plex capability probe does not test the delivery it starts

The decision request permits `directStream=1` while the transcode URL forces
`directStream=0`. A server that will direct-stream but not encode can return a
successful decision, so Vela offers the tier and then hands mpv an HLS URL the
server refuses.

## Standing alongside

`tr-10` (HIGH, `.agents/review/findings/tr-10.md`) was raised by the author
while this review was running and was NOT disclosed to the reviewer. Codex did
not surface it independently, so it remains author-found and unconfirmed by an
external pass.
