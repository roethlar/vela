# Usage details

The README covers installation and first run. This document keeps the deeper
reference material: how Vela picks which copy to play, playback quality and
transcoding, configuration storage, recovery, and privacy.

## Choosing which copy plays

When the same title exists on multiple connected servers, Settings → Player
offers four source policies:

- **Prefer Best** ranks resolution first, then HDR within that resolution, then
  bitrate. A 4K SDR copy therefore beats a 1080p HDR copy.
- **Prefer Compatible** favors versions at or below the playback display's
  detected resolution and matching its current HDR state. Resolution and HDR
  can be overridden independently when native detection is unavailable or
  wrong.
- **Prefer Fastest Source** chooses this machine, then the local network, then
  the internet, using Prefer Best to break ties within a locality tier.
- **Ask Every Time** prompts for every standalone duplicate play. During one
  Vela playlist or TV-continuation run, the first choice is reused until that
  server lacks an item, when Vela asks again. Server-owned playlists never move
  to another server: if their owner goes offline, playback stops.

**Play Version** in a title's menu is a persistent per-title server override in
the three automatic modes. In Ask Every Time it applies only to that play and
is not saved. Resume position remains specific to the copy being played, while
manual watched/unwatched changes and natural completion update every currently
connected copy of the title. Updates are best-effort and are not queued for an
offline server.

## Playback quality

The source policies above choose **which copy** plays. Settings → Player →
Playback quality chooses **how that copy is delivered**, and the two are
independent.

- **Original** streams the file untouched. It is the default, and it is the
  only setting that keeps HDR.
- A **tier** — 1080p at 20/12/10/8 Mbps, 720p, 480p and below — asks the server
  to convert the file for you. Useful on a slow or remote connection.
- **Automatic** starts at Original and drops one tier only if playback cannot
  keep up: sustained decoder frame drops, or a demuxer cache that keeps running
  dry. It steps at most twice per play, never steps back up, and remembers
  nothing — the next play starts at Original again.

**Converting forfeits HDR and drops container chapters.** The server re-encodes
to a plain HLS stream, so HDR metadata does not survive and chapter markers
embedded in the container are lost. That is inherent to server-side
transcoding, not a Vela limitation, which is why Original is the default.

A title's own right-click menu can override the setting for one play:
**Play Version → Quality on \<server\>** when a title exists on more than one
server, or **Play at Quality** when it exists on one. The menu lists only what
that server says it will actually deliver for that copy, and it asks the server
when you open the submenu rather than on every right-click. The choice applies
to the play it starts and is never saved.

Two things Vela will not convert: a version stored as several files (a
transcode addresses one part, so converting would end the film at the first
part boundary), and any copy the server declines to encode. In both cases the
quality entry is simply absent, and a setting-level request falls back to
Original.

**Emby transcoding is best-effort and unverified.** The Jellyfin and Emby paths
share an implementation, and it has been exercised against Jellyfin only. It
may work on Emby; nothing here asserts that it does. Please open an issue with
what you see if you run Emby — a report is more useful than a guess.

## Playing in Vela's own window (experimental)

Settings → Player → Video player chooses where videos play:

- **mpv's own window** (the default): Vela starts the mpv installed on your
  system, in a window of its own. The rest of this page describes that
  player unless it says otherwise.
- **Vela's window (experimental)**: the video plays inside Vela's window,
  through a copy of mpv's library that comes with Vela. This is the embedded
  player.

The embedded player runs on macOS, on Windows, and on Linux in a Wayland
session, NVIDIA's proprietary driver included. Either choice applies to the
next video; no restart is needed. Whenever the embedded player cannot be used, the video plays in
mpv's own window instead and one line on screen says why; Settings shows the
reason under the choice.

It has its own control bar over the picture (pause, seeking, volume, audio
and subtitle tracks, cropping, full screen, close) and a skip button for
intros, credits and commercials. Its keys are `Space` (pause, or skip while
the skip button shows), the arrow keys (5 and 60 seconds), `m`, `f`, `Shift+C`
for cropping, `q` to close, and `Escape`, which leaves full screen and
otherwise closes the player. The Crop button and `Shift+C` are there when
Black-bar cropping is set to Manual or Automatic. The Audio and Subtitles
buttons open a list above the bar: the arrow keys, `Home` and `End` move in
it, `Enter` or `Space` chooses, and `Escape` closes the list and nothing else.
Full screen is Vela's own window going full screen, and it stays that way
from one video to the next.

