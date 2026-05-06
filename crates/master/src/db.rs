use anyhow::Result;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

pub async fn connect(url: &str) -> Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(16)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .connect(url)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;
    tracing::info!("database connected and migrations applied");
    Ok(pool)
}
