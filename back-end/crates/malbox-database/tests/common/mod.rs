//! Throwaway-database harness shared by the DB integration tests.

use malbox_database::connect_and_migrate;
use sqlx::{AssertSqlSafe, Connection, PgConnection, PgPool};

/// A freshly created, migrated database that is dropped by [`TestDb::drop`].
pub(crate) struct TestDb {
    admin_url: String,
    name: String,
    pub(crate) pool: PgPool,
}

impl TestDb {
    /// Create and migrate a uniquely named database, or `None` (skipping the
    /// test) when `MALBOX_TEST_DATABASE_URL` is unset.
    #[expect(
        clippy::print_stdout,
        reason = "tells the test runner output why the test was skipped"
    )]
    pub(crate) async fn create() -> Option<Self> {
        let Ok(admin_url) = std::env::var("MALBOX_TEST_DATABASE_URL") else {
            println!("skipping: MALBOX_TEST_DATABASE_URL not set");
            return None;
        };
        let name = format!("malbox_test_{}", uuid::Uuid::new_v4().simple());
        let mut admin = PgConnection::connect(&admin_url)
            .await
            .expect("admin connect");
        sqlx::query(AssertSqlSafe(format!("create database \"{name}\"")))
            .execute(&mut admin)
            .await
            .expect("create database");
        admin.close().await.expect("close admin");

        let (base, _) = admin_url.rsplit_once('/').expect("url has a database path");
        let pool = connect_and_migrate(&format!("{base}/{name}"))
            .await
            .expect("connect and migrate");
        Some(Self {
            admin_url,
            name,
            pool,
        })
    }

    /// Close the pool and drop the database.
    pub(crate) async fn drop(self) {
        self.pool.close().await;
        let mut admin = PgConnection::connect(&self.admin_url)
            .await
            .expect("admin connect");
        sqlx::query(AssertSqlSafe(format!(
            "drop database if exists \"{}\" with (force)",
            self.name
        )))
        .execute(&mut admin)
        .await
        .expect("drop database");
    }
}
