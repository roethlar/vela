# Reddit launch post — Vela 1.0

Draft for an owner post. Do not post until the owner reviews title, subreddit
choice, and flair. Prefer one primary post and cross-post carefully; many
subreddits dislike multi-sub spam.

**Release:** [Vela 1.0.59](https://github.com/roethlar/vela/releases/latest)
(first public: 1.0.0 on 2026-07-20; current Latest is 1.0.59)

**Suggested image:** `docs/images/vela-1.0-launch.png` (or the social preview
`docs/images/social-preview.png`)

---

## Suggested subreddits (pick 1–2 first)

| Subreddit | Fit | Notes |
| --- | --- | --- |
| r/jellyfin | Stronger candidate | Client discussion is normal there; still read promo rules day-of. Honest scope vs Plex. |
| r/htpc | Maybe | Exists (Home Theater PC). Partial fit for desktop + mpv + HDR living-room path; more hardware/setup oriented. Check sidebar before posting. |
| r/linux | Medium | AppImage/deb/rpm/Arch; HDR needs Wayland path |
| r/opensource | Medium | Link license + GitHub; less product-chatty |
| r/PleX | **Do not launch-post** | Exists (capital X; no separate brand as r/plex). Explicit rule: no self-promotion of your app/service; ban risk. Fine later only as answers if someone asks. |
| r/selfhosted | **Skip** | Owner note: community is mean to people who post AI-coded apps. |

Avoid sounding like an ad. Lead with the problem and what is different.

---

## Title options

1. **(Recommended)** Vela 1.0 — open-source desktop client for Plex/Jellyfin (experimental Emby) that plays video in mpv for real HDR
2. Show HN-style: Vela – multi-server Plex/Jellyfin client that keeps the library UI and hands playback to mpv
3. Short: HDR-first Plex + Jellyfin desktop client (mpv playback) — Vela 1.0

---

## Body (copy-ready)

I built **Vela**, a native desktop client for home media servers. The library
UI lives in the app; video plays in your installed **mpv** in its own window.

That split is deliberate. Webview players are fine for SDR and convenience.
They are a poor home for HDR passthrough, mature codecs, and GPU tone-mapping.
mpv already does that work well, so Vela does not reimplement a player.

**What it does today**

- Connect multiple **Plex**, **Jellyfin**, and **experimental Emby** sources
- Browse, search, infinite scroll, themes (including a true black OLED palette)
- Deduplicated **All** view when the same title exists on more than one server
- Source policies: Prefer Best / Prefer Compatible / Prefer Fastest Source /
  Ask Every Time, plus per-title **Play Version**
- Continue Watching cover-flow, cross-server Vela playlists, server playlists
  (browse only)
- Title-level watched state across currently connected copies
- Packages for **macOS** (universal), **Windows**, and **Linux**
  (AppImage, deb, rpm, Arch)

**Honest status**

- **Plex** is the primary, most exercised path
- **Jellyfin** is supported and real-server tested
- **Emby** ships as an experimental sibling of the Jellyfin path; not yet
  verified against a real Emby server
- Installers are **unsigned** (Gatekeeper / SmartScreen may warn)
- **mpv is not bundled** — install mpv 0.38+ separately (Vela can help on
  first run)
- Plex playback wants a reachable direct HTTPS connection; Relay is not the
  default for HDR

**Links**

- Releases: https://github.com/roethlar/vela/releases/latest
- Source / README: https://github.com/roethlar/vela

Happy to answer questions about HDR paths (Linux Wayland, macOS EDR, Windows
HDR), multi-server selection, or packaging.

---

## Short comment (if posting as link + first comment)

Vela is a Tauri desktop app: polished library UI, playback delegated to system
mpv so HDR and codecs stay on a path that already works. Multi-server Plex +
Jellyfin (Emby experimental). Unsigned builds; mpv required separately. Details
and downloads in the post / repo README.

---

## Checklist before posting

- [ ] Confirm Latest release URL still points at the intended tag
- [ ] Attach launch screenshot or social preview
- [ ] Match subreddit rules (self-promo limits, flair, title format)
- [ ] Disclose unsigned binaries + mpv dependency up front
- [ ] Do not claim official affiliation with Plex, Jellyfin, or Emby
- [ ] Do not oversell Emby or Windows/macOS code-signing
)
