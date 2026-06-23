# Agent Decisions

Record durable repo decisions here. Do not use this as a chat log. Each entry
should make sense without conversation history and should name superseded
guidance when relevant.

## Decisions

### 2026-05-23 - Use external mpv playback for HDR

Status: Active

Decision:
Vela plays video through the system `mpv` binary in its own window. The webview
is used for browsing and controls, not embedded video playback.

Reason:
External mpv is the reliable path for HDR passthrough across Linux, macOS, and
Windows. The app records resume/progress over mpv IPC where the source supports
it.

Supersedes:
None.

### 2026-05-23 - Accept token-bearing media URLs as local-only exposure

Status: Active

Decision:
Plex, Jellyfin, and Emby poster/stream URLs may carry access tokens locally, and
SMB credentials may briefly appear in OS mount process arguments on platforms
that require it. Do not add new logs or error messages that expose those values.

Reason:
The current threat model accepts these as local-only exposure on the user's own
machine. Backend-only Plex calls use header auth where practical, but there is
no token proxy.

Supersedes:
The open policy question in `.review/deduped_action_list.md`.

### 2026-05-23 - Keep local media roots narrow

Status: Active

Decision:
Local folders and mounted remote folders must be validated as specific media
roots before the asset protocol or local source uses them. Filesystem roots and
the user's home directory are rejected, and listing/search/playback paths must
stay inside configured roots after canonicalization.

Reason:
The app intentionally serves local media artwork and files, but an overly broad
asset scope would increase the blast radius of a webview compromise or stale
config.

Supersedes:
The local asset scope concerns in `ISSUES.md` and `.review/gpt_review.md`.

### 2026-05-23 - Keep Linux SMB user-space only by default

Status: Active

Decision:
On Linux, Vela resolves readable GVfs/KIO-FUSE SMB mounts created by the user's
desktop session and does not request root or invoke privileged CIFS mounting by
default. SSH/SFTP folders use `sshfs` with OpenSSH keys, agent, and config; Vela
does not store SSH passwords.

Reason:
Remote mounts should not require privilege escalation from the app. The local
source can browse a user-space mount once the OS or desktop session exposes one.

Supersedes:
None.

### 2026-06-10 - Standard agent guidance is canonical

Status: Active

Decision:
`AGENTS.md`, `.agents/state.md`, `.agents/decisions.md`, and
`.agents/repo-map.json` are the canonical agent guidance and memory files. The
`.review/` files are retained as historical review evidence, not updated as
current state.

Reason:
The repo needs one discoverable current-state entry point and one decision log
so future work does not reconstruct state from historical review documents.

Supersedes:
Current-state use of `.review/deduped_action_list.md` and `.review/gpt_review.md`.

### 2026-06-20 - Bump version on code change, not on build

Status: Active

Decision:
Vela's version is bumped (via `scripts/bump.sh`) as part of a code change, not
at build time. `scripts/build.sh` builds the host platform's installable bundle
and must not change the version.

Reason:
A build is only meaningfully unique when the source is. Tying the version to
builds produces version churn with no code difference; tying it to code edits
makes each version correspond to a real change. The version is shown in the
window footer and the bundle filename.

Supersedes:
The prior "bump on every build" rule previously stated in the `scripts/bump.sh`
header comment.
