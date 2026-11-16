# Codereview 2026-11-16 — the final round before the release of Vela 1.0.65

Outcome: five codereview codex hunts. Four, one per slice, over what landed
since the last reviews (`7b17f63..c093289`, the release-engineering hunt to
`2ffbffe`): **findings** in each, thirteen candidates, thirteen admitted
(nine MEDIUM, four LOW), none declined. A fifth, over the two commits that
landed after that range (the embedded player's stop, `3e0a485`, and the
test repair that showed its fault, `c618d31`): **findings**, one MEDIUM,
admitted. All fourteen are repaired and closed (2026-11-16): codex wrote
each repair as a coding task and the working agent's review closed it.

## Dispatch record

- Owner's dispatch word, verbatim (2026-11-16): "okay, you have permission
  to update whatever you need to to get the release ready. then do a final
  round of reviews with codex, then come to me with actions you cannot do."
  This is the generation half: a defect hunt over a pinned landed range.
- The range holds 38 commits and four kinds of work, so it went to four
  hunts, each given one kind as its mandate, told that the others are
  reviewed separately, and told to report a defect where its slice meets
  another.
- Reviewer: codex / host default / host default / standard
- Harness: codex-cli 0.167.2, CLI transport, headless one-shot, stdin
  closed: `codex exec --sandbox workspace-write --json --output-schema
  <schema> -o <outfile> <prompt>`. Called plainly, with no model and no
  effort argument, per `.agents/machines.md`; the host's codex config is
  what a plain call resolves to on `continuum`.
- Each hunt ran in a detached `git worktree` of this clone of its own
  (`~/dev/vela-agent-keep/wt/rv-final-<slice>`), checked out at the head
  SHA and declared disposable in the prompt; two hunts at a time.
- Tier: standard. T1 evaluated against the range before dispatch: none of
  the 60 changed paths matches a sensitive-path glob; the repo has no
  `.agents/review/sensitive-paths` override.
- **Settings: the Video player control, and every setting saved as it is changed**: thread `01a1e4bf-efb2-7890-8f51-6e45339883d1`; the event stream ends in `turn.completed`; dispatched 2026-11-16 14:25 UTC, finished 14:37 UTC; head `c093289`. Verdict `findings`, 5.
- **The embedded player's view: in-page track lists, and the mouse**: thread `01a1e4bf-efb2-77e0-86ad-b709b6847814`; the event stream ends in `turn.completed`; dispatched 2026-11-16 14:25 UTC, finished 14:38 UTC; head `c093289`. Verdict `findings`, 4.
- **Backend and page fixes: the playback report retry, and a browse grid that has left the page**: thread `01a1e4cb-110a-7452-a535-177bcc47f662`; the event stream ends in `turn.completed`; dispatched 2026-11-16 14:37 UTC, finished 14:51 UTC; head `c093289`. Verdict `findings`, 2.
- **Release engineering: the workflows, the packages, the documents**: thread `01a1e4cb-5775-72b1-8d2e-37f2aecb9e6f`; the event stream ends in `turn.completed`; dispatched 2026-11-16 14:38 UTC, finished 14:49 UTC; head `2ffbffe`. Verdict `findings`, 2.
- The release-engineering hunt was first queued at head `c093289` with the
  other three. Before its turn came the signing script changed again
  (`2ffbffe`), so the orchestrator stopped the queued runner, which was
  still waiting for its lane and had not started codex, and queued the
  hunt again at `2ffbffe` with that change named in its mandate. No
  verdict was discarded.
