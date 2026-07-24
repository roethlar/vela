# Plan: fail-closed settings integrity and preserved recovery

## Status

**Active v1, revision 6 — 2026-07-23.** Approved implementation prerequisite
for `.agents/plans/skip-credits-intros-v2.md`.

The owner approved the core product contract on 2026-07-22: an invalid settings
file never loads through normalization, default substitution, or partial source
restoration; Vela blocks normal use, explains that the file may be damaged or
may have been tampered with, and offers either a private rename plus fresh file
or Exit so the user can repair it manually.

On 2026-07-23 the owner approved separating active server connections and
tokens into a private `connections.json`, with no OS credential vault and no
app-managed encryption. Owner-account file access, credential-safe runtime
handling, and removal of avoidable Plex token URLs/query strings are the
security boundary.

All product choices in **Owner decisions** are settled and recorded in
`.agents/decisions.md`.

The owner resolved the review finding, declined the follow-up external review,
and explicitly activated implementation on 2026-07-23. `.agents/state.md`
names this plan as the active implementation.

On 2026-07-23 the owner expanded recovery to retain three private dated valid
versions independently for settings and connections and to show all available
versions as explicit rollback buttons. Revision 5 adds this approved slice
without weakening exact damaged-file preservation or fresh-file recovery.

Slices 1, 2, and 2A are implemented, verified, committed, and independently
red-proven at versions 1.0.1, 1.0.2, and 1.0.3. Slice 3 is implemented and
canonically verified at version 1.0.4; its post-commit guard proofs and plan
closeout follow its production commit.

No marker-skipping implementation may start until this plan is implemented,
reviewed, verified, and committed, and the marker plan is then activated
separately.

---

## Plan review

`openreview claude` (Claude Code 2.1.218 /
`claude-opus-4-8` at max, owner-selected inline competitive review) over exact
range
`7a4b5b02cb7287559944cab7246d2a4dd0c5c5d2..bf3730a14105465f6f5a7edd6e3fd326acd57132`
returned one schema-valid MEDIUM finding on 2026-07-23. `cir-1` is ADMITTED:
the plan has no safe/honest recovery branch for an already-invalid combined
pre-split config. Detail and proposed guard boundary:
`.agents/review/findings/cir-1.md`.

The owner resolved the product question on 2026-07-23: do not mine or salvage
connections from a damaged old combined config. Treat it as the damaged
settings file, offer **Rename and create new settings** or **Exit**, and require
server reconnection afterward. Revision 4 applies that ruling. The owner
declined a follow-up external review and explicitly activated this repaired
plan on 2026-07-23; no clean follow-up verdict is claimed.

---

## Goal

Give settings and active server connections independent, strict validity and
recovery boundaries:

- `config.json` owns settings, recents, overrides, tombstones, and documented
  compatibility fields;
- `connections.json` owns every active Plex/Jellyfin/Emby connection record,
  including tokens, endpoint identity, and provider-required device/user ids;
- a genuinely absent file is valid first-run state for that file;
- a valid file loads in full;
- a readable but invalid file loads nothing from that file;
- a file Vela cannot safely inspect loads nothing from that file;
- recovery is a deliberate user action, never startup fallback;
- before an ordinary validated write replaces an existing valid file, Vela
  saves that whole prior version in the file's independent private
  three-generation history;
- recovery privately renames the complete invalid file, then installs either
  an explicitly selected validated history version or a validated fresh
  document at the canonical path;
- Exit writes nothing and lets the user repair the original file manually.

An invalid settings file cannot erase, rewrite, or require reauthorization of a
valid post-split connections file. An invalid connections file cannot erase or
reset settings. A damaged pre-split combined config is one invalid settings
file, not a source from which Vela salvages connection rows; after the exact
rename and fresh settings creation, the user reconnects servers. There is no
runtime state in which part of any invalid file is combined with defaults or
partially restored records.

---

## Authority and compatibility contract

The durable owner rulings are
`.agents/decisions.md` **2026-07-22 — Invalid settings fail closed with explicit
preserved recovery**, **2026-07-23 — Unknown active setting names invalidate
the config**, **2026-07-23 — Connections are private file-backed state, not
settings**, **2026-07-23 — Damaged files are renamed, not salvaged**, and
**2026-07-23 — Keep three dated rollback versions for each durable file**. The
legacy local-source rollback contract in
`.agents/repo-guidance.md` remains equally binding.

These are valid compatibility cases, not corruption:

- `config.json` does not exist;
- a documented optional field is missing and therefore takes its documented
  default;
- a pre-split config, pre-multi-Plex config, or retryable partial migration has
  the exact legacy shape supported by the migration;
- inert `local_folders`, `smb_mounts`, and `ssh_mounts` data is present,
  including rollback credentials; those records continue to parse, remain
  unused, and round-trip without loss;
- recents, tombstones, section sorts, or merged overrides refer to a source or
  media item that no longer exists;
- a configured endpoint or `mpv_path` is temporarily unreachable on this
  machine;
- `mpv_extra_args` contains arbitrary user-authored text.

These are invalid:

- malformed JSON or a known field with the wrong JSON type;
- a present constrained setting outside its closed value set;
- a semantically incomplete, unsupported, duplicated, or internally
  inconsistent active media-source row in `connections.json` or in a
  pre-split migration input;
- an invalid in-progress legacy Plex migration record;
- active collection data outside its enforced persistence bounds;
- a per-section sort value outside Vela's sort whitelist;
- an unknown top-level setting field in `config.json` or connection/source
  field in `connections.json`.

Validation is local and deterministic. It never contacts a media server,
probes `mpv`, guesses a source, or changes the file.

---

## Fault classes

Use the same typed result model independently for settings and connections
instead of collapsing failures into `io::Error`, `String`, `Option`, or
`Default`:

```rust
enum DurableLoad<T> {
    Absent(T),
    Valid(T),
}

enum DurableFault {
    Invalid {
        file: DurableFile,
        kind: InvalidDocumentKind,
        layout: DurableLayout,
    },
    Unavailable {
        file: DurableFile,
        kind: ConfigIoKind,
    },
    MigrationBlocked {
        kind: MigrationFailureKind,
    },
}
```

Exact names may follow local Rust conventions, but the distinctions are
required. `DurableLayout` distinguishes a normal post-split file from the
legacy combined pre-split `config.json`; it never authorizes partial parsing or
salvage:

