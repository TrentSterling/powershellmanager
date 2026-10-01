use super::*;

fn record(seconds: f64, switches: u64, timestamp: f64) -> AppRecord {
    AppRecord {
        total_focus_secs: seconds,
        total_switches: switches,
        last_focus_ts: timestamp,
        category: "Terminal".into(),
        last_title: "Build".into(),
    }
}

fn window(process: &str) -> ManagedWindow {
    ManagedWindow {
        hwnd: 1,
        process_name: process.into(),
        title: "Build".into(),
        category: categorize_process(process),
        rect: crate::monitor::Rect {
            x: 0,
            y: 0,
            w: 300,
            h: 200,
        },
        is_minimized: false,
    }
}

#[test]
fn actual_activity_tab_displays_ranked_apps_session_totals_and_handles_unavailable_data() {
    let mut tracker = ActivityTracker::inert();
    let timestamp = now_ts();
    tracker
        .db
        .lock()
        .unwrap()
        .apps
        .insert("pwsh.exe".into(), record(120.0, 4, timestamp));
    tracker.session.insert(
        "pwsh.exe".into(),
        SessionActivity {
            focus_secs: 120.0,
            switch_count: 4,
            last_focus: timestamp,
            category: AppCategory::Terminal,
        },
    );
    let points = format!("{:.0} points", tracker.top_apps(1)[0].1);
    let session = format!(
        "pwsh.exe: 2 min focused / 4 switches / {}",
        AppCategory::Terminal.short_label()
    );
    let mut app = crate::app::PsmApp::preview(crate::config::Config::default());
    app.activity = Arc::new(Mutex::new(tracker));
    app.detail_tab = 2;
    let ctx = egui::Context::default();
    let size = egui::vec2(1080.0, 1200.0);
    let output = crate::ui_tests::frame(&ctx, &mut app, size, vec![]);
    for text in ["pwsh.exe", points.as_str(), session.as_str()] {
        assert!(
            crate::ui_tests::visible_text_position(&ctx, &output, text).is_some(),
            "activity text {text}"
        );
    }
    assert!(std::panic::catch_unwind(|| {
        let _guard = app.activity.lock().unwrap();
        panic!("owned activity state failure");
    })
    .is_err());
    let output = crate::ui_tests::frame(&ctx, &mut app, size, vec![]);
    assert!(crate::ui_tests::visible_text_position(
        &ctx,
        &output,
        "Activity ranking uses focus time, app switches and recency. Stored locally."
    )
    .is_some());
    assert!(crate::ui_tests::visible_text_position(&ctx, &output, "pwsh.exe").is_none());
}

#[test]
fn ranking_does_not_count_session_data_a_second_time() {
    let mut tracker = ActivityTracker::inert();
    let timestamp = 1000.0;
    tracker
        .db
        .lock()
        .unwrap()
        .apps
        .insert("pwsh.exe".into(), record(10.0, 1, timestamp));
    tracker.session.insert(
        "pwsh.exe".into(),
        SessionActivity {
            focus_secs: 10.0,
            switch_count: 1,
            last_focus: timestamp,
            category: AppCategory::Terminal,
        },
    );
    let score = tracker.score_windows_at(&[window("PWSH.EXE")], timestamp)[0];
    let expected = 10.0_f64.ln() * 10.0 + 5.0 + 50.0;
    assert!(
        (score - expected).abs() < 0.0001,
        "expected one 10-second session, got score {score}, expected {expected}"
    );
    assert_eq!(tracker.top_apps_at(1, timestamp)[0].1, score);
}

#[test]
fn session_display_includes_the_open_focus_period() {
    let mut tracker = ActivityTracker::inert();
    let timestamp = 990.0;
    tracker.session.insert(
        "pwsh.exe".into(),
        SessionActivity {
            focus_secs: 0.0,
            switch_count: 1,
            last_focus: timestamp,
            category: AppCategory::Terminal,
        },
    );
    tracker.current_focus = Some(("pwsh.exe".into(), timestamp));
    tracker
        .db
        .lock()
        .unwrap()
        .apps
        .insert("pwsh.exe".into(), record(0.0, 1, timestamp));
    let stats = tracker.session_stats_at(1000.0);
    assert!(
        stats[0].1.focus_secs == 10.0,
        "active focus time must be visible before periodic save"
    );
    let before = tracker.score_windows_at(&[window("pwsh.exe")], 1000.0);
    tracker.flush_current_focus_at(1000.0);
    assert_eq!(
        tracker.db.lock().unwrap().apps["pwsh.exe"].total_focus_secs,
        10.0
    );
    assert_eq!(tracker.session_stats_at(1000.0)[0].1.focus_secs, 10.0);
    assert_eq!(
        tracker.score_windows_at(&[window("pwsh.exe")], 1000.0),
        before
    );
}

