//! Background reconciliation task.
//!
//! Every `TICK` seconds it walks all forwards and reconciles the
//! deterministic pause-reasons (`user_disabled`, `user_expired`,
//! `user_tunnel_disabled`, `user_tunnel_expired`, `tunnel_disabled`)
//! against the current DB truth, plus the aggregate `tunnel_quota_exceeded`
//! reason per `user_tunnel`.
//!
//! Reasons owned by other code paths and **not** managed here:
//!   * `deploy_failed` — written by the config-ack failure handler.
//!
//! When a forward's reason set changes, the affected nodes get a
//! `tunnels_version` bump and a fresh config push.

use sqlx::PgPool;
use std::collections::{BTreeSet, HashSet};
use std::time::Duration;

use crate::pause::{
    self, REASON_TUNNEL_DISABLED, REASON_TUNNEL_QUOTA_EXCEEDED, REASON_USER_DISABLED,
    REASON_USER_EXPIRED, REASON_USER_TUNNEL_DISABLED, REASON_USER_TUNNEL_EXPIRED,
};
use crate::registry::NodeRegistry;

const TICK: Duration = Duration::from_secs(30);

/// Spawn a one-shot tick on the runtime; safe to call from request handlers
/// after a mutation that may have changed lifecycle state.
pub fn kick(db: PgPool, registry: NodeRegistry) {
    tokio::spawn(async move {
        if let Err(e) = tick(&db, &registry).await {
            tracing::warn!(error = %e, "scheduler kick failed");
        }
    });
}

/// Spawn the reconciliation loop. Runs forever; logs and continues on errors.
pub fn spawn(db: PgPool, registry: NodeRegistry) {
    tokio::spawn(async move {
        // Run immediately at boot, then every TICK.
        loop {
            if let Err(e) = tick(&db, &registry).await {
                tracing::warn!(error = %e, "scheduler tick failed");
            }
            tokio::time::sleep(TICK).await;
        }
    });
}

/// One reconciliation pass. Public so other code paths (e.g. admin CRUD that
/// changes user/tunnel/user_tunnel state) can request an immediate pass.
pub async fn tick(db: &PgPool, registry: &NodeRegistry) -> sqlx::Result<()> {
    let mut changed: HashSet<i64> = HashSet::new();

    reconcile_lifecycle_reasons(db, &mut changed).await?;
    reconcile_quota_reasons(db, &mut changed).await?;

    if changed.is_empty() {
        return Ok(());
    }

    let forwards: Vec<i64> = changed.into_iter().collect();
    bump_and_push(db, registry, &forwards).await;
    Ok(())
}

