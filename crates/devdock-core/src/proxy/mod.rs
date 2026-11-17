//! Local reverse proxy: `http://shop.localhost` (no port) reaches the project's
//! running dev server. It listens on loopback only and routes each connection by
//! its `Host` header (or the TLS server name); see `tls` for HTTPS.
//!
//! A connection is routed once, from its first request, then bytes are copied both
//! ways untouched. Browsers keep one connection per origin, so that is exact, and
//! WebSocket upgrades (hot reload) work without any special handling.

pub mod tls;

use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, Ipv6Addr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

/// Where a host goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// The dev server's port on this machine.
    Port(u16),
    /// The host belongs to this project, but nothing with a port is running.
    Idle {
        project: String,
    },
    Unknown,
}

/// Answers "which port serves this host" from live app state.
pub trait Router: Send + Sync {
    fn route(&self, host: &str) -> Route;
}

const MAX_HEAD: usize = 64 * 1024;
const HEAD_TIMEOUT: Duration = Duration::from_secs(20);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

/// A running listener (HTTP or HTTPS). Dropping it stops accepting connections;
/// connections already open finish on their own.
pub struct Proxy {
    pub port: u16,
    pub https: bool,
    /// Why `127.0.0.1:port` couldn't be used, when only `[::1]` is served. Browsers
    /// try `::1` first for `*.localhost`, so addresses still work, but anything
    /// that insists on 127.0.0.1 reaches the other program.
    pub ipv4_error: Option<io::Error>,
    stop: Arc<AtomicBool>,
    addrs: Vec<SocketAddr>,
}

impl Proxy {
    /// Binds `127.0.0.1:port` and `[::1]:port` and starts serving. One of the two is
    /// enough: another server (nginx, IIS) often holds IPv4 port 80 while `::1` is
    /// free. Fails only when neither works. `tls` makes it an HTTPS listener; port
    /// 0 picks a free port (tests).
    pub fn start(
        port: u16,
        router: Arc<dyn Router>,
        tls: Option<Arc<rustls::ServerConfig>>,
    ) -> io::Result<Self> {
        let mut listeners = Vec::new();
        let (port, ipv4_error) = match TcpListener::bind((Ipv4Addr::LOCALHOST, port)) {
            Ok(v4) => {
                let bound = v4.local_addr()?.port();
                listeners.push(v4);
                (bound, None)
            }
            Err(e) => (port, Some(e)),
        };
        match TcpListener::bind((Ipv6Addr::LOCALHOST, port)) {
            Ok(v6) => listeners.push(v6),
            // Neither address: report the IPv4 reason, it's the one people know.
            Err(e) if listeners.is_empty() => return Err(ipv4_error.unwrap_or(e)),
            Err(_) => {}
        }
        let port = listeners[0].local_addr()?.port();
        let stop = Arc::new(AtomicBool::new(false));
        let mut addrs = Vec::new();
        for listener in listeners {
            addrs.push(listener.local_addr()?);
            let (stop, router, tls) = (Arc::clone(&stop), Arc::clone(&router), tls.clone());
            thread::Builder::new()
                .name(format!("devdock-proxy-{port}"))
                .spawn(move || accept_loop(&listener, &stop, &router, tls.as_ref()))?;
        }
        Ok(Self {
            port,
            https: tls.is_some(),
            ipv4_error,
            stop,
            addrs,
        })
    }
}

impl Drop for Proxy {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wake the blocking accept so the thread sees the flag and exits.
        for addr in &self.addrs {
            let _ = TcpStream::connect_timeout(addr, Duration::from_millis(300));
        }
    }
}

fn accept_loop(
    listener: &TcpListener,
    stop: &AtomicBool,
    router: &Arc<dyn Router>,
    tls: Option<&Arc<rustls::ServerConfig>>,
) {
    for conn in listener.incoming() {
        if stop.load(Ordering::SeqCst) {
            return;
        }
        let Ok(client) = conn else { continue };
        let router = Arc::clone(router);
        let tls = tls.cloned();
        let _ = thread::Builder::new()
            .name("devdock-proxy-conn".into())
            .spawn(move || {
                let _ = match tls {
                    Some(config) => tls::serve(client, config, router.as_ref()),
                    None => serve_plain(client, router.as_ref()),
                };
            });
    }
}

