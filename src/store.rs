use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};

use anyhow::{Context, Result};
use directories::ProjectDirs;

use crate::model::AppState;

/// `REPODECK_DATA_DIR` overrides the platform data dir, e.g. to run a second isolated instance.
pub fn data_dir() -> Option<PathBuf> {
    match std::env::var_os("REPODECK_DATA_DIR") {
        Some(dir) => Some(PathBuf::from(dir)),
        None => Some(ProjectDirs::from("dev", "repodeck", "Repodeck")?.data_dir().to_path_buf()),
    }
}

fn state_path() -> Option<PathBuf> {
    Some(data_dir()?.join("state.json"))
}

pub fn load() -> AppState {
    let Some(path) = state_path() else {
        return AppState::default();
    };
    match fs::read_to_string(&path) {
        Ok(json) => serde_json::from_str(&json).unwrap_or_else(|err| {
            log::error!("Corrupt state file {}: {err}", path.display());
            // Keep the unreadable file around instead of overwriting the user's setup.
            let _ = fs::rename(&path, path.with_extension("json.bak"));
            AppState::default()
        }),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => AppState::default(),
        Err(err) => {
            log::error!("Failed to read {}: {err}", path.display());
            AppState::default()
        }
    }
}

/// Writes state snapshots on a background thread; only the newest queued snapshot is written.
pub struct Saver {
    tx: Option<Sender<Vec<u8>>>,
    worker: Option<JoinHandle<()>>,
}

impl Saver {
    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::channel::<Vec<u8>>();
        let worker = thread::Builder::new()
            .name("state-save".into())
            .spawn(move || {
                while let Ok(mut bytes) = rx.recv() {
                    while let Ok(newer) = rx.try_recv() {
                        bytes = newer;
                    }
                    if let Err(err) = write(&bytes) {
                        log::error!("Failed to save state: {err:#}");
                    }
                }
            })
            .expect("failed to spawn state save thread");
        Self { tx: Some(tx), worker: Some(worker) }
    }

    pub fn save(&self, state: &AppState) {
        match serde_json::to_vec_pretty(state) {
            Ok(bytes) => {
                if let Some(tx) = &self.tx {
                    let _ = tx.send(bytes);
                }
            }
            Err(err) => log::error!("Failed to serialize state: {err}"),
        }
    }

    /// Blocks until every queued snapshot is on disk; the saver accepts nothing afterwards.
    pub fn flush(&mut self) {
        self.tx = None;
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn write(bytes: &[u8]) -> Result<()> {
    let path = state_path().context("no data directory available")?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}
