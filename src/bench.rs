//! Drawing counters for comparing builds. With `REPODECK_BENCH=<file>` set, a thread appends one
//! JSON line per second to that file: window frames, terminal paints, terminal rows laid out
//! (rather than replayed from the cache), and the time `ui` took (layout and painting commands,
//! before eframe tessellates and presents). Run the same workload against each build and compare.
//! Without the variable every counter call is a relaxed load of a flag.

use std::io::Write;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

static ENABLED: AtomicBool = AtomicBool::new(false);
static FRAMES: AtomicU64 = AtomicU64::new(0);
static PAINTS: AtomicU64 = AtomicU64::new(0);
static ROWS: AtomicU64 = AtomicU64::new(0);
static UI_MICROS: AtomicU64 = AtomicU64::new(0);
static UI_MICROS_MAX: AtomicU64 = AtomicU64::new(0);

/// Starts the reporter when `REPODECK_BENCH` names a file.
pub fn start() {
    static STARTED: OnceLock<()> = OnceLock::new();
    let Some(path) = std::env::var_os("REPODECK_BENCH") else { return };
    STARTED.get_or_init(|| {
        ENABLED.store(true, Ordering::Relaxed);
        let spawned = std::thread::Builder::new().name("bench".into()).spawn(move || report(path.into()));
        if let Err(err) = spawned {
            log::error!("Failed to start the bench reporter: {err}");
        }
    });
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Records one window frame whose `ui` began at `started`.
pub fn frame(started: Instant) {
    if !enabled() {
        return;
    }
    let micros = started.elapsed().as_micros() as u64;
    FRAMES.fetch_add(1, Ordering::Relaxed);
    UI_MICROS.fetch_add(micros, Ordering::Relaxed);
    UI_MICROS_MAX.fetch_max(micros, Ordering::Relaxed);
}

/// Records one terminal paint that laid out `rows` rows.
pub fn paint(rows: usize) {
    if enabled() {
        PAINTS.fetch_add(1, Ordering::Relaxed);
        ROWS.fetch_add(rows as u64, Ordering::Relaxed);
    }
}

fn report(path: std::path::PathBuf) {
    let mut file = match std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        Ok(file) => file,
        Err(err) => {
            log::error!("Bench output {}: {err}", path.display());
            return;
        }
    };
    let start = Instant::now();
    loop {
        std::thread::sleep(Duration::from_secs(1));
        let frames = FRAMES.swap(0, Ordering::Relaxed);
        let micros = UI_MICROS.swap(0, Ordering::Relaxed);
        let line = serde_json::json!({
            "t": start.elapsed().as_secs(),
            "frames": frames,
            "terminal_paints": PAINTS.swap(0, Ordering::Relaxed),
            "rows_laid_out": ROWS.swap(0, Ordering::Relaxed),
            "ui_us_avg": micros.checked_div(frames).unwrap_or(0),
            "ui_us_max": UI_MICROS_MAX.swap(0, Ordering::Relaxed),
        });
        if writeln!(file, "{line}").is_err() {
            return;
        }
    }
}