fn serve_plain(mut client: TcpStream, router: &dyn Router) -> io::Result<()> {
    client.set_read_timeout(Some(HEAD_TIMEOUT))?;
    let Some(head) = read_head(&mut client)? else {
        return Ok(());
    };
    client.set_read_timeout(None)?;
    let host = host_header(&head).unwrap_or_default();
    match connect_route(router, &host) {
        Ok(mut backend) => {
            backend.write_all(&head)?;
            splice(client, backend)
        }
        Err(page) => client.write_all(&page),
    }
}

/// The backend for `host`, or the error page to send instead.
pub(crate) fn connect_route(router: &dyn Router, host: &str) -> Result<TcpStream, Vec<u8>> {
    match router.route(host) {
        Route::Port(port) => connect_local(port).map_err(|_| {
            error_page(
                "502 Bad Gateway",
                &format!("{host} isn't answering yet"),
                &format!(
                    "DevDock sends {host} to port {port}, but nothing accepts connections there. \
                     The server may still be starting, or it listens on another port."
                ),
            )
        }),
        Route::Idle { project } => Err(error_page(
            "503 Service Unavailable",
            &format!("{project} isn't running"),
            &format!(
                "{host} belongs to {project}. Start one of its commands in DevDock's Run tab, \
                 then reload this page."
            ),
        )),
        Route::Unknown => Err(error_page(
            "404 Not Found",
            &format!("No project uses {host}"),
            "Set a project's address in DevDock (Run tab, Port), then reload this page.",
        )),
    }
}

/// Dev servers bind `127.0.0.1` or `::1` depending on the tool and Node version.
fn connect_local(port: u16) -> io::Result<TcpStream> {
    let v4 = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let v6 = SocketAddr::from((Ipv6Addr::LOCALHOST, port));
    let stream = TcpStream::connect_timeout(&v4, CONNECT_TIMEOUT)
        .or_else(|_| TcpStream::connect_timeout(&v6, CONNECT_TIMEOUT))?;
    stream.set_nodelay(true)?;
    Ok(stream)
}

/// Reads until the end of the request head. Returns everything read (the head and
/// any body bytes after it), or `None` if the client closed first.
pub(crate) fn read_head(r: &mut impl Read) -> io::Result<Option<Vec<u8>>> {
    let mut buf = Vec::with_capacity(4096);
    let mut chunk = [0u8; 8192];
    loop {
        let n = r.read(&mut chunk)?;
        if n == 0 {
            return Ok(None);
        }
        buf.extend_from_slice(&chunk[..n]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
            return Ok(Some(buf));
        }
        if buf.len() > MAX_HEAD {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "request head too large",
            ));
        }
    }
}

/// The `Host` header without its port, lowercased: `Shop.localhost:80` -> `shop.localhost`.
pub fn host_header(head: &[u8]) -> Option<String> {
    let end = head
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .unwrap_or(head.len());
    let text = std::str::from_utf8(&head[..end]).ok()?;
    let value = text.split("\r\n").skip(1).find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.trim()
            .eq_ignore_ascii_case("host")
            .then(|| value.trim())
    })?;
    Some(strip_port(value))
}

pub(crate) fn strip_port(host: &str) -> String {
    let host = if let Some(rest) = host.strip_prefix('[') {
        rest.split(']').next().unwrap_or("")
    } else {
        host.rsplit_once(':').map_or(host, |(h, _)| h)
    };
    host.trim_end_matches('.').to_ascii_lowercase()
}

