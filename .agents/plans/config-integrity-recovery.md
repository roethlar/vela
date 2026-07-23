# Plan: fail-closed settings integrity and preserved recovery

## Status

**Draft v1, revision 2 — 2026-07-23.** Planning-only prerequisite for
`.agents/plans/skip-credits-intros-v2.md`.

The owner approved the core product contract on 2026-07-22: an invalid settings
file never loads through normalization, default substitution, or partial source
restoration; Vela blocks normal use, explains that the file may be damaged or
may have been tampered with, and recommends an explicit backup-then-fresh-config
recovery.

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

## Goal

Treat `config.json` as one credential-bearing settings document with a single
validity boundary:

- a genuinely absent file is valid first-run state;
- a valid file loads in full;
- a readable but invalid file loads nothing;
- a file Vela cannot safely inspect loads nothing;
- recovery is a deliberate user action, never startup fallback;
- recovery first preserves the invalid bytes in a unique private backup, then
  atomically installs a validated fresh config.

There is no runtime state in which some settings or sources came from an
invalid file and the rest came from defaults.

---

## Authority and compatibility contract

The durable owner ruling is
`.agents/decisions.md` **2026-07-22 — Invalid settings fail closed with explicit
preserved recovery**. The legacy local-source rollback contract in
`.agents/repo-guidance.md` remains equally binding.

These are valid compatibility cases, not corruption:

- `config.json` does not exist;
- a documented optional field is missing and therefore takes its documented
  default;
- a pre-multi-Plex config or retryable partial Plex migration has the exact
  legacy shape supported by the existing migration;
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
  inconsistent active media-source row;
- an invalid in-progress legacy Plex migration record;
- active collection data outside its enforced persistence bounds;
- a per-section sort value outside Vela's sort whitelist;
- an unknown top-level or active-source setting field.

Validation is local and deterministic. It never contacts a media server,
probes `mpv`, guesses a source, or changes the file.

---

## Fault classes

Use a typed internal result instead of collapsing all failures into
`io::Error`, `String`, `Option`, or `Default`:

