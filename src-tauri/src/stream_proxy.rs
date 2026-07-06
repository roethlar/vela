//! Loopback HTTP Range proxy: translates mpv's byte-range requests into
//! native SMB positioned reads, so playback needs no OS mount and no
//! smb:// support in mpv/ffmpeg (Linux-family; other platforms play from
//! their OS mounts).
//!
//! Deliberately minimal, dependency-free HTTP/1.1 on std TCP with one
//! thread per connection: mpv is the only intended client, it speaks
//! single-range GET/HEAD, and every response is `Connection: close` (mpv
//! reconnects with a fresh Range on seeks — one extra round-trip on a LAN).
//!
//! Security posture: binds 127.0.0.1 only; URLs carry a single-use-style
//! unguessable token (UUIDv4) that maps to exactly one registered file —
//! no paths, no credentials, no directory service, no request logging. The
//! registry is capped; registering evicts the oldest token. A token is
//! valid until evicted or the app exits, and only ever grants reads of the
//! one file it was minted for.

#![cfg(all(unix, not(target_os = "macos")))]

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use crate::config::SmbMount;

/// Streaming sources the proxy can serve. In-memory targets exist so the
/// HTTP semantics have a real end-to-end test without an SMB server.
enum Target {
    Smb {
        mount: Box<SmbMount>,
        relative: String,
    },
    #[cfg(test)]
    Mem {
        bytes: Vec<u8>,
        /// Counts length probes (the stand-in for SMB's network `stat`) so a
        /// test can assert a seek reuses the cached length instead.
        probes: std::sync::Arc<std::sync::atomic::AtomicUsize>,
        /// Counts backend-session creations (the stand-in for SMB's
        /// `connect_mount`) so a test can assert a seek reuses the cached
        /// session instead of rebuilding one — the felt freeze this slice fixes.
        sessions: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    },
}

/// A live backend session cached per token and reused across every request for
/// that token — the initial open AND every seek — so a seek never rebuilds an
/// SMB session (context create + teardown under the process-wide lifecycle
/// lock), which was the felt freeze on a real NAS. Created lazily on the first
/// request; freed exactly once at playback-end (generation-guarded — see
/// [`release_session`]), never per seek. Each request still opens its OWN file
/// handle on the shared session, so no two connections share a file position.
#[derive(Clone)]
enum Session {
    Smb(std::sync::Arc<crate::smb_client::SmbConnection>),
    /// Presence-only stand-in with no payload; it exists so a test can observe
    /// that the cache reused a session rather than creating a new one.
    #[cfg(test)]
    Fake,
}

impl Session {
    /// The SMB connection this session wraps. Only ever called for a session
    /// built from an SMB target, so the test-only `Fake` arm is unreachable.
    // The match is infallible in non-test builds (where `Fake` does not exist),
    // but must stay a match to cover `Fake` under `cfg(test)`.
    #[allow(clippy::infallible_destructuring_match)]
    fn smb(&self) -> &std::sync::Arc<crate::smb_client::SmbConnection> {
        match self {
            Session::Smb(smb) => smb,
            #[cfg(test)]
            Session::Fake => unreachable!("an SMB target yields an SMB session"),
        }
    }
}

struct Entry {
    token: String,
    target: Target,
    /// Entity length once learned on the first request, so a later Range
    /// request (a seek) skips the per-open `stat`. `None` until discovered.
    len: Option<u64>,
    /// Bumped every time the token is reused for a fresh play. A request
    /// captures it at lookup and may only write its discovered length back
    /// under the same generation, so a slow in-flight request from a prior
    /// play cannot repopulate a length that a replay just cleared. It is also
    /// the *session* generation: [`release_session`] frees the cached session
    /// only if this value still matches the one the finishing play captured.
    generation: u64,
    /// The cached live backend session for this token (see [`Session`]). `None`
    /// until the first request creates it, and again after playback-end frees
    /// it. Kept across a same-file replay (token reuse) so the replay skips the
    /// reconnect too.
    session: Option<Session>,
    /// Bumped by each new streaming (GET) request for this token. A serve
    /// captures the epoch it claimed; when a newer request bumps it, the older
    /// serve sees the mismatch at its next chunk boundary and stops
    /// (cooperative seek-cancel — see [`serve_target`]). Shared in an `Arc` so
    /// the serve loop reads it lock-free after the registry lock is released.
    serve_epoch: std::sync::Arc<AtomicU64>,
}

/// Oldest-first; registration evicts from the front once full. The
/// registry holds PLAYBACK targets only (artwork uses the stable
/// `velasmb:` scheme instead — see smb_vfs), so 64 covers a session's
/// plays many times over and an old token dying is harmless: the next
/// play mints a fresh one.
const REGISTRY_CAP: usize = 64;

struct Proxy {
    port: u16,
    registry: &'static Mutex<VecDeque<Entry>>,
}

fn registry() -> &'static Mutex<VecDeque<Entry>> {
    static REGISTRY: OnceLock<Mutex<VecDeque<Entry>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(VecDeque::new()))
}

fn proxy() -> Result<&'static Proxy, String> {
    static PROXY: OnceLock<Result<Proxy, String>> = OnceLock::new();
    PROXY
        .get_or_init(|| {
            let listener = TcpListener::bind(("127.0.0.1", 0))
                .map_err(|e| format!("could not bind the stream proxy: {e}"))?;
            let port = listener
                .local_addr()
                .map_err(|e| format!("stream proxy has no local address: {e}"))?
                .port();
            std::thread::spawn(move || {
                for conn in listener.incoming().flatten() {
                    std::thread::spawn(move || {
                        let _ = serve_connection(conn);
                    });
                }
            });
            Ok(Proxy {
                port,
                registry: registry(),
            })
        })
        .as_ref()
        .map_err(Clone::clone)
}

