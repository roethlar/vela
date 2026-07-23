# Plan: fail-closed settings integrity and preserved recovery

## Status

**Draft v1, revision 3 — 2026-07-23.** Planning-only prerequisite for
`.agents/plans/skip-credits-intros-v2.md`.

The owner approved the core product contract on 2026-07-22: an invalid settings
file never loads through normalization, default substitution, or partial source
restoration; Vela blocks normal use, explains that the file may be damaged or
may have been tampered with, and recommends an explicit backup-then-fresh-config
recovery.

On 2026-07-23 the owner approved separating active server connections and
tokens into a private `connections.json`, with no OS credential vault and no
app-managed encryption. Owner-account file access, credential-safe runtime
handling, and removal of avoidable Plex token URLs/query strings are the
security boundary.

All product choices in **Owner decisions** are settled and recorded in
`.agents/decisions.md`.

Not active implementation until:

1. The required external plan review is complete and every accepted finding is
   resolved.
2. `.agents/state.md` explicitly names this plan as active implementation. Its
   planning-only mention and Active Sources entry do not activate it.

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

This review is not a clean approval. Do not activate implementation until
`cir-1` is resolved in the plan and the revised exact range receives the
required follow-up review.

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
- recovery first preserves the invalid bytes in a unique private backup, then
  atomically installs a validated fresh document for only the faulted file.

An invalid settings file cannot erase, rewrite, or require reauthorization of a
valid connections file. An invalid connections file cannot erase or reset
settings. There is no runtime state in which part of either invalid file is
combined with defaults or partially restored records.

---

## Authority and compatibility contract

The durable owner rulings are
`.agents/decisions.md` **2026-07-22 — Invalid settings fail closed with explicit
preserved recovery**, **2026-07-23 — Unknown active setting names invalidate
the config**, and **2026-07-23 — Connections are private file-backed state,
not settings**. The legacy local-source rollback contract in
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
required:

| Result | Meaning | Normal app loads? | Fresh-config action? |
|---|---|---:|---:|
| Absent | No file at that resolved path | Yes, that file's documented empty/default state | No |
| Valid | Parse, compatibility migration, and validation succeed | Yes, all of that file | No |
| Invalid | Readable regular file fails syntax, schema, or semantics | No data from that file | Yes, for that file only |
| Unavailable | Permission, symlink/non-regular file, unsafe metadata, or read I/O failure | No data from that file | No |
| Migration blocked | A valid split/Plex migration cannot finish because one atomic step failed | No active sources | No |

An invalid migration *shape* is `Invalid`; a valid migration whose separate
atomic work fails is `MigrationBlocked`. A settings fault does not change a
valid connections result, and a connections fault does not change a valid
settings result. Normal server-backed use still waits until every required
file is ready. Unavailable and migration-blocked screens provide **Try again**
and safe manual guidance, but cannot offer reset: Vela has not proved that it
can preserve the authoritative bytes first.

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
`migration_blocked`. It includes only safe display text and whether recovery is
allowed for that file.

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

Under the selected file's process mutex and cross-process lock:

1. Resolve `config.json` or `connections.json` through the same storage boundary
   used by normal persistence. Refuse symlinks and non-regular files.
2. Reopen and reread the current bytes. Re-run parse and validation. If the
   file is now valid, absent, unavailable, or differs from the invalid snapshot
   represented by the gate, abort the stale recovery and return the new status.
3. Create a unique sibling named
   `<stem>.invalid-<UTC timestamp>-<uuid>.json` with create-new semantics. Never
   overwrite an existing backup.
4. Apply the same private storage protection as the selected source file:
   owner-only `0600` on Unix before writing and a per-user AppData ACL on
   Windows with no ordinary cross-user read access.
5. Write the exact invalid byte sequence, flush it, and sync the file. Confirm
   its length and content hash against the just-read source bytes. Never parse,
   redact, pretty-print, or reserialize the backup.
6. Serialize the selected document's default (`AppConfig::default()` or
   `ConnectionsConfig::default()`), parse/validate it through the same strict
   boundary, and write it using the private atomic-temp-plus-rename primitive.
   Sync the containing directory where supported.
7. Reload both independent files. A settings reset rebuilds the registry from
   the untouched valid connections and therefore does not require Plex,
   Jellyfin, or Emby authorization. A connections reset installs an empty
   registry and requires reconnecting servers, while preserving settings,
   recents, and playlists.
8. Only then mark the gate ready and return the safe backup filename.

If backup creation or verification fails, leave the selected source file
untouched and report failure. If the final atomic replacement fails, the
original remains authoritative; the verified backup may remain and the UI
reports that no fresh document was installed. Never delete a material backup
automatically.

Recovery changes only the explicitly selected invalid file. Vela playlists and
the healthy durable file remain byte-identical.

The storage layer needs focused helpers for:

- distinguishing absent from unreadable/non-regular;
- bounded exact-byte reads of either durable file;
- unique private create-new backup writes;
- validated atomic replacement;
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

Recoverable-invalid settings copy:

