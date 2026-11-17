//! Debounced background saving (T2.2). Commands call `mark_dirty()`; a worker thread
//! writes the file once no change has arrived for `debounce`.

use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::state::{AppState, lock};
use crate::store::{Store, StoreError};

enum Msg {
    Dirty,
    /// Save now if needed, then reply. Used when the app exits.
    Flush(Sender<()>),
}

#[derive(Clone)]
pub struct Saver {
    tx: Sender<Msg>,
}

impl Saver {
    pub fn spawn(
        store: Store,
        state: Arc<Mutex<AppState>>,
        debounce: Duration,
        on_error: impl Fn(StoreError) + Send + 'static,
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        thread::Builder::new()
            .name("devdock-saver".into())
            .spawn(move || {
                let save = || {
                    // Snapshot under the lock, write without it.
                    let snapshot = lock(&state).to_saved();
                    if let Err(err) = store.save(&snapshot) {
                        on_error(err);
                    }
                };
                let mut dirty = false;
                loop {
                    let msg = if dirty {
                        rx.recv_timeout(debounce)
                    } else {
                        rx.recv().map_err(|_| RecvTimeoutError::Disconnected)
                    };
                    match msg {
                        Ok(Msg::Dirty) => dirty = true,
                        Ok(Msg::Flush(ack)) => {
                            if dirty {
                                save();
                                dirty = false;
                            }
                            let _ = ack.send(());
                        }
                        Err(RecvTimeoutError::Timeout) => {
                            save();
                            dirty = false;
                        }
                        Err(RecvTimeoutError::Disconnected) => {
                            if dirty {
                                save();
                            }
                            return;
                        }
                    }
                }
            })
            .map(drop)
            .unwrap_or_else(|err| eprintln!("DevDock could not start the saver thread: {err}"));
        Self { tx }
    }

    pub fn mark_dirty(&self) {
        let _ = self.tx.send(Msg::Dirty);
    }

    /// Blocks until pending changes are written (at most 5 s).
    pub fn flush(&self) {
        let (ack_tx, ack_rx) = mpsc::channel();
        if self.tx.send(Msg::Flush(ack_tx)).is_ok() {
            let _ = ack_rx.recv_timeout(Duration::from_secs(5));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(debounce_ms: u64) -> (tempfile::TempDir, Store, Arc<Mutex<AppState>>, Saver) {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("p");
        std::fs::create_dir(&project).unwrap();
        let store = Store::new(dir.path().join("data"));
        let state = Arc::new(Mutex::new(AppState::default()));
        lock(&state).add(&project, 0).unwrap();
        let saver = Saver::spawn(
            store.clone(),
            state.clone(),
            Duration::from_millis(debounce_ms),
            |_| {},
        );
        (dir, store, state, saver)
    }

    #[test]
    fn flush_writes_pending_changes() {
        let (_dir, store, _state, saver) = setup(10_000);
        saver.mark_dirty();
        saver.flush();
        assert_eq!(store.load().saved.projects.len(), 1);
    }

    #[test]
    fn flush_without_changes_writes_nothing() {
        let (_dir, store, _state, saver) = setup(10_000);
        saver.flush();
        assert!(!store.file().exists());
    }

    #[test]
    fn debounce_waits_for_quiet() {
        let (_dir, store, _state, saver) = setup(150);
        for _ in 0..5 {
            saver.mark_dirty();
        }
        thread::sleep(Duration::from_millis(30));
        assert!(!store.file().exists(), "saved before the debounce elapsed");
        thread::sleep(Duration::from_millis(600));
        assert!(store.file().exists(), "not saved after the debounce");
    }
}