```rust
enum ConfigLoad {
    Absent(AppConfig),
    Valid(AppConfig),
}

enum ConfigFault {
    Invalid {
        kind: InvalidConfigKind,
    },
    Unavailable {
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
| Absent | No file at the resolved path | Yes, documented defaults | No |
| Valid | Parse, compatibility migration, and validation succeed | Yes, all of it | No |
| Invalid | Readable regular file fails syntax, schema, or semantics | No | Yes |
| Unavailable | Permission, symlink/non-regular file, unsafe metadata, or read I/O failure | No | No |
| Migration blocked | A valid migration cannot finish because its playlist/config I/O failed | No | No |

An invalid migration *shape* is `Invalid`; a valid migration whose separate
atomic work fails is `MigrationBlocked`. Unavailable and migration-blocked
screens provide **Try again** and safe manual guidance, but cannot offer reset:
Vela has not proved that it can preserve the authoritative bytes first.

User-facing errors carry only a stable category and generic explanation. Raw
JSON, auth values, URLs containing tokens, serde value excerpts, and config
contents never enter logs, command errors, events, or the UI.

---

## One validated config boundary

### Parse and validate

Add a central, side-effect-free `AppConfig::validate()` (or an equivalent
validator returning a validated newtype). Every successful public load and
every save/update crosses this boundary. No caller may receive or persist an
unvalidated `AppConfig`.

Validation covers:

- `watched_threshold_percent`: missing is its documented default; a present
  value is in `1..=100`;
- closed policies: autocrop (`off`, `manual`, `auto`), Continue Playing
  (`off`, `on`, `only-tv`), playback source
  (`best`, `compatible`, `fastest`, `ask`), resolution override, and HDR
  override accept only their documented values;
- source ids are nonblank and unique; source kind is exactly `plex`,
  `jellyfin`, or `emby`; source name is nonblank;
- each source satisfies the same persisted requirements as its provider
  constructor. Plex requires its token and device/client id and preserves the
  existing endpoint/machine-pin safety rule. Jellyfin/Emby requires one token
  form, user id, device id, and nonblank base URL. Validation does not require
  the server to be reachable;
- provider construction from every validated source succeeds. Change the
  startup constructors from silent `Option` omission to a typed `Result`, and
  share pure validation helpers so the config validator and constructor cannot
  drift;
- `section_sorts` keys obey the existing setter length/nonblank rules and every
  value belongs to `ALLOWED_SORTS`;
- persisted recents and hidden tombstones obey their canonical module bounds.
  Do not duplicate numeric constants in the validator;
- transient `plex_source_migration` is either absent or has the exact
  retry-safe relationship required by the migration;
- stale source/media references remain valid and are filtered only where the
  product already filters them.

Prefer strict serde enums for closed policies, with explicit serde names and
documented missing defaults. If a field must remain a string for IPC or
compatibility, give it a strict parser returning `Result`; do not retain a
normalizer that maps unknown input to a valid choice.

All setting commands parse and reject invalid input before calling
`AppConfig::update`. `set_mpv_advanced`, `set_continue_playing`,
`set_playback_preferences`, and `set_section_sort` must not clamp or normalize
bad input. After a mutation closure succeeds, validate the complete result
before serialization. A failed mutation leaves the original bytes unchanged.

### Legacy Plex migration order

Migration must not modify an otherwise invalid file:

1. Parse into the compatibility schema.
2. Validate every non-migration field and validate that any legacy Plex shape
   is one of the explicitly supported pre- or mid-migration forms.
3. If no migration is needed, return only after full validation.
4. If migration is needed, run the existing lock-protected, retry-safe
   config/playlist migration.
5. Reload and fully validate the post-migration config before making it
   available to the app.

Malformed migration state is recoverable invalid config. Operational failure
after a valid migration began is migration-blocked state: retain the existing
retry data and never replace it with defaults.

### Unknown fields

Owner-approved 2026-07-23: an unknown top-level or active-source setting name
invalidates the whole config. Apply `deny_unknown_fields` (or equivalent
explicit key validation) to `AppConfig`, `SourceConfig`, and other active
settings records. Never ignore or silently delete an unknown active key.

The legacy local/SMB/SSH field names stay known and valid; their nested rollback
payload remains tolerant enough to preserve the documented old shape. Embedded
media snapshots and provider response DTOs are not settings schemas and retain
their existing forward-compatible tolerance.

---

## Startup and runtime state

Add an app-wide config gate to `AppState`. Startup has exactly two modes:

```text
load + migrate + validate config
  ├─ success → build every persisted source → normal application
  └─ fault   → build no persisted source → blocking config-fault application
