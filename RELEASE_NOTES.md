# Vela 1.0.68

This release opens each video's mpv window the way you left the last one,
checks the certificate of HTTPS media servers when playing, saves Settings as
you change them, and adds an experimental player that shows video inside
Vela's own window on Linux (with Wayland), Windows and macOS.

## Highlights

- Every video of a session opens the way you left the one before it: full
  screen or not, maximized or not, its size and its screen. Settings → Player
  → Player window chooses how the first video after Vela starts opens.
- Settings saves each change as you make it. The Save buttons on the Player
  tab are gone.
- The player now checks the certificate of an HTTPS media server. If your
  server uses a self-signed certificate or a private authority, read
  "Upgrading and going back" before you update.
- Experimental, on Linux (Wayland), Windows and macOS: play inside Vela's own
  window, with Vela's own controls, keys and mouse actions, by choosing
  Settings → Player → Video player. mpv's own window stays the default on
  every system.
- Servers that share a name can be told apart by their Plex, Jellyfin or Emby
  mark.

## Upgrading and going back

- **An HTTPS server with a self-signed or privately signed certificate stops
  playing after this update**, until you either name the authority's
  certificate file to mpv with `tls-ca-file` (in the Advanced mpv options or
  your own mpv config) or turn on Settings → Player → mpv player → Skip
  server certificate checks. Browsing is not affected, and neither are plain
  HTTP servers or servers with a publicly trusted certificate. Until now mpv
  did not check, so the stream's sign-in token was sent without knowing who
  received it.
- **Going back to Vela 1.0.58, 1.0.59 or 1.0.60 after using this version
  stops at Settings recovery**, because those versions refuse a `config.json`
  with settings they do not know. This version writes such settings when you
  play a video (the player window is remembered even if you never open its
  setting), when you choose a Player window other than "As you last left it",
  when you choose Vela's window as the video player, and when you change Skip
  server certificate checks, even back to off. Starting fresh settings there
  resets every setting except your server connections.

## Changes

- Show a Plex, Jellyfin, or Emby mark wherever a server's name appears, so
  servers that share a name can be told apart.
- List server playlists directly under their heading, each with its server's
  mark; a server with no playlists to show takes no space, and the heading
  disappears when there are none.
- Emby's experimental status is now shown on its row in Settings.
- Within one run of Vela, open every video's mpv window the way the previous
  one was left: full screen or not, maximized or not, and on the same screen.
  This includes videos you start yourself, not only the next item of a
  playlist or Continue Playing. What Vela carries over takes precedence over
  custom mpv options such as `--fullscreen`.
- Keep the mpv window's size from one video to the next as well. A video that
  opens full screen or maximized returns to that size when you leave full
  screen. The window's position is not carried. The carried size takes
  precedence over a custom `--geometry`.
- Add Settings → Player → Player window, which chooses how the first video
  opens after Vela starts: "As you last left it" (the default; quit from full
  screen and the next session opens full screen), "Full screen", or "Default"
  (mpv and your own options decide).
- With the Prefer Compatible source policy, choose the copy for the screen the
  video will open on, which is now the screen of the previous player window.
- Settings saves each change as you make it; the Save buttons on the Player
  tab are gone. A choice is saved the moment it changes, and text (the mpv
  path, the Advanced mpv options) when you leave its field or close Settings.
  The header says "Saved", or "Not saved" with the reason in the section
  concerned.
- Add an experimental second player that shows the video inside Vela's own
  window, on Linux, Windows and macOS: Settings → Player → Video player. It
  has its own control bar, keys and mouse actions (click to pause,
  double-click for full screen, the wheel for volume), and is off by default.
  On Linux it runs in a Wayland session (not under X11, and not yet with
  NVIDIA's proprietary driver); on Windows it shows mpv's own Direct3D 11
  output inside Vela's window; on macOS it draws through mpv's render API,
  with HDR on a screen with extended dynamic range. Switching to mpv's own
  window applies to the next video. To use Vela's window, restart Vela if
  this run started without embedded playback. Whenever it cannot be used,
  the video plays in mpv's own window and a line on screen says why. It uses
  its own copy of mpv's library, which every package now carries (the Linux
  packages, both Windows installers and the macOS app); Settings says so when
  it is missing. Details are in `docs/usage.md`.

## Fixes

- When a video plays to its end, Home no longer shows it in Continue
  Watching for a moment while the server records it as played. Vela now
  records the finished play before Home looks again.