| Result | Meaning | Normal app loads? | Fresh-config action? |
|---|---|---:|---:|
| Absent | No file at that resolved path | Yes, that file's documented empty/default state | No |
| Valid | Parse, compatibility migration, and validation succeed | Yes, all of that file | No |
| Invalid | Readable regular file fails syntax, schema, or semantics | No data from that file | Yes, for that file only |
| Unavailable | Permission, symlink/non-regular file, unsafe metadata, or read I/O failure | No data from that file | No |
| Migration blocked | A valid split/Plex migration cannot finish because one atomic step failed | No active sources | No |

An invalid migration *shape* is `Invalid`; a valid migration whose separate
atomic work fails is `MigrationBlocked`. A settings fault does not change a
valid post-split connections result, and a connections fault does not change a
valid settings result. An invalid legacy combined config has no independently
valid connection result: its connection bytes remain only in the renamed
backup, and recovery proceeds to reconnection. Normal server-backed use still
waits until every required file is ready. Unavailable and migration-blocked
screens provide **Try again**, **Exit**, and safe manual guidance, but cannot
offer reset: Vela has not proved that it can rename the authoritative file
safely.

User-facing errors carry only a stable category and generic explanation. Raw
JSON, auth values, URLs containing tokens, serde value excerpts, and config
contents never enter logs, command errors, events, or the UI.

---

## Two independently validated file boundaries

### Parse and validate

Add side-effect-free validators (or validated newtypes) for `AppConfig` and
`ConnectionsConfig`. Every successful public load and every save/update crosses
the matching boundary. No caller may receive or persist an unvalidated
document.

```rust
#[derive(Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct ConnectionsConfig {
    sources: Vec<SourceConfig>,
}
```

`AppConfig` no longer owns active `sources`, active Plex singleton credentials,
or any other active provider authorization after migration. It retains
read-only, skip-on-save compatibility fields only long enough to recognize and
complete the one-time split migration.

`AppConfig` validation covers:

- `watched_threshold_percent`: missing is its documented default; a present
  value is in `1..=100`;
- closed policies: autocrop (`off`, `manual`, `auto`), Continue Playing
  (`off`, `on`, `only-tv`), playback source
  (`best`, `compatible`, `fastest`, `ask`), resolution override, and HDR
  override accept only their documented values;
- `section_sorts` keys obey the existing setter length/nonblank rules and every
  value belongs to `ALLOWED_SORTS`;
- persisted recents and hidden tombstones obey their canonical module bounds.
  Do not duplicate numeric constants in the validator;
- transient migration fields are absent or have the exact retry-safe
  relationship required by the relevant migration;
- stale source/media references remain valid and are filtered only where the
  product already filters them.

`ConnectionsConfig` validation covers:

- source ids are nonblank and unique; source kind is exactly `plex`,
  `jellyfin`, or `emby`; source name is nonblank;
- each source satisfies the same persisted requirements as its provider
  constructor. Plex requires its token and device/client id and preserves the
  existing endpoint/machine-pin safety rule. Jellyfin/Emby requires one token
  form, user id, device id, and nonblank base URL. Validation does not require
  the server to be reachable;
- provider construction from every validated source succeeds. Change the
  startup constructors from silent `Option` omission to a typed `Result`, and
  share pure validation helpers so the connections validator and constructor
  cannot drift.

Prefer strict serde enums for closed policies, with explicit serde names and
documented missing defaults. If a field must remain a string for IPC or
compatibility, give it a strict parser returning `Result`; do not retain a
normalizer that maps unknown input to a valid choice.

All setting commands parse and reject invalid input before calling the settings
update boundary. `set_mpv_advanced`, `set_continue_playing`,
`set_playback_preferences`, and `set_section_sort` must not clamp or normalize
bad input. After a mutation closure succeeds, validate the complete result
before serialization. A failed mutation leaves the original bytes unchanged.

Connection/link/remove/binding commands update only `connections.json` under a
dedicated process mutex and `connections.lock`. Settings and media-state
commands update only `config.json`. Never rewrite both files for an ordinary
single-file mutation.

### One-time connection split and legacy Plex migration

The first new build may encounter active sources and legacy Plex singleton
credentials inside `config.json`. Migration must not modify an otherwise
invalid file and must never leave the only usable authorization between files.

Use a fixed lock order (`config` then connections, plus the existing playlist
lock only when the legacy Plex re-key needs it) and a retry-safe phase marker:

1. Read the exact pre-migration `config.json` bytes. Parse and validate every
   settings field, every active source, and any legacy Plex or partial migration
   shape before writing anything.
2. Before the first migration write, create and verify one unique private
   byte-for-byte `config.pre-connections-split-<timestamp>-<uuid>.json` backup.
   This is the rollback path for an older Vela that does not understand
   `connections.json`; do not keep a live token mirror in both files.
3. Complete the existing legacy Plex source-id/playlist migration logically
   before extracting connections. Its minted id and retry marker remain stable
   across a crash.
4. Atomically create and validate `connections.json` with the complete source
   set. Never merge it with a pre-existing different file.
5. Atomically rewrite `config.json` without active sources, active Plex
   singleton credentials, or active provider tokens, and mark the split
   complete.
6. Reload and independently validate both files, then build every source.

Crash recovery is deterministic:

- connections written, legacy fields still present, and both sets identical:
  finish stripping the legacy copy;
- split marked complete and connections valid: use only `connections.json`;
- both files contain different active connection data: migration-blocked, do
  not merge, choose, or overwrite;
- backup, config, connections, or playlist I/O fails: retain the phase marker
  and retry without minting a second identity or discarding either complete
  copy.

If the old combined `config.json` is invalid before the split starts, do not
parse its connection block separately, validate selected rows, or create
`connections.json` from any part of it. Classify the whole file as a damaged
pre-split settings file. The blocking UI offers **Rename and create new
settings** or **Exit** and explicitly says that creating fresh settings will
require reconnecting every server. Recovery renames the complete original file,
creates fresh settings, leaves the absent connections file in its valid empty
state, and routes the user to reconnect. Exit performs no write.

After a successful split, the compatibility fields remain recognized only for
pre-split input and are skipped on save. The private pre-split backup may still
contain tokens and receives the same protection as `connections.json`.

Malformed migration state is recoverable invalid state for the file that owns
the malformed record. Operational failure after a valid migration began is
migration-blocked state: retain all retry data and never replace it with
defaults.

### Unknown fields

Owner-approved 2026-07-23: an unknown top-level setting name or active
connection/source field invalidates its whole owning file. Apply
`deny_unknown_fields` (or equivalent explicit key validation) to `AppConfig`,
`ConnectionsConfig`, `SourceConfig`, and other active persisted records. Never
ignore or silently delete an unknown active key.

The legacy local/SMB/SSH field names stay known and valid; their nested rollback
payload remains tolerant enough to preserve the documented old shape. Embedded
media snapshots and provider response DTOs are not settings schemas and retain
their existing forward-compatible tolerance.

