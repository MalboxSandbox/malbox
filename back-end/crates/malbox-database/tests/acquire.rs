//! `acquire_machine_for` against a real Postgres. Gated on
//! `MALBOX_TEST_DATABASE_URL` (an admin URL); each test creates and drops
//! its own `malbox_test_{uuid}` database.

mod common;

use common::TestDb;
use malbox_database::repositories::machinery::{
    AcquireRow, MachineFilter, MachinePlatform, acquire_machine_for,
};
use sqlx::PgPool;

async fn seed_machine(pool: &PgPool, name: &str, platform: &str, tags: Option<&[&str]>) -> i32 {
    let tags: Option<Vec<String>> = tags.map(|t| t.iter().map(ToString::to_string).collect());
    sqlx::query_scalar(
        "insert into machines (name, arch, platform, tags, status) \
         values ($1, 'x64', $2::machine_platform, $3, 'ready') returning id",
    )
    .bind(name)
    .bind(platform)
    .bind(tags)
    .fetch_one(pool)
    .await
    .expect("insert machine")
}

async fn seed_snapshot(
    pool: &PgPool,
    machine_id: i32,
    name: &str,
    guest_plugins: serde_json::Value,
) {
    sqlx::query(
        "insert into machine_snapshots (machine_id, name, provider_snapshot_id, guest_plugins, is_active) \
         values ($1, $2, $2, $3, true)",
    )
    .bind(machine_id)
    .bind(name)
    .bind(guest_plugins)
    .execute(pool)
    .await
    .expect("insert snapshot");
}

#[tokio::test]
async fn acquire_machine_for_matches_tags_and_guest_plugins() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let pool = db.pool.clone();
    let task: i32 = sqlx::query_scalar(
        "insert into tasks (target, max_depth, max_artifacts, max_detonations, max_total_bytes, \
         created_on, deadline) values ('t', 1, 1, 1, 1, now(), now()) returning id",
    )
    .fetch_one(&pool)
    .await
    .expect("insert task");

    let win = seed_machine(&pool, "win", "windows", Some(&["office"])).await;
    seed_snapshot(&pool, win, "s1", serde_json::json!(["mon"])).await;
    let linux = seed_machine(&pool, "linux", "linux", None).await;
    seed_snapshot(&pool, linux, "base", serde_json::json!([])).await;

    let office = MachineFilter {
        platform: Some(MachinePlatform::Windows),
        tags: vec!["office".into()],
        required_guest_plugins: vec!["mon".into()],
        ..Default::default()
    };

    match acquire_machine_for(&pool, &office, task)
        .await
        .expect("acquire")
    {
        AcquireRow::Acquired { machine_id, .. } => assert_eq!(machine_id, win),
        other => panic!("expected Acquired, got {other:?}"),
    }
    let status: String = sqlx::query_scalar("select status::text from machines where id = $1")
        .bind(win)
        .fetch_one(&pool)
        .await
        .expect("status");
    assert_eq!(status, "assigned");

    assert_eq!(
        acquire_machine_for(&pool, &office, task)
            .await
            .expect("acquire"),
        AcquireRow::Busy
    );

    let gpu = MachineFilter {
        platform: Some(MachinePlatform::Windows),
        tags: vec!["gpu".into()],
        ..Default::default()
    };
    assert_eq!(
        acquire_machine_for(&pool, &gpu, task)
            .await
            .expect("acquire"),
        AcquireRow::Unserviceable
    );

    db.drop().await;
}
