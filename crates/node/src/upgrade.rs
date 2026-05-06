//! Node-side upgrade command handler.
//!
//! Receives `Command{kind=UPGRADE_AGENT}` from master, validates the args,
//! atomically writes a request file the root-level `relay-node-updater`
//! systemd path unit watches, and acks back via `UpgradeReport`.

use relay_proto::v1::{node_message::Payload as NodePayload, Command, NodeMessage, UpgradeReport};
use serde::Serialize;
use std::sync::OnceLock;
use tokio::sync::{mpsc, Mutex};

const REQUEST_PATH: &str = "/var/lib/relay-node/upgrade-request.json";

/// Serialize concurrent UPGRADE_AGENT commands so two in-flight jobs can't
/// race writing the same request file. Master also has a partial unique
/// index, but defense-in-depth.
fn upgrade_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[derive(Serialize)]
struct UpgradeRequestFile<'a> {
    job_id: i64,
    tag: &'a str,
    asset_url_amd64: &'a str,
    asset_url_arm64: &'a str,
    sha256_url: &'a str,
}

pub async fn handle_upgrade_command(cmd: Command, tx: mpsc::Sender<NodeMessage>) {
    let job_id_s = cmd.args.get("job_id").cloned().unwrap_or_default();
    let tag = cmd.args.get("tag").cloned().unwrap_or_default();
    let amd64_url = cmd.args.get("asset_url_amd64").cloned().unwrap_or_default();
    let arm64_url = cmd.args.get("asset_url_arm64").cloned().unwrap_or_default();
    let sha256_url = cmd.args.get("sha256_url").cloned().unwrap_or_default();

    let job_id: i64 = match job_id_s.parse() {
        Ok(n) => n,
        Err(_) => {
            tracing::warn!(job_id_s, "upgrade: invalid job_id");
            return;
        }
    };

    if let Err(reason) = validate(&tag, &amd64_url, &arm64_url, &sha256_url) {
        tracing::warn!(%tag, %reason, "upgrade: rejected");
        send_report(&tx, job_id, "failed", &reason).await;
        return;
    }

    // Hold the per-process lock for the entire write so a second concurrent
    // command can't interleave temp-file rename with this one.
    let _guard = upgrade_lock().lock().await;

    // If the updater hasn't picked up a previous request yet, refuse.
    if std::path::Path::new(REQUEST_PATH).exists() {
        tracing::warn!("upgrade: refused, previous request not yet consumed");
        send_report(
            &tx,
            job_id,
            "failed",
            "previous upgrade request still pending on this node",
        )
        .await;
        return;
    }

    let body = UpgradeRequestFile {
        job_id,
        tag: &tag,
        asset_url_amd64: &amd64_url,
        asset_url_arm64: &arm64_url,
        sha256_url: &sha256_url,
    };
    let json = match serde_json::to_string_pretty(&body) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "upgrade: serialize failed");
            send_report(&tx, job_id, "failed", &format!("serialize: {e}")).await;
            return;
        }
    };

    if let Err(e) = atomic_write(job_id, &json) {
        tracing::error!(error = %e, "upgrade: write request file failed");
        send_report(&tx, job_id, "failed", &format!("write: {e}")).await;
        return;
    }

    tracing::info!(job_id, %tag, "upgrade: request written; updater will pick it up");
    send_report(&tx, job_id, "accepted", "").await;
}

fn validate(tag: &str, amd64_url: &str, arm64_url: &str, sha256_url: &str) -> Result<(), String> {
    if !valid_tag(tag) {
        return Err(format!("malformed tag: {tag}"));
    }
    for url in [amd64_url, arm64_url, sha256_url] {
        if !valid_gh_url(url) {
            return Err(format!("disallowed url host: {url}"));
        }
    }
    Ok(())
}

fn valid_tag(tag: &str) -> bool {
    let Some(rest) = tag.strip_prefix('v') else {
        return false;
    };
    let (ver, rc) = match rest.find("-rc.") {
        Some(i) => (&rest[..i], Some(&rest[i + 4..])),
        None => (rest, None),
    };
    let parts: Vec<&str> = ver.split('.').collect();
    if parts.len() != 3 {
        return false;
    }
    if !parts
        .iter()
        .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
    {
        return false;
    }
    if let Some(rc) = rc {
        if rc.is_empty() {
            return false;
        }
        if !rc.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.') {
            return false;
        }
    }
    true
}

fn valid_gh_url(url: &str) -> bool {
    url.starts_with("https://github.com/")
        || url.starts_with("https://objects.githubusercontent.com/")
}

fn atomic_write(job_id: i64, json: &str) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(parent) = std::path::Path::new(REQUEST_PATH).parent() {
        // Best-effort: parent should already exist (StateDirectory).
        let _ = std::fs::create_dir_all(parent);
    }
    // Per-job tmp filename so two concurrent writers (defense in depth — the
    // upgrade_lock should already serialize them) can't trample each other.
    let tmp = format!("{REQUEST_PATH}.tmp.{job_id}");
    let mut f = std::fs::File::create(&tmp)?;
    f.write_all(json.as_bytes())?;
    f.sync_all()?;
    drop(f);
    std::fs::rename(&tmp, REQUEST_PATH)?;
    Ok(())
}

async fn send_report(tx: &mpsc::Sender<NodeMessage>, job_id: i64, state: &str, error: &str) {
    let msg = NodeMessage {
        payload: Some(NodePayload::UpgradeReport(UpgradeReport {
            job_id,
            state: state.to_string(),
            error: error.to_string(),
        })),
    };
    if tx.send(msg).await.is_err() {
        tracing::warn!("upgrade: outbound channel closed before report send");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bad_url() {
        assert!(validate(
            "v0.2.0",
            "https://evil.example.com/relay-v0.2.0-x86_64-unknown-linux-gnu.tar.gz",
            "https://github.com/foo/bar/releases/download/v0.2.0/relay-v0.2.0-aarch64-unknown-linux-gnu.tar.gz",
            "https://github.com/foo/bar/releases/download/v0.2.0/SHA256SUMS"
        )
        .is_err());
    }
    #[test]
    fn accepts_gh_urls() {
        assert!(validate(
            "v0.2.0",
            "https://github.com/foo/bar/releases/download/v0.2.0/relay-v0.2.0-x86_64-unknown-linux-gnu.tar.gz",
            "https://github.com/foo/bar/releases/download/v0.2.0/relay-v0.2.0-aarch64-unknown-linux-gnu.tar.gz",
            "https://github.com/foo/bar/releases/download/v0.2.0/SHA256SUMS"
        )
        .is_ok());
        assert!(validate(
            "v0.2.0-rc.20260430",
            "https://objects.githubusercontent.com/foo",
            "https://objects.githubusercontent.com/bar",
            "https://github.com/foo/bar/SHA256SUMS"
        )
        .is_ok());
    }
    #[test]
    fn rejects_bad_tag() {
        assert!(!valid_tag("0.2.0"));
        assert!(!valid_tag("v0.2"));
        assert!(!valid_tag("v0.2.0-beta"));
    }
}