---

## Startup and runtime state

Add an app-wide durable-state gate to `AppState`. Startup has exactly two
modes:

```text
load/migrate/validate config.json and connections.json independently
  ├─ both ready → build every persisted source → normal application
  └─ any fault  → build no persisted source → blocking durable-fault UI
```

Source-registry restoration is all-or-nothing. Replace the `if let Some(...)`
startup loop with a function that returns
`Result<SourceRegistry, DurableFault>`; one invalid source row prevents the
registry from being installed.

The fault mode is not represented as an empty source list. The frontend must
be able to distinguish a real new user from a fault in either file before
invoking any settings- or source-dependent command. The gate retains each
file's independent status so recovery never resets the healthy file.

Expose narrowly scoped commands:

```text
get_durable_state_status
retry_durable_state
recover_invalid_file { file: settings | connections }
```

`get_durable_state_status` returns credential-free settings and connections
statuses, each tagged `ready`, `recoverable_invalid`, `unavailable`, or
`migration_blocked`, plus the pre-split/post-split layout needed to render
truthful recovery copy. It includes only safe display text and whether recovery
is allowed for that file.

`retry_durable_state` rereads, migrates, validates, and rebuilds the complete
registry. Success atomically replaces the fault state with ready state; failure
leaves normal commands gated.

Every command that depends on durable state checks the gate. A command may not
bypass it by calling an old static loader. If a later read/update discovers
that a previously valid file became invalid or unavailable, transition only
that file's gate to the corresponding blocking state and notify the frontend
through one credential-free `durable-state-fault` event. The frontend has one
listener that
replaces normal UI with the recovery surface. There is no polling race and no
collection of command-specific fallback behavior.

Keep one backend durable-state facade with separate settings and connections
stores, responsible for:

- loading and validating;
- lock-protected updates;
- classifying faults;
- updating the app-wide gate;
- emitting the safe fault event;
- rebuilding or clearing the registry on a valid state transition.

This is the enforcement boundary that prevents new commands from reintroducing
`unwrap_or_default`, `.ok()`, `if let Ok`, or partial-load behavior.

---

## Recovery transaction

`recover_invalid_file` is accepted only while the selected file's gate holds a
`recoverable_invalid` result for a readable regular file. The button click is
the confirmation; do not add automatic recovery or a second
destructive-looking prompt. The target is a closed backend enum, never an
arbitrary frontend path.

### Valid-version history

Settings and connections have separate bounded histories. Before an ordinary
validated update replaces an existing valid canonical document, preserve its
exact bytes as
`<stem>.valid-<UTC Unix milliseconds>-<sha256>.json`. Verify private
permissions, byte length, the filename-bound SHA-256, and the file's strict
validator before exposing it.
Deduplicate identical bytes and retain only the three newest distinct valid
versions for that stem. History maintenance runs under the same process and
cross-process lock as the owning file. It never snapshots an absent, invalid,
unavailable, partially parsed, or migration-intermediate document.

History discovery accepts only the exact backend-owned filename grammar,
regular private files, and documents that still pass the current strict
validator. The durable gate captures each offered version's opaque SHA-256 id,
timestamp, byte length, and SHA-256 when it captures the invalid current file.
The frontend receives only the opaque id and UTC timestamp. A malformed,
changed, non-private, non-regular, missing, or now-invalid version is omitted
or refused; it never blocks selection onto another version and is never
silently repaired.

`rollback_invalid_file { file, version_id }` shares the fresh recovery
transaction below. Under all locks it proves that the current damaged bytes
still match the gate and that the selected history version still matches its
captured length/hash and validates in full. It then preserves the damaged
current file through the same exact private no-replace rename and installs the
selected version's exact bytes at the canonical path. The recovery marker
records whether the replacement is a strict default or one exact history
version so crash resume cannot change the user's choice.

Under the selected file's process mutex and cross-process lock:

1. Resolve `config.json` or `connections.json` through the same storage boundary
   used by normal persistence. Refuse symlinks and non-regular files.
2. Reopen and reread the current bytes. Re-run parse and validation. If the
   file is now valid, absent, unavailable, or differs from the invalid snapshot
   represented by the gate, abort the stale recovery and return the new status.
3. Choose a unique sibling named
   `<stem>.invalid-<UTC timestamp>-<uuid>.json`. Move the complete source file
   to that path with a platform no-replace rename primitive; never copy selected
   fields, reserialize, or overwrite an existing backup.
4. Apply and verify the same private storage protection on the renamed file:
   owner-only `0600` on Unix and a per-user AppData ACL on Windows with no
   ordinary cross-user read access. Reread it and confirm its length and content
   hash against the bytes from step 2.
5. For fresh-file recovery, serialize the selected document's default
   (`AppConfig::default()` or `ConnectionsConfig::default()`). For rollback,
   reread the selected history version and require the captured length, SHA-256,
   privacy, regular-file, filename, and full-validator checks to still match.
   Install only that selected replacement at the now-absent canonical path
   using a private atomic temporary file and no-replace rename. Parse and
   validate the installed canonical file, and sync the containing directory
   where supported.
6. Reload both independent files. Post-split settings recovery rebuilds from the
   untouched valid connections and does not require reauthorization. A damaged
   legacy combined config has no separate connection file; after fresh settings
   are installed, route to server reconnection without inspecting the renamed
   file. Connections recovery installs an empty valid connections file and
   routes to reconnection while preserving settings, recents, and playlists.
7. Only then mark the gate ready and return the safe renamed filename plus
   whether reconnection is required.

If the no-replace rename fails, the canonical source remains untouched and
recovery fails. If permission hardening, renamed-file verification, or fresh
document installation fails after the rename, keep the complete renamed file,
leave the app blocked, report the safe backup filename when available, and
offer Exit; never delete or partially restore it automatically. The user can
rename it back or repair it manually while Vela is closed.

Post-split recovery changes only the explicitly selected invalid file. Vela
playlists and the other healthy durable file remain byte-identical. Pre-split
combined recovery intentionally renames that whole legacy file and creates no
connection data from it.

The storage layer needs focused helpers for:

- distinguishing absent from unreadable/non-regular;
- bounded exact-byte reads of either durable file;
- unique private no-replace renames;
- validated private fresh-document installation;
- fault-injection tests around every failure point.

If the implementation introduces a file-size limit, an oversized file must
remain recoverable only if Vela can still preserve all of its bytes. Never
truncate a backup.

---

## Blocking frontend

