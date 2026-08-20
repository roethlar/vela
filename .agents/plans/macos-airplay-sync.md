# Plan: Automatic macOS AirPlay A/V synchronization

## Status

**Drafted 2026-08-20 at the owner's request; implementation is not approved.**
The owner rejected a global `--audio-delay=-2.0` workaround because it requires
manual edits whenever the system output changes. The required product behavior
is automatic compensation when macOS routes mpv to AirPlay/HomePod, automatic
removal on other outputs, and no per-route user action. Independent Claude
openreview is pending.

Three recommended details remain part of the plan rather than settled owner
decisions until the owner approves it:

- manage route changes during an active play, not only the route at launch;
- use 2,000 ms as the enabled-by-default AirPlay compensation, with a one-time
  adjustable value in Settings;
- preserve expert ownership when advanced mpv options explicitly select an
  audio device or set `--audio-delay`.

## Goal and acceptance contract

On macOS, ordinary Vela playback follows the current system output. If that
output is an AirPlay device (including a HomePod exposed by CoreAudio as
AirPlay), Vela delays video by the configured compensation. If the output is
not AirPlay, Vela applies no compensation. Vela reevaluates the route for every
play and while mpv remains alive, so changing output before or during playback
does not require editing Settings or restarting Vela.

Acceptance requires all of these independently proved behaviors:

1. Starting on AirPlay launches mpv with the configured negative mpv
   `audio-delay` (positive user-facing "video delay" maps to mpv's negative
   value).
2. Starting on local, USB, HDMI/DisplayPort, Bluetooth, aggregate, virtual, or
   unknown output does not apply the AirPlay compensation.
3. During one live mpv session, AirPlay -> non-AirPlay sets mpv's
   `audio-delay` to `0`, and non-AirPlay -> AirPlay sets it to the configured
   negative value without relaunching mpv.
4. A transient CoreAudio query failure never invents an AirPlay route. At
   launch it falls back to zero compensation; during playback it retains the
   last known value and performs bounded retries.
5. Closing/replacing mpv or exiting Vela terminates the route monitor and
   unregisters its CoreAudio listener; completed plays leave no accumulating
   threads or callbacks.
6. Windows and Linux launch arguments and playback behavior are byte-for-byte
   unaffected by the feature policy.
7. A user can calibrate the AirPlay value once; switching devices thereafter
   remains automatic.

## Evidence and constraints

- Vela deliberately launches the system mpv out of process from
  `src-tauri/src/playback.rs`; that product boundary remains unchanged.
- User-supplied `mpv_extra_args` are appended after Vela defaults, and the UI
  promises that they override those defaults. The feature must preserve that
  contract rather than silently fighting an explicit `--audio-delay`.
- On the affected Mac, macOS reports the current default output as `AirPlay`
  with CoreAudio transport `airplay`; mpv 0.41 selects that device but reports
  only about 11.6 ms of CoreAudio device latency. That omits the observed
  roughly two-second wireless presentation buffer, so mpv cannot correct this
  route from its current latency measurement alone.
- mpv defines positive `audio-delay` as delaying audio and negative values as
  delaying video. A user-facing 2.0-second AirPlay video compensation therefore
  becomes mpv property value `-2.0`.
- The runtime must not shell out to `system_profiler`, parse localized device
  names, or assume names contain "AirPlay" or "HomePod". CoreAudio publishes a
  stable transport-type property for this purpose.
- Existing playback IPC supports multiple independent clients and already uses
  one stop flag per play. The new monitor may own a separate IPC connection;
  it must not consume or serialize through the progress/EOF clients.
- The config document uses `#[serde(default, deny_unknown_fields)]`, validates
  before use/save, preserves missing fields for backward compatibility, and
  must continue failing closed on invalid persisted settings.

## Design

### 1. Pure policy and persisted setting

Add a macOS AirPlay compensation setting to `AppConfig`, stored as a positive
integer number of milliseconds so JSON cannot contain NaN/infinity and command
construction avoids floating-point validation ambiguity:

- field: `airplay_video_delay_ms: Option<u16>`;
- missing value resolves to `2_000` ms;
- accepted persisted/UI range: `0..=5_000` ms;
- zero explicitly disables compensation while retaining automatic route
  detection;
- serialize `None` for the product default so existing and fresh configs stay
  sparse.

Extend `MpvAdvanced`, `get_mpv_advanced`, and `set_mpv_advanced` with the
resolved millisecond value. The new setter argument is optional: omission by
the slice-1 frontend preserves the existing field, while slice 2 begins sending
the explicit value. Reject out-of-range command values before entering
`config::update`; validate the persisted field in `AppConfig::validate` so a
manually corrupted value fails closed like other invalid settings.

Keep route-independent policy in a small testable module. It accepts the
resolved setting, current route classification, last applied value, and expert
override classification, and returns one of:

- `Set(delay)` for a known route whose desired value changed;
- `Keep` for an unknown/transient route during a live play;
- `Unmanaged` when an explicit expert override owns sync/output selection.

At launch, `Unknown` resolves to `Set(0)` because no previous trustworthy route
exists. Convert milliseconds to seconds only at the final mpv boundary.

### 2. CoreAudio route adapter

Add macOS-only dependency `objc2-core-audio` in the existing objc2 0.6 / Apple
framework dependency family, with only its `AudioHardware` feature. Put all
unsafe framework calls behind `src-tauri/src/macos_audio.rs`; non-macOS code
must not compile or link the dependency.

The synchronous detector performs two typed CoreAudio property reads:

1. query `kAudioObjectSystemObject` for
   `kAudioHardwarePropertyDefaultOutputDevice`;
2. query that `AudioDeviceID` for `kAudioDevicePropertyTransportType` and
   compare it with `kAudioDeviceTransportTypeAirPlay`.

Return `AirPlay`, `Other`, or `Unknown`; never classify by device name. Treat a
missing device, unsupported property, wrong data size, or nonzero `OSStatus` as
`Unknown`. Do not log device names or UIDs.

Register `AudioObjectAddPropertyListener` on the system object's default-output
property for live changes. The C callback must be panic-free and nonblocking:
it only marks an atomic dirty flag and wakes a Rust monitor. The owning monitor
keeps callback state alive, calls `AudioObjectRemovePropertyListener` before
dropping it, and documents every unsafe lifetime invariant next to the call.
If registration fails, launch-time compensation still works and playback
continues; emit one generic diagnostic without output identity.

### 3. Launch-time wiring and expert overrides

Before spawning mpv, load config once as Vela already does, parse advanced
arguments once, and classify explicit expert ownership:

- any effective `--audio-delay=...` disables Vela-managed AirPlay compensation
  for that play;
- any effective `--audio-device=...` other than `auto` disables it because mpv
  is no longer guaranteed to follow the macOS default route;
- when duplicate options exist, inspect the last effective value, matching
  mpv's last-option-wins behavior;
- `--audio-device=auto` keeps automatic management enabled.

Do not disable the feature merely because "Use my own mpv config" is enabled:
Vela's command-line defaults already override config-file values, and the
documented advanced-argument escape hatch remains authoritative. State this
precedence in Settings help.

When managed, query CoreAudio immediately before command construction and add
`--audio-delay=<seconds>` before user extra arguments. This order preserves the
existing promise that a user argument wins. Always pass an explicit zero for a
known non-AirPlay route so an own `mpv.conf` delay cannot leave ordinary
system-output playback offset; the explicit advanced argument remains the
supported override.

Extract command application into a helper taking an injected route result so
Rust tests can inspect `Command::get_args`. Add a separate wiring guard proving
the real `play` path invokes that helper; red-proof it by deleting only the
call, observing the guard fail for the missing launch wiring, then restoring.

### 4. Live route monitor over mpv IPC

After mpv is registered and its IPC endpoint exists, start one macOS-only
monitor for managed plays. It owns:

- the CoreAudio default-output listener;
- the play's stop flag;
- a dedicated mpv IPC connection; and
- the last successfully applied delay.

The monitor connects with the existing bounded IPC startup retry. Its own
connection is nonblocking or uses a short read timeout so it can both detect
mpv EOF and react to route notifications. Immediately after registering the
listener it re-queries and applies the current route once, closing the race
between the pre-spawn query and live monitoring. On a known route transition it
sends one JSON IPC `set_property`, `audio-delay`, `<seconds>` command. It drains
only its own command replies/events. IPC EOF, the play stop flag, or Vela
shutdown ends the loop and unregisters the listener.

CoreAudio can briefly expose no default device during a switch. Coalesce
notifications, debounce the first read, and retry `Unknown` on a short bounded
schedule. Preserve the last applied delay during that window; after retries are
exhausted, keep the last value and wait for the next OS notification. Never
poll continuously and never change playback if the route remains unknown.

A monitor spawn/listener failure is nonfatal: retain correct launch-time
behavior and log one generic diagnostic. A post-launch IPC write failure means
mpv is gone or its control channel failed; stop the monitor rather than spin.

### 5. Settings and user-facing behavior

In Settings -> Player -> Advanced mpv options, show a macOS-only numeric control
named **AirPlay video delay**:

- display seconds with 0.05-second steps;
- default 2.00 seconds, accepted range 0.00-5.00;
- explain that Vela applies it automatically only while system output is
  AirPlay/HomePod and removes it on other outputs;
- explain that zero disables compensation and explicit `--audio-delay` or a
  non-auto `--audio-device` advanced option takes ownership for that play.