#[test]
fn losing_foreground_closes_focus_without_registering_an_empty_application() {
    let mut tracker = ActivityTracker::inert();
    let (tx, rx) = mpsc::channel();
    tracker.rx = rx;
    tx.send(FocusEvent {
        process_name: "pwsh.exe".into(),
        title: "Build".into(),
        timestamp: 1000.0,
    })
    .unwrap();
    tx.send(FocusEvent {
        process_name: String::new(),
        title: String::new(),
        timestamp: 1010.0,
    })
    .unwrap();
    tracker.update_at(1010.0);
    assert!(tracker.current_focus.is_none());
    let db = tracker.db.lock().unwrap();
    assert_eq!(db.apps.len(), 1);
    assert_eq!(db.apps["pwsh.exe"].total_focus_secs, 10.0);
    assert!(!tracker.session.contains_key(""));
}

#[test]
fn invalid_events_are_ignored_and_clock_rollback_never_subtracts_focus() {
    let mut tracker = ActivityTracker::inert();
    let (tx, rx) = mpsc::channel();
    tracker.rx = rx;
    for (process, timestamp) in [
        ("pwsh.exe", 1000.0),
        ("invalid.exe", f64::NAN),
        ("invalid.exe", f64::INFINITY),
        ("invalid.exe", -1.0),
        ("Code.exe", 990.0),
    ] {
        tx.send(FocusEvent {
            process_name: process.into(),
            title: "Focus".into(),
            timestamp,
        })
        .unwrap();
    }
    tracker.update_at(1000.0);
    assert_eq!(tracker.db.lock().unwrap().apps.len(), 2);
    assert_eq!(
        tracker.db.lock().unwrap().apps["pwsh.exe"].total_focus_secs,
        0.0
    );
    tracker.flush_current_focus_at(980.0);
    tracker.flush_current_focus_at(-1.0);
    tracker.flush_current_focus_at(1000.0);
    tracker.flush_current_focus_at(f64::NAN);
    tracker.flush_current_focus_at(f64::INFINITY);
    assert_eq!(
        tracker.db.lock().unwrap().apps["code.exe"].total_focus_secs,
        10.0
    );
    assert_eq!(tracker.current_focus.as_ref().unwrap().1, 1000.0);
    assert!(tracker
        .session_stats_at(1000.0)
        .iter()
        .all(|(_, activity)| activity.focus_secs.is_finite() && activity.focus_secs >= 0.0));
}

#[test]
fn periodic_flush_and_decay_preserve_live_totals_without_double_charging() {
    let mut tracker = ActivityTracker::inert();
    let (tx, rx) = mpsc::channel();
    tracker.rx = rx;
    tx.send(FocusEvent {
        process_name: "pwsh.exe".into(),
        title: "Build".into(),
        timestamp: 1000.0,
    })
    .unwrap();
    tracker.last_save = Instant::now() - Duration::from_secs(61);
    tracker.last_decay = Instant::now() - Duration::from_secs(3601);
    tracker.update_at(1010.0);
    assert_eq!(
        tracker.db.lock().unwrap().apps["pwsh.exe"].total_focus_secs,
        10.0
    );
    assert_eq!(tracker.current_focus.as_ref().unwrap().1, 1010.0);
    assert_eq!(tracker.db.lock().unwrap().last_decay_ts, 1010.0);
    tracker.update_at(1020.0);
    assert_eq!(
        tracker.db.lock().unwrap().apps["pwsh.exe"].total_focus_secs,
        10.0
    );
    assert_eq!(tracker.session_stats_at(1020.0)[0].1.focus_secs, 20.0);
}

