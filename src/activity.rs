use crate::windows::{self, AppCategory, ManagedWindow, categorize_process};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    rx: mpsc::Receiver<FocusEvent>,
    session: HashMap<AppId, SessionActivity>,
    db: Arc<Mutex<ActivityDb>>,
    current_focus: Option<(AppId, f64)>, // (app_id, focus_start_ts)
    last_save: Instant,
    last_decay: Instant,
    decay_half_life_days: f64,
}

fn now_ts() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

impl ActivityTracker {
    /// Start the activity tracker with a background focus poller thread.
    pub fn new(decay_half_life_days: f64) -> Self {
        let (tx, rx) = mpsc::channel();

        // Load existing DB
        let db = load_db();
        let db = Arc::new(Mutex::new(db));

        // Spawn focus poller thread (1s interval)
        std::thread::Builder::new()
            .name("activity-poller".into())
            .spawn(move || {
                focus_poller(tx);
            })
            .expect("failed to spawn activity poller thread");

        Self {
            rx,
            session: HashMap::new(),
            db,
            current_focus: None,
            last_save: Instant::now(),
            last_decay: Instant::now(),
            decay_half_life_days,
        }
    }

    /// Process pending focus events. Call from the main update loop.
    pub fn update(&mut self) {
        // Drain all pending events
        while let Ok(event) = self.rx.try_recv() {
            let app_id = event.process_name.to_lowercase();

            // Close out previous focus period
            if let Some((prev_id, start_ts)) = self.current_focus.take() {
                let elapsed = (event.timestamp - start_ts).max(0.0);
                if let Some(session) = self.session.get_mut(&prev_id) {
                    session.focus_secs += elapsed;
                }
                if let Ok(mut db) = self.db.lock() {
                    if let Some(record) = db.apps.get_mut(&prev_id) {
                        record.total_focus_secs += elapsed;
                    }
                }
            }

            // Start new focus period
            let entry = self.session.entry(app_id.clone()).or_insert_with(|| {
                SessionActivity {
                    focus_secs: 0.0,
                    switch_count: 0,
                    last_focus: event.timestamp,
                    category: categorize_process(&event.process_name),
                }
            });
            entry.switch_count += 1;
            entry.last_focus = event.timestamp;

            // Update persistent DB
            if let Ok(mut db) = self.db.lock() {
                let record = db.apps.entry(app_id.clone()).or_insert_with(|| {
                    AppRecord {
                        total_focus_secs: 0.0,
                        total_switches: 0,
                        last_focus_ts: event.timestamp,
                        category: categorize_process(&event.process_name).display_name().to_string(),
                        last_title: String::new(),
                    }
                });
                record.total_switches += 1;
                record.last_focus_ts = event.timestamp;
                record.last_title = event.title.clone();
                record.category = entry.category.display_name().to_string();
            }

            self.current_focus = Some((app_id, event.timestamp));
        }

        // Periodic save (every 60s)
        if self.last_save.elapsed() >= Duration::from_secs(60) {
            self.flush_current_focus();
            self.save();
            self.last_save = Instant::now();
        }

        // Periodic decay (every hour)
        if self.last_decay.elapsed() >= Duration::from_secs(3600) {
            self.apply_decay();
            self.last_decay = Instant::now();
        }
    }

    /// Flush the current focus period into the accumulators (without closing it).
    fn flush_current_focus(&mut self) {
        if let Some((ref app_id, ref mut start_ts)) = self.current_focus {
            let now = now_ts();
            let elapsed = (now - *start_ts).max(0.0);
            if let Some(session) = self.session.get_mut(app_id) {
                session.focus_secs += elapsed;
            }
            if let Ok(mut db) = self.db.lock() {
                if let Some(record) = db.apps.get_mut(app_id) {
                    record.total_focus_secs += elapsed;
                }
            }
            *start_ts = now;
        }
    }

    /// Apply exponential decay to all stored activity.
    fn apply_decay(&mut self) {
        let now = now_ts();
        if let Ok(mut db) = self.db.lock() {
            let last = db.last_decay_ts;
            if last > 0.0 {
                let elapsed_days = (now - last) / 86400.0;
                if elapsed_days > 0.001 {
                    let factor = 0.5_f64.powf(elapsed_days / self.decay_half_life_days);
                    let mut to_remove = Vec::new();
                    for (id, record) in db.apps.iter_mut() {
                        record.total_focus_secs *= factor;
                        record.total_switches = (record.total_switches as f64 * factor) as u64;
                        // Prune dead entries
                        if record.total_focus_secs < 1.0 && record.total_switches == 0 {
                            to_remove.push(id.clone());
                        }
                    }
                    for id in to_remove {
                        db.apps.remove(&id);
                    }
                }
            }
            db.last_decay_ts = now;
        }
    }

