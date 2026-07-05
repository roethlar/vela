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
    Mem(Vec<u8>),
}

struct Entry {
    token: String,
    target: Target,
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
    if let Some(existing) = reg.iter().find(|e| match (&e.target, &target) {
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
        return Ok(format!("http://127.0.0.1:{}/{}", p.port, existing.token));
    }
    if reg.len() >= REGISTRY_CAP {
        reg.pop_front();
    }
    reg.push_back(Entry {
        token: token.clone(),
        target,
    });
    Ok(format!("http://127.0.0.1:{}/{}", p.port, token))
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
    // Look the token up without holding the registry lock during I/O.
    let target: Option<Target> = {
        let reg = registry()
            .lock()
            .map_err(|_| "stream proxy registry poisoned".to_string())?;
        reg.iter().find(|e| e.token == req.token).map(|e| match &e.target {
            Target::Smb { mount, relative } => Target::Smb {
                mount: mount.clone(),
                relative: relative.clone(),
            },
            #[cfg(test)]
            Target::Mem(bytes) => Target::Mem(bytes.clone()),
        })
    };
    let Some(target) = target else {
        return respond_status(&mut conn, "404 Not Found", &[]);
    };
    match target {
        Target::Smb { mount, relative } => {
            let smb = crate::smb_client::connect_mount(&mount)?;
            let handle = smb.open_read(&relative)?;
            let len = handle.len();
            serve_target(&mut conn, &req, len, |offset, buf| {
                handle.read_at(offset, buf)
            })
        }
        #[cfg(test)]
        Target::Mem(bytes) => {
            let len = bytes.len() as u64;
            serve_target(&mut conn, &req, len, move |offset, buf| {
                let start = (offset as usize).min(bytes.len());
                let n = (bytes.len() - start).min(buf.len());
                buf[..n].copy_from_slice(&bytes[start..start + n]);
                Ok(n)
            })
        }
    }
}

/// Write the response for `req` against a `len`-byte entity, pulling bytes
/// through `read_at`. Streams in 256 KiB chunks; a dropped client (seek,
/// stop) surfaces as a write error and simply ends the response.
fn serve_target(
    conn: &mut TcpStream,
    req: &Request,
    len: u64,
    read_at: impl Fn(u64, &mut [u8]) -> Result<usize, String>,
) -> Result<(), String> {
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
        let url = register(Target::Mem(bytes.clone())).unwrap();
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
}
