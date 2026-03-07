use malbox_config::core::DatabaseConfig;
pub use sqlx::Error;
pub use sqlx::PgPool;
pub use sqlx::error::DatabaseError;
use sqlx::postgres::PgPoolOptions;

pub mod error;
pub mod repositories;

// NOTE: Unwrap here or later?
pub async fn init_database(config: &DatabaseConfig) -> sqlx::Pool<sqlx::Postgres> {
    let db = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.host)
        .await
        .unwrap();

    sqlx::migrate!().run(&db).await.unwrap();

    db
}

// TODO: Machine initialization will be handled by the new provider system
// Machines are now managed dynamically through the provider registry
