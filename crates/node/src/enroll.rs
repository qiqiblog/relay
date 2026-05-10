use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use base64::Engine;
use rcgen::{CertificateParams, KeyPair};

pub struct EnrollInput {
    pub pki_dir: PathBuf,
    pub node_id: String,
    pub token: String,
    pub master_enroll_endpoint: String,
    pub ca_cert_pem: String,
}

pub fn pki_complete(pki_dir: &Path) -> bool {
    pki_dir.join("ca.crt").exists()
        && pki_dir.join("node.crt").exists()
        && pki_dir.join("node.key").exists()
}

pub fn decode_ca_cert(b64: &str) -> Result<String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64.trim().as_bytes())
        .context("NODE_CA_CERT_B64 not valid base64")?;
    let s = String::from_utf8(bytes).context("CA cert bytes not UTF-8")?;
    if !s.contains("BEGIN CERTIFICATE") {
        bail!("decoded CA does not look like a PEM certificate");
    }
    Ok(s)
}

/// Build a PEM-encoded CSR for `node_id` from `key`. Subject/SAN are placeholders
/// — the master rebuilds them from the authenticated `node_id`.
pub fn build_csr(node_id: &str, key: &KeyPair) -> Result<String> {
    let mut csr_params = CertificateParams::new(vec![node_id.to_string()]).context("CSR params")?;
    csr_params
        .distinguished_name
        .push(rcgen::DnType::CommonName, node_id);
    let csr = csr_params
        .serialize_request(key)
        .context("serializing CSR")?;
    csr.pem().context("CSR pem")
}

/// Atomic + 0600 secret write (tempfile + rename). Public so the cert
/// renewer in `main.rs` can reuse it.
pub fn write_secret(path: &Path, contents: &[u8]) -> Result<()> {
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
        f.write_all(contents)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)
        .with_context(|| format!("rename {} → {}", tmp.display(), path.display()))?;
    Ok(())
}

pub async fn enroll(input: EnrollInput) -> Result<()> {
    fs::create_dir_all(&input.pki_dir)
        .with_context(|| format!("creating pki dir {}", input.pki_dir.display()))?;

    let key = KeyPair::generate().context("generating node keypair")?;
    let key_pem = key.serialize_pem();
    let csr_pem = build_csr(&input.node_id, &key)?;

    // Add the pinned relay CA so HTTPS enrollment validates against it;
    // reqwest also keeps the system root store, so Let's Encrypt / Caddy works too.
    let ca_cert =
        reqwest::Certificate::from_pem(input.ca_cert_pem.as_bytes()).context("parsing CA cert")?;
    let client = reqwest::ClientBuilder::new()
        .add_root_certificate(ca_cert)
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("building HTTP client")?;

    #[derive(serde::Serialize)]
    struct Req<'a> {
        node_id: &'a str,
        enrollment_token: &'a str,
        csr_pem: &'a str,
    }
    #[derive(serde::Deserialize)]
    struct Resp {
        node_cert_pem: String,
        ca_cert_pem: String,
        not_after_unix_ms: i64,
    }

    let http_resp = client
        .post(&input.master_enroll_endpoint)
        .json(&Req {
            node_id: &input.node_id,
            enrollment_token: &input.token,
            csr_pem: &csr_pem,
        })
        .send()
        .await
        .with_context(|| format!("POST {}", input.master_enroll_endpoint))?;

    if !http_resp.status().is_success() {
        let status = http_resp.status();
        let body = http_resp.text().await.unwrap_or_default();
        bail!("enrollment failed: HTTP {status}: {body}");
    }

    let resp: Resp = http_resp.json().await.context("parsing enroll response")?;

    if resp.node_cert_pem.is_empty() || resp.ca_cert_pem.is_empty() {
        bail!("master returned empty cert(s)");
    }

    // Belt-and-braces: CA in response must match the one pinned at install time.
    if resp.ca_cert_pem.trim() != input.ca_cert_pem.trim() {
        bail!("CA returned by master differs from the one pinned at install time");
    }

    write_secret(&input.pki_dir.join("ca.crt"), resp.ca_cert_pem.as_bytes())?;
    write_secret(
        &input.pki_dir.join("node.crt"),
        resp.node_cert_pem.as_bytes(),
    )?;
    write_secret(&input.pki_dir.join("node.key"), key_pem.as_bytes())?;

    tracing::info!(
        node_id = %input.node_id,
        not_after_unix_ms = resp.not_after_unix_ms,
        "enrollment complete"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pki_complete_check() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!pki_complete(dir.path()));
        for n in ["ca.crt", "node.crt", "node.key"] {
            std::fs::write(dir.path().join(n), b"x").unwrap();
        }
        assert!(pki_complete(dir.path()));
    }

    #[test]
    fn decode_ca_round_trip() {
        let pem = "-----BEGIN CERTIFICATE-----\nABCD\n-----END CERTIFICATE-----\n";
        let b64 = base64::engine::general_purpose::STANDARD.encode(pem);
        assert_eq!(decode_ca_cert(&b64).unwrap(), pem);
        assert!(decode_ca_cert("not-base64!@#").is_err());
        let bad_pem = base64::engine::general_purpose::STANDARD.encode(b"not a cert");
        assert!(decode_ca_cert(&bad_pem).is_err());
    }
}
