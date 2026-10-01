use crate::windows::{self, categorize_process, AppCategory, ManagedWindow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Unique app identifier (lowercase process name).
type AppId = String;

/// Event sent from the focus poller thread.
#[derive(Debug)]
struct FocusEvent {
    process_name: String,
    title: String,
    timestamp: f64, // Unix timestamp seconds
}

/// Per-app session data (in-memory, not persisted).
#[derive(Debug, Clone)]
pub struct SessionActivity {
    pub focus_secs: f64,
    pub switch_count: u32,
    pub last_focus: f64,
    pub category: AppCategory,
}

/// Per-app persistent data (saved to TOML).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppRecord {
    pub total_focus_secs: f64,
    pub total_switches: u64,
    pub last_focus_ts: f64,
    pub category: String,
    #[serde(default)]
    pub last_title: String,
}

/// Persistent activity database.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ActivityDb {
    #[serde(default)]
    pub apps: HashMap<String, AppRecord>,
    #[serde(default)]
    pub last_decay_ts: f64,
}

/// Main activity tracker — owns the poller thread and accumulates data.
pub struct ActivityTracker {
    path: Option<std::path::PathBuf>,
    rx: mpsc::Receiver<FocusEvent>,
    session: HashMap<AppId, SessionActivity>,
    db: Arc<Mutex<ActivityDb>>,
    current_focus: Option<(AppId, f64)>, // (app_id, focus_start_ts)
    last_save: Instant,
    last_decay: Instant,
    decay_half_life_days: f64,
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}

fn now_ts() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

impl ActivityTracker {
    /// Start the activity tracker with a background focus poller thread.
    pub fn new(decay_half_life_days: f64, path: Option<std::path::PathBuf>) -> Self {
        let (tx, rx) = mpsc::channel();
        let db = crate::persistence::load_toml(path.iter().cloned());
        let mut tracker = Self::from_db(rx, db, decay_half_life_days, path);
        let stop = tracker.stop.clone();
        let worker = std::thread::Builder::new()
            .name("activity-poller".into())
            .spawn(move || focus_poller(tx, stop));
        tracker.set_worker(worker);
        tracker.apply_decay_at(now_ts());
        tracker
    }

    fn set_worker(&mut self, worker: std::io::Result<std::thread::JoinHandle<()>>) {
        match worker {
            Ok(worker) => self.worker = Some(worker),
            Err(error) => log::warn!("Activity tracking could not start: {error}"),
        }
    }

    fn from_db(
        rx: mpsc::Receiver<FocusEvent>,
        db: ActivityDb,
        decay_half_life_days: f64,
        path: Option<std::path::PathBuf>,
    ) -> Self {
        Self {
            path,
            rx,
            session: HashMap::new(),
            db: Arc::new(Mutex::new(db)),
            current_focus: None,
            last_save: Instant::now(),
            last_decay: Instant::now(),
            decay_half_life_days: valid_half_life(decay_half_life_days),
            stop: Arc::new(AtomicBool::new(false)),
            worker: None,
        }
    }

    /// Isolated preview: no poller, filesystem reads or persistence on drop.
    pub fn inert() -> Self {
        let (_, rx) = mpsc::channel();
        Self::from_db(rx, ActivityDb::default(), 7.0, None)
    }

    /// Process pending focus events. Call from the main update loop.
    pub fn update(&mut self) {
        self.update_at(now_ts());
    }

