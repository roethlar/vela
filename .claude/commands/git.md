<!-- Installed by governance refresh; do not edit. Any change here is drift and is restored on the next refresh. Route changes through the toolkit owner. -->

Run the `git` playbook operator: read `.agents/playbooks/git.md` and follow
it to perform the requested git operation on the owner's behalf (e.g.
`/git push local`, `/git reconcile all`, `/git add-remote gitea`,
`/git branch-cleanup`). The owner does not operate git directly: explain
state in plain English and ask before anything irreversible, per the
playbook's delegation contract. If the playbook does not exist in this repo,
say so rather than guessing. The playbook is the authoritative definition;
this file is only a pointer.