/// Register one SMB file and return the loopback URL mpv (or the webview,
/// for artwork) can fetch it from. Cheap: no network here.
pub fn register_smb(mount: &SmbMount, relative: &str) -> Result<String, String> {
    register(Target::Smb {
        mount: Box::new(mount.clone()),
        relative: relative.to_string(),
    })
}

fn register(target: Target) -> Result<String, String> {
    let p = proxy()?;
    let token = uuid::Uuid::new_v4().simple().to_string();
    let mut reg = p
        .registry
        .lock()
        .map_err(|_| "stream proxy registry poisoned".to_string())?;
    // Same target already registered → reuse its token (stable artwork URLs).
    if let Some(existing) = reg.iter_mut().find(|e| match (&e.target, &target) {
        (
            Target::Smb { mount, relative },
            Target::Smb {
                mount: m2,
                relative: r2,
            },
        ) => mount.id == m2.id && relative == r2,
        #[cfg(test)]
        _ => false,
    }) {
        // A replay of the same file reuses the token, but any length learned
        // on the earlier play may now be stale (the file could have been
        // replaced or resized since). Drop it so the next request re-stats
        // once — the per-seek cache only needs to span a single playback.
        // Bumping the generation also invalidates any in-flight store from
        // the prior play, so a late writer can't repopulate the stale length.
        existing.len = None;
        existing.generation = existing.generation.wrapping_add(1);
        return Ok(format!("http://127.0.0.1:{}/{}", p.port, existing.token));
    }
    if reg.len() >= REGISTRY_CAP {
        reg.pop_front();
    }
    reg.push_back(Entry {
        token: token.clone(),
        target,
        len: None,
        generation: 0,
        session: None,
        serve_epoch: std::sync::Arc::new(AtomicU64::new(0)),
    });
    Ok(format!("http://127.0.0.1:{}/{}", p.port, token))
}

/// Record the discovered entity length for `token`, learned by a request that
/// captured `generation` at lookup, so later requests (seeks) skip the length
/// probe. Best-effort and generation-guarded: a no-op if the token was evicted
/// meanwhile, or if the token was reused for a fresh play since the request
/// began (generation moved on) — in which case the stale length is dropped and
/// the next request re-probes.
fn store_len(token: &str, generation: u64, len: u64) {
    if let Ok(mut reg) = registry().lock() {
        if let Some(entry) = reg.iter_mut().find(|e| e.token == token) {
            if entry.generation == generation {
                entry.len = Some(len);
            }
        }
    }
}

/// The `(token, generation)` a resolved stream URL maps to, or `None` when the
/// URL is not one of this proxy's loopback URLs — Plex/Jellyfin/Emby, local
/// files, and OS-mounted SMB (macOS/Windows) have no cached session to free.
/// The play path snapshots this at play time (under `play_lock`, right after
/// resolving the stream) and hands it to [`release_session`] at playback-end.
pub fn playback_session_key(url: &str) -> Option<(String, u64)> {
    // register() builds exactly `http://127.0.0.1:{port}/{token}`; the token is
    // the whole remainder after the first slash (a UUID, no further path/query).
    let rest = url.strip_prefix("http://127.0.0.1:")?;
    let (_port, token) = rest.split_once('/')?;
    if token.is_empty() {
        return None;
    }
    let reg = registry().lock().ok()?;
    let generation = reg.iter().find(|e| e.token == token)?.generation;
    Some((token.to_string(), generation))
}

/// Free the cached backend session for `token`, but only if it still owns
/// `generation` (compare-and-remove). Called once at playback-end.
///
/// A same-file replay reuses the token and bumps the generation BEFORE the
/// prior play signals end (`play_by_key` resolves the new stream before killing
/// the old mpv), so the prior play's late call no longer matches and cannot
/// free the session the new play is now using. A no-op if the token was evicted
/// or the session was already freed. The freed session is dropped AFTER the
/// registry lock is released: dropping the last `Arc<SmbConnection>` frees its
/// context (a blocking teardown under the process-wide lifecycle lock), which
/// must never run while the registry lock is held.
pub fn release_session(token: &str, generation: u64) {
    let freed = {
        let mut reg = match registry().lock() {
            Ok(reg) => reg,
            Err(_) => return,
        };
        match reg.iter_mut().find(|e| e.token == token) {
            Some(entry) if entry.generation == generation => {
                let taken = entry.session.take();
                // Close this token's play-epoch so a slow in-flight request that
                // has not yet cached its session cannot store one after this
                // release — such a session would have no owner left to free it
                // (sspf-5). We bump even when there was nothing to take.
                entry.generation = entry.generation.wrapping_add(1);
                taken
            }
            _ => None,
        }
    };
    drop(freed);
}

