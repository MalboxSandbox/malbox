use malbox_database::PgPool;
use malbox_database::repositories::images;
use notify::{Event, EventKind, RecursiveMode, Watcher, recommended_watcher};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

/// Start watching the image store directory for changes.
/// Updates image availability in the database when files appear/disappear.
pub fn spawn_image_watcher(store_path: PathBuf, db: PgPool, token: CancellationToken) {
    let (tx, mut rx) = mpsc::channel::<notify::Result<Event>>(100);

    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let flag_clone = Arc::clone(&shutdown_flag);

    let thread_handle = std::thread::spawn(move || {
        let rt_tx = tx;
        let mut watcher = match recommended_watcher(move |res| {
            let _ = rt_tx.blocking_send(res);
        }) {
            Ok(w) => w,
            Err(e) => {
                warn!(error = %e, "Failed to create filesystem watcher");
                return;
            }
        };

        if let Err(e) = watcher.watch(&store_path, RecursiveMode::Recursive) {
            warn!(path = %store_path.display(), error = %e, "Failed to watch image store");
            return;
        }

        debug!(path = %store_path.display(), "Image store watcher started");

        while !flag_clone.load(std::sync::atomic::Ordering::Acquire) {
            std::thread::park_timeout(std::time::Duration::from_secs(1));
        }
        debug!("Image store watcher thread exiting");
    });

    tokio::spawn(async move {
        loop {
            tokio::select! {
                event = rx.recv() => {
                    match event {
                        Some(Ok(event)) => handle_fs_event(&db, event).await,
                        Some(Err(e)) => warn!(error = %e, "Filesystem watch error"),
                        None => break,
                    }
                }
                _ = token.cancelled() => {
                    debug!("Image store consumer received shutdown");
                    break;
                }
            }
        }
        shutdown_flag.store(true, std::sync::atomic::Ordering::Release);
        thread_handle.thread().unpark();
    });
}

async fn handle_fs_event(db: &PgPool, event: Event) {
    match event.kind {
        EventKind::Remove(_) | EventKind::Create(_) => {
            for path in &event.paths {
                check_image_availability(db, path).await;
            }
        }
        _ => {}
    }
}

async fn check_image_availability(db: &PgPool, _path: &std::path::Path) {
    match images::fetch_all_images(db).await {
        Ok(all_images) => {
            for image in all_images {
                let image_path = std::path::Path::new(&image.path);
                let file_exists = image_path.exists();
                if file_exists != image.available {
                    info!(
                        image = image.name.as_str(),
                        available = file_exists,
                        "Image availability changed"
                    );
                    let _ = images::update_image_availability(db, &image.name, file_exists).await;
                }
            }
        }
        Err(e) => warn!(error = %e, "Failed to check image availability"),
    }
}