    /// Save activity DB to disk.
    pub fn save(&self) {
        if let Some(path) = crate::config::activity_path() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(db) = self.db.lock() {
                match toml::to_string_pretty(&*db) {
                    Ok(content) => {
                        if let Err(e) = std::fs::write(&path, &content) {
                            log::warn!("Failed to save activity: {}", e);
                        }
                    }
                    Err(e) => log::warn!("Failed to serialize activity: {}", e),
                }
            }
        }
    }

    /// Compute a smart score for each window. Returns scores in the same order as input.
    pub fn score_windows(&self, windows: &[ManagedWindow]) -> Vec<f64> {
        let now = now_ts();
        let db = self.db.lock().ok();

        windows.iter().map(|win| {
            let app_id = win.process_name.to_lowercase();

            // Session data
            let session_focus = self.session.get(&app_id).map(|s| s.focus_secs).unwrap_or(0.0);
            let session_switches = self.session.get(&app_id).map(|s| s.switch_count).unwrap_or(0);

            // Persistent data
            let (db_focus, db_switches, last_focus) = db.as_ref()
                .and_then(|db| db.apps.get(&app_id))
                .map(|r| (r.total_focus_secs, r.total_switches, r.last_focus_ts))
                .unwrap_or((0.0, 0, 0.0));

            let total_focus = session_focus + db_focus;
            let total_switches = session_switches as u64 + db_switches;

            // Scoring formula:
            // - ln(focus_secs).max(0) * 10  — log-scale focus prevents browser domination
            // - sqrt(switches) * 5           — frequent-switch apps (chat, terminal) get boost
            // - recency_factor * 50          — 4-hour half-life recency (strongest signal)
            let focus_score = (total_focus.max(1.0).ln()).max(0.0) * 10.0;
            let switch_score = (total_switches as f64).sqrt() * 5.0;
            let recency_hours = if last_focus > 0.0 { (now - last_focus) / 3600.0 } else { 999.0 };
            let recency_score = 0.5_f64.powf(recency_hours / 4.0) * 50.0;

            focus_score + switch_score + recency_score
        }).collect()
    }

    /// Get session activity data for display.
    pub fn session_stats(&self) -> Vec<(String, SessionActivity)> {
        let mut stats: Vec<_> = self.session.iter()
            .map(|(id, act)| (id.clone(), act.clone()))
            .collect();
        stats.sort_by(|a, b| b.1.focus_secs.partial_cmp(&a.1.focus_secs).unwrap_or(std::cmp::Ordering::Equal));
        stats
    }

    /// Get top N apps by persistent score.
    pub fn top_apps(&self, n: usize) -> Vec<(String, f64)> {
        let now = now_ts();
        let mut scored = Vec::new();
        if let Ok(db) = self.db.lock() {
            for (id, record) in &db.apps {
                let focus_score = (record.total_focus_secs.max(1.0).ln()).max(0.0) * 10.0;
                let switch_score = (record.total_switches as f64).sqrt() * 5.0;
                let recency_hours = if record.last_focus_ts > 0.0 { (now - record.last_focus_ts) / 3600.0 } else { 999.0 };
                let recency_score = 0.5_f64.powf(recency_hours / 4.0) * 50.0;
                scored.push((id.clone(), focus_score + switch_score + recency_score));
            }
        }
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(n);
        scored
    }
}

impl Drop for ActivityTracker {
    fn drop(&mut self) {
        self.flush_current_focus();
        self.save();
    }
}

/// Background thread: polls GetForegroundWindow every 1s, sends FocusEvent on change.
fn focus_poller(tx: mpsc::Sender<FocusEvent>) {
    let mut last_hwnd: Option<isize> = None;

    loop {
        std::thread::sleep(Duration::from_secs(1));

        let hwnd = match windows::get_foreground_window() {
            Some(h) => h,
            None => continue,
        };

        // Only send on change
        if last_hwnd == Some(hwnd) {
            continue;
        }
        last_hwnd = Some(hwnd);

        let process_name = match windows::get_process_name_for_hwnd(hwnd) {
            Some(n) => n,
            None => continue,
        };

        let title = windows::get_window_title(hwnd);

        let event = FocusEvent {
            process_name,
            title,
            timestamp: now_ts(),
        };

        if tx.send(event).is_err() {
            break; // Main thread dropped, exit
        }
    }
}

/// Load the activity DB from disk.
fn load_db() -> ActivityDb {
    if let Some(path) = crate::config::activity_path() {
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(db) = toml::from_str::<ActivityDb>(&content) {
                    log::info!("Loaded activity DB from {}", path.display());
                    return db;
                } else {
                    log::warn!("Failed to parse activity DB");
                }
            }
        }
    }
    ActivityDb::default()
}
