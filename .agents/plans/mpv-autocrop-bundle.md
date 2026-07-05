# Plan: Bundle mpv's autocrop.lua + a Settings toggle

## Status

**Owner-approved 2026-07-05 — implementing.** Codex reviewloop accepted at r4 (4
rounds; trail in Review log); owner approved with the three-state (Off/Manual/
Automatic) decision folded in. Owner chose option "C" (2026-07-05): ship
mpv's `autocrop.lua` with Vela and expose an enable toggle. This REVERSES the
2026-07-05 "Letterbox crop feature DROPPED" decision, so approval of this plan
also lands a superseding decision-log entry (drafted below).

## Why this is a decision reversal, and the boundary it keeps

The 2026-07-05 decision dropped a crop feature where **Vela implemented the crop
logic** (sampled bar-detection scan, a D-state-safe correction mechanism, a
per-file cache). That stays dropped. This plan does something narrower: Vela
**distributes and enables mpv's own script**, which is an extension of the same
decision's endorsed escape hatch ("use mpv's own facilities via the
`mpv_extra_args` passthrough"). Vela writes no crop logic; it ships a file and
adds mpv `--script` launch args behind a toggle. The scope boundary "Vela does
not re-implement video-geometry processing" is preserved — the processing is
still 100% mpv's.

## Known risk carried forward (code-side guard, NOT disclosure-only)

`autocrop.lua` applies the crop via mpv's **live `video-crop` property** — the
exact mechanism that wedged the owner's gpu-next/Vulkan/Wayland/HDR stack into
unkillable D-state twice (2026-06-28 confirmed facts, still-valid evidence).
`autocrop.lua` defaults to `auto=true` (`autocrop.lua:30`), so it runs detection
and sets `video-crop` **automatically at every playback start** — i.e. Vela's
toggle would auto-fire the known-hang path on Vela's own default Linux launch
(gpu-next/Vulkan/HDR, `playback.rs:477-507`) with no user action. UI copy alone
does not gate that (codex r1, finding 3).

**OWNER DECISION — RESOLVED 2026-07-05:** the owner wants BOTH modes user-selectable,
so the setting is **three-state: Off (default) / Manual / Automatic**:
- **Off (default):** no `--script` injected. The D-state path cannot fire.
- **Manual:** inject `--script=<path>` **and** `--script-opts-append=autocrop-auto=no`
  — the script loads but only crops on an explicit in-player `Shift+C`. The
  dangerous action is a deliberate per-play keypress, matching the owner's observed
  usage.
- **Automatic:** inject `--script=<path>` with the script's own `auto=true` (crop
  detection + `video-crop` at every playback start). This is the mode that
  auto-fires the known D-state hang path (codex r1, finding 3) — it is therefore an
  **explicit, non-default, owner-chosen opt-in** carrying a prominent hang warning
  in its UI copy, NOT the shipped default. The `auto=no` code guard still protects
  the Manual mode; disclosure covers the Automatic mode the owner deliberately
  selects. Off remaining the default means nothing auto-fires unless the user picks
  Automatic knowing the warning.

## License

mpv is GPLv2+; `autocrop.lua` carries no per-file header, so it inherits mpv's
default GPL. A Lua script loaded at runtime by a **separate** mpv process is
aggregation, not linking — it does not relicense Vela's own code. But because
Vela now **redistributes** the file, we must ship the GPL text and record
provenance. Plan: vendor the script under `src-tauri/resources/mpv-scripts/`
alongside a `LICENSE.GPL` copy and a short `PROVENANCE.md` (source repo, commit,
"unmodified"). Do not edit the script; if a future change is needed, note the
modification per GPL. **Packaging (codex r2):** the GPL text must actually ship in
every package, not just live in the repo — the Tauri `resources` entry covers
the deb/rpm bundles, but the Arch `PKGBUILD` (which installs only Vela's MIT
`LICENSE`, `packaging/arch/PKGBUILD:30`) must add `LICENSE.GPL` + `PROVENANCE.md`
to `/usr/share/licenses/vela/` and add `GPL2` to the PKGBUILD `license=` array
alongside `MIT`.

**Package license METADATA must also change (codex r3):** today `Cargo.toml:6`
declares `license = "MIT"` and `tauri.conf.json`'s bundle block has no license
override, so the Tauri-generated deb/rpm package metadata declares MIT while the
package would ship a GPL script — a false license declaration. Set
`bundle.license` in `tauri.conf.json` to the combined SPDX expression
`MIT AND GPL-2.0-or-later` (matching the Arch `license=('MIT' 'GPL2')` bump) so
the deb/rpm metadata is truthful. This is distinct from shipping the LICENSE.GPL
*file* — both are required.

## Slices (one finding ↔ one commit; reviewloop codex on each)