/// Return the token's cached backend session, creating it once on the first
/// request and reusing it for every later request (seeks included) — the core
/// of the seek fix. `generation` is the play-epoch the calling serve captured at
/// lookup. Fast path: clone and return the cached session. Slow path: build one
/// OFF the registry lock (an SMB `connect` touches libsmbclient's global context
/// state under its own lifecycle lock — the registry lock must never be held
/// across it), then store it iff the token still has none AND this request's play
/// is still the current one. A concurrent first request may have won the race; in
/// that case use the winner and drop ours — also off-lock, since dropping it may
/// free a context.
fn get_or_create_session(
    token: &str,
    generation: u64,
    target: &Target,
) -> Result<Session, String> {
    // Fast path: already cached. No generation check here — handing a live
    // session to a request whose play has ended is harmless (its serve fails on
    // the now-closed socket); the orphan risk is only at the commit below.
    {
        let reg = registry()
            .lock()
            .map_err(|_| "stream proxy registry poisoned".to_string())?;
        match reg.iter().find(|e| e.token == token) {
            None => return Err("stream token is no longer registered".to_string()),
            Some(entry) => {
                if let Some(session) = &entry.session {
                    return Ok(session.clone());
                }
            }
        }
    }
    // Slow path: build the session off-lock.
    let created = match target {
        Target::Smb { mount, .. } => {
            Session::Smb(std::sync::Arc::new(crate::smb_client::connect_mount(mount)?))
        }
        #[cfg(test)]
        Target::Mem { sessions, .. } => {
            sessions.fetch_add(1, Ordering::SeqCst);
            Session::Fake
        }
    };
    // Commit iff still absent. Decide under the lock, then act on the decision
    // AFTER releasing it so any session we drop (the race loser, or a create
    // against an evicted token) frees its context off the registry lock.
    enum Outcome {
        Won,
        Lost(Session),
        Superseded,
    }
    let outcome = {
        let mut reg = registry()
            .lock()
            .map_err(|_| "stream proxy registry poisoned".to_string())?;
        match reg.iter_mut().find(|e| e.token == token) {
            None => Outcome::Superseded, // token evicted while we connected
            // The play that issued this request ended or was replaced while we
            // connected (its generation moved on): storing our session now would
            // orphan it — no `on_end` would ever free it (sspf-5). Drop it
            // off-lock instead.
            Some(entry) if entry.generation != generation => Outcome::Superseded,
            Some(entry) => match &entry.session {
                Some(existing) => Outcome::Lost(existing.clone()),
                None => {
                    entry.session = Some(created.clone());
                    Outcome::Won
                }
            },
        }
    };
    match outcome {
        Outcome::Won => Ok(created), // the entry now holds its own clone
        Outcome::Lost(winner) => {
            drop(created);
            Ok(winner)
        }
        Outcome::Superseded => {
            drop(created);
            Err("stream token was superseded before its session was cached".to_string())
        }
    }
}

/// One parsed request: method, token from the path, and the Range header.
struct Request {
    method: String,
    token: String,
    range: Option<String>,
}

/// What a Range header means against a file of `len` bytes.
#[derive(Debug, PartialEq, Eq)]
enum Span {
    /// Whole file (no or ignorable Range).
    Full,
    /// Inclusive byte range, already clamped to the file.
    Part { start: u64, end: u64 },
    /// Syntactically a range but unsatisfiable for this file.
    Unsatisfiable,
}

/// Parse a single-range `bytes=` header per RFC 9110 §14 (the subset mpv
/// emits): `bytes=a-b`, `bytes=a-`, `bytes=-suffix`. Multi-range and
/// malformed values fall back to `Full` (serving the whole entity is the
/// specified, safe response to a Range we don't understand).
fn parse_range(header: Option<&str>, len: u64) -> Span {
    let Some(raw) = header else {
        return Span::Full;
    };
    let Some(spec) = raw.trim().strip_prefix("bytes=") else {
        return Span::Full;
    };
    if spec.contains(',') {
        return Span::Full; // multi-range: serve full
    }
    let Some((a, b)) = spec.split_once('-') else {
        return Span::Full;
    };
    let (a, b) = (a.trim(), b.trim());
    match (a.is_empty(), b.is_empty()) {
        (true, true) => Span::Full,
        // -suffix: the final `suffix` bytes.
        (true, false) => match b.parse::<u64>() {
            Ok(0) => Span::Unsatisfiable,
            Ok(suffix) => {
                if len == 0 {
                    Span::Unsatisfiable
                } else {
                    Span::Part {
                        start: len.saturating_sub(suffix),
                        end: len - 1,
                    }
                }
            }
            Err(_) => Span::Full,
        },
        // a-: from a to EOF.
        (false, true) => match a.parse::<u64>() {
            Ok(start) if start < len => Span::Part {
                start,
                end: len - 1,
            },
            Ok(_) => Span::Unsatisfiable,
            Err(_) => Span::Full,
        },
        // a-b inclusive, clamped.
        (false, false) => match (a.parse::<u64>(), b.parse::<u64>()) {
            (Ok(start), Ok(end)) => {
                if start > end || start >= len {
                    Span::Unsatisfiable
                } else {
                    Span::Part {
                        start,
                        end: end.min(len - 1),
                    }
                }
            }
            _ => Span::Full,
        },
    }
}

/// Read and parse one request head (bounded, timed out). Body-less
/// GET/HEAD only, so reading up to the blank line is the whole request.
fn read_request(conn: &mut TcpStream) -> Result<Request, String> {
    conn.set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| e.to_string())?;
    let mut head = Vec::with_capacity(1024);
    let mut buf = [0u8; 512];
    while !head.windows(4).any(|w| w == b"\r\n\r\n") {
        if head.len() > 8192 {
            return Err("request head too large".into());
        }
        let n = conn.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("connection closed mid-request".into());
        }
        head.extend_from_slice(&buf[..n]);
    }
    let text = String::from_utf8_lossy(&head);
    let mut lines = text.lines();
    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let path = parts.next().unwrap_or_default();
    let token = path.trim_start_matches('/').to_string();
    let mut range = None;
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("range") {
                range = Some(value.trim().to_string());
            }
        }
    }
    Ok(Request {
        method,
        token,
        range,
    })
}

