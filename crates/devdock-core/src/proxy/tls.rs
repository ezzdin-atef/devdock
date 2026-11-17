//! HTTPS for local addresses: a DevDock certificate authority, created once and
//! kept in the data folder, signs a certificate for each name on first use.
//!
//! The authority is name-constrained to `localhost` and `test`, so even if its key
//! leaked it could not vouch for any real website. The user trusts it with
//! `certutil -user -addstore Root`, which shows Windows' own confirmation dialog.

use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;

use rcgen::{
    BasicConstraints, CertificateParams, CertifiedIssuer, DistinguishedName, DnType,
    ExtendedKeyUsagePurpose, GeneralSubtree, IsCa, Issuer, KeyPair, KeyUsagePurpose,
    NameConstraints,
};
use rustls::crypto::ring as provider;
use rustls::pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey;
use rustls::{ServerConfig, ServerConnection, StreamOwned};

use super::{Router, connect_route, host_header, read_head};
use crate::run::serve;

pub const CA_CERT_FILE: &str = "devdock-ca.pem";
const CA_KEY_FILE: &str = "devdock-ca-key.pem";
pub const CA_NAME: &str = "DevDock Local CA";

#[derive(Debug, thiserror::Error)]
pub enum TlsError {
    #[error("DevDock couldn't read or write its certificate files in {}: {source}", .dir.display())]
    Files {
        dir: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("DevDock couldn't create its local certificate: {0}")]
    Cert(#[from] rcgen::Error),
    #[error("DevDock couldn't set up HTTPS: {0}")]
    Rustls(#[from] rustls::Error),
}

/// The local certificate authority.
pub struct Authority {
    issuer: Issuer<'static, KeyPair>,
    /// PEM file the user trusts (and can import into Firefox by hand).
    pub cert_path: PathBuf,
    /// SHA-1 of the certificate, as `certutil` shows it (uppercase hex).
    pub thumbprint: String,
}

impl std::fmt::Debug for Authority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Authority")
            .field("cert_path", &self.cert_path)
            .finish_non_exhaustive()
    }
}

impl Authority {
    /// Loads the authority from `dir`, creating it the first time.
    pub fn load_or_create(dir: &Path) -> Result<Self, TlsError> {
        let files = |source| TlsError::Files {
            dir: dir.to_path_buf(),
            source,
        };
        let cert_path = dir.join(CA_CERT_FILE);
        let key_path = dir.join(CA_KEY_FILE);
        if !(cert_path.is_file() && key_path.is_file()) {
            fs::create_dir_all(dir).map_err(files)?;
            let key = KeyPair::generate()?;
            let key_pem = key.serialize_pem();
            let ca = CertifiedIssuer::self_signed(ca_params(), key)?;
            fs::write(&key_path, key_pem).map_err(files)?;
            fs::write(&cert_path, ca.pem()).map_err(files)?;
        }
        let cert_pem = fs::read_to_string(&cert_path).map_err(files)?;
        let key = KeyPair::from_pem(&fs::read_to_string(&key_path).map_err(files)?)?;
        let der = pem_to_der(&cert_pem).ok_or(rcgen::Error::CouldNotParseCertificate)?;
        let thumbprint = ring::digest::digest(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, &der)
            .as_ref()
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect();
        Ok(Self {
            issuer: Issuer::from_ca_cert_pem(&cert_pem, key)?,
            cert_path,
            thumbprint,
        })
    }

    /// A certificate for one name, signed by this authority.
    pub fn issue(&self, host: &str) -> Result<CertifiedKey, TlsError> {
        let mut params = CertificateParams::new(vec![host.to_owned()])?;
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, host);
        params.distinguished_name = dn;
        params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        params.use_authority_key_identifier_extension = true;
        let (y, m, d) = today();
        params.not_before = rcgen::date_time_ymd(y - 1, m, d.min(28));
        params.not_after = rcgen::date_time_ymd(y + 1, m, d.min(28));
        let key = KeyPair::generate()?;
        let cert = params.signed_by(&key, &self.issuer)?;
        let key_der = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key.serialize_der()));
        let signer = provider::sign::any_supported_type(&key_der)?;
        Ok(CertifiedKey::new(vec![cert.der().clone()], signer))
    }

    /// Server settings for the HTTPS listener (HTTP/1.1 only, so bytes can be
    /// passed to dev servers as they are).
    pub fn server_config(self: &Arc<Self>) -> Result<Arc<ServerConfig>, TlsError> {
        let mut config =
            ServerConfig::builder_with_provider(Arc::new(provider::default_provider()))
                .with_safe_default_protocol_versions()?
                .with_no_client_auth()
                .with_cert_resolver(Arc::new(Leaves {
                    ca: Arc::clone(self),
                    cache: Mutex::default(),
                }));
        config.alpn_protocols = vec![b"http/1.1".to_vec()];
        Ok(Arc::new(config))
    }
}

