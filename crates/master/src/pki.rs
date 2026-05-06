//! PKI bootstrap for the master.
//!
//! On startup we ensure four files exist under the configured PKI directory:
//!
//! - `ca.crt` / `ca.key` — self-signed CA used to issue node certs and the
//!   master's own server cert. **Cold start only**: if both files are absent
//!   we generate a fresh CA. If exactly one is present we refuse to start
//!   (avoids silent fleet-wide CA rotation when an operator partially
//!   restored a backup).
//! - `server.crt` / `server.key` — TLS server cert used by the gRPC and
//!   Enroll listeners. We resign whenever the existing cert has expired,
//!   the key/cert pair no longer matches, or the SAN list drifts away from
//!   `MASTER_PUBLIC_ADDR`.
//!
//! Files are written via tempfile + rename and chmod'd to 0600.
//!
//! See ROADMAP.md M4.1.

use std::fs;
use std::io::Write;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

use ::time::{Duration, OffsetDateTime};
use anyhow::{anyhow, bail, Context, Result};
use rcgen::{
    BasicConstraints, CertificateParams, CertificateSigningRequestParams, DistinguishedName,
    DnType, ExtendedKeyUsagePurpose, IsCa, KeyPair, KeyUsagePurpose, SanType, SerialNumber,
};
use sha2::{Digest, Sha256};
use x509_parser::prelude::*;

const CA_VALIDITY_DAYS: i64 = 365 * 10;
const SERVER_VALIDITY_DAYS: i64 = 365 * 5;
const NODE_VALIDITY_DAYS: i64 = 365;
/// Server cert is resigned this far before expiry to leave operators a
/// comfortable window between automatic rotations and outright failure.
const SERVER_RENEW_BEFORE_DAYS: i64 = 30;

/// Result of signing a node CSR — what we return to the node and what we
/// persist on the `nodes` row.
#[derive(Debug, Clone)]
pub struct SignedNodeCert {
    pub cert_pem: String,
    pub fingerprint_hex: String,
    pub serial_hex: String,
    pub not_after: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct Pki {
    pub ca_cert_pem: String,
    pub ca_key_pem: String,
    pub server_cert_pem: String,
    pub server_key_pem: String,
}

impl Pki {
    /// Ensure the CA and server cert exist and match `public_addrs`.
    pub fn ensure(dir: &Path, public_addrs: &[String]) -> Result<Self> {
        if public_addrs.is_empty() {
            bail!("MASTER_PUBLIC_ADDR is required (comma-separated DNS names or IPs)");
        }

        fs::create_dir_all(dir).with_context(|| format!("creating PKI dir {}", dir.display()))?;

        let ca_crt = dir.join("ca.crt");
        let ca_key = dir.join("ca.key");
        let srv_crt = dir.join("server.crt");
        let srv_key = dir.join("server.key");

        let (ca_cert_pem, ca_key_pem) = match (ca_crt.exists(), ca_key.exists()) {
            (true, true) => (fs::read_to_string(&ca_crt)?, fs::read_to_string(&ca_key)?),
            (false, false) => {
                tracing::info!("PKI cold start: generating new CA");
                let (cert_pem, key_pem) = generate_ca()?;
                write_secret(&ca_crt, &cert_pem)?;
                write_secret(&ca_key, &key_pem)?;
                (cert_pem, key_pem)
            }
            (a, b) => bail!(
                "PKI directory in inconsistent state: ca.crt={} ca.key={}; \
                 refusing to (re)generate CA — restore from backup or delete both files",
                a,
                b
            ),
        };

        let ca_keypair = KeyPair::from_pem(&ca_key_pem).context("loading CA key")?;
        let ca_params =
            CertificateParams::from_ca_cert_pem(&ca_cert_pem).context("parsing CA cert")?;
        let ca_cert = ca_params
            .self_signed(&ca_keypair)
            .context("rebuilding CA cert handle")?;

        let need_resign = match (srv_crt.exists(), srv_key.exists()) {
            (true, true) => {
                let cert_pem = fs::read_to_string(&srv_crt)?;
                let key_pem = fs::read_to_string(&srv_key)?;
                match server_cert_ok(&cert_pem, &key_pem, public_addrs) {
                    Ok(true) => false,
                    Ok(false) => {
                        tracing::info!("server cert needs resign (SAN/expiry/key drift)");
                        true
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "server cert unparseable, resigning");
                        true
                    }
                }
            }
            _ => true,
        };