fn serve_connection(mut conn: TcpStream) -> Result<(), String> {
    let req = read_request(&mut conn)?;
    if req.method != "GET" && req.method != "HEAD" {
        return respond_status(&mut conn, "405 Method Not Allowed", &[("Allow", "GET, HEAD")]);
    }
    // Look the token up without holding the registry lock during I/O, along
    // with any length learned on a prior request (so a seek skips the probe),
    // the current generation (so a length we learn is only written back if the
    // token has not been reused for a fresh play meanwhile), and the shared
    // serve-epoch (so this serve can notice a later request supersede it).
    let looked_up: Option<(Target, Option<u64>, u64, std::sync::Arc<AtomicU64>)> = {
        let reg = registry()
            .lock()
            .map_err(|_| "stream proxy registry poisoned".to_string())?;
        reg.iter().find(|e| e.token == req.token).map(|e| {
            let target = match &e.target {
                Target::Smb { mount, relative } => Target::Smb {
                    mount: mount.clone(),
                    relative: relative.clone(),
                },
                #[cfg(test)]
                Target::Mem {
                    bytes,
                    probes,
                    sessions,
                } => Target::Mem {
                    bytes: bytes.clone(),
                    probes: probes.clone(),
                    sessions: sessions.clone(),
                },
            };
            (target, e.len, e.generation, e.serve_epoch.clone())
        })
    };
    let Some((target, cached_len, generation, serve_epoch)) = looked_up else {
        return respond_status(&mut conn, "404 Not Found", &[]);
    };

    // Claim this serve's place in the token's supersede order. A GET streams a
    // body, so it bumps the epoch: any older in-flight serve for this token now
    // observes a newer epoch and stops at its next chunk boundary (cooperative
    // seek-cancel). A HEAD carries no body and returns before the stream loop,
    // so it only reads the epoch — bumping it would spuriously cancel an
    // in-flight GET.
    let my_epoch = if req.method == "GET" {
        serve_epoch.fetch_add(1, Ordering::SeqCst) + 1
    } else {
        serve_epoch.load(Ordering::SeqCst)
    };

    // Reuse the token's cached backend session, creating it once on the first
    // request. This is the heart of the seek fix: a seek opens a fresh file
    // handle on the existing session instead of rebuilding an SMB context. The
    // captured `generation` guards the create against orphaning a session for a
    // play that ended mid-connect (sspf-5).
    let session = get_or_create_session(&req.token, generation, &target)?;

    match target {
        Target::Smb { relative, .. } => {
            let smb = session.smb();
            // On a seek the length is already known: open without a redundant
            // network stat. On the first request, learn it and cache it. Each
            // connection opens its OWN file handle (its own SMB file position),
            // so a seek's reads never disturb the initial stream's handle.
            let handle = match cached_len {
                Some(len) => smb.open_read_with_len(&relative, len)?,
                None => {
                    let handle = smb.open_read(&relative)?;
                    store_len(&req.token, generation, handle.len());
                    handle
                }
            };
            let len = handle.len();
            serve_target(&mut conn, &req, len, &serve_epoch, my_epoch, |offset, buf| {
                handle.read_at(offset, buf)
            })
        }
        #[cfg(test)]
        Target::Mem { bytes, probes, .. } => {
            // The fake session was cached and reuse-counted (via
            // `get_or_create_session`) exactly as the SMB path caches its
            // connection; the in-memory bytes are served directly, so the
            // session itself is not read here — it only exists to prove reuse.
            let len = match cached_len {
                Some(len) => len,
                None => {
                    probes.fetch_add(1, Ordering::SeqCst);
                    let len = bytes.len() as u64;
                    store_len(&req.token, generation, len);
                    len
                }
            };
            serve_target(
                &mut conn,
                &req,
                len,
                &serve_epoch,
                my_epoch,
                move |offset, buf| {
                    let start = (offset as usize).min(bytes.len());
                    let n = (bytes.len() - start).min(buf.len());
                    buf[..n].copy_from_slice(&bytes[start..start + n]);
                    Ok(n)
                },
            )
        }
    }
}

/// Per-response write deadline, in milliseconds. `read_request` already bounds
/// the read side; without a matching write bound a client that stops draining
/// mid-body — a stuck/dead peer, or a very long pause — blocks `write_all`
/// forever and pins this connection's thread. On expiry the write fails and
/// the response ends.
///
/// It is a resource BACKSTOP, deliberately generous, not a pause-killer: a
/// paused mpv is a healthy client that will resume, and dropping it early would
/// force a fresh SMB session on resume — re-incurring exactly the per-seek
/// session cost Bug 1 removes. So the default sits well past any normal pause.
/// If it does fire (a truly long idle, or a dead peer), the loopback stream is
/// reconnect-enabled on the mpv side (`playback::proxy_reconnect_args`), so mpv
/// reopens with a fresh `Range` and playback continues — a drop is safe, not a
/// broken stream. It also gives sub-slice 3's cooperative seek-cancel a bounded
/// point to notice supersession. Tests lower it; `0` disables it (the
/// pre-deadline behavior, for the guard proof).
const DEFAULT_WRITE_TIMEOUT_MS: u64 = 300_000;
static WRITE_TIMEOUT_MS: AtomicU64 = AtomicU64::new(DEFAULT_WRITE_TIMEOUT_MS);

fn write_timeout() -> Option<Duration> {
    match WRITE_TIMEOUT_MS.load(Ordering::Relaxed) {
        0 => None,
        ms => Some(Duration::from_millis(ms)),
    }
}

