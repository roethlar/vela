<!-- toolkit-owned; edits are drift — see AGENTS.md -->

# Playbook: delegated git operations (`git`)

Read the repo's AGENTS.md and push policy. Explain what happens to the
owner's work in plain English; do not ask them to choose git mechanics.

## Delegation contract

- Gather and report facts before mutations. Preserve unrelated work.
- Fetching, pruning stale tracking refs, and fast-forwarding onto
  already-published work are authorized by these operations.
- Merges, branch deletion, server-repository creation, and history rewrites
  need explicit authority for the named action. Reuse authority already
  given; an implementation go alone does not authorize merge or deletion.
- Do not move refs over uncommitted work. Stop and identify it.
- Never use force or a rewrite to recover from a refusal. An explicitly
  requested rewrite is governed by AGENTS.md Git Safety; this playbook
  grants none.
- Repo-specific rules apply, including expected mirror lag. Machine facts
  go to `.agents/machines.md`, keyed by machine and dated.

## Branch setup and closeout

Here, **main** means the repository's integration branch, including an
existing name such as `master`. Resolve it from repo guidance and verify
against the canonical remote's HEAD; ask if they disagree or it is
ambiguous. Do not rename a branch to satisfy this vocabulary.

Repo policy determines whether implementation uses main or a work branch.
When using a work branch, record its name and base commit with the task.
Direct-to-main work closes after verification and required records/pushes;
the branch closeout below applies when a work branch exists.

Branch work proceeds through **in progress → verified → awaiting merge →
awaiting deletion → complete**. A review verdict or scoped operator can
finish while the underlying work remains pending. Keep its state entry,
plan and tracker open until closeout; update records as each step lands.

1. Verify the work and finish its records on the work branch. Record the
   reviewed/tested head and the intended main. Present any missing merge
   or deletion authorization as a concrete proposal.
2. With merge authority, refresh main, inspect intervening changes, and
   integrate the work without rewriting existing commits. Resolve conflicts
   only within authorized scope; rerun affected verification after changes.
   Push according to the repo's push policy.
3. Prove the intended content is present in current main with `git diff`,
   accounting for later changes. Ancestry and patch-equivalence alone do
   not prove the result survived. If main has a canonical remote, verify
   the integration reached it before deleting published work branches.
4. With deletion authority, remove the work branch locally and from every
   remote where it exists. Check each remote tip still matches the verified
   scope before deletion; a changed tip needs reassessment. Remove only
   disposable, clean worktrees belonging to this task, under the same
   explicit cleanup authority.
5. Verify local and remote branch absence. An unreachable remote leaves
   deletion unverified and work pending. Only then mark work complete and
   close its tracker/plan. Commit the completion receipt on main under the
   existing bookkeeping authority; it does not start another work branch.

No missing approval, failed push, or unfinished cleanup becomes “done.”

## Remote classification (`local|remote|all`)

Public forge hosts (`github.com`, `gitlab.com`, `bitbucket.org`) are
**remote**; other hosts are **local**; **all** includes both. Ask if the
requested class has no configured remote or a URL is unclassifiable.
Scope words are literal: `git push remote`, never `push remote(all)`.

## `git push [local|remote|all]`

Default scope: all. Push the current branch and its tags to the named
remotes, creating that branch there if absent. The request authorizes the
push; do not ask again. Push policy governs agent-initiated pushes only.
State what goes where, report refusals without force, and mention other
local branches with unpushed work.

## `git reconcile [local|remote|all]`

Fetch every remote in scope. For each remote/branch pair:

- In sync: report it.
- Behind: fast-forward onto published work and report what arrived.
- Ahead: push if authorized; otherwise propose it.
- Diverged: describe the work on each side and propose a plain merge.
  Execute only with merge authority.

Report declared mirror lag as expected; do not “repair” it.

## `git add-remote <server>`

Resolve the server from the request, configured remotes, or dated machine
facts. If a server repository must be created, propose its exact name and
visibility (private by default), then create it only on a go. Add the
remote and verify by fetching. Use existing forge credentials; if
unauthenticated, explain the required login without collecting secrets.

## `git branch-cleanup`

Inventory local branches, remote branches, and worktrees; prune stale
tracking refs. Exclude main and other retained integration branches from
deletion candidates.

Classify each candidate by the content proof in Branch setup and closeout.
Present verified branches for deletion as one named batch; an existing
approval covering that batch stands. Present each work-carrying branch
separately: integrate it, keep it pending, or explicitly discard its work.
Deletion that discards work needs its own approval and is cancellation,
never completion.

Follow closeout for approved candidates. Report branches remaining and why;
a local-only deletion is not completion when a published copy remains.