/// Copies both ways until both sides are done.
fn splice(client: TcpStream, backend: TcpStream) -> io::Result<()> {
    let mut client_r = client.try_clone()?;
    let mut backend_w = backend.try_clone()?;
    let up = thread::spawn(move || {
        let _ = io::copy(&mut client_r, &mut backend_w);
        let _ = backend_w.shutdown(Shutdown::Write);
    });
    let (mut backend_r, mut client_w) = (backend, client);
    let _ = io::copy(&mut backend_r, &mut client_w);
    let _ = client_w.shutdown(Shutdown::Write);
    let _ = up.join();
    Ok(())
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub(crate) fn error_page(status: &str, title: &str, body: &str) -> Vec<u8> {
    let html = format!(
        "<!doctype html><meta charset=utf-8><title>{t}</title>\
         <body style=\"font:15px/1.5 system-ui,sans-serif;max-width:560px;margin:15vh auto;padding:0 24px;color:#222\">\
         <p style=\"color:#888;font-size:13px;margin:0\">DevDock</p><h1 style=\"font-size:22px\">{t}</h1><p>{b}</p></body>",
        t = escape(title),
        b = escape(body),
    );
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nConnection: close\r\n\r\n{html}",
        html.len()
    )
    .into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed(Route);
    impl Router for Fixed {
        fn route(&self, _: &str) -> Route {
            self.0.clone()
        }
    }

    /// A tiny backend that answers once with the Host header it got.
    fn echo_backend() -> u16 {
        let l = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = l.local_addr().unwrap().port();
        thread::spawn(move || {
            for c in l.incoming() {
                let mut c = c.unwrap();
                let head = read_head(&mut c).unwrap().unwrap();
                let host = host_header(&head).unwrap();
                let body = format!("hello {host}");
                let _ = write!(
                    c,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        port
    }

    fn get(port: u16, host: &str) -> String {
        let mut s = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        write!(s, "GET / HTTP/1.1\r\nHost: {host}\r\n\r\n").unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        out
    }

    #[test]
    fn routes_by_host_to_the_backend() {
        let backend = echo_backend();
        let proxy = Proxy::start(0, Arc::new(Fixed(Route::Port(backend))), None).unwrap();
        let out = get(proxy.port, "Shop.localhost");
        assert!(out.starts_with("HTTP/1.1 200"), "{out}");
        assert!(out.ends_with("hello shop.localhost"));
    }

    #[test]
    fn idle_and_unknown_hosts_get_a_page() {
        let proxy = Proxy::start(
            0,
            Arc::new(Fixed(Route::Idle {
                project: "Shop".into(),
            })),
            None,
        )
        .unwrap();
        let out = get(proxy.port, "shop.localhost");
        assert!(out.starts_with("HTTP/1.1 503"));
        assert!(out.contains("Shop isn't running"));

        let proxy = Proxy::start(0, Arc::new(Fixed(Route::Unknown)), None).unwrap();
        assert!(get(proxy.port, "<x>.localhost").contains("&lt;x&gt;.localhost"));
    }

    #[test]
    fn serves_on_ipv6_when_ipv4_is_taken() {
        // Something else holds 127.0.0.1:port, as nginx holds port 80.
        let blocker = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = blocker.local_addr().unwrap().port();
        let Ok(proxy) = Proxy::start(port, Arc::new(Fixed(Route::Unknown)), None) else {
            return; // this machine has no IPv6 loopback
        };
        assert_eq!(proxy.port, port);
        assert!(proxy.ipv4_error.is_some());
        let mut s = TcpStream::connect((Ipv6Addr::LOCALHOST, port)).unwrap();
        write!(s, "GET / HTTP/1.1\r\nHost: x.localhost\r\n\r\n").unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        assert!(out.starts_with("HTTP/1.1 404"), "{out}");
    }

    #[test]
    fn dropping_stops_listening() {
        let proxy = Proxy::start(0, Arc::new(Fixed(Route::Unknown)), None).unwrap();
        let port = proxy.port;
        drop(proxy);
        thread::sleep(Duration::from_millis(100));
        // The listener thread has exited, so the port can be bound again.
        assert!(TcpListener::bind((Ipv4Addr::LOCALHOST, port)).is_ok());
    }

    #[test]
    fn host_parsing() {
        let head = b"GET / HTTP/1.1\r\nhost: Api.Shop.localhost:8080\r\n\r\n";
        assert_eq!(host_header(head).as_deref(), Some("api.shop.localhost"));
        assert_eq!(strip_port("[::1]:80"), "::1");
        assert_eq!(strip_port("shop.test."), "shop.test");
        assert_eq!(host_header(b"GET / HTTP/1.1\r\n\r\n"), None);
    }
}