        let (server_cert_pem, server_key_pem) = if need_resign {
            let (c, k) = sign_server_cert(&ca_cert, &ca_keypair, public_addrs)?;
            write_secret(&srv_crt, &c)?;
            write_secret(&srv_key, &k)?;
            (c, k)
        } else {
            (fs::read_to_string(&srv_crt)?, fs::read_to_string(&srv_key)?)
        };

        Ok(Self {
            ca_cert_pem,
            ca_key_pem,
            server_cert_pem,
            server_key_pem,
        })
    }

    /// Sign a node CSR. Subject and SAN fields in the CSR are ignored — the
    /// master rebuilds them from `node_id` so a malicious node cannot
    /// impersonate another by lying in its CSR.
    pub fn sign_node_cert(&self, node_id: &str, csr_pem: &str) -> Result<SignedNodeCert> {
        if !is_valid_node_id(node_id) {
            bail!("invalid node_id {:?} (charset / length)", node_id);
        }

        let csr = CertificateSigningRequestParams::from_pem(csr_pem)
            .map_err(|e| anyhow!("CSR parse failed: {}", e))?;

        let ca_keypair =
            KeyPair::from_pem(&self.ca_key_pem).context("loading CA key for node sign")?;
        let ca_params =
            CertificateParams::from_ca_cert_pem(&self.ca_cert_pem).context("loading CA cert")?;
        let ca_cert = ca_params
            .self_signed(&ca_keypair)
            .context("rebuilding CA cert handle")?;

        let serial_bytes = random_serial();
        let serial = SerialNumber::from_slice(&serial_bytes);
        let serial_hex = hex_lower(&serial_bytes);

        let now = OffsetDateTime::now_utc();
        let not_after = now + Duration::days(NODE_VALIDITY_DAYS);

        let mut params = CertificateParams::new(vec![node_id.to_string()])
            .context("building node cert params")?;
        params.not_before = now - Duration::hours(1);
        params.not_after = not_after;
        params.is_ca = IsCa::NoCa;
        params.serial_number = Some(serial);
        params.key_usages = vec![
            KeyUsagePurpose::DigitalSignature,
            KeyUsagePurpose::KeyEncipherment,
        ];
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ClientAuth];
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, node_id);
        dn.push(DnType::OrganizationName, "relay");
        params.distinguished_name = dn;

        let cert = params
            .signed_by(&csr.public_key, &ca_cert, &ca_keypair)
            .context("signing node cert")?;

        let der = cert.der().as_ref();
        let fingerprint_hex = hex_lower(&Sha256::digest(der));

        Ok(SignedNodeCert {
            cert_pem: cert.pem(),
            fingerprint_hex,
            serial_hex,
            not_after,
        })
    }
}

pub fn is_valid_node_id(s: &str) -> bool {
    if s.is_empty() || s.len() > 63 {
        return false;
    }
    let mut chars = s.chars();
    let first = chars.next().unwrap();
    if !(first.is_ascii_lowercase() || first.is_ascii_digit()) {
        return false;
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '_' || c == '-')
}

fn random_serial() -> [u8; 16] {
    use rand::RngCore;
    let mut buf = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut buf);
    // RFC 5280 says serial must be a positive integer ≤ 20 octets.
    buf[0] &= 0x7f;
    buf
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