    fn update_at(&mut self, now: f64) {
        // Drain all pending events
        while let Ok(event) = self.rx.try_recv() {
            if !event.timestamp.is_finite() || event.timestamp < 0.0 {
                continue;
            }
            let app_id = event.process_name.trim().to_lowercase();

            // Close out previous focus period
            if let Some((prev_id, start_ts)) = self.current_focus.take() {
                let elapsed = elapsed_seconds(event.timestamp, start_ts);
                if let Some(session) = self.session.get_mut(&prev_id) {
                    session.focus_secs = accumulate_seconds(session.focus_secs, elapsed);
                }
                if let Ok(mut db) = self.db.lock() {
                    if let Some(record) = db.apps.get_mut(&prev_id) {
                        record.total_focus_secs =
                            accumulate_seconds(record.total_focus_secs, elapsed);
                    }
                }
            }

            // Loss of foreground or process access ends the old interval without
            // inventing an empty application or continuing to charge the old one.
            if app_id.is_empty() {
                continue;
            }

            // Start new focus period
            let entry = self
                .session
                .entry(app_id.clone())
                .or_insert_with(|| SessionActivity {
                    focus_secs: 0.0,
                    switch_count: 0,
                    last_focus: event.timestamp,
                    category: categorize_process(&app_id),
                });
            entry.switch_count = entry.switch_count.saturating_add(1);
            entry.last_focus = event.timestamp;

            // Update persistent DB
            if let Ok(mut db) = self.db.lock() {
                let record = db.apps.entry(app_id.clone()).or_insert_with(|| AppRecord {
                    total_focus_secs: 0.0,
                    total_switches: 0,
                    last_focus_ts: event.timestamp,
                    category: categorize_process(&event.process_name)
                        .display_name()
                        .to_string(),
                    last_title: String::new(),
                });
                record.total_switches = record.total_switches.saturating_add(1);
                record.last_focus_ts = event.timestamp;
                record.last_title = event.title.clone();
                record.category = entry.category.display_name().to_string();
            }

            self.current_focus = Some((app_id, event.timestamp));
        }

        // Periodic save (every 60s)
        if self.last_save.elapsed() >= Duration::from_secs(60) {
            self.flush_current_focus_at(now);
            self.save();
            self.last_save = Instant::now();
        }

        // Periodic decay (every hour)
        if self.last_decay.elapsed() >= Duration::from_secs(3600) {
            self.flush_current_focus_at(now);
            self.apply_decay_at(now);
            self.last_decay = Instant::now();
        }
    }

    /// Flush the current focus period into the accumulators (without closing it).
    fn flush_current_focus_at(&mut self, now: f64) {
        if let Some((ref app_id, ref mut start_ts)) = self.current_focus {
            if !now.is_finite() || now < 0.0 || now < *start_ts {
                return;
            }
            let elapsed = elapsed_seconds(now, *start_ts);
            if let Some(session) = self.session.get_mut(app_id) {
                session.focus_secs = accumulate_seconds(session.focus_secs, elapsed);
            }
            if let Ok(mut db) = self.db.lock() {
                if let Some(record) = db.apps.get_mut(app_id) {
                    record.total_focus_secs = accumulate_seconds(record.total_focus_secs, elapsed);
                }
            }
            *start_ts = now;
        }
    }

    /// Apply exponential decay to all stored activity.
    fn apply_decay_at(&mut self, now: f64) {
        if !now.is_finite() || now < 0.0 {
            return;
        }
        if let Ok(mut db) = self.db.lock() {
            let last = db.last_decay_ts;
            if last.is_finite() && last > 0.0 {
                let elapsed_days = (now - last) / 86400.0;
                if elapsed_days > 0.001 {
                    let factor = 0.5_f64.powf(elapsed_days / self.decay_half_life_days);
                    let mut to_remove = Vec::new();
                    for (id, record) in db.apps.iter_mut() {
                        record.total_focus_secs = finite_seconds(record.total_focus_secs) * factor;
                        record.total_switches = (record.total_switches as f64 * factor) as u64;
                        // Prune dead entries
                        if record.total_focus_secs < 1.0
                            && record.total_switches == 0
                            && self
                                .current_focus
                                .as_ref()
                                .is_none_or(|(current, _)| current != id)
                        {
                            to_remove.push(id.clone());
                        }
                    }
                    for id in to_remove {
                        db.apps.remove(&id);
                    }
                }
            }
            // Clock rollback must not make the same interval decay twice.
            db.last_decay_ts = if last.is_finite() { now.max(last) } else { now };
        }
    }

    /// Save activity DB to disk.
    pub fn save(&self) {
        if let Some(path) = &self.path {
            if let Ok(db) = self.db.lock() {
                if let Err(error) = crate::persistence::save_toml(path, &*db) {
                    log::warn!("{error}");
                }
            }
        }
    }

    /// Persistent totals already include this session; only the unflushed tail is added.
    pub fn score_windows(&self, windows: &[ManagedWindow]) -> Vec<f64> {
        self.score_windows_at(windows, now_ts())
    }

    fn score_windows_at(&self, windows: &[ManagedWindow], now: f64) -> Vec<f64> {
        let db = self.db.lock().ok();
        windows
            .iter()
            .map(|window| {
                let app_id = window.process_name.to_lowercase();
                let record = db.as_ref().and_then(|db| db.apps.get(&app_id));
                app_score(record, self.pending_focus(&app_id, now), now)
            })
            .collect()
    }

    fn pending_focus(&self, app_id: &str, now: f64) -> f64 {
        self.current_focus
            .as_ref()
            .filter(|(current, _)| current == app_id)
            .map(|(_, start)| elapsed_seconds(now, *start))
            .unwrap_or(0.0)
    }