```

Source-registry restoration is all-or-nothing. Replace the `if let Some(...)`
startup loop with a function that returns `Result<SourceRegistry, ConfigFault>`;
one invalid source row prevents the registry from being installed.

The fault mode is not represented as an empty source list. The frontend must
be able to distinguish a real new user from a blocked config before invoking
any settings- or source-dependent command.

Expose narrowly scoped commands:

```text
get_config_status
retry_config
recover_invalid_config
```

`get_config_status` returns a credential-free tagged DTO with `ready`,
`recoverable_invalid`, `unavailable`, or `migration_blocked`. It includes only
safe display text and whether recovery is allowed.

`retry_config` rereads, migrates, validates, and rebuilds the complete registry.
Success atomically replaces the fault state with ready state; failure leaves
normal commands gated.

Every command that depends on config checks the gate. A command may not bypass
it by calling the old static loader. If a later read/update discovers that a
previously valid on-disk config became invalid or unavailable, transition the
gate to the corresponding blocking state and notify the frontend through one
credential-free `config-fault` event. The frontend has one listener that
replaces normal UI with the recovery surface. There is no polling race and no
collection of command-specific fallback behavior.

Keep a single backend access facade responsible for:

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

`recover_invalid_config` is accepted only while the gate holds a
`recoverable_invalid` result for a readable regular `config.json`. The button
click is the confirmation; do not add automatic recovery or a second
destructive-looking prompt.

Under the existing process mutex and cross-process config lock:

1. Resolve `config.json` through the same storage boundary used by normal
   persistence. Refuse symlinks and non-regular files.
2. Reopen and reread the current bytes. Re-run parse and validation. If the
   file is now valid, absent, unavailable, or differs from the invalid snapshot
   represented by the gate, abort the stale recovery and return the new status.
3. Create a unique sibling such as
   `config.invalid-<UTC timestamp>-<uuid>.json` with create-new semantics.
   Never overwrite an existing backup.
4. Apply the same private storage protection as `config.json`: owner-only
   `0600` on Unix before writing and inherited app-config-directory protection
   on other platforms.
5. Write the exact invalid byte sequence, flush it, and sync the file. Confirm
   its length and content hash against the just-read source bytes. Never parse,
   redact, pretty-print, or reserialize the backup.
6. Serialize `AppConfig::default()`, parse/validate it through the same strict
   boundary, and write it using the existing private atomic-temp-plus-rename
   primitive. Sync the containing directory where supported.
7. Reload the new config and build the empty registry through the normal
   validated startup path.
8. Only then mark the gate ready and return the safe backup filename.

If backup creation or verification fails, leave `config.json` untouched and
report failure. If the final atomic replacement fails, the original remains
authoritative; the verified backup may remain and the UI reports that no fresh
settings were installed. Never delete a material backup automatically.

Recovery changes only `config.json`. Vela playlists and every other app-data
file remain untouched.

The storage layer needs focused helpers for:

- distinguishing absent from unreadable/non-regular;
- bounded exact-byte reads of config;
- unique private create-new backup writes;
- validated atomic replacement;
- fault-injection tests around every failure point.

If the implementation introduces a config-size limit, an oversized file must
remain recoverable only if Vela can still preserve all of its bytes. Never
truncate a backup.

---

## Blocking frontend

`src/routes/+page.svelte` performs `get_config_status` before `check_mpv`,
`get_sources`, Continue Playing, settings, home, playlist, or navigation
requests. Until status is ready it renders a non-dismissible full-page state,
not the Welcome screen and not the ordinary transient error banner.

Recoverable-invalid copy:

> Vela could not safely read your settings. The file may be damaged or may
> have been tampered with. Nothing from it was loaded.

> We recommend starting with a new settings file. Vela will preserve the
> current file as a private backup first.

Controls:

- primary real HTML button: **Back up and create new settings**;
- secondary real HTML button: **Try again**.

Disable both controls while their request is in flight. On failure, keep the
blocking screen, show a credential-free error, and re-enable the actions
allowed by the returned status. On successful recovery, show the safe backup
filename, then enter the genuine fresh-config Welcome state.

Unavailable or migration-blocked copy explains that Vela loaded nothing and
that the file could not safely be backed up or migration could not finish.
Show **Try again** and manual location/help text, but do not render the fresh
settings action.

The screen must have an alert/status relationship suitable for assistive
technology, move focus to its heading on fault transition, keep normal app
content inert/unrendered, and return focus to the normal root after successful
retry or recovery.

`loadSourceList()` and boot no longer suppress source/config errors into `[]`.
Any remaining best-effort catches must be for data that is explicitly
independent of config health.

---

## Existing fallback removal

Audit every `config::load_config`, `AppConfig::update`, constrained-setting
normalizer, and provider restore callsite. At minimum, remove the known
conflicts in:

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
config access boundary. Keep the guard semantic enough that unrelated Option
defaults remain legal. Red-prove it by inserting each prohibited production
pattern.

Update stale comments that currently promise tolerant unknown-value
normalization.

---

## Security and privacy

- Config and backup contents are secrets. Tests use unmistakably synthetic
  tokens and assert those values do not appear in errors, events, or logs.
- Do not expose a raw serde error containing source text. A safe category may
  include JSON line/column only if tests prove no input fragment is included;
  generic copy is preferred.
- Backups live beside `config.json`, never in downloads, logs, temp output, or
  the repository.
- Keep atomic-save and cross-process-lock behavior. Do not weaken either to
  make recovery easier.
- Refuse recovery for a symlink or non-regular file. Do not follow a link and
  copy or replace a target outside Vela's config directory.
- Do not use network reachability as integrity validation: an offline server is
  not damaged settings.

---

## Verification

Every new guard is independently red-proven: land the behavior, inject one
specific regression, prove the intended test fails for the intended reason,
restore from committed bytes, and rerun green.

### Rust unit/integration matrix

- absent file returns documented defaults without creating a file;
- valid minimal and populated configs load;
- every documented missing field uses its documented default;
- malformed JSON, wrong types, every unknown closed value, invalid thresholds,
  invalid section sorts, duplicate/incomplete/unknown sources, bad pinned Plex
  endpoint state, malformed migration state, and bounded-collection overflow
  fail the whole config;
- stale media/source references remain valid;
- legacy local/SMB/SSH payloads, including synthetic credentials and old nested
  fields, survive load-update-save;
- valid pre- and mid-Plex migrations remain retry-safe; an unrelated invalid
  field prevents migration from writing;
- all persisted-source constructors succeed after validation and a forced
  constructor failure prevents all registry installation;
- every setter rejects invalid input and a failed update leaves original bytes
  identical;
- startup faults never install a partial registry or expose default settings;
- runtime invalidation moves the gate to fault and emits one safe event;
- retry moves to ready only after complete validation and registry rebuild;
- recovery creates a unique byte-identical private backup and a valid fresh
  config; playlist bytes are unchanged;
- backup create/write/sync/verify failures and replacement failures preserve
  the original; a stale snapshot, symlink, non-regular file, permission error,
  and migration-blocked state cannot recover;
- no error/event/log assertion contains the synthetic secret.

Guard unknown top-level and active-source fields as invalid, while proving
legacy rollback payloads and non-settings media snapshots remain tolerant.

### Frontend/static tests

- boot requests config status first and issues no normal boot invoke while
  blocked;
- invalid status renders the exact blocking recovery copy and both real
  buttons;
- unavailable/migration-blocked status omits the fresh-config button;
- recovery and retry have correct disabled, failure, and success states;
- a runtime `config-fault` event replaces normal content and moves focus;
- config/source errors cannot become `[]`, Welcome, or a normal transient
  banner;
- the static fallback guard fails for each prohibited Rust fallback mutation.

### Real-app E2E

Add a hermetic `configrecovery` scenario using the existing per-scenario
throwaway config root:

1. Seed malformed JSON containing a synthetic secret and seed a separate Vela
   playlist.
2. Launch the real app and assert the blocking screen appears before any normal
   app/home/settings content.
3. Assert **Try again** keeps the screen while the file remains invalid.
4. Click **Back up and create new settings**.
5. Assert exactly one private backup exists and is byte-identical to the seeded
   file, `config.json` is a valid serialized default, the playlist is
   byte-identical, no secret is visible, and the app enters Welcome.
6. Restart and assert the fresh config loads normally without showing recovery.

Add focused seeded cases for a semantically invalid constrained value and an
incomplete source row so E2E proves this is validation, not only JSON syntax
handling. The owner's media library needs no damaged real config fixture.

Run the full canonical cross-side verification from
`.agents/repo-guidance.md`, including the real-app E2E suite. This work changes
startup, commands, persistence, and frontend behavior.

---

## Implementation slices

Each slice is one reviewed, verified commit. Do not start the next slice while
the current slice has uncommitted finished work.

### Slice 1 — strict schema, validator, and persistence boundary

- Introduce typed config load/fault results and strict constrained policies.
- Implement pre-/post-migration validation and validate-before-save updates.
- Convert persisted source construction to shared typed validation.
- Add storage classification and unit matrices.
- Remove setting-input normalization.
- Bump every release version surface from `1.0.0` to `1.0.1`.

This slice may add types used by later UI work, but startup must not yet claim
recovery exists. If the slice cannot leave the app coherent without the gate,
combine it with Slice 2 rather than temporarily shipping a fallback.

### Slice 2 — app-wide gate and all-or-nothing startup/runtime

- Add the config gate/status/retry commands and safe runtime fault event.
- Rebuild the source registry atomically or not at all.
- Remove every load-error/default/partial-source fallback.
- Gate frontend boot and add the blocking retry-only states.
- Add the static fallback guard and its red proofs.
- Bump all version surfaces from `1.0.1` to `1.0.2`.

### Slice 3 — preserved recovery and end-to-end proof

- Implement the unique private exact-byte backup and validated atomic reset.
- Add the recoverable-invalid frontend copy and real buttons.
- Add failure-injection, privacy, accessibility, and real-app E2E coverage.
- Update README user recovery documentation.
- Run full canonical verification and independent code review.
- Bump all version surfaces from `1.0.2` to `1.0.3`.

If slices are combined during implementation, version only the landed coherent
commits and update the numeric sequence in this plan before proceeding. After
the prerequisite lands, rebase the marker plan's example version sequence from
the actual release version; never reuse stale planned numbers.

---

## Expected files

- `src-tauri/src/config.rs`
- `src-tauri/src/storage.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/commands.rs`
- `src-tauri/src/playback.rs`
- persisted-source restoration in `src-tauri/src/source/plex.rs` and
  `src-tauri/src/source/jellyfin.rs`
- any config-reading selection/display modules found by the complete callsite
  audit
- `src/routes/+page.svelte` and a small shared frontend config-status type/store
  if needed
- frontend/static guard tests
- `tests/e2e/scenarios/configrecovery.mjs` and focused E2E helpers/docs
- README and every canonical release-version surface
- `.agents/state.md`, `.agents/decisions.md`, and this plan as rulings and
  implementation evidence land

Do not edit generated `build/`, `.svelte-kit/`, `node_modules/`,
`src-tauri/target/`, or `src-tauri/gen/`.

---

## Owner decisions

### Settled — reject unknown active setting fields

Owner-approved 2026-07-23: an otherwise valid `config.json` fails when it
contains an unrecognized top-level or active-source setting key. Vela does not
guess at, ignore, or silently delete active settings this build cannot
understand. The exact invalid file remains eligible for the approved private
backup recovery, so a file from a future Vela is preserved even if a downgrade
cannot load it.

Known-field wrong types and unknown constrained values also fail. Documented
legacy local/SMB/SSH fields remain valid, and provider media-response extras and
embedded cached-media snapshot extras remain outside the active settings
schema.

### Settled — invalid settings and recovery

Owner-approved 2026-07-22 and canonical in `.agents/decisions.md`: invalid
settings fail closed; the app loads no guessed/default/partial interpretation;
the user is warned the file may be damaged or may have been tampered with; a new
config is recommended; and explicit recovery preserves a unique private
byte-for-byte backup before atomically installing validated defaults.

---

## Done criteria

- Every successful config read and write crosses one strict validator.
- No invalid or unavailable file produces normal app state, default settings,
  or a partial source registry.
- Every constrained value and setting command is strict.
- Valid documented omissions, legacy Plex migration, and rollback-preserved
  local/SMB/SSH fields remain compatible.
- The blocking UI clearly distinguishes invalid, unavailable, and
  migration-blocked state.
- Recovery is explicit, available only for a safely reread invalid regular
  file, and proves exact private backup before replacement.
- All fallback and partial-restore callsites are removed and statically guarded.
- Unit, frontend, privacy, fault-injection, red-proof, canonical, and real-app
  E2E evidence is recorded in this plan.
- The required external review is clean or every accepted finding is closed.
- The work is committed, `.agents/state.md` records it landed, and only then may
  the marker plan be explicitly activated.