#[test]
fn decay_applies_one_half_life_prunes_stale_records_and_ignores_clock_rollback() {
    let mut tracker = ActivityTracker::inert();
    {
        let mut db = tracker.db.lock().unwrap();
        db.last_decay_ts = 1000.0;
        db.apps.insert("pwsh.exe".into(), record(64.0, 16, 1000.0));
        db.apps.insert("dead.exe".into(), record(0.5, 0, 1000.0));
        db.apps.insert("recent.exe".into(), record(0.5, 4, 1000.0));
    }
    let later = 1000.0 + 7.0 * 86400.0;
    tracker.apply_decay_at(later);
    let db = tracker.db.lock().unwrap();
    assert_eq!(db.apps["pwsh.exe"].total_focus_secs, 32.0);
    assert_eq!(db.apps["pwsh.exe"].total_switches, 8);
    assert!(!db.apps.contains_key("dead.exe"));
    assert_eq!(db.apps["recent.exe"].total_focus_secs, 0.25);
    assert_eq!(db.apps["recent.exe"].total_switches, 2);
    drop(db);
    for timestamp in [later - 86400.0, f64::NAN, f64::INFINITY, -1.0] {
        tracker.apply_decay_at(timestamp);
    }
    assert_eq!(tracker.db.lock().unwrap().last_decay_ts, later);
    assert_eq!(
        tracker.db.lock().unwrap().apps["pwsh.exe"].total_focus_secs,
        32.0
    );
    tracker.apply_decay_at(later + 10.0);
    assert_eq!(
        tracker.db.lock().unwrap().apps["pwsh.exe"].total_focus_secs,
        32.0
    );
    tracker.db.lock().unwrap().last_decay_ts = f64::NAN;
    tracker.apply_decay_at(later);
    assert_eq!(tracker.db.lock().unwrap().last_decay_ts, later);
}

#[test]
fn corrupt_numeric_values_and_future_timestamps_cannot_create_invalid_scores() {
    for half_life in [0.0, -1.0, f64::NAN, f64::INFINITY, 0.001, 10000.0] {
        let (_, rx) = mpsc::channel();
        let tracker = ActivityTracker::from_db(rx, ActivityDb::default(), half_life, None);
        assert!((0.01..=3650.0).contains(&tracker.decay_half_life_days));
        assert!(tracker.score_windows_at(&[window("unknown.exe")], 1000.0)[0].is_finite());
    }
    let mut tracker = ActivityTracker::inert();
    tracker
        .db
        .lock()
        .unwrap()
        .apps
        .insert("pwsh.exe".into(), record(f64::INFINITY, 0, f64::INFINITY));
    assert_eq!(
        tracker.score_windows_at(&[window("pwsh.exe")], 1000.0),
        vec![0.0]
    );
    tracker
        .db
        .lock()
        .unwrap()
        .apps
        .insert("pwsh.exe".into(), record(0.0, 0, 1.0e12));
    assert_eq!(
        tracker.score_windows_at(&[window("pwsh.exe")], 1000.0),
        vec![50.0]
    );
    assert_eq!(
        tracker.score_windows_at(&[window("pwsh.exe")], f64::NAN),
        vec![0.0]
    );
    tracker.session.insert(
        "pwsh.exe".into(),
        SessionActivity {
            focus_secs: f64::INFINITY,
            switch_count: 0,
            last_focus: 0.0,
            category: AppCategory::Terminal,
        },
    );
    assert_eq!(tracker.session_stats_at(1000.0)[0].1.focus_secs, 0.0);
    assert_eq!(accumulate_seconds(f64::MAX, f64::MAX), f64::MAX);
    assert_eq!(finite_seconds(-1.0), 0.0);
    assert_eq!(elapsed_seconds(1000.0, f64::NAN), 0.0);
    assert_eq!(elapsed_seconds(f64::NAN, 1000.0), 0.0);
    assert_eq!(elapsed_seconds(f64::INFINITY, 1000.0), 0.0);
}

#[test]
fn missing_activity_entries_do_not_resurrect_old_records_or_lose_the_next_focus() {
    let mut tracker = ActivityTracker::inert();
    tracker.current_focus = Some(("removed.exe".into(), 1000.0));
    tracker.flush_current_focus_at(1005.0);
    assert_eq!(tracker.current_focus.as_ref().unwrap().1, 1005.0);
    assert!(tracker.session.is_empty());
    assert!(tracker.db.lock().unwrap().apps.is_empty());

    let (tx, rx) = mpsc::channel();
    tracker.rx = rx;
    tx.send(FocusEvent {
        process_name: "pwsh.exe".into(),
        title: "Next focus".into(),
        timestamp: 1010.0,
    })
    .unwrap();
    tracker.update_at(1010.0);
    assert_eq!(tracker.session.len(), 1);
    assert_eq!(tracker.session["pwsh.exe"].focus_secs, 0.0);
    assert_eq!(tracker.current_focus, Some(("pwsh.exe".into(), 1010.0)));
    let db = tracker.db.lock().unwrap();
    assert_eq!(db.apps.len(), 1);
    assert_eq!(db.apps["pwsh.exe"].last_title, "Next focus");
}

