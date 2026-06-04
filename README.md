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
- **SMB:** mounted by the app via the OS (`mount_smbfs` on macOS, `net use` on
  Windows), then browsed through the local source. On Linux, mount the share
  yourself and add the path as a local folder.

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
- **`mpv` 0.38+** on `PATH` (required for `gpu-next`/libplacebo HDR). Vela can install a
  current build for you on first run. On macOS: `brew install mpv`. On Windows: install
  mpv and add it to `PATH`.
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

## Build (release)

```bash
npm run tauri build
```

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
folders, SMB mounting).

Verification note: Plex is exercised end-to-end. The Jellyfin/Emby, local, and
SMB paths are implemented and unit-tested where logic allows, but live
integration against real servers/shares is still pending.

Known limitations: multi-version items play the first available part (no
HDR/bitrate scoring yet). Server stream/poster URLs carry the access token
(Plex/Jellyfin/Emby), so the token is visible locally — in the webview DOM and
in mpv's process arguments. This is an accepted **local-only** exposure (your
own machine, not the network); there is no token proxy. SMB credentials are
stored in the (`0600` on Unix) config so shares can remount on launch, and are
passed to the OS mount tool (`mount_smbfs` / `net use`), so they also appear
briefly in process arguments — same accepted local-only exposure. HDR fidelity
depends on the platform's mpv GPU backend and display support.