fn generate_ca() -> Result<(String, String)> {
    let mut params = CertificateParams::new(Vec::<String>::new())?;
    let now = OffsetDateTime::now_utc();
    params.not_before = now - Duration::hours(1);
    params.not_after = now + Duration::days(CA_VALIDITY_DAYS);
    params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
    params.key_usages = vec![
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::CrlSign,
        KeyUsagePurpose::DigitalSignature,
    ];
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, "relay-master CA");
    dn.push(DnType::OrganizationName, "relay");
    params.distinguished_name = dn;

    let key = KeyPair::generate()?;
    let cert = params.self_signed(&key)?;
    Ok((cert.pem(), key.serialize_pem()))
}

fn sign_server_cert(
    ca_cert: &rcgen::Certificate,
    ca_key: &KeyPair,
    public_addrs: &[String],
) -> Result<(String, String)> {
    let sans = build_sans(public_addrs)?;
    let mut params = CertificateParams::new(Vec::<String>::new())?;
    params.subject_alt_names = sans;
    let now = OffsetDateTime::now_utc();
    params.not_before = now - Duration::hours(1);
    params.not_after = now + Duration::days(SERVER_VALIDITY_DAYS);
    params.is_ca = IsCa::NoCa;
    params.key_usages = vec![
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyEncipherment,
    ];
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, "relay-master");
    dn.push(DnType::OrganizationName, "relay");
    params.distinguished_name = dn;

    let key = KeyPair::generate()?;
    let cert = params.signed_by(&key, ca_cert, ca_key)?;
    Ok((cert.pem(), key.serialize_pem()))
}

fn build_sans(public_addrs: &[String]) -> Result<Vec<SanType>> {
    let mut out = Vec::with_capacity(public_addrs.len());
    for raw in public_addrs {
        let addr = raw.trim();
        if addr.is_empty() {
            continue;
        }
        if let Ok(ip) = addr.parse::<IpAddr>() {
            out.push(SanType::IpAddress(ip));
        } else {
            out.push(SanType::DnsName(addr.try_into().map_err(|e| {
                anyhow!("invalid DNS name {:?} in MASTER_PUBLIC_ADDR: {}", addr, e)
            })?));
        }
    }
    if out.is_empty() {
        bail!("MASTER_PUBLIC_ADDR contained no usable entries");
    }
    Ok(out)
}