#[cfg(test)]
fn set_write_timeout_ms_for_test(ms: u64) {
    WRITE_TIMEOUT_MS.store(ms, Ordering::Relaxed);
}

/// Write the response for `req` against a `len`-byte entity, pulling bytes
/// through `read_at`. Streams in 256 KiB chunks; a dropped client (seek,
/// stop) surfaces as a write error and simply ends the response.
fn serve_target(
    conn: &mut TcpStream,
    req: &Request,
    len: u64,
    serve_epoch: &AtomicU64,
    my_epoch: u64,
    read_at: impl Fn(u64, &mut [u8]) -> Result<usize, String>,
) -> Result<(), String> {
    // Bound every write on this response so a non-draining client cannot pin
    // the thread (see WRITE_TIMEOUT_MS). A timed-out write surfaces exactly
    // like a client that went away: the head write errors out, a body write
    // breaks the loop — both end the response.
    if let Some(deadline) = write_timeout() {
        conn.set_write_timeout(Some(deadline))
            .map_err(|e| e.to_string())?;
    }
    let span = parse_range(req.range.as_deref(), len);
    let (status, start, end) = match span {
        Span::Unsatisfiable => {
            let cr = format!("bytes */{len}");
            return respond_status(
                conn,
                "416 Range Not Satisfiable",
                &[("Content-Range", &cr)],
            );
        }
        Span::Full => ("200 OK", 0, len.saturating_sub(1)),
        Span::Part { start, end } => ("206 Partial Content", start, end),
    };
    let body_len = if len == 0 { 0 } else { end - start + 1 };
    let mut head = format!(
        "HTTP/1.1 {status}\r\nAccept-Ranges: bytes\r\nContent-Length: {body_len}\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n"
    );
    if status.starts_with("206") {
        head.push_str(&format!("Content-Range: bytes {start}-{end}/{len}\r\n"));
    }
    head.push_str("\r\n");
    conn.write_all(head.as_bytes()).map_err(|e| e.to_string())?;
    if req.method == "HEAD" || body_len == 0 {
        return Ok(());
    }
    let mut buf = vec![0u8; 256 * 1024];
    let mut pos = start;
    while pos <= end {
        // Cooperative seek-cancel: a newer request for this token bumped the
        // epoch, so this serve is superseded — stop and let the newer one
        // serve. Checked at each chunk boundary. A write already blocked on a
        // non-draining client is bounded by the write deadline instead, and a
        // blocked SMB read by the connection's per-op timeout, so a stuck serve
        // is never worse than today's worst case.
        if serve_epoch.load(Ordering::SeqCst) != my_epoch {
            break;
        }
        let want = ((end - pos + 1).min(buf.len() as u64)) as usize;
        let n = read_at(pos, &mut buf[..want])?;
        if n == 0 {
            break; // file shrank server-side; end the body early
        }
        if conn.write_all(&buf[..n]).is_err() {
            break; // client went away (seek/stop): normal, not an error
        }
        pos += n as u64;
    }
    Ok(())
}

