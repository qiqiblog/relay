use std::time::Duration;

use sqlx::PgPool;

pub fn spawn(db: PgPool) {
    let db1 = db.clone();
    tokio::spawn(async move {
        loop {
            if let Err(e) = run(&db1).await {
                tracing::warn!(error = %e, "maintenance task failed");
            }
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
    });

    // Upgrade-job timeout sweeper: every 60s, mark in-flight jobs older than
    // 10 minutes as timed_out. Cheap enough to run frequently.
    let db2 = db;
    tokio::spawn(async move {
        loop {
            if let Err(e) = sweep_upgrade_timeouts(&db2).await {
                tracing::warn!(error = %e, "upgrade timeout sweeper failed");
            }
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    });
}

async fn run(db: &PgPool) -> sqlx::Result<()> {
    let deleted =
        sqlx::query("DELETE FROM node_availability WHERE recorded_at < now() - INTERVAL '90 days'")
            .execute(db)
            .await?
            .rows_affected();

    if deleted > 0 {
        tracing::info!(deleted, "pruned old node_availability records");
    }

    Ok(())
}

async fn sweep_upgrade_timeouts(db: &PgPool) -> sqlx::Result<()> {
    // 10 分钟没收到 succeeded → timed_out（仍在 in-flight 状态的）
    let n = sqlx::query(
        "UPDATE upgrade_jobs
            SET state = 'timed_out',
                error = COALESCE(error, '10min timeout waiting for new-version heartbeat'),
                completed_at = now()
          WHERE state IN ('queued','dispatched','accepted')
            AND requested_at < now() - INTERVAL '10 minutes'",
    )
    .execute(db)
    .await?
    .rows_affected();
    if n > 0 {
        tracing::warn!(jobs = n, "upgrade jobs timed out");
    }
    Ok(())
}
