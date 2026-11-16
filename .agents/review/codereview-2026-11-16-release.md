# Codereview 2026-11-16 — the final round before the release of Vela 1.0.65

Outcome: four codereview codex hunts over what landed since the last
reviews (`7b17f63..c093289`, the release-engineering hunt to `2ffbffe`),
one per slice: **findings** in each, thirteen candidates in all, thirteen
admitted (8 MEDIUM, 5 LOW), none declined. A fifth hunt, over the two
commits that landed after that range (the embedded player's stop and the
test repair that showed its fault), was dispatched afterwards; its result
is added below when it returns.

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
| fr-1  | MEDIUM   | Changing one Settings control can save over another setting that is stored with it | `[ ]` | — | codex/default/standard |
| fr-2  | MEDIUM   | A Settings save that is refused after the dialog has closed is never shown | `[ ]` | — | codex/default/standard |
| fr-3  | MEDIUM   | The certificate checkbox can become usable while showing a value that was never loaded | `[ ]` | — | codex/default/standard |
| fr-4  | LOW      | The Settings tests pass when a choice saves its previous value | `[ ]` | — | codex/default/standard |
| fr-5  | LOW      | Settings and the guide say a change of player waits for a restart; going back to mpv's own window does not | `[ ]` | — | codex/default/standard |
| fr-6  | MEDIUM   | A right-click, or a track chosen, while a wheel command is unanswered is dropped with "The player is busy" | `[ ]` | — | codex/default/standard |
| fr-7  | MEDIUM   | A long wheel burst adds up to a seek the player refuses, and all of it is lost | `[ ]` | — | codex/default/standard |
| fr-8  | LOW      | The volume can drift by one point near 0 and near 100 | `[ ]` | — | codex/default/standard |
| fr-9  | LOW      | The view's tests pass with several of its rules removed | `[ ]` | — | codex/default/standard |
| fr-10 | MEDIUM   | A Jellyfin start report whose answer was lost is sent again, and the server counts the play twice | `[ ]` | — | codex/default/standard |
| fr-11 | MEDIUM   | A report's failure is logged with whatever the server put in its status line | `[ ]` | — | codex/default/standard |
| fr-12 | MEDIUM   | CI compiles the macOS app for one of the two architectures the release builds | `[ ]` | — | codex/default/standard |
| fr-13 | LOW      | The guide says any value of `WEBKIT_DISABLE_DMABUF_RENDERER` rules out the embedded player; `0` and empty do not | `[ ]` | — | codex/default/standard |

## Notes

- The repairs were handed to codex as four coding tasks, one per slice
  (worktrees `cx-rvsettings`, `cx-rvview`, `cx-rvfixes`, `cx-rvrelease`
  under `~/dev/vela-agent-keep/wt/` on `continuum`), each told to confirm
  a finding before repairing it, to keep the findings apart so that each
  lands as a commit of its own, and to prove each test red without its
  repair. Under the owner's decision of 2026-11-14 a repair codex wrote is
  closed by the working agent's review (the diff read, the checks and the
  end-to-end runs codex's sandbox cannot make, the tests red-proved), not
  by a second codex verdict: codex cannot be the reviewer of code it wrote.