fn respond_status(
    conn: &mut TcpStream,
    status: &str,
    extra: &[(&str, &str)],
) -> Result<(), String> {
    let mut head = format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n");
    for (name, value) in extra {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    conn.write_all(head.as_bytes()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The registry is process-global; tests that populate it must not
    /// interleave (eviction in one would drop the other's tokens).
    fn test_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn range_parsing_covers_mpv_forms() {
        assert_eq!(parse_range(None, 100), Span::Full);
        assert_eq!(parse_range(Some("bytes=0-99"), 100), Span::Part { start: 0, end: 99 });
        assert_eq!(parse_range(Some("bytes=10-19"), 100), Span::Part { start: 10, end: 19 });
        assert_eq!(parse_range(Some("bytes=90-"), 100), Span::Part { start: 90, end: 99 });
        assert_eq!(parse_range(Some("bytes=-10"), 100), Span::Part { start: 90, end: 99 });
        // Clamping and edges.
        assert_eq!(parse_range(Some("bytes=10-1000"), 100), Span::Part { start: 10, end: 99 });
        assert_eq!(parse_range(Some("bytes=-1000"), 100), Span::Part { start: 0, end: 99 });
        assert_eq!(parse_range(Some("bytes=100-"), 100), Span::Unsatisfiable);
        assert_eq!(parse_range(Some("bytes=50-40"), 100), Span::Unsatisfiable);
        assert_eq!(parse_range(Some("bytes=-0"), 100), Span::Unsatisfiable);
        assert_eq!(parse_range(Some("bytes=0-"), 0), Span::Unsatisfiable);
        // Unknown forms serve the whole entity.
        assert_eq!(parse_range(Some("bytes=0-10,20-30"), 100), Span::Full);
        assert_eq!(parse_range(Some("items=0-10"), 100), Span::Full);
        assert_eq!(parse_range(Some("bytes=x-y"), 100), Span::Full);
    }

    fn http(port: u16, request: &str) -> (String, Vec<u8>) {
        let mut conn = TcpStream::connect(("127.0.0.1", port)).unwrap();
        conn.write_all(request.as_bytes()).unwrap();
        let mut raw = Vec::new();
        conn.read_to_end(&mut raw).unwrap();
        let split = raw.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
        (
            String::from_utf8_lossy(&raw[..split]).to_string(),
            raw[split + 4..].to_vec(),
        )
    }

    #[test]
    fn serves_full_head_range_and_errors_end_to_end() {
        let _guard = test_lock();
        let bytes: Vec<u8> = (0..=255u8).collect();
        let url = register(Target::Mem {
            bytes: bytes.clone(),
            probes: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            sessions: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        })
        .unwrap();
        let (port, token) = {
            let rest = url.strip_prefix("http://127.0.0.1:").unwrap();
            let (port, token) = rest.split_once('/').unwrap();
            (port.parse::<u16>().unwrap(), token.to_string())
        };

        let (head, body) = http(port, &format!("GET /{token} HTTP/1.1\r\nHost: x\r\n\r\n"));
        assert!(head.starts_with("HTTP/1.1 200"), "{head}");
        assert!(head.contains("Content-Length: 256"), "{head}");
        assert!(head.contains("Accept-Ranges: bytes"), "{head}");
        assert_eq!(body, bytes);

        let (head, body) = http(
            port,
            &format!("GET /{token} HTTP/1.1\r\nHost: x\r\nRange: bytes=10-19\r\n\r\n"),
        );
        assert!(head.starts_with("HTTP/1.1 206"), "{head}");
        assert!(head.contains("Content-Range: bytes 10-19/256"), "{head}");
        assert_eq!(body, (10..=19u8).collect::<Vec<_>>());

        let (head, body) = http(port, &format!("HEAD /{token} HTTP/1.1\r\nHost: x\r\n\r\n"));
        assert!(head.starts_with("HTTP/1.1 200"), "{head}");
        assert!(head.contains("Content-Length: 256"), "{head}");
        assert!(body.is_empty());

        let (head, _) = http(
            port,
            &format!("GET /{token} HTTP/1.1\r\nHost: x\r\nRange: bytes=999-\r\n\r\n"),
        );
        assert!(head.starts_with("HTTP/1.1 416"), "{head}");
        assert!(head.contains("Content-Range: bytes */256"), "{head}");

        let (head, _) = http(port, "GET /not-a-token HTTP/1.1\r\nHost: x\r\n\r\n");
        assert!(head.starts_with("HTTP/1.1 404"), "{head}");

        let (head, _) = http(port, &format!("POST /{token} HTTP/1.1\r\nHost: x\r\n\r\n"));
        assert!(head.starts_with("HTTP/1.1 405"), "{head}");
    }

    #[test]
    fn seek_reuses_cached_length_without_a_second_probe() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let _guard = test_lock();
        let bytes: Vec<u8> = (0..=255u8).collect();
        let probes = std::sync::Arc::new(AtomicUsize::new(0));
        let url = register(Target::Mem {
            bytes: bytes.clone(),
            probes: probes.clone(),
            sessions: std::sync::Arc::new(AtomicUsize::new(0)),
        })
        .unwrap();
        let (port, token) = {
            let rest = url.strip_prefix("http://127.0.0.1:").unwrap();
            let (port, token) = rest.split_once('/').unwrap();
            (port.parse::<u16>().unwrap(), token.to_string())
        };

        // First request (initial open) learns and caches the length.
        let (head, body) = http(port, &format!("GET /{token} HTTP/1.1\r\nHost: x\r\n\r\n"));
        assert!(head.starts_with("HTTP/1.1 200"), "{head}");
        assert_eq!(body, bytes);
        assert_eq!(
            probes.load(Ordering::SeqCst),
            1,
            "first request probes the length exactly once"
        );

        // Second request is a seek (Range from a mid-file offset). It must
        // reuse the cached length and perform no new probe — this is the
        // redundant per-seek stat the fix removes.
        let (head, body) = http(
            port,
            &format!("GET /{token} HTTP/1.1\r\nHost: x\r\nRange: bytes=10-19\r\n\r\n"),
        );
        assert!(head.starts_with("HTTP/1.1 206"), "{head}");
        assert_eq!(body, (10..=19u8).collect::<Vec<_>>());
        assert_eq!(
            probes.load(Ordering::SeqCst),
            1,
            "seek reuses the cached length, no second probe"
        );
    }

    /// Read an entry's cached length directly from the registry (test-only).
    fn entry_len(token: &str) -> Option<u64> {
        registry()
            .lock()
            .unwrap()
            .iter()
            .find(|e| e.token == token)
            .and_then(|e| e.len)
    }

    /// Read an entry's current generation directly from the registry (test-only).
    fn entry_gen(token: &str) -> u64 {
        registry()
            .lock()
            .unwrap()
            .iter()
            .find(|e| e.token == token)
            .map(|e| e.generation)
            .unwrap()
    }

    /// Whether the token currently has a cached session (test-only).
    fn entry_has_session(token: &str) -> bool {
        registry()
            .lock()
            .unwrap()
            .iter()
            .find(|e| e.token == token)
            .map(|e| e.session.is_some())
            .unwrap_or(false)
    }

    /// Cache a presence-only fake session on the token, standing in for the SMB
    /// session the first request would create (test-only).
    fn seed_session(token: &str) {
        let mut reg = registry().lock().unwrap();
        if let Some(entry) = reg.iter_mut().find(|e| e.token == token) {
            entry.session = Some(Session::Fake);
        }
    }

    #[test]
    fn reregistering_a_token_clears_a_stale_cached_length() {
        let _guard = test_lock();
        let m = |id: &str| SmbMount {
            id: id.into(),
            ..SmbMount::default()
        };
        // First play mints a token; a request then learns and caches the size.
        let url1 = register_smb(&m("x"), "movie.mkv").unwrap();
        let token = url1.rsplit('/').next().unwrap().to_string();
        store_len(&token, entry_gen(&token), 4242);
        assert_eq!(entry_len(&token), Some(4242), "first play caches the size");

        // Replaying the same file reuses the token — but the cached size must
        // be dropped, or a mid-session resize/replace would serve a stale
        // Content-Length (416 on a now-valid tail, or a short/truncated body).
        let url2 = register_smb(&m("x"), "movie.mkv").unwrap();
        assert_eq!(url1, url2, "same mount+path reuses its token");
        assert_eq!(
            entry_len(&token),
            None,
            "re-registration drops the stale cached length so it is re-statted"
        );
    }

    #[test]
    fn a_stale_generation_store_len_is_ignored() {
        let _guard = test_lock();
        let m = |id: &str| SmbMount {
            id: id.into(),
            ..SmbMount::default()
        };
        // Play 1's first request captures the token's generation.
        let url = register_smb(&m("x"), "movie.mkv").unwrap();
        let token = url.rsplit('/').next().unwrap().to_string();
        let g_old = entry_gen(&token);

        // A replay reuses the token, bumping the generation and clearing len.
        let _ = register_smb(&m("x"), "movie.mkv").unwrap();
        let g_new = entry_gen(&token);
        assert_ne!(g_old, g_new, "reuse bumps the generation");

        // Play 1's slow request finally finishes its stat and writes back under
        // the OLD generation: it must be ignored, or it repopulates the length
        // the replay just cleared and the new play serves a stale size.
        store_len(&token, g_old, 4242);
        assert_eq!(
            entry_len(&token),
            None,
            "a stale-generation store is rejected"
        );
        // The new play's own request (current generation) still caches normally.
        store_len(&token, g_new, 8888);
        assert_eq!(
            entry_len(&token),
            Some(8888),
            "a current-generation store is accepted"
        );
    }

    #[test]
    fn write_deadline_unpins_a_client_that_stops_reading() {
        use std::sync::mpsc;
        let _guard = test_lock();
        // Short deadline for the test; restored before we return.
        set_write_timeout_ms_for_test(300);

        // A body larger than any plausible socket buffer, so the body write
        // blocks once the client stops draining.
        let big = vec![0u8; 16 * 1024 * 1024];
        let url = register(Target::Mem {
            bytes: big,
            probes: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            sessions: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        })
        .unwrap();
        let token = url.rsplit('/').next().unwrap().to_string();

        // A real connected TCP pair; hand the server end to serve_connection.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let mut client = TcpStream::connect(addr).unwrap();
        let (server, _) = listener.accept().unwrap();

        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(serve_connection(server));
        });

        // Ask for the whole body, then never read it.
        client
            .write_all(format!("GET /{token} HTTP/1.1\r\nHost: x\r\n\r\n").as_bytes())
            .unwrap();

        // With the deadline the blocked body write times out and
        // serve_connection returns; without it the write blocks forever and
        // this recv times out instead.
        let finished = rx.recv_timeout(Duration::from_secs(5));
        set_write_timeout_ms_for_test(DEFAULT_WRITE_TIMEOUT_MS); // restore for other tests
        assert!(
            finished.is_ok(),
            "the write deadline must unpin serve_connection when the client stops reading"
        );
        drop(client); // keep the non-draining client alive until the assertion
    }

    #[test]
    fn registry_reuses_tokens_and_evicts_oldest() {
        let _guard = test_lock();
        let m = |id: &str| SmbMount {
            id: id.into(),
            ..SmbMount::default()
        };
        let a1 = register_smb(&m("a"), "x.mkv").unwrap();
        let a2 = register_smb(&m("a"), "x.mkv").unwrap();
        assert_eq!(a1, a2, "same mount+path reuses its token");
        let b = register_smb(&m("b"), "x.mkv").unwrap();
        assert_ne!(a1, b);
        for i in 0..REGISTRY_CAP {
            let _ = register_smb(&m(&format!("evict-{i}")), "y.mkv").unwrap();
        }
        let reg = registry().lock().unwrap();
        assert!(reg.len() <= REGISTRY_CAP, "registry stays capped");
    }

    #[test]
    fn seek_reuses_the_cached_session_without_recreating_it() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let _guard = test_lock();
        let bytes: Vec<u8> = (0..=255u8).collect();
        let sessions = std::sync::Arc::new(AtomicUsize::new(0));
        let url = register(Target::Mem {
            bytes: bytes.clone(),
            probes: std::sync::Arc::new(AtomicUsize::new(0)),
            sessions: sessions.clone(),
        })
        .unwrap();
        let (port, token) = {
            let rest = url.strip_prefix("http://127.0.0.1:").unwrap();
            let (port, token) = rest.split_once('/').unwrap();
            (port.parse::<u16>().unwrap(), token.to_string())
        };

        // First request (initial open) creates the session exactly once.
        let (head, _) = http(port, &format!("GET /{token} HTTP/1.1\r\nHost: x\r\n\r\n"));
        assert!(head.starts_with("HTTP/1.1 200"), "{head}");
        assert_eq!(
            sessions.load(Ordering::SeqCst),
            1,
            "the first request creates one session"
        );

        // A seek (Range from a mid-file offset) must REUSE the cached session,
        // not rebuild one — rebuilding an SMB session per seek is exactly the
        // felt freeze this slice removes.
        let (head, body) = http(
            port,
            &format!("GET /{token} HTTP/1.1\r\nHost: x\r\nRange: bytes=10-19\r\n\r\n"),
        );
        assert!(head.starts_with("HTTP/1.1 206"), "{head}");
        assert_eq!(body, (10..=19u8).collect::<Vec<_>>());
        assert_eq!(
            sessions.load(Ordering::SeqCst),
            1,
            "the seek reuses the cached session — no new session created"
        );
    }

    #[test]
    fn a_superseded_serve_stops_without_streaming_the_body() {
        let _guard = test_lock();
        // A real connected TCP pair; the server end is handed a full-range GET,
        // but the token's serve-epoch already sits AHEAD of this serve's claimed
        // epoch — i.e. a newer request has superseded it. It must send the head
        // and then break at the first chunk boundary, streaming (almost) none of
        // the promised body.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let mut client = TcpStream::connect(addr).unwrap();
        let (mut server, _) = listener.accept().unwrap();

        let body_len: u64 = 8 * 1024 * 1024;
        let serve_epoch = AtomicU64::new(5); // a newer serve already claimed epoch 5
        let my_epoch = 3; // this serve is stale
        let req = Request {
            method: "GET".to_string(),
            token: "t".to_string(),
            range: None,
        };
        let worker = std::thread::spawn(move || {
            // read_at would happily supply bytes, but the epoch check must break
            // the loop before the first read.
            serve_target(&mut server, &req, body_len, &serve_epoch, my_epoch, |_off, buf| {
                Ok(buf.len())
            })
        });

        client
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut raw = Vec::new();
        let _ = client.read_to_end(&mut raw);
        let served = worker.join().unwrap();
        assert!(served.is_ok(), "a superseded serve ends cleanly");

        let split = raw.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
        let head = String::from_utf8_lossy(&raw[..split]).to_string();
        let body = &raw[split + 4..];
        assert!(head.starts_with("HTTP/1.1 200"), "{head}");
        assert!(head.contains(&format!("Content-Length: {body_len}")), "{head}");
        // Superseded before the first chunk, so the body is far short of the
        // promised length (empty in practice). Without the epoch check the loop
        // would stream all 8 MiB.
        assert!(
            (body.len() as u64) < body_len / 2,
            "superseded serve streamed {} of {body_len} bytes",
            body.len()
        );
    }

    #[test]
    fn release_session_frees_only_on_matching_generation() {
        let _guard = test_lock();
        let m = |id: &str| SmbMount {
            id: id.into(),
            ..SmbMount::default()
        };
        let url = register_smb(&m("x"), "movie.mkv").unwrap();
        let token = url.rsplit('/').next().unwrap().to_string();
        let g = entry_gen(&token);
        seed_session(&token);
        assert!(entry_has_session(&token), "the session is cached");

        // A stale-generation release (a superseded play's late on_end) must NOT
        // free the session the current play is using.
        release_session(&token, g.wrapping_sub(1));
        assert!(
            entry_has_session(&token),
            "release with a stale generation is a no-op"
        );

        // The owning generation frees it.
        release_session(&token, g);
        assert!(
            !entry_has_session(&token),
            "release with the owning generation frees the session"
        );
    }

    #[test]
    fn a_replays_late_end_does_not_free_the_new_plays_session() {
        let _guard = test_lock();
        let m = |id: &str| SmbMount {
            id: id.into(),
            ..SmbMount::default()
        };
        // Play 1 registers the token, caches a session, and snapshots its key.
        let url1 = register_smb(&m("x"), "movie.mkv").unwrap();
        let token = url1.rsplit('/').next().unwrap().to_string();
        let (t1, g1) = playback_session_key(&url1).unwrap();
        seed_session(&token);

        // Play 2 replays the SAME file: it reuses the token and bumps the
        // generation BEFORE play 1's end fires (play_by_key resolves the new
        // stream before killing the old mpv). The cached session is kept for
        // play 2.
        let url2 = register_smb(&m("x"), "movie.mkv").unwrap();
        let (t2, g2) = playback_session_key(&url2).unwrap();
        assert_eq!(t1, token);
        assert_eq!(t2, token);
        assert_ne!(g1, g2, "the replay bumps the generation");
        assert!(
            entry_has_session(&token),
            "the session is kept across the replay"
        );

        // Play 1's LATE end fires now, carrying the stale generation: a no-op.
        release_session(&t1, g1);
        assert!(
            entry_has_session(&token),
            "play 1's late end must not free play 2's session"
        );

        // Play 2's own end (the owning generation) frees it.
        release_session(&t2, g2);
        assert!(
            !entry_has_session(&token),
            "play 2's end frees the session it owned"
        );
    }

    #[test]
    fn a_create_after_the_plays_release_is_refused_not_orphaned() {
        use std::sync::atomic::AtomicUsize;
        let _guard = test_lock();
        let url = register(Target::Mem {
            bytes: vec![0u8; 8],
            probes: std::sync::Arc::new(AtomicUsize::new(0)),
            sessions: std::sync::Arc::new(AtomicUsize::new(0)),
        })
        .unwrap();
        let token = url.rsplit('/').next().unwrap().to_string();
        let g0 = entry_gen(&token);

        // The request captured g0 at lookup. Its play ends and releases BEFORE it
        // has cached a session: nothing to take, but the play-epoch closes.
        release_session(&token, g0);
        assert!(!entry_has_session(&token));
        assert_ne!(entry_gen(&token), g0, "a matching release closes the play-epoch");

        // The stale request finishes connecting and commits with the OLD
        // generation — it must be refused, or the stored session would have no
        // owner to ever free it (sspf-5).
        let target = Target::Mem {
            bytes: vec![0u8; 8],
            probes: std::sync::Arc::new(AtomicUsize::new(0)),
            sessions: std::sync::Arc::new(AtomicUsize::new(0)),
        };
        let r = get_or_create_session(&token, g0, &target);
        assert!(r.is_err(), "a create for an already-ended play is refused");
        assert!(
            !entry_has_session(&token),
            "no orphaned session is stored for the ended play"
        );
    }
}