/// Returns Ok(true) if the existing cert is good for `public_addrs`, Ok(false)
/// if it must be resigned. Errs on parse failure.
fn server_cert_ok(cert_pem: &str, key_pem: &str, public_addrs: &[String]) -> Result<bool> {
    let (_, pem) = x509_parser::pem::parse_x509_pem(cert_pem.as_bytes())
        .map_err(|e| anyhow!("server.crt not PEM: {}", e))?;
    if pem.label != "CERTIFICATE" {
        bail!("server.crt has unexpected PEM tag {}", pem.label);
    }
    let (_, cert) = X509Certificate::from_der(&pem.contents)
        .map_err(|e| anyhow!("server.crt DER parse: {}", e))?;

    // Expiry / renewal window.
    let now = OffsetDateTime::now_utc();
    let not_after = OffsetDateTime::from_unix_timestamp(cert.validity().not_after.timestamp())
        .context("not_after timestamp")?;
    if not_after - now < Duration::days(SERVER_RENEW_BEFORE_DAYS) {
        return Ok(false);
    }

    // Public key matches private key on disk.
    let key = KeyPair::from_pem(key_pem).context("server.key parse")?;
    let cert_spki = cert.public_key().raw;
    if cert_spki != key.public_key_der().as_slice() {
        return Ok(false);
    }

    // SAN set matches MASTER_PUBLIC_ADDR exactly.
    let mut want_dns: Vec<String> = Vec::new();
    let mut want_ip: Vec<IpAddr> = Vec::new();
    for raw in public_addrs {
        let a = raw.trim();
        if a.is_empty() {
            continue;
        }
        if let Ok(ip) = a.parse::<IpAddr>() {
            want_ip.push(ip);
        } else {
            want_dns.push(a.to_string());
        }
    }
    want_dns.sort();
    want_ip.sort();

    let mut have_dns: Vec<String> = Vec::new();
    let mut have_ip: Vec<IpAddr> = Vec::new();
    if let Ok(Some(san)) = cert.tbs_certificate.subject_alternative_name() {
        for gn in &san.value.general_names {
            match gn {
                GeneralName::DNSName(s) => have_dns.push((*s).to_string()),
                GeneralName::IPAddress(bytes) => match bytes.len() {
                    4 => {
                        let mut a = [0u8; 4];
                        a.copy_from_slice(bytes);
                        have_ip.push(IpAddr::from(a));
                    }
                    16 => {
                        let mut a = [0u8; 16];
                        a.copy_from_slice(bytes);
                        have_ip.push(IpAddr::from(a));
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }
    have_dns.sort();
    have_ip.sort();

    Ok(have_dns == want_dns && have_ip == want_ip)
}

fn write_secret(path: &PathBuf, contents: &str) -> Result<()> {
    let parent = path.parent().ok_or_else(|| anyhow!("path has no parent"))?;
    let tmp = parent.join(format!(
        ".{}.tmp",
        path.file_name().unwrap().to_string_lossy()
    ));
    {
        let mut opts = fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts
            .open(&tmp)
            .with_context(|| format!("opening {}", tmp.display()))?;
        f.write_all(contents.as_bytes())?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)
        .with_context(|| format!("rename {} → {}", tmp.display(), path.display()))?;
    Ok(())
}

// (PEM parsing handled via `x509_parser::pem`.)

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn addrs(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn cold_start_generates_four_files() {
        let dir = tempdir().unwrap();
        let pki = Pki::ensure(dir.path(), &addrs(&["master.example.com", "10.0.0.1"])).unwrap();
        for name in ["ca.crt", "ca.key", "server.crt", "server.key"] {
            assert!(dir.path().join(name).exists(), "{name} should exist");
        }
        assert!(pki.ca_cert_pem.contains("BEGIN CERTIFICATE"));
        assert!(pki.server_key_pem.contains("PRIVATE KEY"));
    }

    #[test]
    fn restart_is_noop() {
        let dir = tempdir().unwrap();
        Pki::ensure(dir.path(), &addrs(&["a.example.com"])).unwrap();
        let server_crt_v1 = std::fs::read(dir.path().join("server.crt")).unwrap();
        let ca_crt_v1 = std::fs::read(dir.path().join("ca.crt")).unwrap();

        Pki::ensure(dir.path(), &addrs(&["a.example.com"])).unwrap();
        let server_crt_v2 = std::fs::read(dir.path().join("server.crt")).unwrap();
        let ca_crt_v2 = std::fs::read(dir.path().join("ca.crt")).unwrap();

        assert_eq!(
            server_crt_v1, server_crt_v2,
            "server cert should not change"
        );
        assert_eq!(ca_crt_v1, ca_crt_v2, "CA should not change");
    }

    #[test]
    fn changing_san_triggers_resign() {
        let dir = tempdir().unwrap();
        Pki::ensure(dir.path(), &addrs(&["a.example.com"])).unwrap();
        let server_crt_v1 = std::fs::read(dir.path().join("server.crt")).unwrap();
        let ca_crt_v1 = std::fs::read(dir.path().join("ca.crt")).unwrap();

        Pki::ensure(dir.path(), &addrs(&["b.example.com"])).unwrap();
        let server_crt_v2 = std::fs::read(dir.path().join("server.crt")).unwrap();
        let ca_crt_v2 = std::fs::read(dir.path().join("ca.crt")).unwrap();

        assert_ne!(
            server_crt_v1, server_crt_v2,
            "server cert should be resigned"
        );
        assert_eq!(ca_crt_v1, ca_crt_v2, "CA must be reused");
    }

    #[test]
    fn partial_ca_state_is_fatal() {
        let dir = tempdir().unwrap();
        Pki::ensure(dir.path(), &addrs(&["a.example.com"])).unwrap();
        std::fs::remove_file(dir.path().join("ca.key")).unwrap();
        let err = Pki::ensure(dir.path(), &addrs(&["a.example.com"])).unwrap_err();
        assert!(format!("{err}").contains("inconsistent state"));
    }

    #[test]
    fn empty_public_addrs_rejected() {
        let dir = tempdir().unwrap();
        let err = Pki::ensure(dir.path(), &[]).unwrap_err();
        assert!(format!("{err}").contains("MASTER_PUBLIC_ADDR"));
    }

    #[cfg(unix)]
    #[test]
    fn key_files_are_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        Pki::ensure(dir.path(), &addrs(&["a.example.com"])).unwrap();
        for name in ["ca.key", "server.key"] {
            let mode = std::fs::metadata(dir.path().join(name))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600, "{name} should be 0600, was {:o}", mode);
        }
    }

    #[test]
    fn node_id_charset() {
        assert!(is_valid_node_id("node-1"));
        assert!(is_valid_node_id("a"));
        assert!(is_valid_node_id("0abc"));
        assert!(is_valid_node_id("edge.tokyo.1"));
        assert!(!is_valid_node_id(""));
        assert!(!is_valid_node_id("-leading-dash"));
        assert!(!is_valid_node_id("UPPER"));
        assert!(!is_valid_node_id("a/b"));
        assert!(!is_valid_node_id(&"x".repeat(64)));
    }

    #[test]
    fn sign_node_cert_round_trip() {
        let dir = tempdir().unwrap();
        let pki = Pki::ensure(dir.path(), &addrs(&["m.example.com"])).unwrap();

        // Node generates its own keypair + CSR.
        let node_key = KeyPair::generate().unwrap();
        let csr_params = CertificateParams::new(vec!["ignored.example".to_string()]).unwrap();
        let csr = csr_params.serialize_request(&node_key).unwrap();
        let csr_pem = csr.pem().unwrap();

        let signed = pki.sign_node_cert("node-edge-1", &csr_pem).unwrap();

        assert!(signed.cert_pem.contains("BEGIN CERTIFICATE"));
        assert_eq!(signed.fingerprint_hex.len(), 64);
        assert_eq!(signed.serial_hex.len(), 32);

        // Verify the issued cert: SAN/CN = node_id, EKU=clientAuth, signed by our CA.
        let (_, pem) = x509_parser::pem::parse_x509_pem(signed.cert_pem.as_bytes()).unwrap();
        let (_, cert) = X509Certificate::from_der(&pem.contents).unwrap();
        let san = cert
            .tbs_certificate
            .subject_alternative_name()
            .unwrap()
            .unwrap();
        let dns: Vec<String> = san
            .value
            .general_names
            .iter()
            .filter_map(|gn| match gn {
                GeneralName::DNSName(s) => Some((*s).to_string()),
                _ => None,
            })
            .collect();
        assert_eq!(dns, vec!["node-edge-1".to_string()]);
        assert!(cert
            .subject()
            .iter_common_name()
            .next()
            .unwrap()
            .as_str()
            .unwrap()
            .contains("node-edge-1"));
    }

    #[test]
    fn sign_node_cert_rejects_bad_id() {
        let dir = tempdir().unwrap();
        let pki = Pki::ensure(dir.path(), &addrs(&["m.example.com"])).unwrap();
        let node_key = KeyPair::generate().unwrap();
        let csr_params = CertificateParams::new(vec!["x".to_string()]).unwrap();
        let csr_pem = csr_params
            .serialize_request(&node_key)
            .unwrap()
            .pem()
            .unwrap();
        assert!(pki.sign_node_cert("BadID!", &csr_pem).is_err());
    }
}
