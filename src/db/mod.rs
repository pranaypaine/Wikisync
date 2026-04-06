use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use anyhow::Result;
use std::str::FromStr;

pub async fn create_pool(database_url: &str) -> Result<SqlitePool> {
    let opts = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true);

    let pool = SqlitePool::connect_with(opts).await?;

    sqlx::query("PRAGMA journal_mode=WAL").execute(&pool).await?;
    sqlx::query("PRAGMA synchronous=NORMAL").execute(&pool).await?;
    sqlx::query("PRAGMA foreign_keys=ON").execute(&pool).await?;
    sqlx::query("PRAGMA cache_size=-64000").execute(&pool).await?;
    sqlx::query("PRAGMA temp_store=MEMORY").execute(&pool).await?;

    run_migrations(&pool).await?;
    Ok(pool)
}

async fn run_migrations(pool: &SqlitePool) -> Result<()> {
    for migration_sql in &[
        include_str!("../../migrations/001_init.sql"),
        include_str!("../../migrations/002_versions.sql"),
    ] {
        for stmt in migration_sql.split(';') {
            let stmt = stmt.trim();
            if !stmt.is_empty() {
                sqlx::query(stmt).execute(pool).await?;
            }
        }
    }
    Ok(())
}
