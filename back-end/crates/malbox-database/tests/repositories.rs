//! Task/artifact/job/report/cache repositories against a real Postgres.
//! Gated on `MALBOX_TEST_DATABASE_URL`.

mod common;

use common::TestDb;
use malbox_database::repositories::artifacts::{NewArtifact, TaskArtifact};
use malbox_database::repositories::jobs::Job;
use malbox_database::repositories::plugin_reports::{self, DbClassification, InsertPluginReport};
use malbox_database::repositories::tasks::{self, NewTask, Task, TaskFinalization, TaskState};
use malbox_database::repositories::{artifacts, jobs, result_cache};
use sqlx::{PgConnection, PgPool};
use time::OffsetDateTime;

async fn seed_sample(pool: &PgPool, sha: &str) -> i64 {
    i64::from(
        sqlx::query_scalar::<_, i32>(
            "insert into samples (file_size, file_type, md5, crc32, sha1, sha256, sha512, ssdeep) \
             values (3, 'data', $1, $1, $1, $1, $1, '') returning id",
        )
        .bind(sha)
        .fetch_one(pool)
        .await
        .expect("insert sample"),
    )
}

/// Insert a pending task with its root artifact in one transaction.
async fn insert_task_with_root(
    pool: &PgPool,
    sample_id: i64,
    empty: &serde_json::Value,
) -> (Task, TaskArtifact, NewArtifact<'static>) {
    let mut tx = pool.begin().await.expect("begin");
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
            params: empty,
            max_depth: 6,
            max_artifacts: 10,
            max_detonations: 1,
            max_total_bytes: 1 << 20,
            deadline: OffsetDateTime::now_utc(),
            sample_id: Some(sample_id),
            parent_task_id: None,
            owner: None,
            tags: None,
            shared_data: empty,
        },
    )
    .await
    .expect("insert task");
    let root_new = NewArtifact {
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
    assert!(created, "first insert creates the root");
    tx.commit().await.expect("commit");
    (task, root, root_new)
}