`src/routes/+page.svelte` performs `get_durable_state_status` before `check_mpv`,
`get_sources`, Continue Playing, settings, home, playlist, or navigation
requests. Until status is ready it renders a non-dismissible full-page state,
not the Welcome screen and not the ordinary transient error banner.

Post-split recoverable-invalid settings copy:

> Vela could not safely read your settings. The file may be damaged or may
> have been tampered with. Nothing from it was loaded.

> You can rename the damaged file and create new settings, or exit Vela and
> repair it yourself. Your server connections are stored separately and will
> not be changed.

Controls:

- up to three real HTML buttons, newest first, each labeled with its dated
  valid version and restoring only that exact version;
- primary real HTML button: **Rename and create new settings**;
- secondary real HTML button: **Exit Vela**.

Pre-split recoverable-invalid settings copy explains that this older damaged
file also contains the server connections and Vela will not extract or guess
them. It says that **Rename and create new settings** preserves the whole old
file under a new private name, creates fresh settings, and then requires the
user to reconnect servers. The alternative is **Exit Vela** and repair the file
manually. It must not claim that connections are already separate.

Recoverable-invalid connections copy names the server-connections file,
explains that no connection or token was loaded, and offers each available
dated valid version, **Rename damaged connections and reconnect**, or **Exit
Vela**. A version rollback preserves the whole damaged file and restores that
exact connection document. The fresh action preserves the whole damaged file,
creates an empty valid connections file, and opens the normal server-connection
flow. Neither action implies that settings, recents, or playlists will be
reset.

Disable both controls while their request is in flight. On failure, keep the
blocking screen, show a credential-free error, and re-enable the actions
allowed by the returned status. On successful recovery, show the safe backup
filename. Post-split settings recovery continues with preserved connections;
pre-split combined recovery and connections recovery enter the genuine
no-sources reconnect flow.

Unavailable or migration-blocked copy explains that Vela loaded nothing and
that the file could not safely be renamed or migration could not finish. Show
**Try again**, **Exit Vela**, and manual location/help text, but do not render a
fresh-file action for that file.

The screen must have an alert/status relationship suitable for assistive
technology, move focus to its heading on fault transition, keep normal app
content inert/unrendered, and return focus to the normal root or reconnect flow
after successful recovery. **Exit Vela** closes the application without writing
either durable file.

`loadSourceList()` and boot no longer suppress source/durable-state errors into
`[]`.
Any remaining best-effort catches must be for data that is explicitly
independent of durable-state health.

---

## Existing fallback removal

Audit every `config::load_config`, settings/connection update, constrained-
setting normalizer, and provider restore callsite. Move source mutations,
binding persistence, linking, removal, and startup restoration to the
connections store. At minimum, remove the known conflicts in:

- `src-tauri/src/lib.rs`: startup `unwrap_or_else(AppConfig::default)` and
  source-row skipping;
- `src-tauri/src/playback.rs`: `.ok()` / `unwrap_or_default()` around mpv and
  play settings;
- `src-tauri/src/commands.rs`: default substitution in mpv, Continue Playing,
  playback preferences, merged overrides, local resume, sections/sorts, and
  any `if let Ok` that discards a config fault;
- `src-tauri/src/source/plex.rs` and `source/jellyfin.rs`: persisted source
  constructors returning `None` for invalid rows;
- `src/routes/+page.svelte`: boot/source catches that make a fault look like an
  empty library.

Add a narrow static guard over production Rust sources rejecting config-load
fallback patterns (`unwrap_or_default`, `.ok()`, and ignored `Result`) at the
durable-state boundary, direct `AppConfig.sources` access after migration, and
token serialization into Plex URLs/query parameters. Keep the guard semantic
enough that unrelated Option defaults and noncredential query parameters remain
legal. Red-prove it by inserting each prohibited production pattern.

Update stale comments that currently promise tolerant unknown-value
normalization.

---

## Security and privacy

- `connections.json` deliberately stores provider tokens as plaintext. There
  is no OS credential vault, user passphrase, or app-managed encryption.
  Encryption with a key stored beside the file would not improve the same-user
  threat boundary and is prohibited as security theater.
- The security boundary is the owner OS account. On Unix create the config
  directory as `0700` and `connections.json`, its temporary file, lock, split
  backup, invalid-file backups, and mpv header include as `0600` from their
  first byte. On Windows use the per-user AppData directory and ensure these
  artifacts inherit or receive an ACL with no ordinary `Users`/`Everyone`
  read access (normal current-user, SYSTEM, and administrator control remains).
  A failure to establish the required protection fails closed before token
  bytes are written.
- Keep `config.json` equally private because compatibility fields may include
  inert legacy SMB credentials. A successful connection split removes active
  provider tokens from live `config.json`; the private pre-split backup still
  contains them.
- Persisted token fields must not derive an exposing `Debug`. Use a redacted
  secret wrapper or a custom `Debug` implementation, and keep connection DTOs
  sent to the frontend credential-free.
- Tests use unmistakably synthetic tokens and assert those values do not appear
  in errors, events, logs, frontend DTOs, process arguments, or returned URLs.
- Do not expose a raw serde error containing source text. A safe category may
  include JSON line/column only if tests prove no input fragment is included;
  generic copy is preferred.
- Backups live beside their source file, never in downloads, logs, general temp
  output, or the repository.
- Keep atomic-save and cross-process-lock behavior. Do not weaken either to
  make recovery easier.
- Refuse recovery for a symlink or non-regular file. Do not follow a link and
  copy or replace a target outside Vela's config directory.
- Do not use network reachability as integrity validation: an offline server is
  not damaged settings.
- Change `plex_api.rs` progress and timeline authentication from
  `X-Plex-Token` query parameters to the existing request-header convention.
  Query strings, errors, and mock request logs must not contain the token.
- Replace `PlexLibrary::thumb_url` token-bearing server URLs with a
  credential-free Vela artwork URL. Register an asynchronous app-local protocol
  whose path carries only source id and validated artwork parameters; the Rust
  handler resolves the live source and fetches Plex artwork with
  `X-Plex-Token` as a request header. The protocol response must enforce a
  bounded image body, accepted image MIME types, no redirects to an untrusted
  origin, and credential-free errors. Frontend `<img>` elements never receive a
  Plex token.
- Preserve Plex playback's owner-only mpv header include; never regress to a
  token-bearing media URL or argv. Remove the include after the child has
  consumed it where platform semantics are proven safe, otherwise at child
  exit, and always before a later launch writes a replacement.
- This design protects against other local accounts, accidental file sharing,
  settings recovery, logs, URLs, and process inspection. It does not claim to
  protect tokens from malware already executing as the same OS user. Without a
  vault or passphrase, no lightweight app-only mechanism can honestly provide
  that boundary.

