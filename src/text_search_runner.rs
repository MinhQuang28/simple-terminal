//! Runs the Search panel's queries off the UI thread, a newer query cancelling the one still
//! running, and keeps what came back.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui;

use crate::text_search::{Query, Results, search};

/// Typing waits this long for the next key before searching.
const DEBOUNCE: Duration = Duration::from_millis(150);

/// What the Search panel last asked for and what came back.
pub struct TextSearch {
    pub query: Query,
    /// Root and query of the newest search started, so an unchanged one isn't run again.
    started: Option<(PathBuf, Query, bool)>,
    pub results: Option<Arc<Results>>,
    pub error: Option<String>,
    pub running: bool,
    /// Asks the panel to focus the query box on its next frame.
    pub focus: bool,
    /// When the query text last changed, to wait out a burst of typing.
    edited_at: Option<Instant>,
    generation: Arc<AtomicU64>,
    tx: Sender<(u64, Results)>,
    rx: Receiver<(u64, Results)>,
    ctx: egui::Context,
}

impl TextSearch {
    pub fn new(ctx: egui::Context) -> Self {
        let (tx, rx) = mpsc::channel();
        let generation = Arc::new(AtomicU64::new(0));
        Self {
            query: Query::default(),
            started: None,
            results: None,
            error: None,
            running: false,
            focus: false,
            edited_at: None,
            generation,
            tx,
            rx,
            ctx,
        }
    }

    /// Takes in finished results, and starts a search when the query, root or ignore setting
    /// changed since the last one. An empty query clears the results.
    pub fn update(&mut self, root: &Path, show_ignored: bool) {
        while let Ok((generation, results)) = self.rx.try_recv() {
            if generation == self.generation.load(Ordering::Relaxed) {
                self.results = Some(Arc::new(results));
                self.running = false;
            }
        }
        if let Some(wait) = self.edited_at.and_then(|at| DEBOUNCE.checked_sub(at.elapsed())) {
            self.ctx.request_repaint_after(wait);
            return;
        }
        self.edited_at = None;
        let wanted = (root.to_path_buf(), self.query.clone(), show_ignored);
        if self.started.as_ref() == Some(&wanted) {
            return;
        }
        self.started = Some(wanted);
        let generation = self.generation.fetch_add(1, Ordering::Relaxed) + 1;
        self.error = None;
        if self.query.text.is_empty() {
            (self.results, self.running) = (None, false);
            return;
        }
        let regex = match self.query.compile() {
            Ok(regex) => regex,
            Err(err) => {
                (self.error, self.running) = (Some(err), false);
                return;
            }
        };
        self.running = true;
        let (root, tx, ctx, current) =
            (root.to_path_buf(), self.tx.clone(), self.ctx.clone(), self.generation.clone());
        let spawned = thread::Builder::new().name("text-search".into()).spawn(move || {
            let cancelled = || current.load(Ordering::Relaxed) != generation;
            let results = search(&root, &regex, show_ignored, &cancelled);
            if !cancelled() && tx.send((generation, results)).is_ok() {
                ctx.request_repaint();
            }
        });
        if let Err(err) = spawned {
            (self.error, self.running) = (Some(format!("Could not start the search: {err}")), false);
        }
    }

    /// Notes that the query text changed; the search waits for typing to pause.
    pub fn edited(&mut self) {
        self.edited_at = Some(Instant::now());
    }

    /// Searches again with the same query, e.g. after files changed.
    pub fn refresh(&mut self) {
        self.started = None;
    }
}