### Slice 1 — Vendor the script + wire it into the bundle
- Add `src-tauri/resources/mpv-scripts/autocrop.lua` (verbatim copy from mpv
  `TOOLS/lua/autocrop.lua`), `LICENSE.GPL`, and `PROVENANCE.md`.
- Add a `resources` entry to `src-tauri/tauri.conf.json` so the folder ships in
  the Tauri-bundled deb/rpm targets (`build:linux` = `--bundles deb,rpm`; AppImage is not built by the standard script and is out of scope here). Use the **map form** so the shipped
  path matches the resolver below and does NOT keep the `resources/` prefix (codex
  r3, minor): `"resources": { "resources/mpv-scripts/": "mpv-scripts/" }` — a plain
  array like `["resources/mpv-scripts/autocrop.lua"]` would resolve at
  `resources/mpv-scripts/autocrop.lua`, which the resolver's `mpv-scripts/...`
  path would miss. The dev/bundle/Arch runtime checks below must confirm the
  chosen mapping actually resolves.
- Runtime path resolution (single pinned resolver; codex r1, finding 2): resolve
  `mpv-scripts/autocrop.lua` via `AppHandle::path().resolve("mpv-scripts/autocrop.lua",
  BaseDirectory::Resource)`. Do this in the command/state layer (where AppState's
  `app_handle` lives, `commands.rs:2939`) and pass the resolved absolute path down
  into the play spec — keeping `playback.rs` free of Tauri path concerns, matching
  how `spec` already carries computed values (headers, title, url). The
  `bundled_mpv()` naive exe-relative pattern (`playback.rs:264-278`) is NOT used.
