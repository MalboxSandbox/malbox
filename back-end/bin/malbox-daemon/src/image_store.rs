use malbox_database::PgPool;
use malbox_database::repositories::images;
use notify::{Event, EventKind, RecursiveMode, Watcher, recommended_watcher};
use std::path::PathBuf;
use tokio::sync::mpsc;
use tracing::{info, warn};

/// Start watching the image store directory for changes.
/// Updates image availability in the database when files appear/disappear.
pub fn spawn_image_watcher(store_path: PathBuf, db: PgPool) {
    let (tx, mut rx) = mpsc::channel::<notify::Result<Event>>(100);

    std::thread::spawn(move || {
        let rt_tx = tx;
        let mut watcher = match recommended_watcher(move |res| {
            let _ = rt_tx.blocking_send(res);
        }) {
            Ok(w) => w,
            Err(e) => {
                warn!("Failed to create filesystem watcher: {}", e);
                return;
            }
        };

        if let Err(e) = watcher.watch(&store_path, RecursiveMode::Recursive) {
            warn!("Failed to watch image store at {}: {}", store_path.display(), e);
            return;
        }

        info!("Watching image store: {}", store_path.display());

        // Keep the watcher alive
        loop {
            std::thread::park();
        }
    });

    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                Ok(event) => handle_fs_event(&db, event).await,
                Err(e) => warn!("Filesystem watch error: {}", e),
            }
        }
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
        Err(e) => warn!("Failed to check image availability: {}", e),
    }
}
