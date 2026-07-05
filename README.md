# Vela

A native, HDR-capable media client for Linux, macOS, and Windows. It browses
**Plex, Jellyfin, Emby, and local/SMB** libraries in a custom UI — unified
across sources or one at a time — and plays video through **mpv** in its own
window, which is the reliable way to get true HDR passthrough (10-bit PQ/BT.2020
negotiated with the display).

## Architecture

- **UI:** [Tauri 2](https://tauri.app) + SvelteKit (TypeScript). The frontend
  (`src/`) talks to a Rust backend (`src-tauri/`) over Tauri commands.
- **Sources:** each backend implements a `MediaSource` trait (`src-tauri/src/source/`)
  behind a `SourceRegistry`. Plex (`plex.rs`), Jellyfin/Emby (`jellyfin.rs`, one
  client with a `Flavor` for the small differences), and a local-folder source
  (`local.rs`) with keyless metadata (`metadata.rs`). Item keys are
  source-namespaced; commands fan out for the unified view or scope to one source.
- **Playback:** the system **`mpv`** binary is launched as its own window. HDR is
  negotiated by mpv (`--vo=gpu-next --target-colorspace-hint=yes`); the app does
  not embed video in the webview (that would force SDR). The GPU backend is
  chosen per platform — Vulkan over the Wayland/X11 WSI on Linux, Vulkan over
  Metal (`macvk`) on macOS, and D3D11 (DXGI HDR) on Windows.
- **Progress/Resume:** tracked over mpv's JSON IPC channel (a Unix domain socket
  on Linux/macOS, an emulated named pipe on Windows) and reported back to the
  server — Plex timelines, Jellyfin/Emby playback check-ins — so resume works
  across sessions. Local files play without progress tracking.
- **Local metadata:** sidecar `.nfo` + local artwork first, then keyless online
  lookup (iTunes Search for movies/shows, TVmaze for episodes; cached), then the
  filename as the floor.
- **SMB:** on Linux, Vela speaks SMB natively in-process (libsmbclient) —
  no mounts, no root, nothing to set up; browsing lists over the wire and
  playback streams to mpv through a localhost-only HTTP range proxy.
  Sidecar posters are served over a private `velasmb:` scheme. Requires
  the `smbclient` package (libsmbclient). On macOS/Windows the share is
  mounted by the app via the OS (`mount_smbfs` / `net use`) and browsed
  through the local source.

## Requirements

Vela exists to get **HDR on Wayland**, so it targets a reasonably modern GPU and an HDR
display. The hardware below is what the HDR experience expects. Vela won't stop you
running it on weaker hardware — it just won't perform well, and HDR won't engage. Your
call.

**Display + session**
- An **HDR-capable display in HDR mode** (HDR10/PQ).
- **Linux:** a **Wayland** compositor with HDR/color-management — **KDE Plasma 6+** or a
  recent **Hyprland**. X11 is unsupported for HDR.
- **macOS:** an EDR-capable display (most modern Macs / XDR displays).
- **Windows:** Windows 10/11 with HDR enabled.

**GPU (working Vulkan + HDR output)**
- **AMD:** Radeon RX 400 / Polaris or newer on the `amdgpu` driver. RDNA or newer
  recommended for 4K HDR. Legacy `radeon`-driver GPUs, including TeraScale / HD 5000/6000
  and older GCN cards requiring manual `amdgpu` enablement, are unsupported.
- **NVIDIA:** Maxwell / GTX 900 or newer with proprietary driver 535+; RTX-class
  recommended for 4K HDR.
- **Intel:** Skylake / Gen9 or newer; Arc/Xe recommended for 4K HDR.

> Note: the low end of "supported" (e.g. a 2015 iGPU) may not sustain **4K60 HDR** with
> the full `gpu-hq` profile. 1080p HDR is the realistic floor on such parts.

**Player**
- **`mpv` 0.38+** (required for `gpu-next`/libplacebo HDR). Vela can install a
  current build for you on first run, auto-detects common install locations, and
  lets you point Settings → Player at a custom mpv executable.