- On macOS, Vela now says why it connects to servers on your local network,
  as macOS 15 and later expect before they let an app reach them. Without it,
  a server on your own network could be unreachable from Vela on a Mac.
- On KDE Plasma under Wayland, Vela's window has the desktop's own title bar,
  with its usual height, buttons and Vela's icon, and KDE's window settings
  (such as hiding the title bar) apply to it. It used to have a taller one,
  drawn by Vela's toolkit, that KDE could not change.
- Send a playback report again, once, when the connection it was sent on had
  already been closed by the server. The final report of a play could be lost
  this way, leaving the server showing the item as playing and without its
  last position.
- Check the certificate of an HTTPS media server when playing (see "Upgrading
  and going back").
- When mpv cannot play a video at all, end the play at once. Vela used to wait
  about five seconds before the next playlist item started or the page came
  back.
- With "Use my own mpv config" on, make mpv exit when a file finishes even if
  your `mpv.conf`, or a profile it includes, tells mpv to stay open. Continue
  Playing and playlists stopped there, and the finished item's watched state
  was not saved.
- On Arch Linux, show the skip button and skip intros, credits and
  commercials. The Arch package left out the script that does it; the other
  packages were not affected.
- On Windows, make mpv exit when a file finishes. It previously stayed open and
  idle, so Continue Playing and playlists never advanced and the finished
  item's watched state and resume position were not saved.
- Keep macOS AirPlay/HomePod video and audio synchronized by preferring mpv's
  AVFoundation audio output, with CoreAudio retained as fallback and expert
  overrides preserved.
- Update `h2` to 0.4.20 (`RUSTSEC-2026-0258`) and `rustls` to 0.23.46 with
  `rustls-webpki` 0.103.16 (`RUSTSEC-2026-0285`, `RUSTSEC-2026-0348`,
  `RUSTSEC-2026-0349` and two further rustls advisories).
- Remove SvelteKit from the front end. Vela used none of its features (it
  ships one static page), and advisories against SvelteKit kept failing the
  dependency check every release has to pass. The page is now built by Svelte
  and Vite alone; nothing about it looks or behaves differently.
- Update the front end's other build dependencies for published advisories:
  `devalue` 5.9.4 (`GHSA-9rgm-9g3h-6x36` and related), Vite 8.3.5 and
  `source-map-js` 1.2.2.

## Downloads

Native packages are attached for macOS, Windows, Debian/Ubuntu, RPM-based
Linux, Arch Linux, and AppImage users. `SHA256SUMS` covers every promised
package. All packages are attached to a tagged draft only after every package
job's checks pass; a re-run refuses to change an already published release.
Manual workflow runs only produce workflow artifacts. The macOS download is
the checked universal DMG. The duplicate `.app.tar.gz`, which was never in
`SHA256SUMS`, is no longer released; it contained the same app as the DMG.

The macOS app is signed and notarized. The Windows installers are not signed
in this release (those of 1.0.60 were): Windows SmartScreen shows "Windows
protected your PC" before running one; choose "More info", then "Run anyway",
and check the file against `SHA256SUMS` if you want to be sure of it. The Linux
packages and the Arch package are unsigned, as before. mpv 0.38 or newer
remains a separate required installation: it is the default player on every
system.

Every package is larger than before (the Linux packages by about 15 MB): each
now carries the library the experimental embedded player uses, an LGPL build
of mpv and of the libraries it needs, as separate shared libraries, with each
component's licence text, built from the sources pinned in
`packaging/libmpv/versions.txt`. On macOS the libraries are signed with the
app.

## Server status

- **Plex:** primary and most deeply exercised integration.
- **Jellyfin:** supported.
- **Emby:** experimental; it shares the Jellyfin-family implementation but has
  not been verified against a real Emby server.

## Known limitations

- The rare queued watch-edit race disclosed in 1.0.0 remains an accepted
  limitation.
- The embedded player is experimental: on Linux it needs a Wayland session
  and does not yet work with the NVIDIA proprietary driver; on Linux and macOS
  its picture is drawn by a different mpv renderer than mpv's own window; it
  does not crop HDR video. `docs/usage.md` lists the differences.
- With two monitors of the same model, Vela cannot tell them apart, and the
  player window can open on the other one.

Full setup and privacy details are in the
[README](https://github.com/roethlar/vela#readme).

[Changes since 1.0.60](https://github.com/roethlar/vela/compare/v1.0.60...v1.0.68)
