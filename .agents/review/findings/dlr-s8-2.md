# dlr-s8-2: the E2E harness keeps an obsolete skewed WebKit driver fixture

**Severity**: MEDIUM — the harness deliberately runs a mismatched browser
driver even though the validation OS now publishes an exact compatible match.
**Status**: In progress
**Branch**: `main` (approved dependency-refresh Slice 8)
**Commit**: pending

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

Pending: prove an existing unstamped Debian cache is replaced by the expected
Ubuntu driver, verify the downloaded package/driver identity and system linkage,
then run the full Linux E2E suite. Separately inject a wrong SHA in a disposable
copy and require the checksum gate to fail before extraction.

## Coder dispute (if any)

None. The approved Slice 8 audit explicitly requires replacement when a newer
packaged compatible driver exists and passes the handshake.

## Known gaps

ARM64 is runtime-proven. AMD64 has official package-index, URL, checksum, and
payload-format proof only because no current AMD64 E2E checkout is available.
The Ubuntu package requires a glibc 2.38/ICU78-era system; that is proven on the
approved Ubuntu 26.04 venue, while other Linux distributions remain best-effort.

## Reviewer comments

Pending external Grok review.