- Tooling to build: Rust and Node.js, plus the platform's Tauri prerequisites
  (Linux: `webkit2gtk-4.1`, `libsoup-3.0`; macOS: Xcode Command Line Tools;
  Windows: WebView2 runtime + MSVC Build Tools). See the
  [Tauri prerequisites](https://tauri.app/start/prerequisites/).

## Run (development)

```bash
npm install
npm run tauri dev
```

On first launch you'll get a `plex.tv/link` code (with a QR/clickable link) to
authorize a Plex account. Use the **⚙ Sources** panel to add more: Jellyfin/Emby
servers (username + password, or an API key), local folders, or SMB shares. The
header source switcher toggles between the unified view and a single source.
Click a movie to play, or drill into a show → season → episode.

> NVIDIA + Wayland (Linux only): the app disables WebKitGTK's DMABUF renderer
> (`WEBKIT_DISABLE_DMABUF_RENDERER=1`) at startup to avoid a known webview crash.
> This has no effect on macOS (WKWebView) or Windows (WebView2).

## Test

```bash
npm run check   # svelte-check (frontend types)
cd src-tauri && cargo test --locked && cargo clippy --all-targets --locked -- -D warnings
```

End-to-end tests drive the real debug app (Linux only) — WebDriver for the
UI, mpv's JSON IPC for playback — headless on a private Xvfb display, with
a throwaway config so your real `~/.config/vela` is never touched:

```bash
npm run e2e                    # build the debug app, run all scenarios
npm run e2e -- --skip-build    # reuse the existing debug binary
npm run e2e -- playback        # one scenario by name
```

Requires `tauri-driver` (`cargo install tauri-driver`), `Xvfb`, `ffmpeg`,
`mpv`, `bsdtar`, and `curl`; the first run downloads a pinned
WebKitWebDriver into a gitignored vendor dir. Screenshots and driver logs
land in `tests/e2e/artifacts/`. Knobs: `VELA_E2E_HEADED=1` runs on the
real desktop, `VELA_E2E_DEBUG=1` logs each WebDriver call with timing.
Details: `tests/e2e/README.md`.

## Build (release)

```bash
npm run tauri build
```

Linux release installers can be built directly:

```bash
npm run build:linux
```

The Linux build emits `.deb` and `.rpm` artifacts under
`src-tauri/target/release/bundle/`. These installers register Vela through the
freedesktop application database, installing the desktop entry under
`/usr/share/applications` and icons under `/usr/share/icons/hicolor`, so GNOME
and KDE show Vela in their application launchers after install.

On Arch Linux, build a native pacman package instead:

```bash
npm run build:arch
```

This emits `packaging/arch/vela-<version>-1-x86_64.pkg.tar.zst` from the local
checkout using the PKGBUILD in `packaging/arch/`. It installs the same desktop
entry and hicolor icons, and pacman's desktop/icon hooks refresh the launchers
when the package is installed.

## Configuration

Config (including the Plex auth token) is stored in the platform config dir:
`~/.config/vela/config.json` on Linux,
`~/Library/Application Support/com.vela.vela/config.json`
on macOS, and `%APPDATA%\vela\vela\config\config.json`
on Windows. On Unix it is written `0600`.

## Status

Builds and runs on Linux, macOS, and Windows (the mpv IPC layer is
platform-abstracted: Unix domain socket on Linux/macOS, named pipe on Windows).

Working: Plex device-PIN auth + server discovery, multi-source library browsing
(unified or per-source) with infinite scroll, show/season/episode drill-down,
search, mpv HDR playback, Plex progress/resume, local-folder indexing with
keyless metadata, and an in-app source manager (Jellyfin/Emby connect, local
folders, SMB shares — native on Linux, OS-mounted on macOS/Windows).

Verification note: Plex is exercised end-to-end, and Jellyfin has been
smoke-tested against a real server. The Emby, local, and SMB paths are
implemented and unit-tested where logic allows, but live integration against
real servers/shares is still pending.

Known limitations: Plex/Jellyfin/Emby media-version selection is heuristic: Vela
prefers direct-play/direct-stream candidates, HDR, higher resolution, and higher
bitrate where the source exposes that metadata, but it does not yet offer a
manual version picker. Server stream/poster URLs carry the access token
(Plex/Jellyfin/Emby), so the token is visible locally — in the webview DOM and
in mpv's process arguments. This is an accepted **local-only** exposure (your
own machine, not the network); there is no token proxy. SMB credentials are
stored in the (`0600` on Unix) config so shares reconnect on launch. On
Linux they never leave the process (libsmbclient auth callback — no
process arguments, no URLs); on macOS/Windows they are passed to the OS
mount tool (`mount_smbfs` / `net use`) and appear briefly in process
arguments — same accepted local-only exposure. The Linux playback proxy
binds 127.0.0.1 only and its URLs carry an unguessable per-play token,
never paths or credentials. HDR fidelity
depends on the platform's mpv GPU backend and display support.