#[test]
fn equal_activity_scores_and_session_times_use_stable_alphabetical_order() {
    let mut tracker = ActivityTracker::inert();
    for process in ["pwsh.exe", "code.exe", "firefox.exe"] {
        tracker
            .db
            .lock()
            .unwrap()
            .apps
            .insert(process.into(), record(10.0, 1, 1000.0));
        tracker.session.insert(
            process.into(),
            SessionActivity {
                focus_secs: 10.0,
                switch_count: 1,
                last_focus: 1000.0,
                category: categorize_process(process),
            },
        );
    }
    let top = tracker.top_apps_at(2, 1000.0);
    assert_eq!(
        top.iter()
            .map(|(process, _)| process.as_str())
            .collect::<Vec<_>>(),
        vec!["code.exe", "firefox.exe"]
    );
    assert_eq!(
        tracker
            .session_stats_at(1000.0)
            .iter()
            .map(|(process, _)| process.as_str())
            .collect::<Vec<_>>(),
        vec!["code.exe", "firefox.exe", "pwsh.exe"]
    );
    assert!(tracker.top_apps_at(0, 1000.0).is_empty());
}

#[test]
fn large_switch_counters_saturate_and_title_updates_remain_local() {
    let mut tracker = ActivityTracker::inert();
    tracker
        .db
        .lock()
        .unwrap()
        .apps
        .insert("pwsh.exe".into(), record(0.0, u64::MAX, 1000.0));
    tracker.session.insert(
        "pwsh.exe".into(),
        SessionActivity {
            focus_secs: 0.0,
            switch_count: u32::MAX,
            last_focus: 1000.0,
            category: AppCategory::Terminal,
        },
    );
    let (tx, rx) = mpsc::channel();
    tracker.rx = rx;
    tx.send(FocusEvent {
        process_name: "PWSH.EXE".into(),
        title: "日本語 / updated".into(),
        timestamp: 1010.0,
    })
    .unwrap();
    tracker.update_at(1010.0);
    assert_eq!(tracker.session["pwsh.exe"].switch_count, u32::MAX);
    assert_eq!(
        tracker.db.lock().unwrap().apps["pwsh.exe"].total_switches,
        u64::MAX
    );
    assert_eq!(
        tracker.db.lock().unwrap().apps["pwsh.exe"].last_title,
        "日本語 / updated"
    );
    assert!(tracker.score_windows_at(&[window("pwsh.exe")], 1010.0)[0].is_finite());
}

#[test]
fn poller_sends_changes_and_foreground_loss_and_retries_missing_process_information() {
    let (tx, rx) = mpsc::channel();
    let samples = [
        (Some(1), Some("pwsh.exe"), 1000.0),
        (Some(1), Some("PWSH.EXE"), 1001.0),
        (Some(2), None, 1002.0),
        (Some(2), None, 1003.0),
        (Some(2), Some("pwsh.exe"), 1004.0),
        (None, None, 1005.0),
        (Some(3), Some("invalid.exe"), f64::NAN),
        (Some(3), Some("invalid.exe"), -1.0),
        (Some(3), Some("Code.exe"), 1006.0),
    ];
    let mut samples = samples
        .into_iter()
        .map(|(hwnd, process, timestamp)| FocusSample {
            hwnd,
            process_name: process.map(str::to_owned),
            title: "Illustrative title".into(),
            timestamp,
        });
    poll_events(tx, &mut || samples.next());
    let events: Vec<_> = rx.try_iter().collect();
    assert_eq!(
        events
            .iter()
            .map(|event| event.process_name.as_str())
            .collect::<Vec<_>>(),
        vec!["pwsh.exe", "", "pwsh.exe", "", "Code.exe"]
    );
    assert_eq!(
        events
            .iter()
            .map(|event| event.timestamp)
            .collect::<Vec<_>>(),
        vec![1000.0, 1002.0, 1004.0, 1005.0, 1006.0]
    );
}

#[test]
fn poller_exits_when_receiver_disappears_or_shutdown_is_already_requested() {
    let (tx, rx) = mpsc::channel();
    drop(rx);
    let mut calls = 0;
    poll_events(tx, &mut || {
        calls += 1;
        Some(FocusSample {
            hwnd: Some(1),
            process_name: Some("pwsh.exe".into()),
            title: String::new(),
            timestamp: 1000.0,
        })
    });
    assert_eq!(calls, 1);
    let (tx, rx) = mpsc::channel();
    focus_poller(tx, Arc::new(AtomicBool::new(true)));
    assert!(rx.try_recv().is_err());
}

#[test]
fn shutdown_signals_and_joins_the_owned_worker_even_when_it_panicked() {
    let mut tracker = ActivityTracker::inert();
    let stop = tracker.stop.clone();
    tracker.worker = Some(std::thread::spawn(|| panic!("isolated worker failure")));
    drop(tracker);
    assert!(stop.load(Ordering::Acquire));
}

