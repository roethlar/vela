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

## Player behavior notes

By default Vela uses a predictable `--no-config` mpv profile. Settings → Player
can opt into your own `mpv.conf` or append custom mpv options; those settings
can also override Vela's HDR defaults or prevent playback, so change them
deliberately.

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

On NVIDIA + Wayland, Vela disables WebKitGTK's DMABUF renderer at startup to
avoid a known webview crash. This affects the library UI renderer, not mpv's
video output, and has no effect on macOS or Windows.

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
