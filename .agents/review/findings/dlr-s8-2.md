# dlr-s8-2: the E2E harness keeps an obsolete skewed WebKit driver fixture

**Severity**: MEDIUM — the harness deliberately runs a mismatched browser
driver even though the validation OS now publishes an exact compatible match.
**Status**: Verified — external reviewer Grok accepted r1
**Branch**: `main` (approved dependency-refresh Slice 8)
**Commit**: `ec7c43e`

## Evidence

`tests/e2e/fetch-driver.sh` pinned Debian WebKitWebDriver 2.50.6 and three ICU
72 libraries because no distribution packaged a driver matching WebKitGTK
2.52 in July 2026. Ubuntu 26.04 now publishes `webkitgtk-webdriver`
2.52.3-0ubuntu0.26.04.2 for ARM64 and AMD64. The ARM64 package exactly matches
the VM's installed WebKitGTK 2.52.3 and passed an isolated no-install Vela
session/IPC/UI/screenshot handshake.

## Predicted observable failure

A future WebKit 2.52 protocol change can break the deliberately skewed 2.50.6
driver while Vela's harness continues reusing its old gitignored cache and
never attempts the now-available matching binary. That would strand the full
Linux UI gate on avoidable driver skew.

## What

Replace the Debian 2.50.6/ICU72 fetch with SHA-pinned official Ubuntu 2.52.3
packages matching the E2E venue, and invalidate pre-update cached drivers.

## Approach

Select the official Ubuntu archive host and SHA per supported architecture,
extract the `data.tar.zst` driver payload, and write the exact package filename
as a cache stamp. An executable without that stamp (including every old cache)
is refreshed. The wrapper directly executes the matching system-linked driver;
the obsolete ICU72 library injection is removed.

## Files changed

- `tests/e2e/fetch-driver.sh` — current package URLs, checksums, extraction, and
  cache identity.
- `tests/e2e/wkdriver-wrapper.sh` — direct execution without ICU72 injection.
- `.agents/plans/e2e-harness.md` — durable amendment to the superseded sourcing
  rationale.

## Guard proof

- Before the first new-script run, the VM cache had no package stamp and its
  driver SHA256 was `e682e150…`. Running the committed fetch script without
  `--force` replaced it with driver SHA256 `7f0bc618…`, wrote the exact ARM64
  package name to `.package`, removed the old `lib/` shim directory, and left
  no unresolved `ldd` dependency.
- Official ARM64 and AMD64 package downloads independently matched the two new
  manifest SHA256 values. Changing only the ARM64 expected SHA in a disposable
  committed worktree made `sha256sum` exit 1 with `FAILED`; no driver payload
  was installed. Restoring the SHA left that worktree byte-clean before removal.
- The full Linux real-app suite passed 18/18 with the new cached driver. The
  earlier isolated ARM64 probe also proved session creation, Tauri IPC/JS,
  element find/click, and screenshot behavior against WebKitGTK 2.52.3.

## Coder dispute (if any)

None. The approved Slice 8 audit explicitly requires replacement when a newer
packaged compatible driver exists and passes the handshake.

## Known gaps

ARM64 is runtime-proven. AMD64 has official package-index, URL, checksum, and
payload-format proof only because no current AMD64 E2E checkout is available.
The Ubuntu package requires a glibc 2.38/ICU78-era system; that is proven on the
approved Ubuntu 26.04 venue, while other Linux distributions remain best-effort.

## Reviewer comments

**r1 — 2026-07-15T18:24:31Z — accepted.** Grok 0.2.101 independently
reviewed exact base `76c844c` and head `f3e5601`, verified both official
package URLs/SHA256 values and payloads, proved an unstamped cache refresh,
injected a wrong ARM64 checksum and observed fail-closed behavior, restored
its disposable worktree clean, and returned `guard_confirmed: true` with no
comments.