---

## Verification

Every new guard is independently red-proven: land the behavior, inject one
specific regression, prove the intended test fails for the intended reason,
restore from committed bytes, and rerun green.

### Rust unit/integration matrix

- each absent file returns its documented default/empty state without creating
  a file;
- valid minimal and populated settings/connections documents load
  independently;
- every documented missing field uses its documented default;
- malformed JSON, wrong types, every unknown closed value, invalid thresholds,
  invalid section sorts, duplicate/incomplete/unknown sources, bad pinned Plex
  endpoint state, malformed migration state, and bounded-collection overflow
  fail the whole owning file;
- stale media/source references remain valid;
- legacy local/SMB/SSH payloads, including synthetic credentials and old nested
  fields, survive load-update-save;
- a valid combined 1.0.0 file migrates every source into `connections.json`,
  removes active provider tokens from live `config.json`, and first creates an
  exact private pre-split backup;
- each injected split-migration crash resumes deterministically; equal duplicate
  source sets finish cleanup, differing sets block without merge or overwrite,
  and no token/identity is lost or minted twice;
- valid pre- and mid-Plex migrations remain retry-safe across the split; an
  unrelated invalid field prevents every migration write;
- all persisted-source constructors succeed after validation and a forced
  constructor failure prevents all registry installation;
- every setter rejects invalid input and a failed update leaves original bytes
  identical;
- setting commands do not write `connections.json`; link/remove/binding commands
  do not write `config.json`;
- startup faults never install a partial registry or expose default settings;
- runtime invalidation moves the gate to fault and emits one safe event;
- retry moves to ready only after complete validation and registry rebuild;
- post-split settings recovery privately renames the exact invalid file and
  installs a valid fresh config while connections/playlists remain
  byte-identical and every source restores without reauthorization;
- pre-split combined recovery privately renames the complete invalid file,
  installs fresh settings, extracts no connection row, and requires server
  reconnection;
- connections recovery privately renames the complete invalid file and installs
  an empty valid connections file while settings/playlists remain
  byte-identical, then requires server reconnection;
- ordinary settings and connection writes preserve only complete strictly valid
  prior documents, deduplicate identical bytes, retain the three newest
  versions independently, and keep every version private on Unix and Windows;
- invalid-file status exposes at most the three newest still-private,
  still-valid versions as opaque ids plus UTC timestamps, never paths,
  filenames, contents, tokens, or a guessed fallback;
- settings and connections rollback each preserve the exact damaged current
  file, install only the explicitly selected exact validated version, leave the
  other durable file and playlists byte-identical, and restore the resulting
  source registry without partial rows;
- changed, removed, malformed, non-private, non-regular, hash-mismatched, or
  now-invalid versions are refused, and crash resume remains bound to the exact
  selected replacement;
- Exit from every fault screen performs no durable write;
- no-replace rename failures preserve the canonical original; permission,
  verification, and fresh-install failures after rename preserve the renamed
  original and remain blocked; a stale snapshot, symlink, non-regular file,
  unavailable state, and migration-blocked state cannot recover;
- Unix directory/file/lock/temp/backup modes and Windows per-user ACL behavior
  are natively verified;
- connection `Debug`, errors, events, logs, DTOs, argv, Plex artwork URLs, and
  Plex request URLs never contain the synthetic token;
- progress, timeline, normal Plex API, artwork, preflight, and mpv playback
  authentication reaches the mock server through the required header;
- the artwork protocol rejects traversal, unknown source ids, untrusted
  redirects, non-image responses, and oversized bodies without exposing a
  credential.

Guard unknown top-level and active-source fields as invalid, while proving
legacy rollback payloads and non-settings media snapshots remain tolerant.

### Frontend/static tests

- boot requests durable-state status first and issues no normal boot invoke while
  blocked;
- post-split invalid settings, pre-split combined invalid settings, and invalid
  connections render distinct exact copy and the correct real Rename/Reconnect
  and Exit buttons;
- each invalid-file surface renders its available dated versions newest first
  as real disabled-while-busy buttons, invokes rollback with only the owning
  enum and opaque version id, and never renders more than three;
- unavailable/migration-blocked status omits that file's reset button;
- recovery, retry, and Exit have correct disabled, failure, success, and
  no-write states;
- a runtime `durable-state-fault` event replaces normal content and moves focus;
- durable-state/source errors cannot become `[]`, Welcome, or a normal transient
  banner;
- Plex artwork references reaching `<img>` are credential-free;
- the static guard fails for each prohibited fallback, post-migration
  `AppConfig.sources`, token-query, and token-URL mutation.

### Real-app E2E

Add hermetic split-migration, `configrecovery`, and `connectionrecovery`
scenarios using the existing per-scenario throwaway config root.

For settings recovery:

1. Seed a valid `connections.json` with a synthetic Plex connection, malformed
   `config.json`, and a separate Vela playlist.
2. Launch the real app and assert the blocking screen appears before any normal
   app/home/settings content.
3. Click **Rename and create new settings**.
4. Assert the original moved to exactly one private byte-identical sibling,
   `config.json` is a valid serialized default, the playlist is
   byte-identical, `connections.json` is byte-identical, no token is visible,
   and the existing Plex source loads without relinking.
5. Restart and assert the fresh settings and preserved connection load normally
   without showing recovery.

For connection recovery, seed a valid settings file and an invalid source row;
assert the connection-specific warning, **Rename damaged connections and
reconnect**, exact private rename, empty fresh connections file, untouched
settings/playlist bytes, and reconnect flow after recovery. Add a pre-split
damaged combined-file scenario: assert the copy discloses reconnection, the
whole file is renamed byte-for-byte, no connection is extracted, fresh settings
load, and reconnect is required. In each fault state, **Exit Vela** must close
without changing either file. For valid split migration, seed a valid combined
1.0.0 config; assert the exact private pre-split backup, token-free live
settings file, private connections file, restored source, credential-free UI
artwork URL, and header-authenticated mock artwork/playback/progress requests.

Add focused cases for an unknown constrained setting, unknown top-level setting
key, unknown connection key, and malformed JSON so E2E proves this is strict
validation, not only one syntax path. The owner's media library needs no
damaged real config fixture.

Add one settings-history and one connections-history path. Drive four distinct
valid writes so the oldest is pruned, damage the canonical file, assert the
three remaining UTC-dated choices appear newest first, select the middle
version, and prove the damaged current file is preserved while only the owning
canonical file becomes byte-identical to the selected version. Tamper with a
same-length history version and prove it is not offered and cannot be selected.