    pub fn session_stats(&self) -> Vec<(String, SessionActivity)> {
        self.session_stats_at(now_ts())
    }

    fn session_stats_at(&self, now: f64) -> Vec<(String, SessionActivity)> {
        let mut stats: Vec<_> = self
            .session
            .iter()
            .map(|(id, activity)| {
                let mut activity = activity.clone();
                activity.focus_secs =
                    accumulate_seconds(activity.focus_secs, self.pending_focus(id, now));
                (id.clone(), activity)
            })
            .collect();
        stats.sort_by(|a, b| {
            b.1.focus_secs
                .total_cmp(&a.1.focus_secs)
                .then_with(|| a.0.cmp(&b.0))
        });
        stats
    }

    pub fn top_apps(&self, count: usize) -> Vec<(String, f64)> {
        self.top_apps_at(count, now_ts())
    }

    fn top_apps_at(&self, count: usize, now: f64) -> Vec<(String, f64)> {
        let mut scored = Vec::new();
        if let Ok(db) = self.db.lock() {
            for (id, record) in &db.apps {
                scored.push((
                    id.clone(),
                    app_score(Some(record), self.pending_focus(id, now), now),
                ));
            }
        }
        scored.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        scored.truncate(count);
        scored
    }
}

fn finite_seconds(value: f64) -> f64 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

fn elapsed_seconds(now: f64, start: f64) -> f64 {
    if now.is_finite() && start.is_finite() {
        (now - start).max(0.0)
    } else {
        0.0
    }
}

fn accumulate_seconds(current: f64, elapsed: f64) -> f64 {
    (finite_seconds(current) + finite_seconds(elapsed)).min(f64::MAX)
}

fn valid_half_life(days: f64) -> f64 {
    if days.is_finite() && days > 0.0 {
        days.clamp(0.01, 3650.0)
    } else {
        7.0
    }
}

fn app_score(record: Option<&AppRecord>, pending: f64, now: f64) -> f64 {
    let focus = accumulate_seconds(
        record.map(|record| record.total_focus_secs).unwrap_or(0.0),
        pending,
    );
    let switches = record.map(|record| record.total_switches).unwrap_or(0);
    let last = record.map(|record| record.last_focus_ts).unwrap_or(0.0);
    let recency = if last.is_finite() && last > 0.0 && now.is_finite() {
        0.5_f64.powf(elapsed_seconds(now, last) / 14400.0) * 50.0
    } else {
        0.0
    };
    focus.max(1.0).ln() * 10.0 + (switches as f64).sqrt() * 5.0 + recency
}

impl Drop for ActivityTracker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            if worker.join().is_err() {
                log::debug!("Activity worker exited after a panic");
            }
        }
        // Join before draining so the worker cannot enqueue an event after the final save.
        let now = now_ts();
        self.update_at(now);
        self.flush_current_focus_at(now);
        self.current_focus = None;
        self.save();
    }
}

struct FocusSample {
    hwnd: Option<isize>,
    process_name: Option<String>,
    title: String,
    timestamp: f64,
}

/// Sampling stops even if the foreground window never changes during shutdown.
fn focus_poller(tx: mpsc::Sender<FocusEvent>, stop: Arc<AtomicBool>) {
    poll_events(tx, &mut || {
        if stop.load(Ordering::Acquire) {
            return None;
        }
        std::thread::sleep(Duration::from_secs(1));
        if stop.load(Ordering::Acquire) {
            return None;
        }
        let hwnd = windows::get_foreground_window();
        Some(FocusSample {
            hwnd,
            process_name: hwnd.and_then(windows::get_process_name_for_hwnd),
            title: hwnd.map(windows::get_window_title).unwrap_or_default(),
            timestamp: now_ts(),
        })
    });
}

fn poll_events(tx: mpsc::Sender<FocusEvent>, sample: &mut dyn FnMut() -> Option<FocusSample>) {
    let mut last = None;
    while let Some(sample) = sample() {
        if !sample.timestamp.is_finite() || sample.timestamp < 0.0 {
            continue;
        }
        let key = sample
            .hwnd
            .zip(sample.process_name.as_ref())
            .map(|(hwnd, process)| (hwnd, process.to_lowercase()));
        if last == key {
            continue;
        }
        last = key;
        let event = FocusEvent {
            process_name: sample.process_name.unwrap_or_default(),
            title: sample.title,
            timestamp: sample.timestamp,
        };
        if tx.send(event).is_err() {
            break;
        }
    }
}
#[cfg(test)]
#[path = "activity/tests.rs"]
mod tests;
