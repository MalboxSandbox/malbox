use malbox_config::Config;
use malbox_database::{init_database, init_machines};
use malbox_http::http;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, subscriber};

mod error;
pub use error::DaemonError;

pub async fn run(config: Config) -> error::Result<()> {
    let db = init_database(&config.database).await;

    // FIXME:
    // init_machines(&db, &config.machinery).await.unwrap();

    // let node = host_ipc.node.as_ref().unwrap();

    // while node.wait(Duration::from_secs(1)).is_ok() {
    //     match host_ipc.receive_message() {
    //         Ok(Some(message)) => match message {
    //             ChannelMessage::Event(event) => {
    //                 debug!("Received event from plugin: {:?}", event.plugin_id);

    //                 debug!("{:#?}", event);
    //             }
    //             _ => debug!("other"),
    //         },
    //         Ok(None) => {}
    //         Err(e) => {
    //             eprintln!("Error receiving message: {:#?}", e);
    //         }
    //     }
    // }

    // init_scheduler().await;

    http::serve(config.clone(), db)
        .await
        .map_err(|e| DaemonError::Internal(e.to_string()))
}
