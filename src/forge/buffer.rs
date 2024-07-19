// Jackson Coxson

use super::Forge;
use std::sync::{atomic::AtomicU8, Arc};
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct ForgeRing {
    ring: Vec<Arc<Mutex<Forge>>>,
    pointer: Arc<AtomicU8>,
}

impl ForgeRing {
    pub fn new(ring: Vec<Arc<Mutex<Forge>>>) -> Self {
        if ring.is_empty() {
            panic!("No items were supplied to the forge ring!");
        }
        Self {
            ring,
            pointer: Arc::new(AtomicU8::new(0)),
        }
    }

    pub fn get(&self) -> Arc<Mutex<Forge>> {
        let pointer = self.pointer.load(std::sync::atomic::Ordering::SeqCst);
        let pointer = if pointer as usize == self.ring.len() - 1 {
            0
        } else {
            pointer + 1
        };
        self.pointer
            .store(pointer, std::sync::atomic::Ordering::SeqCst);
        self.ring[pointer as usize].clone()
    }

    /// Spawns a thread to watch the forge folder for changes
    /// When an update is detected, update each forge appropriately
    pub fn watch(&self) {
        let forges = self.ring.clone();
        tokio::task::spawn(async move {
            println!("Watching the forge folder");
            let mut watcher =
                notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
                    match res {
                        Ok(event) => {
                            if let notify::EventKind::Create(_)
                            | notify::EventKind::Modify(_)
                            | notify::EventKind::Remove(_) = event.kind
                            {
                                // Reload the tree
                                forges.iter().for_each(|forge| {
                                    // TODO: make this actually reload the tree
                                    if let Err(e) = forge.blocking_lock().reload() {
                                        eprintln!("Failed to reload Forge: {e:?}");
                                    }
                                });
                            }
                        }
                        Err(e) => println!("watch error: {:?}", e),
                    }
                })
                .expect("Watcher failed to create");

            // Add a path to be watched. All files and directories at that path and
            // below will be monitored for changes.
            notify::Watcher::watch(
                &mut watcher,
                std::path::Path::new("forge"),
                notify::RecursiveMode::Recursive,
            )
            .expect("Watcher crashed");

            loop {
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            }
        });
    }
}