#[test]
fn unavailable_worker_start_is_nonfatal_and_tracker_still_accepts_updates_and_shutdown() {
    crate::action_tests::init_logging();
    let mut tracker = ActivityTracker::inert();
    tracker.set_worker(Err(std::io::Error::other("isolated start failure")));
    assert!(tracker.worker.is_none());
    tracker.update();
    tracker.save();
    assert!(tracker.score_windows(&[window("owned.exe")])[0].is_finite());
    let stop = tracker.stop.clone();
    drop(tracker);
    assert!(stop.load(Ordering::Acquire));
}

#[test]
fn shutdown_accounts_for_queued_focus_events_before_the_final_flush() {
    let mut tracker = ActivityTracker::inert();
    let (tx, rx) = mpsc::channel();
    tracker.rx = rx;
    tracker.worker = Some(std::thread::spawn(move || {
        tx.send(FocusEvent {
            process_name: "pwsh.exe".into(),
            title: "Build".into(),
            timestamp: 1000.0,
        })
        .unwrap();
        tx.send(FocusEvent {
            process_name: String::new(),
            title: String::new(),
            timestamp: 1010.0,
        })
        .unwrap();
    }));
    let db = tracker.db.clone();
    drop(tracker);
    assert_eq!(db.lock().unwrap().apps["pwsh.exe"].total_focus_secs, 10.0);
}

#[test]
fn active_focus_is_retained_when_old_records_decay_below_the_pruning_threshold() {
    let mut tracker = ActivityTracker::inert();
    tracker.current_focus = Some(("pwsh.exe".into(), 1000.0));
    {
        let mut db = tracker.db.lock().unwrap();
        db.last_decay_ts = 1000.0;
        db.apps.insert("pwsh.exe".into(), record(1.0, 1, 1000.0));
        db.apps.insert("closed.exe".into(), record(1.0, 1, 1000.0));
    }
    tracker.apply_decay_at(1000.0 + 14.0 * 86400.0);
    assert!(tracker.db.lock().unwrap().apps.contains_key("pwsh.exe"));
    assert!(!tracker.db.lock().unwrap().apps.contains_key("closed.exe"));
}

#[test]
fn native_poller_uses_only_its_supplied_storage_and_stops_after_shutdown() {
    let root = std::path::PathBuf::from("target/activity-isolated-poller");
    let path = root.join("activity.toml");
    let mut db = ActivityDb::default();
    db.apps
        .insert("audit.exe".into(), record(10.0, 1, now_ts()));
    crate::persistence::save_toml(&path, &db).unwrap();
    let mut tracker = ActivityTracker::new(7.0, Some(path.clone()));
    assert!(tracker.db.lock().unwrap().apps.contains_key("audit.exe"));
    // Observation is read-only. This test never activates or moves the foreground HWND.
    std::thread::sleep(Duration::from_millis(1100));
    tracker.update();
    assert!(tracker.score_windows(&[window("audit.exe")])[0] > 0.0);
    let _ = tracker.session_stats();
    let _ = tracker.top_apps(10);
    let stop = tracker.stop.clone();
    drop(tracker);
    assert!(stop.load(Ordering::Acquire));
    let loaded: ActivityDb = crate::persistence::load_toml([path]);
    assert!(loaded.apps.contains_key("audit.exe"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn unavailable_activity_storage_and_poisoned_database_are_nonfatal() {
    let root = std::path::PathBuf::from("target/activity-inaccessible-store");
    std::fs::create_dir_all(&root).unwrap();
    let mut tracker = ActivityTracker::inert();
    tracker.path = Some(root.clone());
    tracker.save();
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
    let db = tracker.db.clone();
    let _ = std::thread::spawn(move || {
        let _guard = db.lock().unwrap();
        panic!("isolated lock failure");
    })
    .join();
    let (tx, rx) = mpsc::channel();
    tracker.rx = rx;
    for (process, timestamp) in [("pwsh.exe", 1000.0), ("code.exe", 1010.0)] {
        tx.send(FocusEvent {
            process_name: process.into(),
            title: "Audit".into(),
            timestamp,
        })
        .unwrap();
    }
    tracker.update_at(1010.0);
    tracker.flush_current_focus_at(1020.0);
    tracker.apply_decay_at(1020.0);
    tracker.save();
    assert!(tracker.top_apps_at(10, 1020.0).is_empty());
    assert!(tracker.score_windows_at(&[window("code.exe")], 1020.0)[0].is_finite());
    assert_eq!(tracker.session_stats_at(1020.0)[0].1.focus_secs, 10.0);
    drop(tracker);
    std::fs::remove_dir_all(root).unwrap();
}