> Vela could not safely read your settings. The file may be damaged or may
> have been tampered with. Nothing from it was loaded.

> We recommend starting with a new settings file. Vela will preserve the
> current file as a private backup first. Your server connections are stored
> separately and will not be changed.

Controls:

- primary real HTML button: **Back up and create new settings**;
- secondary real HTML button: **Try again**.

Recoverable-invalid connections copy names the server-connections file,
explains that no connection or token was loaded, and warns that creating a new
connections file requires reconnecting servers. Its primary real HTML button
is **Back up and remove invalid connections**. It never implies that settings,
recents, or playlists will be reset.

Disable both controls while their request is in flight. On failure, keep the
blocking screen, show a credential-free error, and re-enable the actions
allowed by the returned status. On successful recovery, show the safe backup
filename. A settings recovery continues with the preserved connections; a
connections recovery enters the genuine no-sources Welcome state.

Unavailable or migration-blocked copy explains that Vela loaded nothing and
that the file could not safely be backed up or migration could not finish.
Show **Try again** and manual location/help text, but do not render a reset
action for that file.

The screen must have an alert/status relationship suitable for assistive
technology, move focus to its heading on fault transition, keep normal app
content inert/unrendered, and return focus to the normal root after successful
retry or recovery.

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
- settings recovery creates a unique byte-identical private backup and a valid
  fresh config while connections/playlists remain byte-identical and every
  source restores without reauthorization;
- connections recovery creates its own unique byte-identical private backup and
  an empty valid connections file while settings/playlists remain
  byte-identical;
- backup create/write/sync/verify failures and replacement failures preserve
  the original; a stale snapshot, symlink, non-regular file, permission error,
  and migration-blocked state cannot recover;
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
- invalid settings and invalid connections render their distinct exact copy and
  correct real buttons;
- unavailable/migration-blocked status omits that file's reset button;
- recovery and retry have correct disabled, failure, and success states;
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
3. Assert **Try again** keeps the screen while the file remains invalid.
4. Click **Back up and create new settings**.
5. Assert exactly one private backup exists and is byte-identical to the seeded
   file, `config.json` is a valid serialized default, the playlist is
   byte-identical, `connections.json` is byte-identical, no token is visible,
   and the existing Plex source loads without relinking.
6. Restart and assert the fresh settings and preserved connection load normally
   without showing recovery.

For connection recovery, seed a valid settings file and an invalid source row;
assert the connection-specific warning/reset, exact connection backup, empty
fresh connections file, untouched settings/playlist bytes, and Welcome after
recovery. For split migration, seed a valid combined 1.0.0 config; assert the
exact private pre-split backup, token-free live settings file, private
connections file, restored source, credential-free UI artwork URL, and
header-authenticated mock artwork/playback/progress requests.

Add focused cases for an unknown constrained setting, unknown top-level setting
key, unknown connection key, and malformed JSON so E2E proves this is strict
validation, not only one syntax path. The owner's media library needs no
damaged real config fixture.

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

### Slice 2 — independent preserved recovery

- Implement targeted exact-byte backup and atomic reset for settings and
  connections.
- Add the distinct recoverable-invalid copy and real buttons.
- Prove settings recovery retains every connection without reauthorization and
  connections recovery preserves settings/playlists.
- Add failure-injection, privacy, accessibility, static fallback guards, and
  real-app recovery coverage.
- Bump all version surfaces from `1.0.1` to `1.0.2`.

### Slice 3 — Plex token exposure hardening and closeout

- Move progress/timeline tokens from query parameters to headers.
- Replace token-bearing Plex artwork URLs with the bounded credential-free
  app-local artwork protocol.
- Complete mpv header-include cleanup and token nonexposure guards.
- Update README configuration, backup, rollback, and honest threat-boundary
  documentation.
- Run full canonical verification and independent code review.
- Bump all version surfaces from `1.0.2` to `1.0.3`.

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
tokens live in `connections.json`, independently of `config.json`. Resetting
invalid settings leaves valid connections byte-identical and does not require
Plex reauthorization. Resetting invalid connections is a separate explicit
action and does not reset settings or playlists.

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
byte-for-byte backup before atomically installing validated defaults. Under the
later split decision, this recovery targets only the invalid owning file.

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
  proves exact private backup before replacement, and leaves the healthy file
  and playlists untouched.
- Settings recovery restores the unchanged connections without
  reauthorization; only connections recovery empties the registry.
- Plex tokens are private-file-backed and redacted, never appear in frontend
  DTOs, returned URLs, query strings, argv, logs, or errors, and use the guarded
  request/mpv header paths.
- All fallback, partial-restore, post-split `AppConfig.sources`, and token-URL/
  query callsites are removed and statically guarded.
- Unit, frontend, privacy, fault-injection, red-proof, canonical, and real-app
  E2E evidence is recorded in this plan.
- The required external review is clean or every accepted finding is closed.
- The work is committed, `.agents/state.md` records it landed, and only then may
  the marker plan be explicitly activated.