/// Reconcile the deterministic per-forward reasons derived from
/// users / user_tunnels / tunnels lifecycle columns.
async fn reconcile_lifecycle_reasons(db: &PgPool, changed: &mut HashSet<i64>) -> sqlx::Result<()> {
    #[derive(sqlx::FromRow)]
    struct Row {
        forward_id: i64,
        user_status: String,
        user_expired: bool,
        ut_enabled: bool,
        ut_expired: bool,
        tunnel_enabled: bool,
    }

    // SECURITY: this query joins through user_tunnels — any forward whose
    // user_tunnel was deleted is already gone via FK ON DELETE RESTRICT
    // (delete is blocked) so we never see orphans here.
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT f.id                                            AS forward_id,
                u.status                                        AS user_status,
                (u.expires_at IS NOT NULL AND u.expires_at <= now()) AS user_expired,
                ut.enabled                                      AS ut_enabled,
                (ut.expires_at IS NOT NULL AND ut.expires_at <= now()) AS ut_expired,
                t.enabled                                       AS tunnel_enabled
           FROM forwards f
           JOIN user_tunnels ut ON ut.id = f.user_tunnel_id
           JOIN tunnels t       ON t.id  = ut.tunnel_id
           JOIN users u         ON u.id  = ut.user_id",
    )
    .fetch_all(db)
    .await?;

    for r in rows {
        // Compute expected set of lifecycle reasons.
        let mut expected: BTreeSet<&str> = BTreeSet::new();
        if r.user_status == "disabled" {
            expected.insert(REASON_USER_DISABLED);
        }
        if r.user_status == "expired" || r.user_expired {
            expected.insert(REASON_USER_EXPIRED);
        }
        if !r.ut_enabled {
            expected.insert(REASON_USER_TUNNEL_DISABLED);
        }
        if r.ut_expired {
            expected.insert(REASON_USER_TUNNEL_EXPIRED);
        }
        if !r.tunnel_enabled {
            expected.insert(REASON_TUNNEL_DISABLED);
        }

        // Diff against current. We only manage the lifecycle reasons here;
        // do not touch tunnel_quota_exceeded or deploy_failed.
        let current: BTreeSet<String> = pause::list_pause_reasons(db, r.forward_id)
            .await?
            .into_iter()
            .filter(|s| {
                matches!(
                    s.as_str(),
                    REASON_USER_DISABLED
                        | REASON_USER_EXPIRED
                        | REASON_USER_TUNNEL_DISABLED
                        | REASON_USER_TUNNEL_EXPIRED
                        | REASON_TUNNEL_DISABLED
                )
            })
            .collect();

        // Add missing.
        for reason in &expected {
            if !current.contains(*reason)
                && pause::write_pause_reason(db, r.forward_id, reason).await?
            {
                changed.insert(r.forward_id);
            }
        }
        // Remove stale.
        for reason in &current {
            if !expected.contains(reason.as_str())
                && pause::clear_pause_reason(db, r.forward_id, reason).await?
            {
                changed.insert(r.forward_id);
            }
        }
    }

    // Best-effort: also flip users.status from 'active' to 'expired' when
    // expires_at has passed. The reason write above covers the runtime
    // guard regardless of this column value, but keeping the column in
    // sync makes the UI honest.
    let _ = sqlx::query(
        "UPDATE users
            SET status = 'expired'
          WHERE status = 'active'
            AND expires_at IS NOT NULL
            AND expires_at <= now()",
    )
    .execute(db)
    .await;

    Ok(())
}

/// Per-user_tunnel quota check. Writes/clears `tunnel_quota_exceeded` on every
/// forward in a user_tunnel based on the aggregated (in+out) bytes.
async fn reconcile_quota_reasons(db: &PgPool, changed: &mut HashSet<i64>) -> sqlx::Result<()> {
    #[derive(sqlx::FromRow)]
    struct Row {
        ut_id: i64,
        flow_limit_bytes: i64,
        used_bytes: i64,
    }

    let rows: Vec<Row> = sqlx::query_as(
        "SELECT ut.id                                                       AS ut_id,
                ut.flow_limit_bytes                                         AS flow_limit_bytes,
                COALESCE(SUM(f.in_flow_bytes + f.out_flow_bytes), 0)::BIGINT AS used_bytes
           FROM user_tunnels ut
           LEFT JOIN forwards f ON f.user_tunnel_id = ut.id
          GROUP BY ut.id, ut.flow_limit_bytes",
    )
    .fetch_all(db)
    .await?;

    for r in rows {
        // Resolve the affected forward set once per user_tunnel.
        let forwards: Vec<(i64,)> =
            sqlx::query_as("SELECT id FROM forwards WHERE user_tunnel_id = $1")
                .bind(r.ut_id)
                .fetch_all(db)
                .await
                .unwrap_or_default();

        let exceeded = r.flow_limit_bytes > 0 && r.used_bytes >= r.flow_limit_bytes;
        for (fid,) in forwards {
            if exceeded {
                if pause::write_pause_reason(db, fid, REASON_TUNNEL_QUOTA_EXCEEDED).await? {
                    changed.insert(fid);
                }
            } else if pause::clear_pause_reason(db, fid, REASON_TUNNEL_QUOTA_EXCEEDED).await? {
                changed.insert(fid);
            }
        }
    }

    Ok(())
}

async fn bump_and_push(db: &PgPool, registry: &NodeRegistry, forward_ids: &[i64]) {
    if forward_ids.is_empty() {
        return;
    }
    let nodes: Vec<(String,)> = match sqlx::query_as(
        "SELECT DISTINCT node_id FROM forward_ports WHERE forward_id = ANY($1)",
    )
    .bind(forward_ids)
    .fetch_all(db)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "scheduler bump_and_push: fetch nodes failed");
            return;
        }
    };
    for (nid,) in &nodes {
        let _ = sqlx::query("UPDATE nodes SET tunnels_version = tunnels_version + 1 WHERE id = $1")
            .bind(nid)
            .execute(db)
            .await;
        registry.push_config(db, nid).await;
    }
}