With the mouse, on the picture: a click pauses and resumes (after a quarter of
a second, so that a double-click does not also pause); a double-click toggles
full screen; a right-click pauses and resumes without the quarter-second wait;
the wheel changes the volume, two points a notch, and a sideways wheel or
two-finger swipe seeks ten seconds. Over the seek bar the wheel seeks, and
pointing at it shows the time at that spot; over the volume slider the wheel
changes the volume. The middle, back and forward buttons do nothing.

What is different from mpv's own window, and why it is marked experimental:

- On Linux and macOS the picture is drawn by a different mpv renderer
  (`gpu`, where mpv's own window uses `gpu-next`); on Windows it is mpv's own
  `gpu-next` output, inside Vela's window. HDR reaches the screen where the
  system offers it: on Linux through a display and compositor that pass it
  through, on Windows with Windows' HDR turned on, on macOS on a screen with
  extended dynamic range (such as a MacBook Pro's). It can look different
  from mpv's own window.
- Black-bar cropping is done by a script of Vela's own, and it does not crop
  HDR video.
- With "Use my own mpv config" on, it applies your `mpv.conf` but not your
  mpv scripts, per-script option files or key bindings.
- The Player window setting below is about mpv's own window and does not
  apply to it.
- Its library comes in every Vela package: the Linux ones (AppImage, deb,
  rpm and Arch, x86_64), the Windows installers and the macOS app. A Linux
  package you build yourself carries it only when it is built with
  `scripts/build.sh --with-libmpv`
  ([docs/development.md](development.md#packaging-the-set)). Settings says so
  when the library is missing.

The embedded player's library is an LGPL build of mpv and of the libraries it
needs, shipped as separate shared libraries with their licence texts.

## Player behavior notes

By default Vela uses a predictable `--no-config` mpv profile. On macOS, that
profile prefers AVFoundation audio with CoreAudio as fallback so AirPlay and
local-output transitions remain synchronized; Vela does not guess or apply an
AirPlay delay. Settings → Player can opt into your own `mpv.conf`, which also
hands audio-output selection to that file. Custom mpv options are appended
after Vela's defaults and can override the audio output, HDR defaults, or other
playback behavior, so change them deliberately.

Within one run of Vela, each play opens the mpv window the way the previous
play left it: fullscreen or not, maximized or not, on the same screen, and
at the same size when it is not fullscreen or maximized. This applies to
plays you start yourself as well as
to the next item of a playlist or Continue Playing. What Vela carries over
takes precedence over custom mpv options and `mpv.conf`: a custom
`--fullscreen` or `--window-maximized` is overridden, and a custom
`--geometry` gives way to the carried size. With Player window set to
Default, your options still decide how the first play of a run opens; with
As you last left it, the saved window does once there is one.

Settings → Player → Player window chooses how that first play opens:

- **As you last left it** (the default): as the last play of the previous run
  left its window — fullscreen, maximized, screen, and size. Quit from
  fullscreen, and the next run opens fullscreen. Vela saves this in
  `config.json` when each play ends, and when you quit Vela with a video
  still playing.
- **Full screen**: fullscreen, whatever the last run did.
- **Default**: mpv's own default, or your custom mpv options or `mpv.conf`,
  decide fullscreen, maximized and size. Vela still asks mpv to open on the
  screen that holds Vela's window, as it always has.

If the saved screen is no longer connected, mpv opens on its default screen.
Switching away from As you last left it deletes the saved window.

Vela 1.0.58 to 1.0.60 refuse a `config.json` with settings they do not know,
and this version writes several: the player window, which in the default mode
is saved after the first video you play even if you never open the setting; a
Player window choice other than As you last left it; Vela's window as the
video player; and Skip server certificate checks once it has been changed,
even back to off. Going back to one of those releases after any of these
therefore stops at Settings recovery, where starting fresh settings resets
every setting except your server connections.

The player checks the certificate of an HTTPS media server, as Vela itself
does when it browses the server. A server whose certificate mpv does not
accept (a self-signed one, or one signed by a private authority that mpv does
not know) does not play, and the notice for a video that ended before it
started points at the certificate. There are two ways on. For a private
authority, name its certificate file to mpv with `tls-ca-file` in the
Advanced options or in your own mpv config; the check stays on. Or turn the
check off with Settings → Player → mpv player → Skip server certificate
checks; your sign-in token is then sent without knowing who receives it.
Either applies to the next video. Plain HTTP servers are not affected.

Black-bar cropping is Off by default. Manual mode runs on `Shift+C`; Automatic
mode attempts every video. Automatic crop detection can be unreliable with HDR
on some GPU/Wayland combinations and may occasionally hang mpv, so Manual is
the safer option when that occurs.

Intro, credits and commercial skipping uses only the marker ranges your media
server publishes for a title — Vela never detects or guesses them, so titles
without markers are unaffected. Each kind has its own Off / Button / Auto-skip
setting, and Button is the default. In Button mode a skip button appears on the
video while the range is playing: click it, or press `Space` while it is
visible. `Space` keeps its normal pause behaviour at every other moment. Plex
and Jellyfin publish these ranges; Emby currently has no equivalent API, so
skipping is unavailable there.

On Linux, every start uses WebKitGTK's DMABUF (GPU) renderer, which the
embedded player needs, whichever player is chosen, so that switching players
never needs a restart. This affects the library UI renderer, not mpv's video
output, and has no effect on macOS or Windows. Vela used to turn that renderer
off because it once crashed the webview on NVIDIA with Wayland; with a current
WebKitGTK it works there (tested on an RTX 5090). If Vela closes before its
window has loaded, the next start uses the renderer-off window and says why.
If Vela's window comes up blank, start Vela once with
`WEBKIT_DISABLE_DMABUF_RENDERER=1`, which starts it with the renderer off,
and choose mpv's own window in Settings. A
`WEBKIT_DISABLE_DMABUF_RENDERER` you set yourself is left as it is. It rules
the embedded player out only when its value is nonempty and not `0`. An empty
value or `0` does not cause that refusal; the other requirements for embedded
playback still apply.

## Configuration, recovery, and privacy

Vela stores three independent files in the platform configuration directory:

- `config.json` contains settings, recent-play state, and source preferences.
- `connections.json` contains active server connections and their credentials.
- `playlists.json` contains Vela playlists.

The location is:

- Linux: `~/.config/vela/`
- macOS: `~/Library/Application Support/com.vela.vela/`
- Windows: `%APPDATA%\vela\vela\config\`

Back up the whole directory to preserve connected servers, preferences,
recents, playlists, and Vela's rollback history. Before replacing a valid
settings or connections file, Vela retains its three newest distinct valid
versions. If either current file is damaged or has been tampered with, startup
fails closed: Vela loads none of that file, shows the three dated valid versions
available for explicit rollback, and also offers to rename the damaged file and
create a fresh one or exit without writing anything. Settings recovery leaves a
separate valid connections file unchanged; connections recovery requires
reconnecting servers.

On Unix, Vela creates its configuration directory and sensitive files with
owner-only permissions. Active credentials remain plaintext within that
owner-account boundary; Vela does not claim to protect them from malware
already running as the same OS user.

Plex API, artwork, progress, and stream credentials are sent as HTTP headers.
The webview receives only credential-free Vela artwork URLs, and mpv receives
stream headers through a unique owner-only include file that is removed when
its exact child exits rather than through the media URL or process arguments.
Jellyfin/Emby stream and server-image URLs can contain access tokens, so those
tokens remain visible locally to the Vela webview or mpv process. Vela does not
send analytics or proxy credentials through a third party.

Configs written by older Vela builds may still contain removed local-folder,
SMB, or SSH fields, including old SMB credentials. Current builds preserve but
ignore those fields so rollback remains possible. Removing them permanently is
a manual edit of `config.json`.