Run the full canonical cross-side verification from
`.agents/repo-guidance.md`, including the real-app E2E suite. This work changes
startup, commands, persistence, and frontend behavior.

---

## Implementation slices

Each slice is one reviewed, verified commit. Do not start the next slice while
the current slice has uncommitted finished work.

### Slice 1 — private connection split and strict durable-state boundary

- Add private `connections.json`, its independent lock/validator, redacted
  credential representation, and strict constrained settings.
- Implement the exact-backup, fixed-lock-order, retry-safe split/Plex migration.
- Route every source read/write and provider constructor through the connection
  store; remove active provider tokens from live settings.
- Add the two-file fault gate, all-or-nothing registry restoration, retry-only
  blocking UI, storage classification, permission tests, and unit matrices.
- Remove setting-input normalization and every load/default/partial-source
  fallback.
- Bump every release version surface from `1.0.0` to `1.0.1`.

This slice must land as one coherent upgrade: do not persist a split file while
startup still reads sources from `AppConfig`, and do not enable strict failure
while startup can still substitute defaults.

Implementation evidence (2026-07-23):

- `config.json` and `connections.json` now cross independent strict validators;
  source construction is all-or-nothing; ordinary source writes target only
  `connections.json`; and normal commands remain behind the two-file gate.
- The one-time split holds the settings lock before the connections lock,
  creates and verifies an owner-private byte-exact backup, records its byte
  length and SHA-256 for crash retries, reuses the Plex identity marker, refuses
  differing connection sets, and removes active authorization from live
  settings only after the complete connection file exists.
- Unix mode tests cover the `0700` directory and `0600` files/locks/backups. A
  native Windows test on `netwatch-01` proved the protected current-user/SYSTEM/
  Administrators DACL path for directory, JSON, and lock creation.
- Canonical local verification passed: exact Node/npm check, clean `npm ci`,
  zero-vulnerability npm audit, 44 Node tests, Svelte check with zero
  diagnostics, production frontend build, Rust 1.89 and stable checks, clippy
  with warnings denied, 219 Rust tests, and Cargo audit with no vulnerabilities
  (17 existing allowed unmaintained/unsoundness warnings).
- The checksum-matched Linux source passed the complete real-app E2E suite
  31/31. Existing combined-config scenarios exercised the split before source
  removal, restart, playback, and library operations.
- Slice 1 landed as `016a958`. Post-commit red proofs independently caught and
  then restored regressions in unknown-field rejection for both settings and
  sources; invalid-combined no-salvage behavior; strict autocrop and Continue
  Playing values; load/default fallbacks; boot gate ordering; byte-exact backup
  creation plus same-length SHA-256 tampering; stable Plex retry identity;
  differing split-set refusal; Unix directory, JSON, lock, and backup modes;
  native Windows ACL privacy; connection-store routing; secret redaction; the
  provider validation matrix; and genuinely-absent-only defaults. The
  connection-store static guard was initially vacuous when one of two upserts
  was redirected; it now globally rejects settings-store upserts, and that
  exact regression fails the strengthened guard before restoration.

### Slice 2 — independent preserved recovery

- Implement targeted exact-byte no-replace rename and fresh-file installation
  for settings and connections.
- Add distinct post-split settings, pre-split combined, and connections copy
  with real Rename/Reconnect and Exit buttons.
- Prove post-split settings recovery retains every connection without
  reauthorization; pre-split combined recovery salvages nothing and requires
  reconnection; connections recovery preserves settings/playlists and requires
  reconnection; Exit writes nothing.
- Add failure-injection, privacy, accessibility, static fallback guards, and
  real-app recovery coverage.
- Bump all version surfaces from `1.0.1` to `1.0.2`.

Implementation evidence (2026-07-23):

- The backend accepts only the closed settings/connections enum and only while
  that file's gate retains a readable regular invalid file's exact byte length
  and SHA-256. It rereads and revalidates under the file locks before using the
  platform's atomic no-replace rename, verifies the private byte-identical
  backup, installs and validates the selected default, then reloads both files.
- A strict private `durable-recovery.json` record closes the crash window
  between rename and fresh-file installation. Startup blocks on the record;
  Retry resumes only the exact pre-rename, post-rename, or post-install state.
  A missing, malformed, changed, symlinked, non-regular, or otherwise ambiguous
  state remains blocked without merge, overwrite, or guessed defaults.
- The blocking surface has real disabled-while-busy HTML buttons for **Rename
  and create new settings**, **Rename damaged connections and reconnect**, and
  **Exit Vela**. Distinct copy describes post-split settings preservation,
  legacy combined-file reconnection, and connection-only recovery. Native
  button semantics make Space activate the focused recovery action.
- Post-split settings recovery preserves the complete connections and playlist
  bytes and restores the existing source without reauthorization. Legacy
  combined recovery creates no connection file or row. Connections recovery
  preserves settings/playlists and installs an empty connection document. Exit
  leaves every seeded durable byte unchanged.
- Canonical local verification passed on the committed production behavior:
  exact Node/npm check, clean `npm ci`, zero-vulnerability npm audit, 47 Node
  tests, Svelte check with zero diagnostics, production frontend build, Rust
  1.89 and stable checks, clippy with warnings denied, 236 Rust tests, and Cargo
  audit with no vulnerabilities (17 existing allowed unmaintained/unsoundness
  warnings). `bash -n` accepted the version-bumped Arch PKGBUILD; a complete
  Arch package was not built because the standing Ubuntu venue has no
  `makepkg`.
- Checksum-identical source passed native Windows no-replace rename, private
  ACL, settings/connections/legacy recovery, restart-record, crash-resume, and
  ambiguous-state refusal tests on `netwatch-01`. The checksum-identical Linux
  real app passed all 35/35 E2E scenarios, including click recovery, Exit
  no-write checks, Space activation, restart, and recorded crash resume.
- Slice 2 landed as `0c9b48f`. Post-commit regressions independently proved the
  guards for same-length SHA-256 tampering, no-replace collision, backup
  rewriting, recovery without an eligible gate, a non-button recovery control,
  legacy connection-file creation, startup record bypass, ambiguous-state
  acceptance, and exposing another recovery button while recovery is
  incomplete. Removing the recovery button's busy-disabled state initially
  passed, exposing a vacuous static check; the check was strengthened, the
  exact regression then failed, and the committed button was restored.

### Slice 2A — three-version validated rollback

- Preserve complete prior validated settings and connection documents in
  independent private three-generation histories under their existing locks.
- Expose only the three newest still-private, still-valid versions as opaque
  ids and UTC timestamps on a recoverable-invalid gate.
