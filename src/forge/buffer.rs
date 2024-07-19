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
        println!("Watching the forge folder");
        let mut watcher = notify::recommended_watcher(move |res| match res {
            Ok(_event) => {
                let _ = forges.iter().map(|f| f.blocking_lock().print_tree());
            }
            Err(e) => println!("watch error: {:?}", e),
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
    }
}
