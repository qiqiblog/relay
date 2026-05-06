//! Forward pause-reason state machine and effective-enabled computation.

use sqlx::PgPool;

#[allow(dead_code)]
pub const REASON_TUNNEL_QUOTA_EXCEEDED: &str = "tunnel_quota_exceeded";
#[allow(dead_code)]
pub const REASON_USER_TUNNEL_EXPIRED: &str = "user_tunnel_expired";
#[allow(dead_code)]
pub const REASON_USER_TUNNEL_DISABLED: &str = "user_tunnel_disabled";
#[allow(dead_code)]
pub const REASON_TUNNEL_DISABLED: &str = "tunnel_disabled";
#[allow(dead_code)]
pub const REASON_USER_DISABLED: &str = "user_disabled";
#[allow(dead_code)]
pub const REASON_USER_EXPIRED: &str = "user_expired";
#[allow(dead_code)]
pub const REASON_DEPLOY_FAILED: &str = "deploy_failed";

pub async fn write_pause_reason(db: &PgPool, forward_id: i64, reason: &str) -> sqlx::Result<bool> {
    let row: Option<(i64,)> = sqlx::query_as(
        "INSERT INTO forward_pause_reasons (forward_id, reason)
         VALUES ($1, $2)
         ON CONFLICT (forward_id, reason) DO NOTHING
         RETURNING forward_id",
    )
    .bind(forward_id)
    .bind(reason)
    .fetch_optional(db)
    .await?;
    Ok(row.is_some())
}

pub async fn clear_pause_reason(db: &PgPool, forward_id: i64, reason: &str) -> sqlx::Result<bool> {
    let res =
        sqlx::query("DELETE FROM forward_pause_reasons WHERE forward_id = $1 AND reason = $2")
            .bind(forward_id)
            .bind(reason)
            .execute(db)
            .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn list_pause_reasons(db: &PgPool, forward_id: i64) -> sqlx::Result<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT reason FROM forward_pause_reasons WHERE forward_id = $1 ORDER BY reason",
    )
    .bind(forward_id)
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(|(r,)| r).collect())
}

/// `(effective_enabled, pause_reasons)`.
pub async fn compute_effective_enabled(
    db: &PgPool,
    forward_id: i64,
) -> sqlx::Result<(bool, Vec<String>)> {
    #[allow(clippy::type_complexity)]
    let row: Option<(
        bool,                                  // forwards.desired_enabled
        bool,                                  // tunnels.enabled
        bool,                                  // user_tunnels.enabled
        Option<chrono::DateTime<chrono::Utc>>, // user_tunnels.expires_at
        String,                                // users.status
        Option<chrono::DateTime<chrono::Utc>>, // users.expires_at
    )> = sqlx::query_as(
        "SELECT f.desired_enabled, t.enabled, ut.enabled, ut.expires_at, u.status, u.expires_at
           FROM forwards f
           JOIN user_tunnels ut ON ut.id = f.user_tunnel_id
           JOIN tunnels t       ON t.id  = ut.tunnel_id
           JOIN users u         ON u.id  = ut.user_id
          WHERE f.id = $1",
    )
    .bind(forward_id)
    .fetch_optional(db)
    .await?;
    let Some((desired, tunnel_enabled, ut_enabled, ut_expires, user_status, user_expires)) = row
    else {
        return Ok((false, Vec::new()));
    };
    let reasons = list_pause_reasons(db, forward_id).await?;
    let now = chrono::Utc::now();
    let user_active = user_status == "active" && user_expires.map(|t| t > now).unwrap_or(true);
    let ut_active = ut_enabled && ut_expires.map(|t| t > now).unwrap_or(true);
    let effective = desired && reasons.is_empty() && tunnel_enabled && ut_active && user_active;
    Ok((effective, reasons))
}