fn ca_params() -> CertificateParams {
    let mut params = CertificateParams::default();
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, CA_NAME);
    dn.push(DnType::OrganizationName, "DevDock (this computer only)");
    params.distinguished_name = dn;
    params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
    params.key_usages = vec![
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::CrlSign,
        KeyUsagePurpose::DigitalSignature,
    ];
    params.name_constraints = Some(NameConstraints {
        permitted_subtrees: vec![
            GeneralSubtree::DnsName("localhost".into()),
            GeneralSubtree::DnsName("test".into()),
        ],
        excluded_subtrees: Vec::new(),
    });
    let (y, m, d) = today();
    params.not_before = rcgen::date_time_ymd(y - 1, m, d.min(28));
    params.not_after = rcgen::date_time_ymd(y + 10, m, d.min(28));
    params
}

fn pem_to_der(pem: &str) -> Option<Vec<u8>> {
    let body: String = pem
        .lines()
        .skip_while(|l| !l.starts_with("-----BEGIN CERTIFICATE"))
        .skip(1)
        .take_while(|l| !l.starts_with("-----END"))
        .collect();
    base64_decode(body.trim())
}

fn base64_decode(s: &str) -> Option<Vec<u8>> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in s.bytes().filter(|c| !c.is_ascii_whitespace() && *c != b'=') {
        let v = ALPHABET.iter().position(|a| *a == c)? as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// Today's UTC date as (year, month, day).
fn today() -> (i32, u8, u8) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    civil_from_days(i64::try_from(secs / 86_400).unwrap_or(0))
}

/// Days since 1970-01-01 to a calendar date (Howard Hinnant's algorithm).
fn civil_from_days(z: i64) -> (i32, u8, u8) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (
        i32::try_from(y).unwrap_or(2026),
        u8::try_from(m).unwrap_or(1),
        u8::try_from(d).unwrap_or(1),
    )
}

/// Picks (and caches) a certificate for the name the browser asked for.
struct Leaves {
    ca: Arc<Authority>,
    cache: Mutex<HashMap<String, Arc<CertifiedKey>>>,
}

impl std::fmt::Debug for Leaves {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Leaves")
    }
}

impl ResolvesServerCert for Leaves {
    fn resolve(&self, hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        let host = hello.server_name()?.to_ascii_lowercase();
        if !serve::is_local_host(&host) {
            return None;
        }
        let mut cache = lock(&self.cache);
        if let Some(found) = cache.get(&host) {
            return Some(Arc::clone(found));
        }
        let issued = Arc::new(self.ca.issue(&host).ok()?);
        cache.insert(host, Arc::clone(&issued));
        Some(issued)
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn to_io(e: rustls::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, e)
}

/// One HTTPS connection: handshake, route by server name, then relay decrypted
/// bytes to the dev server over plain TCP.
pub(super) fn serve(
    client: TcpStream,
    config: Arc<ServerConfig>,
    router: &dyn Router,
) -> io::Result<()> {
    client.set_read_timeout(Some(super::HEAD_TIMEOUT))?;
    let conn = ServerConnection::new(config).map_err(to_io)?;
    let mut tls = StreamOwned::new(conn, client);
    let Some(head) = read_head(&mut tls)? else {
        return Ok(());
    };
    tls.sock.set_read_timeout(None)?;
    let host = tls
        .conn
        .server_name()
        .map(str::to_owned)
        .or_else(|| host_header(&head))
        .unwrap_or_default();
    match connect_route(router, &host) {
        Ok(mut backend) => {
            backend.write_all(&head)?;
            relay(tls, backend)
        }
        Err(page) => {
            tls.write_all(&page)?;
            tls.conn.send_close_notify();
            tls.flush()?;
            let _ = tls.sock.shutdown(Shutdown::Both);
            Ok(())
        }
    }
}

/// Both directions share the TLS state behind a lock; socket reads happen outside
/// it, so neither direction waits on the other's idle socket.
fn relay(tls: StreamOwned<ServerConnection, TcpStream>, backend: TcpStream) -> io::Result<()> {
    let StreamOwned { conn, sock } = tls;
    let conn = Arc::new(Mutex::new(conn));
    let sock_in = sock.try_clone()?;
    let mut sock_ack = sock.try_clone()?;
    let backend_in = backend.try_clone()?;
    let up_conn = Arc::clone(&conn);
    let up = thread::spawn(move || {
        let _ = client_to_backend(&up_conn, sock_in, &mut sock_ack, backend_in);
    });
    let mut sock_out = sock;
    let _ = backend_to_client(&conn, backend, &mut sock_out);
    let _ = up.join();
    let _ = sock_out.shutdown(Shutdown::Both);
    Ok(())
}

fn client_to_backend(
    conn: &Mutex<ServerConnection>,
    mut sock: TcpStream,
    ack: &mut TcpStream,
    mut backend: TcpStream,
) -> io::Result<()> {
    let mut buf = [0u8; 16 * 1024];
    let mut plain = Vec::new();
    let result = (|| loop {
        let n = sock.read(&mut buf)?;
        if n == 0 {
            return Ok(());
        }
        let mut closed = false;
        {
            let mut c = lock(conn);
            let mut slice = &buf[..n];
            while !slice.is_empty() {
                c.read_tls(&mut slice)?;
                let state = c.process_new_packets().map_err(to_io)?;
                let mut tmp = [0u8; 16 * 1024];
                loop {
                    match c.reader().read(&mut tmp) {
                        Ok(0) => {
                            closed = true;
                            break;
                        }
                        Ok(k) => plain.extend_from_slice(&tmp[..k]),
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                        Err(e) => return Err(e),
                    }
                }
                closed |= state.peer_has_closed();
            }
            while c.wants_write() {
                c.write_tls(ack)?;
            }
        }
        if !plain.is_empty() {
            backend.write_all(&plain)?;
            plain.clear();
        }
        if closed {
            return Ok(());
        }
    })();
    let _ = backend.shutdown(Shutdown::Write);
    result
}

fn backend_to_client(
    conn: &Mutex<ServerConnection>,
    mut backend: TcpStream,
    sock: &mut TcpStream,
) -> io::Result<()> {
    let mut buf = [0u8; 16 * 1024];
    loop {
        let n = backend.read(&mut buf)?;
        let mut c = lock(conn);
        if n == 0 {
            c.send_close_notify();
        } else {
            c.writer().write_all(&buf[..n])?;
        }
        while c.wants_write() {
            c.write_tls(sock)?;
        }
        if n == 0 {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_774), (2026, 11, 17));
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
    }

