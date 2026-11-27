//! Task/artifact/job/report/cache repositories against a real Postgres.
//! Gated on `MALBOX_TEST_DATABASE_URL`.

mod common;

use common::TestDb;
use malbox_database::repositories::plugin_reports::{self, DbClassification, InsertPluginReport};
use malbox_database::repositories::tasks::{self, NewTask, TaskFinalization, TaskState};
use malbox_database::repositories::{artifacts, jobs, result_cache};
use time::OffsetDateTime;

async fn seed_sample(pool: &sqlx::PgPool, sha: &str) -> i64 {
    sqlx::query_scalar::<_, i32>(
        "insert into samples (file_size, file_type, md5, crc32, sha1, sha256, sha512, ssdeep) \
         values (3, 'data', $1, $1, $1, $1, $1, '') returning id",
    )
    .bind(sha)
    .fetch_one(pool)
    .await
    .expect("insert sample") as i64
}

#[tokio::test]
async fn task_artifact_job_lifecycle() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let pool = db.pool.clone();
    let sample_id = seed_sample(&pool, "aa").await;

    let empty = serde_json::json!({});
    let mut tx = pool.begin().await.unwrap();
    let task = tasks::insert_task(
        &mut tx,
        &NewTask {
            target: "a.zip",
            plugins: &[],
            excluded_plugins: &[],
            timeout: 300,
            priority: 1,
            machine_id: None,
            platform: None,
            snapshot_id: None,
            params: &empty,
            max_depth: 6,
            max_artifacts: 10,
            max_detonations: 1,
            max_total_bytes: 1 << 20,
            deadline: OffsetDateTime::now_utc(),
            sample_id: Some(sample_id),
            parent_task_id: None,
            owner: None,
            tags: None,
            shared_data: &empty,
        },
    )
    .await
    .expect("insert task");
    let root_new = artifacts::NewArtifact {
        task_id: task.id,
        sha256: "aa",
        size_bytes: 3,
        storage_path: Some("samples/aa"),
        analyze: true,
        sample_id: Some(sample_id),
        depth: 0,
        artifact_type: "archive/zip",
        mime: "application/zip",
        state: "pending",
        drop_reason: None,
        priority: 1,
        analyze_requested: false,
        dynamic_recursion_bypass: false,
    };
    let (root, created) = artifacts::insert_or_get_artifact(&mut tx, &root_new)
        .await
        .expect("root");
    assert!(created);
    tx.commit().await.unwrap();
    assert_eq!(task.status, TaskState::Pending);
    assert_eq!(
        tasks::fetch_active_task_ids(&pool).await.unwrap(),
        vec![task.id]
    );

    let mut conn = pool.acquire().await.unwrap();
    let (again, created) = artifacts::insert_or_get_artifact(&mut conn, &root_new)
        .await
        .unwrap();
    assert!(!created);
    assert_eq!(again.id, root.id);

    let (child, _) = artifacts::insert_or_get_artifact(
        &mut conn,
        &artifacts::NewArtifact {
            sha256: "bb",
            storage_path: Some("samples/bb"),
            analyze: false,
            sample_id: None,
            depth: 1,
            state: "stored",
            analyze_requested: true,
            ..root_new.clone()
        },
    )
    .await
    .unwrap();
    artifacts::set_analyze_requested(&mut *conn, child.id, true)
        .await
        .unwrap();
    artifacts::set_artifact_admission_dropped(&mut *conn, child.id, "max_depth")
        .await
        .unwrap();

    let job = jobs::insert_job(
        &mut *conn,
        &jobs::NewJob {
            task_id: task.id,
            artifact_id: root.id,
            kind: "host",
            analyzer: "extractor",
            stage: "extract",
            priority: 1,
            state: "blocked",
            max_attempts: 3,
            ignore_cache: false,
            cache_key: Some("k"),
            snapshot_id: None,
            guest_plugins: None,
            detonation: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(jobs::release_jobs(&mut *conn, &[job.id]).await.unwrap(), 1);
    let claimed = jobs::claim_host_job(&pool, &[], 8)
        .await
        .unwrap()
        .expect("claim");
    assert_eq!(
        (claimed.id, claimed.attempts, claimed.state.as_str()),
        (job.id, 1, "running")
    );
    assert!(jobs::claim_host_job(&pool, &[], 8).await.unwrap().is_none());

    artifacts::insert_edge(
        &mut *conn,
        task.id,
        root.id,
        child.id,
        "extracted",
        Some(job.id),
        Some("extractor"),
        Some("b.exe"),
    )
    .await
    .unwrap();
    let from_job = artifacts::fetch_edges_from_job(&pool, job.id)
        .await
        .unwrap();
    assert_eq!(from_job.len(), 1);
    assert_eq!(from_job[0].0.relation, "extracted");
    assert_eq!(from_job[0].1.state, "dropped");
    assert!(from_job[0].1.dynamic_recursion_bypass);

    jobs::schedule_retry(
        &mut *conn,
        job.id,
        "recoverable",
        "x",
        OffsetDateTime::now_utc(),
    )
    .await
    .unwrap();
    jobs::claim_job_by_id(&mut *conn, job.id)
        .await
        .unwrap()
        .expect("reclaim");
    assert_eq!(jobs::recover_running_jobs(&pool).await.unwrap(), 1);
    jobs::set_job_state(&mut *conn, job.id, "completed", None, None)
        .await
        .unwrap();
    let done = jobs::fetch_job(&pool, job.id).await.unwrap().unwrap();
    assert!(done.is_terminal() && done.completed_on.is_some());

    let labels: Vec<String> = vec![];
    let report = plugin_reports::upsert_plugin_report(
        &pool,
        &InsertPluginReport {
            task_id: task.id,
            artifact_id: root.id,
            job_id: Some(job.id),
            report_path: "results/1/1/extractor/report.json",
            plugin_name: "extractor",
            display_name: None,
            plugin_version: "1",
            classification: Some(DbClassification::Suspicious),
            score: Some(50),
            confidence: None,
            labels: &labels,
            indicators: &serde_json::json!([]),
            ttps: &serde_json::json!([]),
            summary: None,
            section_count: 0,
            artifact_count: 1,
        },
    )
    .await
    .unwrap();
    // A second upsert replaces rather than failing.
    let again = plugin_reports::upsert_plugin_report(
        &pool,
        &InsertPluginReport {
            task_id: task.id,
            artifact_id: root.id,
            job_id: Some(job.id),
            report_path: "results/1/1/extractor/report.json",
            plugin_name: "extractor",
            display_name: None,
            plugin_version: "2",
            classification: None,
            score: None,
            confidence: None,
            labels: &labels,
            indicators: &serde_json::json!([]),
            ttps: &serde_json::json!([]),
            summary: None,
            section_count: 0,
            artifact_count: 1,
        },
    )
    .await
    .unwrap();
    assert_eq!(report.id, again.id);
    assert_eq!(again.plugin_version, "2");

    result_cache::store(&pool, "k", job.id, false, 60)
        .await
        .unwrap();
    assert_eq!(
        result_cache::lookup(&pool, "k").await.unwrap(),
        Some((job.id, false))
    );
    assert_eq!(result_cache::purge_expired(&pool).await.unwrap(), 0);

    let mut shared_tx = pool.begin().await.unwrap();
    let shared = artifacts::fetch_task_shared_data_for_update(&mut *shared_tx, task.id)
        .await
        .unwrap();
    assert_eq!(shared, empty);
    artifacts::set_task_shared_data(&mut *shared_tx, task.id, &serde_json::json!({"a": 1}))
        .await
        .unwrap();
    shared_tx.commit().await.unwrap();

    let mut bytes_tx = pool.begin().await.unwrap();
    assert_eq!(
        artifacts::lock_task_artifact_bytes(&mut bytes_tx, task.id)
            .await
            .unwrap(),
        6
    );
    bytes_tx.commit().await.unwrap();
    assert_eq!(
        artifacts::fetch_root_artifact_ids(&pool, &[task.id])
            .await
            .unwrap()
            .get(&task.id),
        Some(&root.id)
    );

    tasks::set_task_running(&pool, task.id).await.unwrap();
    tasks::finalize_task(
        &pool,
        task.id,
        &TaskFinalization {
            status: TaskState::Completed,
            error_count: 0,
            artifact_count: 1,
            partial: true,
            verdict_classification: Some(DbClassification::Suspicious),
            verdict_score: Some(50),
            verdict_artifact_id: Some(root.id),
        },
    )
    .await
    .unwrap();
    let t = tasks::fetch_task(&pool, task.id).await.unwrap().unwrap();
    assert_eq!(t.status, TaskState::Completed);
    assert!(t.partial && t.started_on.is_some() && t.completed_on.is_some());
    assert_eq!(t.shared_data, serde_json::json!({"a": 1}));

    assert_eq!(
        plugin_reports::fetch_latest_plugin_reports_for_sample(&pool, sample_id)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        plugin_reports::count_reported_tasks_for_sample(&pool, sample_id)
            .await
            .unwrap(),
        1
    );

    drop(conn);
    db.drop().await;
}
