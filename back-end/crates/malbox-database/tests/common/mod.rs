//! Throwaway-database harness shared by the DB integration tests.

use malbox_database::connect_and_migrate;
use sqlx::{AssertSqlSafe, Connection, PgConnection, PgPool};

pub struct TestDb {
    admin_url: String,
    name: String,
    pub pool: PgPool,
}

impl TestDb {
    pub async fn create() -> Option<Self> {
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

    pub async fn drop(self) {
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