- Add real dated rollback buttons and an exact-selection backend command.
- Reuse the damaged-file rename, verification, recovery marker, crash-resume,
  gate reload, and healthy-file preservation boundary for selected rollback.
- Add unit, static/frontend, Unix/Windows privacy, failure-injection, and real
  app settings/connections rollback coverage, including pruning and
  same-length history tampering.
- Bump all version surfaces from `1.0.2` to `1.0.3`.

Implementation evidence (2026-07-23):

- Ordinary validated settings and connection updates preserve the exact prior
  bytes under the owning process and cross-process locks. History creation
  rejects invalid input, deduplicates identical bytes, verifies private
  regular files after creation, and prunes each file's inventory to its three
  newest distinct valid versions.
- History discovery accepts only the owning file's exact timestamp/hash
  filename grammar, private regular files whose bytes match that filename's
  full SHA-256, and documents that still pass the current strict validator.
  The frontend receives only that opaque hash id and Unix-millisecond date,
  never a path, filename, document, or credential.
- A recoverable-invalid gate snapshots the exact eligible history inventory.
  Rollback requires an id from that gate, then rechecks both the unchanged
  damaged current file and exact selected history under the recovery and file
  locks before moving anything. It privately renames and verifies the damaged
  whole file, installs and validates only the chosen bytes, reloads the
  two-file gate, and never substitutes a different version.
- The strict recovery record now carries either a backward-compatible fresh
  replacement or the exact selected history filename, id, date, byte length,
  and hash. Restart resumes only that recorded replacement. An implementation
  audit found and closed nested unknown-field tolerance in the new replacement
  object; a behavioral test now rejects that altered journal shape.
- The blocking surface renders every offered version newest first as a real
  disabled-while-busy HTML button with a localized date/time. Native button
  semantics make Space activate the focused rollback choice. Fresh-file
  recovery and Exit remain available.
- Settings and connections real-app scenarios each seed three versions, choose
  the middle button, verify the exact damaged backup and exact selected
  canonical bytes, prove the other durable file and playlists unchanged, and
  restart successfully. The connections case restores the existing Plex
  source without reconnecting or exposing its token.
- Canonical local verification passed: exact Node/npm check, clean `npm ci`,
  zero-vulnerability npm audit, 48 Node tests, Svelte check with zero
  diagnostics, production frontend build, Rust 1.89 and stable checks, clippy
  with warnings denied, 242 Rust tests, and Cargo audit with no vulnerabilities
  (17 existing allowed unmaintained/unsoundness warnings). `bash -n` accepted
  the version-bumped Arch PKGBUILD.
- The first native Windows history run exposed that UUID plus full-hash
  filenames exceeded the host's legacy path limit in deep temporary paths.
  Removing the redundant UUID kept the full content hash as the opaque id and
  shortened the private filename. The checksum-identical final source passed
  all 28 durable tests and the four Windows storage/privacy tests on
  `netwatch-01`. The checksum-identical rebuilt Linux real app passed all 37/37
  E2E scenarios.
- Production implementation landed as `b09b610`. Post-commit regressions
  independently proved the literal three-version cap, identical-version
  deduplication, filename/content hash binding, settings and connections
  history routing, exact selected-version installation, native button/busy
  semantics, gate-bound version ids, nested recovery-record strictness,
  refusal of changed same-length valid history before the damaged file moves,
  and exact post-install crash-resume matching.
- The red-proof pass found three insufficient Rust guards: the history-ring
  test compared against the mutable production limit, the resume test covered
  only the post-rename state, and the selected-history tamper fixture became
  invalid rather than remaining a different valid same-length document. They
  were strengthened in `ee79573`, `b8d2860`, and `ac65b0f` respectively. Each
  exact regression then failed for its intended reason and the restored source
  reran green.

### Slice 3 — Plex token exposure hardening and closeout

- Move progress/timeline tokens from query parameters to headers.
- Replace token-bearing Plex artwork URLs with the bounded credential-free
  app-local artwork protocol.
- Complete mpv header-include cleanup and token nonexposure guards.
- Update README configuration, backup, rollback, and honest threat-boundary
  documentation.
- Run full canonical verification and independent code review.
- Bump all version surfaces from `1.0.3` to `1.0.4`.

Implementation evidence (2026-07-23):

- Plex progress and timeline requests now authenticate only through the
  `X-Plex-Token` header. Provider-supplied media-part URLs are rebuilt on the
  selected server, keep only credential-free query data, and fail closed when
  their decoded path, key, or value contains the active credential, including
  embedded and renamed forms.
- Plex item DTOs contain an opaque `vela-artwork` marker rather than a server
  URL. The frontend passes that marker through Tauri's platform-aware
  `convertFileSrc`; a bounded custom protocol validates the source, path,
  dimensions, redirects, response size, and image MIME before making a
  header-authenticated server request. Both Unix custom-scheme and Windows
  WebView2 HTTP protocol origins are admitted by CSP.
- Existing token-bearing Plex artwork persisted by 1.0.0 through 1.0.3 is
  converted to the credential-free marker when its legacy transcode shape is
  safe, otherwise removed. Settings recents and playlists sanitize at both
  read and ordinary write boundaries, and startup persists the safe form where
  the owning durable file can be updated.
- Each Plex mpv launch owns a unique header include inside Vela's private
  configuration directory. Windows applies and verifies the private ACL before
  token bytes are written; partial writes remove the file; replacement removes
  the consumed predecessor include before creating a successor; uncertain
  process status retains cleanup ownership; confirmed exit, app exit, and the
  queued-child drain all retry cleanup.
- Errors and mock-server records carry only credential-free categories,
  status, paths, and token-presence/match booleans. Authenticated discovery no
  longer reflects provider response bodies, names, or URIs through logs or the
  frontend.
- Independent `gpt-5.6-sol` review at xhigh first returned two HIGH, three
  MEDIUM, and two LOW findings: legacy persisted artwork, provider Part
  queries, Windows protocol conversion, Windows include privacy, include
  lifecycle cleanup, mock-log credentials, and missing real-app
  progress/timeline coverage. A follow-up review returned two MEDIUM and three
  LOW findings: embedded provider credentials, queued cleanup at exit,
  embedded mock-log credentials, discovery body reflection, and an unguarded
  confirmed-exit branch. Every finding was admitted and resolved; both review
  passes returned findings, so no clean independent verdict is claimed.
- Canonical local verification passed: exact Node/npm check, clean `npm ci`,
  zero-vulnerability npm audit, 51 Node tests, Svelte check with zero
  diagnostics, production frontend build, Rust 1.89 and stable checks, clippy
  with warnings denied, 259 Rust tests, and Cargo audit with no vulnerabilities
  (17 existing allowed unmaintained/unsoundness warnings). `bash -n` accepted
  the version-bumped Arch PKGBUILD.