/// Re-inserting the root is a no-op; a child refused by a limit ends up dropped.
async fn insert_dropped_child(
    conn: &mut PgConnection,
    root: &TaskArtifact,
    root_new: &NewArtifact<'static>,
) -> TaskArtifact {
    let (again, created) = artifacts::insert_or_get_artifact(conn, root_new)
        .await
        .expect("re-insert root");
    assert!(!created, "re-inserting the root returns the existing row");
    assert_eq!(again.id, root.id, "re-insert yields the same root");

    let (child, _) = artifacts::insert_or_get_artifact(
        conn,
        &NewArtifact {
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
    .expect("insert child");
    artifacts::set_analyze_requested(&mut *conn, child.id, true)
        .await
        .expect("set analyze requested");
    artifacts::set_artifact_admission_dropped(&mut *conn, child.id, "max_depth")
        .await
        .expect("drop child");
    child
}

/// A blocked host job is released, claimed once, and not claimable again.
async fn insert_and_claim_host_job(
    pool: &PgPool,
    conn: &mut PgConnection,
    task_id: i32,
    root_id: i64,
) -> Job {
    let job = jobs::insert_job(
        &mut *conn,
        &jobs::NewJob {
            task_id,
            artifact_id: root_id,
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
    .expect("insert job");
    let released = jobs::release_jobs(&mut *conn, &[job.id])
        .await
        .expect("release");
    assert_eq!(released, 1, "the blocked job is released");
    let claimed = jobs::claim_host_job(pool, &[], 8)
        .await
        .expect("claim query")
        .expect("claim");
    assert_eq!(
        (claimed.id, claimed.attempts, claimed.state.as_str()),
        (job.id, 1, "running"),
        "claiming starts the first attempt"
    );
    let second = jobs::claim_host_job(pool, &[], 8)
        .await
        .expect("second claim query");
    assert!(second.is_none(), "a running job cannot be claimed twice");
    job
}

/// An edge emitted by the job is returned with its (dropped) child.
async fn check_job_edge(
    pool: &PgPool,
    conn: &mut PgConnection,
    task_id: i32,
    root_id: i64,
    child_id: i64,
    job_id: i64,
) {
    artifacts::insert_edge(
        &mut *conn,
        task_id,
        root_id,
        child_id,
        "extracted",
        Some(job_id),
        Some("extractor"),
        Some("b.exe"),
    )
    .await
    .expect("insert edge");
    let from_job = artifacts::fetch_edges_from_job(pool, job_id)
        .await
        .expect("edges from job");
    assert_eq!(from_job.len(), 1, "one edge from the job");
    assert_eq!(from_job[0].0.relation, "extracted", "edge relation");
    assert_eq!(from_job[0].1.state, "dropped", "child state");
    assert!(
        from_job[0].1.dynamic_recursion_bypass,
        "bypass set by set_analyze_requested"
    );
}

/// Retry, reclaim, restart recovery, then completion stamps `completed_on`.
async fn retry_and_complete_job(pool: &PgPool, conn: &mut PgConnection, job_id: i64) {
    jobs::schedule_retry(
        &mut *conn,
        job_id,
        "recoverable",
        "x",
        OffsetDateTime::now_utc(),
    )
    .await
    .expect("schedule retry");
    jobs::claim_job_by_id(&mut *conn, job_id)
        .await
        .expect("reclaim query")
        .expect("reclaim");
    let recovered = jobs::recover_running_jobs(pool).await.expect("recover");
    assert_eq!(recovered, 1, "the reclaimed job is recovered");
    jobs::set_job_state(&mut *conn, job_id, "completed", None, None)
        .await
        .expect("complete");
    let done = jobs::fetch_job(pool, job_id)
        .await
        .expect("fetch job")
        .expect("job exists");
    assert!(
        done.is_terminal() && done.completed_on.is_some(),
        "completion is terminal and stamped"
    );
}

/// A second upsert for the same (artifact, plugin) replaces the first.
async fn check_plugin_report_upsert(pool: &PgPool, task_id: i32, root_id: i64, job_id: i64) {
    let labels: Vec<String> = vec![];
    let report = plugin_reports::upsert_plugin_report(
        pool,
        &InsertPluginReport {
            task_id,
            artifact_id: root_id,
            job_id: Some(job_id),
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
    .expect("first upsert");
    // A second upsert replaces rather than failing.
    let again = plugin_reports::upsert_plugin_report(
        pool,
        &InsertPluginReport {
            task_id,
            artifact_id: root_id,
            job_id: Some(job_id),
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
    .expect("second upsert");
    assert_eq!(report.id, again.id, "upsert keeps the row");
    assert_eq!(again.plugin_version, "2", "upsert replaces the columns");
}

/// Result cache round trip, shared-data locking, and stored-byte accounting.
async fn check_cache_and_shared_data(
    pool: &PgPool,
    task_id: i32,
    root_id: i64,
    job_id: i64,
    empty: &serde_json::Value,
) {
    result_cache::store(pool, "k", job_id, false, 60)
        .await
        .expect("cache store");
    assert_eq!(
        result_cache::lookup(pool, "k").await.expect("cache lookup"),
        Some((job_id, false)),
        "stored entry is live"
    );
    let purged = result_cache::purge_expired(pool)
        .await
        .expect("cache purge");
    assert_eq!(purged, 0, "nothing has expired yet");

    let mut shared_tx = pool.begin().await.expect("begin shared");
    let shared = artifacts::fetch_task_shared_data_for_update(&mut *shared_tx, task_id)
        .await
        .expect("lock shared data");
    assert_eq!(&shared, empty, "shared data starts empty");
    artifacts::set_task_shared_data(&mut *shared_tx, task_id, &serde_json::json!({"a": 1}))
        .await
        .expect("set shared data");
    shared_tx.commit().await.expect("commit shared");

    let mut bytes_tx = pool.begin().await.expect("begin bytes");
    let stored_bytes = artifacts::lock_task_artifact_bytes(&mut bytes_tx, task_id)
        .await
        .expect("lock bytes");
    assert_eq!(stored_bytes, 6, "root and child blobs are both stored");
    bytes_tx.commit().await.expect("commit bytes");
    let roots = artifacts::fetch_root_artifact_ids(pool, &[task_id])
        .await
        .expect("root ids");
    assert_eq!(roots.get(&task_id), Some(&root_id), "root id per task");
}

/// Finalizing records the verdict and the sample-level report projections.
async fn finalize_and_check(pool: &PgPool, task_id: i32, root_id: i64, sample_id: i64) {
    tasks::set_task_running(pool, task_id)
        .await
        .expect("set running");
    tasks::finalize_task(
        pool,
        task_id,
        &TaskFinalization {
            status: TaskState::Completed,
            error_count: 0,
            artifact_count: 1,
            partial: true,
            verdict_classification: Some(DbClassification::Suspicious),
            verdict_score: Some(50),
            verdict_artifact_id: Some(root_id),
        },
    )
    .await
    .expect("finalize");
    let t = tasks::fetch_task(pool, task_id)
        .await
        .expect("fetch task")
        .expect("task exists");
    assert_eq!(t.status, TaskState::Completed, "final status");
    assert!(
        t.partial && t.started_on.is_some() && t.completed_on.is_some(),
        "finalization fields"
    );
    assert_eq!(
        t.shared_data,
        serde_json::json!({"a": 1}),
        "shared data persisted"
    );

    let latest = plugin_reports::fetch_latest_plugin_reports_for_sample(pool, sample_id)
        .await
        .expect("latest reports");
    assert_eq!(latest.len(), 1, "one latest report per plugin");
    let reported = plugin_reports::count_reported_tasks_for_sample(pool, sample_id)
        .await
        .expect("reported task count");
    assert_eq!(reported, 1, "one terminal task reported");
}

#[tokio::test]
async fn task_artifact_job_lifecycle() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let pool = db.pool.clone();
    let sample_id = seed_sample(&pool, "aa").await;

    let empty = serde_json::json!({});
    let (task, root, root_new) = insert_task_with_root(&pool, sample_id, &empty).await;
    assert_eq!(task.status, TaskState::Pending);
    assert_eq!(
        tasks::fetch_active_task_ids(&pool).await.unwrap(),
        vec![task.id]
    );

    let mut conn = pool.acquire().await.unwrap();
    let child = insert_dropped_child(&mut conn, &root, &root_new).await;
    let job = insert_and_claim_host_job(&pool, &mut conn, task.id, root.id).await;
    check_job_edge(&pool, &mut conn, task.id, root.id, child.id, job.id).await;
    retry_and_complete_job(&pool, &mut conn, job.id).await;

    check_plugin_report_upsert(&pool, task.id, root.id, job.id).await;
    check_cache_and_shared_data(&pool, task.id, root.id, job.id, &empty).await;
    finalize_and_check(&pool, task.id, root.id, sample_id).await;

    drop(conn);
    db.drop().await;
}