- **THREE delivery modes must each land the file where the resolver looks (codex
  r2):** `BaseDirectory::Resource` resolves to a DIFFERENT location per mode, and
  the repo's Arch path does not use Tauri bundling at all —
  1. **dev** (`npm run tauri dev`): resource root is the dev resource dir.
  2. **Tauri bundle** (`npm run build:linux` → deb/rpm only): the `resources`
     entry ships the tree; resolver finds it under the packaged resource dir.
  3. **Arch** (`npm run build:arch` = `tauri build --no-bundle` + `makepkg`,
     `package.json:9`): `--no-bundle` SKIPS Tauri resource packaging, and
     `packaging/arch/PKGBUILD:22-30` installs only the binary + MIT LICENSE. So
     the Arch package must **explicitly** `install -Dm644` the script (and the
     GPL/provenance files) into the exact directory `BaseDirectory::Resource`
     resolves to for the installed `/usr/bin/vela` binary. Slice 1 must
     **empirically determine** that resolved path for the installed binary and
     install to match. If `BaseDirectory::Resource` cannot be made to point at a
     PKGBUILD-writable path for the `--no-bundle` binary, fall back to a Vela-owned
     resource resolver that probes a short list of KNOWN install paths (mirroring
     the repo's existing `mpv_candidates()` idiom, `playback.rs:281+`) — a
     well-known-system-path list, NOT the naive next-to-exe probe r1 rejected. Pin
     which of the two in implementation and record it.
- Verify all three modes resolve the path at runtime: dev, a real installed Tauri
  bundle, AND an installed Arch package (extract/install it and confirm the Rust
  resolver finds `mpv-scripts/autocrop.lua`). Required checks, not optional — Arch
  is the owner's own platform, so silent non-shipping there is the worst failure.

**Implementation notes (slice 1, landed 2026-07-05):**
- EMPIRICAL resolver path (resolves the r2 Arch open question): a `build:linux`
  deb/rpm build stages the mapped resources at `mpv-scripts/autocrop.lua` (the
  `resources/` prefix IS stripped by the map form — codex r3 minor confirmed
  correct) and the deb installs them to `/usr/lib/Vela/mpv-scripts/`. Tauri's
  `BaseDirectory::Resource` computes `/usr/lib/<productName>/` (productName=`Vela`)
  for a `/usr/bin/vela` binary, so the Arch `--no-bundle` install must target
  `/usr/lib/Vela/mpv-scripts/` — now wired into `packaging/arch/PKGBUILD`. No
  Vela-owned candidate-path resolver was needed; the pinned `BaseDirectory::Resource`
  approach works for all three modes.
- SLICING REFINEMENT: the runtime *resolver code* (Rust `AppHandle::path().resolve`)
  is deferred to slice 2, where it has a consumer (the injection) and can be
  exercised end-to-end; slice 1 is packaging-only (vendored files + `tauri.conf.json`
  + `PKGBUILD`), which is independently verifiable by the package builds.
- VERIFICATION STATUS: deb confirmed to ship autocrop.lua + LICENSE.GPL +
  PROVENANCE.md at `/usr/lib/Vela/mpv-scripts/`; `bundle.license` accepted
  (Tauri v2 `BundleConfig.license`). rpm `License` tag NOT locally verifiable (no
  rpm tooling on the Arch dev host) — relying on the documented `bundle.license`
  field. Arch package build verification pending.

### Slice 2 — Config field + launch injection
- Add a tri-state `mpv_autocrop` mode to `config.rs` (e.g. an
  `Option<String>`/enum serialized as `"off" | "manual" | "auto"`, default `off`;
  keep it forward/back-compatible with a missing value = `off`; preserve the
  config's defensive save/permissions). Mirror the plumbing of `mpv_use_own_config`.
- Thread it through `MpvAdvanced` (`commands.rs:1542`) and
  `get_mpv_advanced`/`set_mpv_advanced` (`:1548`, `:1560`).
- In `spawn_mpv` (`playback.rs`, alongside the extra-args append at `:510-519`),
  branch on the mode, only if the script path actually resolves (skip + log if
  missing rather than passing a bad `--script=` that mpv rejects):
  - `off` → inject nothing.
  - `manual` → `--script=<path>` **and** `--script-opts-append=autocrop-auto=no`.
  - `auto` → `--script=<path>` (script's own `auto=true`; do NOT append
    `autocrop-auto=no`).
  Place them with the user extra args (before the re-asserted IPC/title/URL block
  so they can't clobber the socket).
- Guard tests (Rust): `manual` → argv contains `--script=<path>` AND
  `--script-opts-append=autocrop-auto=no`; `auto` → argv contains `--script=<path>`
  and NOT `autocrop-auto=no`; `off` → argv contains neither. Red→green proven.

### Slice 3 — Settings UI (three-state control)
- Add a three-state control to `src/lib/Settings.svelte` in the mpv/Advanced block
  (radio group or select — Off / Manual (Shift+C) / Automatic), bound through the
  extended `set_mpv_advanced`. Honest labels + per-mode helper text:
  - **Off** — default.
  - **Manual (Shift+C)** — "Loads mpv's autocrop script; press Shift+C during
    playback to crop black bars. Nothing happens automatically."
  - **Automatic** — MUST carry the prominent warning: "Crops automatically at the
    start of every video. May be unreliable on HDR content and can occasionally
    hang mpv (unkillable) on this graphics stack — if playback freezes, switch back
    to Off/Manual." (the owner-selected, non-default risky mode.)
- Verify the control round-trips all three states (set → persisted → reflected on
  reload) and that each maps to the correct injection from slice 2.

## Verification
- Rust: the argv guard test (slice 2), red→green.
- E2E (if practical this pass): drive the toggle on and assert the launched
  mpv's args include `--script=` via the driver/probe. Actual cropping behavior
  (visual, HDR) stays an owner playtest — automation can't judge it.
- Owner playtest: toggle on, play a letterboxed file, press `Shift+C`, confirm
  the crop applies and that the disclosed D-state hang risk is acceptable in
  practice. (With the `auto=no` guard, cropping does not fire automatically at
  start — `Shift+C` is the trigger.)
- Full CI set for a both-sides change: `npm run check`, `npm run build`,
  `cargo check --locked`, `cargo clippy --all-targets --locked -- -D warnings`,
  `cargo test --locked`.
- Packaging (required, codex r2 + r3): `npm run build:linux` AND
  `npm run build:arch`, then for EACH resulting package extract/install it and
  confirm (a) the resolved `mpv-scripts/autocrop.lua` path the Rust resolver uses
  exists, (b) `LICENSE.GPL` is present in the install, and (c) the package's
  declared license **metadata** reads `MIT AND GPL-2.0-or-later` (inspect the
  deb/rpm metadata and the Arch `.PKGINFO`), not MIT alone. Arch is the owner's
  platform and uses `--no-bundle`, so its packaging step is the one most likely to
  silently drop the file — verify it explicitly. Bump version per landed code
  slice (routine).

## Non-goals
- No Vela-authored crop logic, scan, cache, or correction mechanism (the dropped
  feature stays dropped).
- No fetch-on-demand / network download of the script (option B, rejected).
- No modification of the vendored script.
- macOS/Windows resource bundling is included by the `resources` entry but the
  D-state risk note and owner playtest are Linux-stack specific; cross-platform
  crop behavior is not validated here.

## Proposed decision-log entry (lands on approval)

> ### 2026-07-05 - Ship mpv's autocrop.lua behind an opt-in toggle
> Status: Active
> Decision: Vela bundles mpv's unmodified `autocrop.lua` (GPLv2+, provenance
> recorded) as a resource and adds an **off-by-default, three-state** Settings
> control (Off / Manual / Automatic) that injects mpv `--script` launch args.
> Off injects nothing. Manual appends `--script=<bundled>` plus
> `--script-opts-append=autocrop-auto=no`, so cropping fires only on an explicit
> in-player `Shift+C`. Automatic appends `--script=<bundled>` with the script's own
> `auto=true` (crop at every playback start). Vela writes no crop logic; all
> geometry processing remains mpv's. The Automatic mode auto-fires the recorded
> live-`video-crop` D-state hang path and is therefore an explicit, non-default,
> owner-chosen opt-in guarded by a prominent UI warning; Off/Manual carry the
> `auto=no` code guard. Owner directed 2026-07-05 that both modes be user-selectable.
> Reason: Owner reversed the drop after confirming the script works via manual
> `--script=`/Shift+C, and wanted it distributable without users hand-managing
> the file. Bundling + a toggle is an extension of the prior decision's endorsed
> `mpv_extra_args` passthrough, not a re-implementation.
> Supersedes: The 2026-07-05 "Letterbox crop feature DROPPED" decision for this
> narrow bundled-script case — specifically its "ships no crop feature", "no
> design/no code", and "existing `mpv_extra_args` passthrough only" clauses, which
> no longer hold now that Vela ships and enables mpv's own crop script behind a
> toggle. What REMAINS in force from that entry (explicitly not superseded): the
> scope boundary that "Vela launches and controls mpv but does not re-implement
> video-geometry processing" (this plan writes no geometry code), and the
> 2026-06-28 confirmed facts (live `video-crop` D-state wedge; unreliable
> cropdetect on HDR/PQ) — which are the reason for both the `auto=no` guard and
> the warning copy.

## Review log
- **r1** 2026-07-05 `codex` (codex-cli 0.142.5), reviewed the plan against code at
  HEAD `7c1ef0b`: **reopened**, 3 majors (`checked_against_code: true`), all
  admitted and addressed in this revision:
  1. Decision supersession too narrow — broadened to supersede the prior drop's
     "ships no crop feature / no design-no code / passthrough-only" clauses for
     this narrow bundled-script case, while preserving the geometry-processing
     boundary and the 2026-06-28 D-state facts.
  2. Exe-relative resource fallback unsound for packaged apps — removed; slice 1
     now pins a single `BaseDirectory::Resource` resolver (`mpv-scripts/autocrop.lua`)
     with required dev + real-bundle verification.
  3. Disclosure-only mitigation insufficient for the auto-fired D-state path —
     added a code-side guard: inject `--script-opts-append=autocrop-auto=no` so
     cropping only fires on explicit `Shift+C`; auto-crop-on-start deferred to a
     separate owner opt-in. Slice 2 guard test and slice 3 toggle copy updated.
- **r2** 2026-07-05 `codex`, reviewed the revised plan against code at HEAD
  `7c1ef0b`: **reopened**, 1 major (r1 findings confirmed resolved) — the plan
  relied on the Tauri `resources` entry to ship the script, but `build:arch`
  (`package.json:9`) runs `tauri build --no-bundle` and `packaging/arch/PKGBUILD`
  installs only the binary + MIT LICENSE, so the script (and GPL text) would not
  ship on Arch and `BaseDirectory::Resource` would resolve to nothing there.
  Admitted (verified against `package.json:9` + `PKGBUILD:22-30`): slice 1 now
  enumerates all three delivery modes (dev / Tauri bundle / Arch `--no-bundle`)
  with an explicit Arch PKGBUILD install of the script + GPL + provenance into the
  resolver's path (or a known-path resolver fallback), `license=` metadata bump,
  and per-package extract/install verification in the Verification section.
- **r3** 2026-07-05 `codex`, reviewed the revised plan against code at HEAD
  `7c1ef0b`: **reopened**, 1 major + 1 minor (r1/r2 findings confirmed resolved) —
  (major) Tauri bundle license metadata stays MIT-only (`Cargo.toml:6` = MIT, no
  `bundle.license` override) while the deb/rpm bundle would ship a GPL script, a
  false declaration; (minor) a plain `resources` array keeps the `resources/`
  prefix so the pinned resolver path would miss the file. Both admitted (verified
  against `Cargo.toml:6` + `tauri.conf.json:30`): License section now requires
  `bundle.license = "MIT AND GPL-2.0-or-later"`; slice 1 pins the `resources` map
  form (`resources/mpv-scripts/` → `mpv-scripts/`); Verification now inspects
  package license metadata, not just LICENSE.GPL presence.
- **r4** 2026-07-05 `codex`, reviewed the revised plan against code at HEAD
  `7c1ef0b`: **accepted** (`checked_against_code: true`), 1 minor (r1/r2/r3 all
  confirmed resolved) — `build:linux` is `tauri build --bundles deb,rpm`
  (`package.json:10`), so it does not build AppImage; the plan had referenced
  deb/rpm/AppImage. Fixed: AppImage dropped from scope/verification; the plan now
  says deb/rpm only. **Plan converged.** Awaiting owner approval to implement.
