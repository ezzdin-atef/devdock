//! Health checks (T7.3): a TCP connect, or a plain HTTP GET, each with a timeout.
//! Written on `std::net` alone: no HTTP client crate for one request line.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

use super::model::Health;

/// Checks target local dev servers; on Windows a refused localhost connection can
/// take about a second per address, so keep this short.
pub const CHECK_TIMEOUT: Duration = Duration::from_millis(800);

/// `Ok` when healthy; `Err` says why not, in words for the UI.
pub fn check(health: &Health, timeout: Duration) -> Result<(), String> {
    match health {
        Health::Tcp { port } => connect(&localhost(*port), timeout)
            .map(drop)
            .map_err(|_| format!("nothing accepts connections on port {port}")),
        Health::Http { url, expect } => http_get_status(url, timeout).and_then(|status| {
            let ok = match expect {
                Some(e) => status == *e,
                None => (200..400).contains(&status),
            };
            if ok {
                Ok(())
            } else {
                Err(match expect {
                    Some(e) => format!("{url} answered {status}, expected {e}"),
                    None => format!("{url} answered {status}"),
                })
            }
        }),
    }
}

fn localhost(port: u16) -> Vec<SocketAddr> {
    // Dev servers bind to either; try both.
    vec![
        SocketAddr::from(([127, 0, 0, 1], port)),
        SocketAddr::from((std::net::Ipv6Addr::LOCALHOST, port)),
    ]
}

fn connect(addrs: &[SocketAddr], timeout: Duration) -> std::io::Result<TcpStream> {
    let mut last = std::io::Error::other("no address");
    for a in addrs {
        match TcpStream::connect_timeout(a, timeout) {
            Ok(s) => return Ok(s),
            Err(e) => last = e,
        }
    }
    Err(last)
}

/// Splits `http://host[:port]/path` into parts. https is not supported.
pub fn parse_http_url(url: &str) -> Result<(String, u16, String), String> {
    let rest = url.strip_prefix("http://").ok_or_else(|| {
        format!("{url} must start with http:// (https isn't supported for checks)")
    })?;
    let (authority, path) = rest
        .split_once('/')
        .map_or((rest, "/".to_owned()), |(a, p)| (a, format!("/{p}")));
    // The last ':' starts a port unless it's inside an IPv6 literal like `[::1]`.
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) if !p.contains(']') => (
            h.to_owned(),
            p.parse::<u16>()
                .map_err(|_| format!("{p} isn't a port number"))?,
        ),
        _ => (authority.to_owned(), 80),
    };
    if host.is_empty() {
        return Err(format!("{url} has no host"));
    }
    Ok((host, port, path))
}

fn http_get_status(url: &str, timeout: Duration) -> Result<u16, String> {
    let (host, port, path) = parse_http_url(url)?;
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    let addrs: Vec<SocketAddr> = if bare.eq_ignore_ascii_case("localhost") {
        localhost(port)
    } else {
        (bare, port)
            .to_socket_addrs()
            .map_err(|e| format!("can't resolve {host}: {e}"))?
            .collect()
    };
    let mut stream =
        connect(&addrs, timeout).map_err(|_| format!("nothing answers at {host}:{port}"))?;
    let _ = stream.set_read_timeout(Some(timeout));
    let _ = stream.set_write_timeout(Some(timeout));
    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}:{port}\r\nUser-Agent: DevDock-health\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| format!("couldn't send the request: {e}"))?;
    let mut head = [0u8; 64];
    let n = stream
        .read(&mut head)
        .map_err(|e| format!("no answer from {host}:{port}: {e}"))?;
    parse_status_line(&String::from_utf8_lossy(&head[..n]))
        .ok_or_else(|| format!("{host}:{port} didn't answer with HTTP"))
}

/// `HTTP/1.1 204 No Content` -> 204.
pub fn parse_status_line(text: &str) -> Option<u16> {
    let mut parts = text.lines().next()?.split_whitespace();
    parts.next().filter(|v| v.starts_with("HTTP/"))?;
    parts.next()?.parse().ok()
}

/// Counts misses; "3 misses = unhealthy" (T7.3).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Misses(pub u32);

impl Misses {
    pub const LIMIT: u32 = 3;

    /// Records one result; returns true while the service still counts as healthy.
    pub fn record(&mut self, ok: bool) -> bool {
        self.0 = if ok { 0 } else { self.0 + 1 };
        self.0 < Self::LIMIT
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    fn serve_once(status: &'static str) -> u16 {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        thread::spawn(move || {
            if let Ok((mut s, _)) = l.accept() {
                let mut buf = [0u8; 512];
                let _ = s.read(&mut buf);
                let _ = s.write_all(
                    format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\n\r\n").as_bytes(),
                );
            }
        });
        port
    }

    #[test]
    fn tcp_check_sees_a_listener_and_not_a_free_port() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        assert!(check(&Health::Tcp { port }, CHECK_TIMEOUT).is_ok());
        drop(l);
        let err = check(&Health::Tcp { port }, Duration::from_millis(300)).unwrap_err();
        assert!(err.contains(&port.to_string()));
    }

    #[test]
    fn http_check_compares_the_status() {
        let port = serve_once("200 OK");
        let url = format!("http://localhost:{port}/health");
        assert!(
            check(
                &Health::Http {
                    url,
                    expect: Some(200)
                },
                CHECK_TIMEOUT
            )
            .is_ok()
        );

        let port = serve_once("503 Service Unavailable");
        let url = format!("http://127.0.0.1:{port}/");
        let err = check(&Health::Http { url, expect: None }, CHECK_TIMEOUT).unwrap_err();
        assert!(err.contains("503"), "{err}");
    }

    #[test]
    fn url_parsing() {
        assert_eq!(
            parse_http_url("http://localhost:3000/api/health").unwrap(),
            ("localhost".into(), 3000, "/api/health".into())
        );
        assert_eq!(
            parse_http_url("http://example.test").unwrap(),
            ("example.test".into(), 80, "/".into())
        );
        assert_eq!(
            parse_http_url("http://[::1]:8080/").unwrap(),
            ("[::1]".into(), 8080, "/".into())
        );
        assert_eq!(
            parse_http_url("http://[::1]/x").unwrap(),
            ("[::1]".into(), 80, "/x".into())
        );
        assert!(parse_http_url("https://x").is_err());
        assert!(parse_http_url("http://:80/").is_err());
        assert!(parse_http_url("http://h:notaport/").is_err());
    }

    #[test]
    fn status_line_parsing() {
        assert_eq!(parse_status_line("HTTP/1.1 204 No Content\r\n"), Some(204));
        assert_eq!(parse_status_line("SSH-2.0-OpenSSH"), None);
    }

    #[test]
    fn three_misses_make_unhealthy_and_one_success_resets() {
        let mut m = Misses::default();
        assert!(m.record(false));
        assert!(m.record(false));
        assert!(!m.record(false));
        assert!(m.record(true));
        assert_eq!(m, Misses(0));
    }
}