    #[test]
    fn authority_is_created_once_and_issues_leaves() {
        let dir = tempfile::tempdir().unwrap();
        let a = Authority::load_or_create(dir.path()).unwrap();
        let first = fs::read_to_string(dir.path().join(CA_CERT_FILE)).unwrap();
        assert_eq!(a.thumbprint.len(), 40);
        let b = Authority::load_or_create(dir.path()).unwrap();
        assert_eq!(a.thumbprint, b.thumbprint);
        assert_eq!(
            first,
            fs::read_to_string(dir.path().join(CA_CERT_FILE)).unwrap()
        );
        let leaf = b.issue("shop.localhost").unwrap();
        assert_eq!(leaf.cert.len(), 1);
    }

    #[test]
    fn https_round_trip_through_the_proxy() {
        use std::net::{Ipv4Addr, TcpListener};

        use rustls::pki_types::{CertificateDer, ServerName};
        use rustls::{ClientConfig, ClientConnection, RootCertStore};

        use crate::proxy::{Proxy, Route};

        struct To(u16);
        impl Router for To {
            fn route(&self, _: &str) -> Route {
                Route::Port(self.0)
            }
        }
        // Backend: answers with the host it was asked for.
        let l = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let backend = l.local_addr().unwrap().port();
        thread::spawn(move || {
            for c in l.incoming() {
                let mut c = c.unwrap();
                let head = read_head(&mut c).unwrap().unwrap();
                let body = format!("secure {}", host_header(&head).unwrap());
                let _ = write!(
                    c,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });

        let dir = tempfile::tempdir().unwrap();
        let ca = Arc::new(Authority::load_or_create(dir.path()).unwrap());
        let proxy =
            Proxy::start(0, Arc::new(To(backend)), Some(ca.server_config().unwrap())).unwrap();

        // A client that trusts only DevDock's authority (name constraints included).
        let pem = fs::read_to_string(&ca.cert_path).unwrap();
        let mut roots = RootCertStore::empty();
        roots
            .add(CertificateDer::from(pem_to_der(&pem).unwrap()))
            .unwrap();
        let config = ClientConfig::builder_with_provider(Arc::new(provider::default_provider()))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates(roots)
            .with_no_client_auth();
        let name = ServerName::try_from("shop.localhost").unwrap();
        let conn = ClientConnection::new(Arc::new(config), name).unwrap();
        let sock = TcpStream::connect((Ipv4Addr::LOCALHOST, proxy.port)).unwrap();
        let mut tls = StreamOwned::new(conn, sock);
        tls.write_all(b"GET / HTTP/1.1\r\nHost: shop.localhost\r\n\r\n")
            .unwrap();
        tls.flush().unwrap();
        let mut out = Vec::new();
        let _ = tls.read_to_end(&mut out);
        let out = String::from_utf8_lossy(&out);
        assert!(out.starts_with("HTTP/1.1 200"), "{out}");
        assert!(out.ends_with("secure shop.localhost"), "{out}");
    }

    #[test]
    fn real_sites_get_no_certificate() {
        assert!(!serve::is_local_host("example.com"));
    }

    #[test]
    fn base64_round_trip() {
        assert_eq!(base64_decode("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(base64_decode("aGk=").unwrap(), b"hi");
        assert!(base64_decode("a$b").is_none());
    }
}
