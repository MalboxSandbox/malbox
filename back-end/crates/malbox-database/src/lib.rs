use malbox_config::core::DatabaseConfig;
pub use sqlx::Error;
pub use sqlx::PgPool;
pub use sqlx::error::DatabaseError;
use sqlx::postgres::PgPoolOptions;

pub mod error;
pub mod repositories;

/// Connect to the database and apply the embedded migrations. Errors are
/// returned rather than panicking so daemon startup can fail with an
/// actionable message.
pub async fn init_database(
    config: &DatabaseConfig,
) -> Result<sqlx::Pool<sqlx::Postgres>, sqlx::Error> {
    let db = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.host)
        .await?;

    sqlx::migrate!().run(&db).await?;

    Ok(db)
}

// TODO: Machine initialization will be handled by the new provider system
// Machines are now managed dynamically through the provider registry
