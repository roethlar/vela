#!/usr/bin/env python3
"""Claude Code PreToolUse hook: deny file-edit tools access to governance
artifacts installed by the toolkit's refresh.

Installed governance is toolkit-owned (owner ruling 2026-07-16): no in-repo
edit to an installed copy is legitimate; changes route through the toolkit
and propagate by refresh. This hook is defense in depth for the one harness
with verified blocking hooks - the primary layers are the AGENTS.md
invariant and refresh's converge-to-shipped restore. Exit 2 blocks the tool
call and surfaces the message to the model; every failure mode inside the
hook exits 0 (fail-open) so a broken hook can never break editing.

The PROTECTED set is the shipped target list from tools/shipped-set.json,
kept in lockstep by a toolkit test - edit it only via the toolkit.
"""
import json
import os
import sys

PROTECTED = frozenset({
    "AGENTS.md",
    "CLAUDE.md",
    "GEMINI.md",
    ".claude/commands/catchup.md",
    ".claude/commands/decision.md",
    ".claude/commands/drift.md",
    ".claude/commands/handoff.md",
    ".claude/commands/plan.md",
    ".claude/commands/playbook.md",
    ".claude/commands/codereview.md",
    ".claude/commands/openreview.md",
    ".claude/commands/review.md",
    ".claude/commands/harness-update.md",
    ".claude/commands/git.md",
    ".claude/commands/update-governance.md",
    ".claude/settings.json",
    ".claude/hooks/protect-governance.py",
    ".agents/playbooks/codereview.md",
    ".agents/playbooks/openreview.md",
    ".agents/playbooks/harness-update.md",
    ".agents/playbooks/git.md",
    ".agents/skills/catchup/SKILL.md",
    ".agents/skills/handoff/SKILL.md",
    ".agents/skills/drift/SKILL.md",
    ".agents/skills/decision/SKILL.md",
    ".agents/skills/plan/SKILL.md",
    ".agents/skills/playbook/SKILL.md",
    ".agents/skills/update-governance/SKILL.md",
    ".agents/skills/codereview/SKILL.md",
    ".agents/skills/openreview/SKILL.md",
    ".agents/skills/review/SKILL.md",
    ".agents/skills/harness-update/SKILL.md",
    ".agents/skills/git/SKILL.md",
})


def main() -> int:
    try:
        payload = json.load(sys.stdin)
        tool_input = payload.get("tool_input") or {}
        raw = tool_input.get("file_path") or tool_input.get("notebook_path")
        if not raw:
            return 0
        root = os.environ.get("CLAUDE_PROJECT_DIR") or os.getcwd()
        # realpath both sides: symlinked tmp roots (macOS /var -> /private/var)
        # and symlinks pointed AT protected files must compare equal
        real = os.path.realpath(raw)
        rel = os.path.relpath(real, os.path.realpath(root))
        rel = rel.replace(os.sep, "/")
        hit = rel in PROTECTED
        if not hit and os.path.exists(real):
            # Case-insensitive filesystems (macOS, Windows): "agents.md"
            # names the existing AGENTS.md but misses the string lookup -
            # compare identity against each protected file that exists.
            for p in PROTECTED:
                cand = os.path.join(root, p)
                if os.path.exists(cand) and os.path.samefile(real, cand):
                    hit = True
                    rel = p
                    break
        if hit:
            sys.stderr.write(
                "BLOCKED: {} was installed by governance refresh and is "
                "toolkit-owned. Editing installed copies is out of bounds; "
                "any local change is drift and is restored on the next "
                "refresh. Route the change to the owner for the toolkit "
                "instead.\n".format(rel))
            return 2
    except Exception:
        return 0
    return 0


if __name__ == "__main__":
    sys.exit(main())