Save through the existing single advanced-settings command so one save remains
atomic. Do not expose a per-device toggle or require restart. Update
`docs/usage.md` with the same behavior and precedence, without presenting the
old global argument workaround as the product solution.

## Delivery slices

### Slice 1 - complete backend behavior

- Add config field, validation, resolved default, command DTO/setter coverage,
  pure policy, and expert-argument classification.
- Add the macOS CoreAudio detector/listener adapter and target-only dependency.
- Wire launch-time arguments and the live IPC monitor.
- Add all Rust policy, config, argument, monitor-lifetime, and IPC transition
  guards listed below.
- Keep default behavior automatic at 2,000 ms even before the UI control lands;
  this slice is therefore complete and useful rather than dead configuration.
- Update `.agents/state.md` and commit the slice with its guard proof.

### Slice 2 - calibration UI and documentation

- Extend the Settings TypeScript shape/state/save flow and render the macOS-only
  control with honest precedence copy.
- Add frontend guards for resolved load, validation/save payload, and platform
  visibility.
- Update `docs/usage.md`, `.agents/state.md`, and this plan's evidence/status.
- Run packaged macOS acceptance, then commit UI/docs and evidence together.

## Verification and red proof

### Automated behavior guards

- Config: missing -> 2,000; zero and 5,000 accepted; 5,001 rejected; setter
  rejection leaves the prior durable config byte-for-byte unchanged.
- Policy table: AirPlay/non-AirPlay/unknown at launch; AirPlay -> other and other
  -> AirPlay live transitions; repeated same route emits no duplicate IPC set;
  unknown preserves the last live value.
- Expert precedence: last `--audio-delay` wins ownership; explicit non-auto
  `--audio-device` disables management; `--audio-device=auto` does not; comments
  and unrelated options do not trigger either rule.
- Command argv: managed AirPlay emits the negative configured seconds before
  extra args; managed other emits zero; unmanaged emits no Vela sync argument;
  Linux/Windows helper paths emit none.
- Monitor integration with injected route events and a fake mpv Unix socket:
  assert exact JSON commands for AirPlay and reset-to-zero, no command for
  unknown/repeated routes, termination on IPC EOF, and listener cleanup on
  every exit path.
- Wiring: the real `play` path invokes both initial command configuration and
  monitor startup under macOS management.
- Frontend: control loads 2.00 from 2,000 ms, saves rounded validated
  milliseconds, rejects out-of-range input without invoking backend, and is not
  rendered on non-macOS.

Red-proof each claimed behavior separately. At minimum inject and restore:

1. invert the AirPlay delay sign;
2. remove the non-AirPlay reset-to-zero command;
3. remove IPC-EOF monitor termination;
4. make `Unknown` reset to zero during a live route switch;
5. remove the real `play` wiring call; and
6. bypass the expert `--audio-delay` override.

Each corresponding guard must fail for the exact intended reason before the
slice is called complete.

### Canonical and platform checks

Run the full both-sides command set from `.agents/repo-guidance.md`: exact JS
toolchain check, `npm ci`, both audits, `npm run check`, `npm run build`, Rust
1.89 and stable checks, stable clippy/tests, and Cargo audit. Run focused Rust
and frontend guards during development. On macOS also run the signed universal
wrapper so the target-only framework link is proved in the actual bundle.

Packaged owner acceptance on the signed app and mpv 0.41+:

1. remove any old global audio-delay workaround;
2. start a known lip-sync clip on Mac speakers and verify mpv IPC reports
   `audio-delay=0`;
3. switch to AirPlay/HomePod during the same play and verify IPC reports the
   configured negative value and perceived sync is correct;
4. switch back during the same play and verify IPC returns to zero and sync is
   correct;
5. begin new plays on each route to prove launch-time behavior independently;
6. close mpv after repeated switches and verify no named monitor thread or
   CoreAudio callback remains; and
7. set an explicit advanced `--audio-delay`, repeat a route switch, and verify
   Vela does not overwrite it.

Record the tested device kind, configured milliseconds, mpv version, signed
artifact hash, and observed results in `.agents/machines.md`; keep device names
out of portable state.

## Non-goals

- No embedded player or libmpv migration.
- No global `mpv.conf` edit and no requirement that users toggle advanced args
  when outputs change.
- No device-name matching, `system_profiler` subprocess, periodic process-wide
  polling, or assumption that Bluetooth has AirPlay's delay.
- No attempt to compensate a deliberately pinned custom mpv audio device; the
  explicit advanced configuration owns that path.
- No server, transcoding, watched-state, or media-timestamp changes.
- No claim that exactly 2,000 ms fits every receiver; the automatic route
  behavior is fixed, while the one-time value remains calibratable.

## Review log

- Pending: independent `openreview claude` over the committed plan range.
