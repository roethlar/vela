<!-- push-policy: always -->
Always push: after a commit lands, push the branch without asking.

Owner ruling 2026-11-12, verbatim: "change push policy to always."
Supersedes the earlier "Always ask before pushing."

- Scope (agent's reading, not the owner's words): every configured remote,
  so `origin` and `github` stay in step.
- This covers plain pushes only. Force-pushes and any other history rewrite
  still need their own explicit go (`AGENTS.md`, Git Safety), and a push a
  remote rejects is reported, never forced.
