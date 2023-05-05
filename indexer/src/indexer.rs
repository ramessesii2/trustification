use std::time::Duration;

use bombastic_event_bus::{Event, EventBus};
use bombastic_index::Index;
use bombastic_storage::Storage;
use futures::future::{select, Either};
use futures::pin_mut;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::time::{sleep_until, Instant};

#[derive(Deserialize, Debug)]
pub struct StorageEvent {
    #[serde(rename = "EventName")]
    event_name: String,
    #[serde(rename = "Key")]
    key: String,
}

const PUT_EVENT: &str = "s3:ObjectCreated:Put";

pub async fn run<E: EventBus>(
    mut index: Index,
    storage: Storage,
    event_bus: E,
    sync_interval: Duration,
) -> Result<(), anyhow::Error> {
    let mut next_sync = Instant::now() + sync_interval;
    loop {
        let tick = sleep_until(next_sync);
        pin_mut!(tick);
        match select(event_bus.poll(), tick).await {
            Either::Left((bus, _)) => match bus {
                Ok(event) => loop {
                    if let Some(payload) = event.payload() {
                        if let Ok(data) = serde_json::from_slice::<StorageEvent>(payload) {
                            tracing::info!("Got payload from event: {:?}", data);
                            if data.event_name == PUT_EVENT {
                                if storage.is_index(&data.key) {
                                    break;
                                }
                                if let Some(key) = storage.extract_key(&data.key) {
                                    match storage.get(key).await {
                                        Ok(data) => {
                                            // TODO: Handle SPDX and move this out to a separate module, and use serde instead of raw JSON.
                                            let j = serde_json::from_slice::<serde_json::Value>(&data).unwrap();
                                            let purl = j["metadata"]["component"]["purl"].as_str().unwrap();
                                            let mut hasher = Sha256::new();
                                            hasher.update(&data);
                                            let hash = hasher.finalize();
                                            match index.insert(purl, &format!("{:x}", hash), key).await {
                                                Ok(_) => tracing::info!("Inserted entry into index"),
                                                Err(e) => tracing::warn!("Error inserting entry into index: {:?}", e),
                                            }
                                        }
                                        Err(_e) => {}
                                    }
                                } else {
                                    tracing::warn!("Error extracting key from event: {:?}", data)
                                };
                            }
                        }
                    }
                    match event.commit() {
                        Ok(_) => {
                            tracing::info!("Event committed successfully");
                            break;
                        }
                        Err(e) => {
                            tracing::warn!("Error committing event: {:?}", e)
                        }
                    }
                },
                Err(e) => {
                    tracing::warn!("Error polling for event: {:?}", e);
                }
            },
            Either::Right(_) => {
                match index.snapshot() {
                    Ok(data) => match storage.put_index(&data).await {
                        Ok(_) => {
                            tracing::info!("Index updated successfully");
                        }
                        Err(e) => {
                            tracing::warn!("Error updating index: {:?}", e)
                        }
                    },
                    Err(e) => {
                        tracing::warn!("Error taking index snapshot: {:?}", e);
                    }
                }
                next_sync = Instant::now() + sync_interval;
            }
        }
    }
}
