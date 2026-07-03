#!/usr/bin/env python3
"""Pre-edit tripwire for AGENTS.md (governance boundary, layer 2).

Fires from a PreToolUse hook when an agent is about to edit/patch AGENTS.md.
ADVISORY, not a gate: it injects a model-visible reminder via
hookSpecificOutput.additionalContext and always exits 0 so the edit proceeds.
It never blocks (never emits permissionDecision "deny", never exits 2).

One JSON object arrives on stdin. The target path lives in different fields per
harness, so both are checked:
  - Claude Code Edit/Write/MultiEdit: tool_input.file_path
  - Codex apply_patch:               tool_input.command (the patch body text)

Standard library only (the toolkit's baseline; no pip). Same file ships to every
harness that uses a script hook; keep the copies identical.
"""
import json
import sys


REMINDER = (
    "You are about to edit AGENTS.md. AGENTS.md is governance only and must stay "
    "portable: every line should survive being copied unchanged into an unrelated "
    "repo. Repo-specific facts (file paths, the repo's own name, current state, "
    "verification commands) belong in .agents/, not here. AGENTS.md is normally "
    "written only by a gated bootstrap or update run. Before proceeding, confirm "
    "to the user that this edit is part of a bootstrap/update run and that the "
    "content is portable; otherwise the fact belongs in .agents/."
)


def _targets_agents_md(tool_input):
    """True if the pending edit appears to target an AGENTS.md file."""
    path = tool_input.get("file_path") or ""
    if path == "AGENTS.md" or path.endswith("/AGENTS.md"):
        return True
    # Codex apply_patch: the path is inside the patch body, not a field.
    command = tool_input.get("command") or ""
    return "AGENTS.md" in command


def main():
    try:
        data = json.load(sys.stdin)
    except (json.JSONDecodeError, ValueError):
        # Malformed input: do nothing, never block.
        return 0
    tool_input = data.get("tool_input") or {}
    if _targets_agents_md(tool_input):
        json.dump({
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "additionalContext": REMINDER,
            }
        }, sys.stdout)
    return 0


if __name__ == "__main__":
    sys.exit(main())