- The prompts said that the end-to-end suite cannot run in the reviewer's
  sandbox and is to be read, named the commands that can run there, and
  said that what is recorded as known and left (`.agents/state.md`,
  Blockers; the plans' logs) is not a finding unless the record is false.

## Reviewed range

- base: `7b17f63558a9d9c78709b6b717673a155394f95a`
- head: `c0932895e22d475f9a6bf68237a8c6e552dab85e` (Settings, the view, the fixes)
- head: `2ffbffeb9d5261d2ac9ecde6966d99daa32f1be5` (release engineering)

## Acceptance (computed by the orchestrator)

For each of the four: the payload matches the schema; `verdict` is
`findings` and `findings` is non-empty; `reviewed_sha` and `base_sha` equal
the dispatched pins; `capability_ok` is true; the event stream ends in
`turn.completed`. All four verdicts are valid.

## Intake

Every candidate was read against the code or the reviewer's reproduction
and admitted; each record's "What" says on which. None was declined. Two
are faults in what the working agent itself decided or wrote: `fr-10` (the
agent told the coder of e50aa83 that every report is safe to send twice;
Jellyfin's start report is not) and `fr-12` (the CI job the agent added
checks one of the release's two macOS architectures).

| ID    | Severity | Impact (one line) | Status | Commit | Reviewer |
|-------|----------|-------------------|--------|--------|----------|
| fr-1  | MEDIUM   | Changing one Settings control can save over another setting that is stored with it | `[x]` | `72270d8` | codex/default/standard; codex wrote the repair; reviewed by the agent |
| fr-2  | MEDIUM   | A Settings save that is refused after the dialog has closed is never shown | `[x]` | `88aa965` | codex/default/standard; codex wrote the repair; reviewed by the agent |
| fr-3  | MEDIUM   | The certificate checkbox can become usable while showing a value that was never loaded | `[x]` | `00693e5` | codex/default/standard; codex wrote the repair; reviewed by the agent |
| fr-4  | LOW      | The Settings tests pass when a choice saves its previous value | `[x]` | `1f3c4f5` | codex/default/standard; codex wrote the repair; reviewed by the agent |
| fr-5  | LOW      | Settings and the guide say a change of player waits for a restart; going back to mpv's own window does not | `[x]` | `861cc3f` | codex/default/standard; codex wrote the repair; reviewed by the agent |
| fr-6  | MEDIUM   | A right-click, or a track chosen, while a wheel command is unanswered is dropped with "The player is busy" | `[x]` | `e453ad1` | codex/default/standard; codex wrote the repair; reviewed by the agent |
| fr-7  | MEDIUM   | A long wheel burst adds up to a seek the player refuses, and all of it is lost | `[x]` | `0cd99a2` | codex/default/standard; codex wrote the repair; reviewed by the agent |
| fr-8  | LOW      | The volume can drift by one point near 0 and near 100 | `[x]` | `1f35c2b` | codex/default/standard; codex wrote the repair; reviewed by the agent |
| fr-9  | LOW      | The view's tests pass with several of its rules removed | `[x]` | `b7e2caa` | codex/default/standard; codex wrote the repair; reviewed by the agent |
| fr-10 | MEDIUM   | A Jellyfin start report whose answer was lost is sent again, and the server counts the play twice | `[x]` | `935e964` | codex/default/standard; codex wrote the repair; reviewed by the agent |
| fr-11 | MEDIUM   | A report's failure is logged with whatever the server put in its status line | `[x]` | `9d05964` | codex/default/standard; codex wrote the repair; reviewed by the agent |
| fr-12 | MEDIUM   | CI compiles the macOS app for one of the two architectures the release builds | `[x]` | `ceed9be` | codex/default/standard; codex wrote the repair; reviewed by the agent |
| fr-13 | LOW      | The guide says any value of `WEBKIT_DISABLE_DMABUF_RENDERER` rules out the embedded player; `0` and empty do not | `[x]` | `5e48e11` | codex/default/standard; codex wrote the repair; reviewed by the agent |
| fr-14 | MEDIUM   | The end-to-end judge let a play report its stop twice | `[x]` | `9171b2e` | codex/default/standard; codex wrote the repair; reviewed by the agent |
| fr-15 | LOW      | The records still told whoever releases to repair Azure signing, and said a run by hand signs everything | `[x]` | `88092f8` | codex/default/standard; records only, corrected by the agent |
| fr-16 | LOW      | The guard of the page's mount target accepted `data-id="app"` | `[x]` | `1dc3f62` | codex/default/standard; codex wrote the repair; reviewed by the agent |

## The fifth hunt

- Range: base `2ffbffeb9d5261d2ac9ecde6966d99daa32f1be5`, head `c618d317bc61fcd0fca0a7c5b9aa1836d67b8b3d`: two
  commits. `3e0a485` is product code that landed after the four hunts were
  dispatched (codex wrote it; the working agent reviewed it), and the owner
  had asked for a final round over what is to be released, so it was given
  a hunt of its own, with the test repair `c618d31` that showed its fault.
- Reviewer: codex / host default / host default / standard. Same harness
  and call as the four; thread `01a1e4db-8446-7042-a024-560a56a01276`;
  the event stream ends in `turn.completed`; dispatched 2026-11-16 14:55
  UTC, finished 15:06 UTC. T1: no changed path matches a sensitive-path
  glob.
- Acceptance: the payload matches the schema; `reviewed_sha` and `base_sha`
  equal the dispatched pins; `capability_ok` is true. Verdict `findings`,
  one: `fr-14`, in the test repair. Nothing was found in the stop fix
  itself.

## The sixth hunt

- Range: base `67de4aa5b1eb9771b9b9765a9bcbb1e781c87820`, head `fc991a896dbc447284658ecb66454a9fefd78644`: one commit,
  written by the working agent after the round had closed: the owner
  dropped Azure Trusted Signing that afternoon and the release workflow
  stopped signing the Windows installers. Code the agent writes is
  reviewed by codex, so it got a hunt of its own.
- Reviewer: codex / host default / host default / standard. Same harness
  and call as the others; thread `01a1e52b-796f-7362-a464-3e7cb9f48dde`; the event stream ends in
  `turn.completed`; dispatched 2026-11-16 16:23 UTC, finished 16:28 UTC.
  T1: no changed path matches a sensitive-path glob.
- Acceptance: the payload matches the schema; `reviewed_sha` and
  `base_sha` equal the dispatched pins; `capability_ok` is true. Verdict
  `findings`, one, LOW: `fr-15`, about records that still spoke of Azure
  signing as in use. Nothing was found in the workflow change itself.

## The seventh hunt

- Range: base `cce384837dcbb3b47fb3b33132020786ec22e0c0`, head `a13be713cba1bf8b5dc9ef8696dd5f0f33cf3b12`: the whole
  change of the branch `remove-sveltekit` against `main`, which the branch
  had just merged. This is the Review step of
  `.agents/plans/remove-sveltekit.md`, owed since October; the owner told
  the agent to land the removal on 2026-11-16.
- Reviewer: codex / host default / host default / standard. Same harness
  and call as the others, in a worktree with the branch's own
  `node_modules`; thread `01a1e53b-38cb-7c03-9759-e48816e34b53`; the event stream ends in
  `turn.completed`; dispatched 2026-11-16 16:40 UTC, finished 16:49 UTC.
  T1: no changed path matches a sensitive-path glob.
- Acceptance: the payload matches the schema; `reviewed_sha` and
  `base_sha` equal the dispatched pins; `capability_ok` is true. Verdict
  `findings`, one, LOW: `fr-16`, a guard of the page's shell that accepted
  `data-id="app"` as the mount target. Nothing was found in the removal
  itself or in how `main` was merged into it.

## Notes

- The repairs were handed to codex as five coding tasks (worktrees
  `cx-rvsettings`, `cx-rvview`, `cx-rvfixes`, `cx-rvrelease`, `cx-rvstop`
  under `~/dev/vela-agent-keep/wt/` on `continuum`), each told to confirm a
  finding before repairing it, to keep the findings apart, and to prove
  each test red without its repair. Every finding landed as a commit of its
  own; the table has them.
- Under the owner's decision of 2026-11-14 a repair codex wrote is closed
  by the working agent's review, not by a second codex verdict: codex
  cannot be the reviewer of code it wrote. For each finding the agent read
  the diff, ran the checks, and made the tests fail again itself: by
  putting the product files back to what they were before the repair where
  that builds, and by a named mutation where it does not (`fr-4`, `fr-9`,
  `fr-10`). Each record's "Approach" says what was seen. Two amendments
  were the agent's: a paragraph describing the view's command queue was
  taken out of the user guide (`fr-6`), and for `fr-14` the agent ruled a
  simpler direction than the reviewer's (its record says why).
- End to end, on `continuum`, with each task's repairs applied to `main`:
  the view (`fr-6` to `fr-9`): six embedded scenarios, 6 of 6. Settings
  (`fr-1` to `fr-5`): seven scenarios in the external player and six in
  the embedded one, all passed, among them two throwaway checks of the
  agent's that drive the saving hard (40 bursts of 13 changes, each stored
  and shown). `fr-14`: `multiplex` three times in each player, with
  exactly one stopped report every time. `fr-10` and `fr-11` change what
  is sent and logged when a report fails, which no scenario provokes; the
  full suites run at the final commit cover the ordinary path.
- `fr-12` changes CI itself: whether both macOS architectures are checked
  on GitHub's runner is seen in the runs from `ceed9be` on.