- Checksum-identical final source passed all 255 native Windows library tests
  on `netwatch-01`, including ACL and header-include lifecycle coverage. One
  history test failed transiently on the first parallel run, then passed alone
  and in the complete rerun. The checksum-identical rebuilt Linux real app
  passed all 37/37 E2E scenarios; multiplex exercised credential-free artwork,
  playable Plex media, progress, and timeline while asserting that no token
  entered a URL query or captured mock record.

If slices are combined during implementation, version only the landed coherent
commits and update the numeric sequence in this plan before proceeding. After
the prerequisite lands, rebase the marker plan's example version sequence from
the actual release version; never reuse stale planned numbers.

---

## Expected files

- `src-tauri/src/config.rs`
- new `src-tauri/src/connections.rs` (or an equivalently focused module)
- `src-tauri/src/storage.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/commands.rs`
- `src-tauri/src/playback.rs`
- `src-tauri/src/plex_api.rs` and `src-tauri/src/plex_library.rs`
- persisted-source restoration in `src-tauri/src/source/plex.rs` and
  `src-tauri/src/source/jellyfin.rs`
- any durable-state-reading selection/display modules found by the complete
  callsite audit
- Tauri protocol/CSP/capability configuration required by credential-free Plex
  artwork
- `src/routes/+page.svelte`, image consumers, and a small shared frontend
  durable-status type/store if needed
- frontend/static guard tests
- split-migration, `configrecovery`, `connectionrecovery`, and Plex auth E2E
  scenarios plus focused helpers/docs
- README and every canonical release-version surface
- `.agents/state.md`, `.agents/decisions.md`, and this plan as rulings and
  implementation evidence land

Do not edit generated `build/`, `.svelte-kit/`, `node_modules/`,
`src-tauri/target/`, or `src-tauri/gen/`.

---

## Owner decisions

### Settled — separate private connections, no credential vault or pretend encryption

Owner-approved 2026-07-23: active Plex/Jellyfin/Emby connection records and
tokens live in `connections.json`, independently of `config.json`. Once that
split exists, resetting invalid settings leaves valid connections
byte-identical and does not require Plex reauthorization. Resetting invalid
connections is a separate explicit action and does not reset settings or
playlists.

Tokens remain plaintext behind owner-account filesystem protection; Vela does
not use an OS credential vault, user passphrase, or an app-managed encryption
key stored beside the ciphertext. The connection file, locks, temporary files,
and backups are private from their first byte. Token values are redacted from
debug/errors/DTOs, removed from Plex URLs and query parameters, and carried in
private request/header paths. This does not claim protection from malware
already running as the same OS user.

### Settled — reject unknown active setting fields

Owner-approved 2026-07-23: an otherwise valid `config.json` fails when it
contains an unrecognized top-level setting key, and an otherwise valid
`connections.json` fails when it contains an unrecognized connection/source
key. Vela does not guess at, ignore, or silently delete persisted fields this
build cannot understand. The exact invalid owning file remains eligible for the
approved private backup recovery, so a file from a future Vela is preserved
even if a downgrade cannot load it.

Known-field wrong types and unknown constrained values also fail. Documented
legacy local/SMB/SSH fields remain valid, and provider media-response extras and
embedded cached-media snapshot extras remain outside the active settings
schema.

### Settled — invalid settings and recovery

Owner-approved 2026-07-22 and canonical in `.agents/decisions.md`: invalid
settings fail closed; the app loads no guessed/default/partial interpretation;
the user is warned the file may be damaged or may have been tampered with; a new
config is recommended; and explicit recovery preserves a unique private
byte-for-byte renamed original before installing validated defaults. Under the
later split decision, post-split recovery targets only the invalid owning file.

### Settled — damaged files are renamed whole or left for manual repair

Owner-approved 2026-07-23: a damaged settings file offers **Rename and create
new settings** or **Exit Vela**. A damaged connections file offers **Rename
damaged connections and reconnect** or **Exit Vela**. The later dated-history
decision adds explicit rollback choices to both surfaces. Exit writes nothing.

An invalid old combined config is treated as one damaged settings file. Vela
does not extract or validate a connection subsection from it. Renaming and
creating fresh settings therefore requires reconnecting servers; the UI says so
before the action. The no-reauthorization guarantee applies only when a separate
valid `connections.json` already exists.

### Settled — three dated valid rollback versions per file

Owner-approved 2026-07-23: settings and connections independently retain the
three newest complete, distinct, private prior versions that passed their
strict validators. A damaged-file screen shows all available versions newest
first as dated real buttons. The user chooses one exact version; Vela never
automatically selects the newest.

Rollback uses an opaque backend id, revalidates the exact selected version,
preserves the complete damaged current file first, and restores only the owning
file. Fresh-file recovery and Exit remain available. Invalid or changed history
is never offered or substituted with another version.

---

## Done criteria

- Every successful settings or connections read/write crosses its strict
  validator.
- Active connection records and provider tokens live only in private
  `connections.json` after a retry-safe, exact-backup migration.
- No invalid or unavailable file produces normal app state, defaults for that
  file, or a partial source registry.
- Every constrained value and setting command is strict.
- Valid documented omissions, connection split, legacy Plex migration, and
  rollback-preserved local/SMB/SSH fields remain compatible.
- The blocking UI distinguishes the owning file and invalid, unavailable, and
  migration-blocked state.
- Recovery is explicit, targets only a safely reread invalid regular file,
  privately renames the complete original before selected-version or fresh-file
  installation, and leaves any separate healthy file and playlists untouched.
- Settings and connections independently retain and display at most the three
  newest private, distinct, strictly valid rollback versions with no path or
  credential exposure.
- Post-split settings recovery restores unchanged connections without
  reauthorization. Pre-split combined recovery and connections recovery
  salvage no connection data and route to reconnection.
- Exit from every durable fault screen writes nothing.
- Plex tokens are private-file-backed and redacted, never appear in frontend
  DTOs, returned URLs, query strings, argv, logs, or errors, and use the guarded
  request/mpv header paths.
- All fallback, partial-restore, post-split `AppConfig.sources`, and token-URL/
  query callsites are removed and statically guarded.
- Unit, frontend, privacy, fault-injection, red-proof, canonical, and real-app
  E2E evidence is recorded in this plan.
- Every accepted review finding is resolved; any owner-waived follow-up is
  recorded without claiming a clean external verdict.
- The work is committed, `.agents/state.md` records it landed, and only then may
  the marker plan be explicitly activated.
