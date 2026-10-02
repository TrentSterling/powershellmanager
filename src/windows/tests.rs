use super::*;
use std::cell::RefCell;

#[test]
fn an_owned_process_handle_without_query_access_returns_no_name() {
    use windows::Win32::System::Threading::{GetCurrentProcessId, PROCESS_SYNCHRONIZE};
    let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, GetCurrentProcessId()) }.unwrap();
    assert!(process_name_from_owned_handle(handle).is_none());
}

struct Queries {
    visible: bool,
    style: u32,
    rect: Option<RECT>,
    pid: u32,
    process: Option<String>,
    class: String,
    title: String,
    minimized: bool,
    calls: RefCell<Vec<&'static str>>,
}

impl Default for Queries {
    fn default() -> Self {
        Self {
            visible: true,
            style: 0,
            rect: Some(RECT {
                left: -1200,
                top: -400,
                right: -800,
                bottom: -100,
            }),
            pid: 42,
            process: Some("PWSH.exe".into()),
            class: "TerminalClass".into(),
            title: "測試 terminal 🧪".into(),
            minimized: false,
            calls: RefCell::new(vec![]),
        }
    }
}

impl WindowQueries for Queries {
    fn visible(&self, _: isize) -> bool {
        self.calls.borrow_mut().push("visible");
        self.visible
    }
    fn ex_style(&self, _: isize) -> u32 {
        self.calls.borrow_mut().push("style");
        self.style
    }
    fn rect(&self, _: isize) -> Option<RECT> {
        self.calls.borrow_mut().push("rect");
        self.rect
    }
    fn process_id(&self, _: isize) -> u32 {
        self.calls.borrow_mut().push("pid");
        self.pid
    }
    fn process_name(&self, pid: u32) -> Option<String> {
        assert_eq!(pid, self.pid);
        self.calls.borrow_mut().push("process");
        self.process.clone()
    }
    fn class_name(&self, _: isize) -> String {
        self.calls.borrow_mut().push("class");
        self.class.clone()
    }
    fn title(&self, _: isize) -> String {
        self.calls.borrow_mut().push("title");
        self.title.clone()
    }
    fn minimized(&self, _: isize) -> bool {
        self.calls.borrow_mut().push("minimized");
        self.minimized
    }
}

fn inspect(queries: &Queries, filter: &TargetFilter, excluded: &[String]) -> Option<ManagedWindow> {
    inspect_window(queries, 10, filter, 99, excluded)
}

#[test]
fn aliases_accept_whitespace_case_and_custom_lists_ignore_empty_entries() {
    for alias in ["powershell", "ps", "terminal", "wt", "terminals"] {
        assert_eq!(
            TargetFilter::from_str(&format!(" \t{}\n", alias.to_uppercase())),
            TargetFilter::Terminals
        );
    }
    for alias in ["all", "universal", "", " , , "] {
        assert_eq!(
            TargetFilter::from_str(&format!(" {alias} ")),
            TargetFilter::Universal
        );
    }
    let custom = TargetFilter::from_str(" PWSH.exe, , code.EXE ,");
    assert_eq!(
        custom,
        TargetFilter::Custom(vec!["pwsh.exe".into(), "code.exe".into()])
    );
    assert!(custom.matches("PwSh.EXE"));
    assert!(!custom.matches("cmd.exe"));
    assert_eq!(custom.display_name(), "Custom");
    assert_eq!(TargetFilter::Universal.display_name(), "Universal");
    assert_eq!(TargetFilter::Terminals.display_name(), "Terminals");
    assert!(TargetFilter::Universal.matches("anything.exe"));
    assert!(!TargetFilter::Terminals.matches("code.exe"));
}

#[test]
fn known_processes_have_consistent_categories_labels_and_terminal_matching() {
    for (category, label, display, names) in [
        (AppCategory::Terminal, "T", "Terminal", "powershell.exe pwsh.exe cmd.exe windowsterminal.exe alacritty.exe wezterm-gui.exe hyper.exe mintty.exe conhost.exe conemu64.exe conemu.exe tabby.exe terminus.exe kitty.exe rio.exe warp.exe"),
        (AppCategory::Browser, "B", "Browser", "chrome.exe firefox.exe msedge.exe brave.exe vivaldi.exe opera.exe arc.exe waterfox.exe librewolf.exe"),
        (AppCategory::Editor, "E", "Editor", "code.exe devenv.exe rider64.exe idea64.exe sublime_text.exe notepad++.exe notepad.exe zed.exe cursor.exe windsurf.exe"),
        (AppCategory::Chat, "C", "Chat", "discord.exe slack.exe teams.exe telegram.exe signal.exe element.exe zoom.exe"),
        (AppCategory::Media, "M", "Media", "spotify.exe vlc.exe obs64.exe obs.exe audacity.exe foobar2000.exe mpv.exe"),
        (AppCategory::Game, "G", "Game", "steam.exe epicgameslauncher.exe gogalaxy.exe"),
        (AppCategory::DevTool, "D", "DevTool", "unity.exe unrealengine.exe blender.exe gimp-2.10.exe gimp.exe figma.exe postman.exe gitextensions.exe sourcetree.exe fork.exe filezilla.exe docker.exe winscp.exe putty.exe"),
        (AppCategory::System, "S", "System", "explorer.exe taskmgr.exe mmc.exe regedit.exe control.exe perfmon.exe resmon.exe"),
        (AppCategory::Other, "?", "Other", "unknown.exe 新程式.exe"),
    ] {
        assert_eq!(category.short_label(), label);
        assert_eq!(category.display_name(), display);
        for process in names.split_whitespace() {
            assert_eq!(categorize_process(process), category);
            assert_eq!(categorize_process(&process.to_uppercase()), category);
            assert_eq!(TargetFilter::Terminals.matches(&process.to_uppercase()), category == AppCategory::Terminal);
        }
    }
}

#[test]
fn discovery_preserves_unicode_negative_bounds_and_minimized_windows() {
    let mut queries = Queries::default();
    for minimized in [false, true] {
        queries.minimized = minimized;
        let window = inspect(&queries, &TargetFilter::Terminals, &[]).unwrap();
        assert_eq!(window.hwnd, 10);
        assert_eq!(window.process_name, "PWSH.exe");
        assert_eq!(window.title, queries.title);
        assert_eq!(
            (window.rect.x, window.rect.y, window.rect.w, window.rect.h),
            (-1200, -400, 400, 300)
        );
        assert_eq!(window.category, AppCategory::Terminal);
        assert_eq!(window.is_minimized, minimized);
    }
    assert!(!queries.calls.borrow().contains(&"class"));
    queries.title.clear();
    assert!(inspect(&queries, &TargetFilter::Universal, &[])
        .unwrap()
        .title
        .is_empty());
}

#[test]
fn discovery_rejects_own_hidden_tool_invalid_bounds_and_missing_processes_lazily() {
    let own = Queries::default();
    assert!(inspect_window(&own, 10, &TargetFilter::Universal, 10, &[]).is_none());
    assert!(own.calls.borrow().is_empty());
    let mut cases = vec![
        Queries {
            visible: false,
            ..Default::default()
        },
        Queries {
            style: WS_EX_TOOLWINDOW.0,
            ..Default::default()
        },
        Queries {
            rect: None,
            ..Default::default()
        },
        Queries {
            pid: 0,
            ..Default::default()
        },
        Queries {
            process: None,
            ..Default::default()
        },
        Queries {
            process: Some(String::new()),
            ..Default::default()
        },
    ];
    for rect in [
        RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 5,
        },
        RECT {
            left: 0,
            top: 0,
            right: 5,
            bottom: 0,
        },
        RECT {
            left: 5,
            top: 0,
            right: 0,
            bottom: 5,
        },
        RECT {
            left: 0,
            top: 5,
            right: 5,
            bottom: 0,
        },
        RECT {
            left: i32::MIN,
            top: 0,
            right: i32::MAX,
            bottom: 5,
        },
        RECT {
            left: 0,
            top: i32::MIN,
            right: 5,
            bottom: i32::MAX,
        },
    ] {
        cases.push(Queries {
            rect: Some(rect),
            ..Default::default()
        });
    }
    for queries in cases {
        assert!(inspect(&queries, &TargetFilter::Universal, &[]).is_none());
        assert!(!queries.calls.borrow().contains(&"title"));
        assert!(!queries.calls.borrow().contains(&"minimized"));
    }
}

#[test]
fn user_exclusions_are_case_insensitive_and_custom_targets_reject_other_processes() {
    let queries = Queries::default();
    assert!(inspect(
        &queries,
        &TargetFilter::Universal,
        &["other.exe".into(), " PWSH.EXE ".into()]
    )
    .is_none());
    assert!(inspect(&queries, &TargetFilter::Terminals, &["pwsh.exe".into()]).is_none());
    assert!(inspect(&queries, &TargetFilter::from_str("code.exe"), &[]).is_none());
    assert!(inspect(
        &queries,
        &TargetFilter::from_str("PWSH.exe"),
        &["code.exe".into()]
    )
    .is_some());
    let editor = Queries {
        process: Some("Code.exe".into()),
        ..Default::default()
    };
    assert!(inspect(&editor, &TargetFilter::Terminals, &[]).is_none());
    assert!(inspect(&editor, &TargetFilter::from_str("code.exe"), &[]).is_some());
}

#[test]
fn universal_filter_excludes_shell_resources_and_accepts_only_file_explorer_windows() {
    for process in EXCLUDED_PROCESSES {
        let queries = Queries {
            process: Some(process.to_uppercase()),
            ..Default::default()
        };
        assert!(
            inspect(&queries, &TargetFilter::Universal, &[]).is_none(),
            "{process}"
        );
        assert!(!queries.calls.borrow().contains(&"class"));
    }
    for class in EXCLUDED_CLASSES {
        let queries = Queries {
            class: class.to_string(),
            ..Default::default()
        };
        assert!(
            inspect(&queries, &TargetFilter::Universal, &[]).is_none(),
            "{class}"
        );
        assert!(inspect(&queries, &TargetFilter::Terminals, &[]).is_some());
    }
    for class in ["", "DesktopClass", "CabinetWClass"] {
        let queries = Queries {
            process: Some("EXPLORER.EXE".into()),
            class: class.into(),
            ..Default::default()
        };
        assert_eq!(
            inspect(&queries, &TargetFilter::Universal, &[]).is_some(),
            class == "CabinetWClass"
        );
        assert!(inspect(&queries, &TargetFilter::from_str("explorer.exe"), &[]).is_some());
    }
}

#[test]
fn process_basename_preserves_unicode_and_long_paths_without_truncation() {
    for (path, expected) in [
        ("C:\\Tools\\PWSH.exe", Some("PWSH.exe")),
        ("C:/Tools/新程式.exe", Some("新程式.exe")),
        ("pwsh.exe", Some("pwsh.exe")),
        ("", None),
        ("C:\\folder\\", None),
        ("C:/folder/", None),
    ] {
        assert_eq!(process_name_from_path(path).as_deref(), expected);
    }
    let path = format!("C:\\{}\\powershell.exe", "long-folder\\".repeat(80));
    assert_eq!(
        process_name_from_path(&path).as_deref(),
        Some("powershell.exe")
    );
}

type MoveRecord = (isize, i32, i32, i32, i32);

struct Placement {
    owners: RefCell<std::collections::VecDeque<u32>>,
    owner: u32,
    placement: windows::Win32::UI::WindowsAndMessaging::WINDOWPLACEMENT,
    visible: bool,
    read_error: bool,
    move_error: bool,
    restore_error: bool,
    calls: RefCell<Vec<&'static str>>,
    moved: RefCell<Vec<MoveRecord>>,
    restored: RefCell<
        Vec<(
            isize,
            windows::Win32::UI::WindowsAndMessaging::WINDOWPLACEMENT,
        )>,
    >,
}

impl Default for Placement {
    fn default() -> Self {
        use windows::Win32::UI::WindowsAndMessaging::WINDOWPLACEMENT;
        Self {
            owners: RefCell::new(Default::default()),
            owner: 42,
            placement: WINDOWPLACEMENT {
                length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
                showCmd: SW_MINIMIZE.0 as u32,
                rcNormalPosition: RECT {
                    left: -900,
                    top: 50,
                    right: -500,
                    bottom: 350,
                },
                ..Default::default()
            },
            visible: true,
            read_error: false,
            move_error: false,
            restore_error: false,
            calls: RefCell::new(vec![]),
            moved: RefCell::new(vec![]),
            restored: RefCell::new(vec![]),
        }
    }
}

impl PlacementApi for Placement {
    fn owner_process_id(&self, _: isize) -> u32 {
        self.calls.borrow_mut().push("owner");
        self.owners.borrow_mut().pop_front().unwrap_or(self.owner)
    }
    fn window_visible(&self, _: isize) -> bool {
        self.visible
    }
    fn read_placement(
        &self,
        _: isize,
    ) -> Result<windows::Win32::UI::WindowsAndMessaging::WINDOWPLACEMENT, String> {
        self.calls.borrow_mut().push("read");
        if self.read_error {
            Err("read denied".into())
        } else {
            Ok(self.placement)
        }
    }
    fn move_to(&self, hwnd: isize, slot: &crate::layout::Slot) -> Result<(), String> {
        self.moved
            .borrow_mut()
            .push((hwnd, slot.x, slot.y, slot.w, slot.h));
        if self.move_error {
            Err("move denied".into())
        } else {
            Ok(())
        }
    }
    fn write_placement(
        &self,
        hwnd: isize,
        placement: &windows::Win32::UI::WindowsAndMessaging::WINDOWPLACEMENT,
    ) -> Result<(), String> {
        self.restored.borrow_mut().push((hwnd, *placement));
        if self.restore_error {
            Err("restore denied".into())
        } else {
            Ok(())
        }
    }
}

#[test]
fn snapshots_require_a_live_stable_owner_and_report_native_read_failures() {
    for visible in [false, true] {
        let api = Placement {
            visible,
            ..Default::default()
        };
        let captured = capture_with(&api, 10).unwrap();
        assert_eq!(
            (captured.hwnd, captured.process_id, captured.visible),
            (10, 42, visible)
        );
        assert_eq!(captured.placement, api.placement);
    }
    let closed = Placement {
        owner: 0,
        ..Default::default()
    };
    assert_eq!(
        capture_with(&closed, 10).unwrap_err(),
        "Could not snapshot window 10: window is closed"
    );
    assert_eq!(*closed.calls.borrow(), ["owner"]);
    let denied = Placement {
        read_error: true,
        ..Default::default()
    };
    assert_eq!(
        capture_with(&denied, 10).unwrap_err(),
        "Could not snapshot window 10: read denied"
    );
    for replacement in [0, 99] {
        let changed = Placement {
            owners: RefCell::new([42, replacement].into_iter().collect()),
            ..Default::default()
        };
        assert_eq!(
            capture_with(&changed, 10).unwrap_err(),
            "Window 10 closed or changed owner while taking its snapshot"
        );
        assert!(changed.moved.borrow().is_empty());
    }
}

#[test]
fn apply_and_restore_check_ownership_keep_exact_states_and_preserve_errors_for_retry() {
    let api = Placement::default();
    let snapshot = capture_with(&api, 10).unwrap();
    let slot = crate::layout::Slot {
        x: -1000,
        y: 100,
        w: 300,
        h: 500,
    };
    position_with(&api, &snapshot, &slot).unwrap();
    assert_eq!(*api.moved.borrow(), [(10, -1000, 100, 300, 500)]);
    assert_eq!(
        restore_with(&api, &snapshot).unwrap(),
        crate::history::RestoreStatus::Restored
    );
    assert_eq!(*api.restored.borrow(), [(10, snapshot.placement)]);
    for owner in [0, 99] {
        let changed = Placement {
            owner,
            ..Default::default()
        };
        assert_eq!(
            position_with(&changed, &snapshot, &slot).unwrap_err(),
            "Window 10 closed or changed owner before Apply"
        );
        assert_eq!(
            restore_with(&changed, &snapshot).unwrap(),
            crate::history::RestoreStatus::Closed
        );
        assert!(changed.moved.borrow().is_empty());
        assert!(changed.restored.borrow().is_empty());
    }
    let denied = Placement {
        move_error: true,
        restore_error: true,
        ..Default::default()
    };
    assert_eq!(
        position_with(&denied, &snapshot, &slot).unwrap_err(),
        "Could not position window 10: move denied"
    );
    assert_eq!(
        restore_with(&denied, &snapshot).unwrap_err(),
        "Could not restore window 10: restore denied"
    );
    let hidden = WindowSnapshot {
        visible: false,
        ..snapshot
    };
    restore_with(&api, &hidden).unwrap();
    let actual = api.restored.borrow().last().copied().unwrap();
    assert_eq!(actual.1.showCmd, SW_HIDE.0 as u32);
    assert_eq!(
        actual.1.rcNormalPosition,
        snapshot.placement.rcNormalPosition
    );
}

#[test]
#[ignore = "Reads real metadata and discovers only test-owned offscreen windows; no desktop input"]
fn native_audit_discovery_reads_full_unicode_titles_and_limited_process_information() {
    use windows::Win32::UI::WindowsAndMessaging::*;
    struct Owned(HWND);
    impl Drop for Owned {
        fn drop(&mut self) {
            unsafe {
                let _ = DestroyWindow(self.0);
            }
        }
    }
    let title = "測試 terminal 🧪 ".repeat(100);
    let text: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    let owned = Owned(
        unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                windows::core::w!("STATIC"),
                windows::core::PCWSTR(text.as_ptr()),
                WS_POPUP,
                -32000,
                -32000,
                300,
                200,
                None,
                None,
                None,
                None,
            )
        }
        .unwrap(),
    );
    let hwnd = owned.0 .0 as isize;
    let process = std::env::current_exe()
        .unwrap()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let filter = TargetFilter::from_str(&process);
    assert_eq!(get_window_title(hwnd), title);
    assert_eq!(
        get_process_name_for_hwnd(hwnd).as_deref(),
        Some(process.as_str())
    );
    assert!(inspect_window(&NativeQueries, hwnd, &filter, 0, &[]).is_none());
    assert_eq!(NativeQueries.class_name(hwnd), "Static");
    assert_eq!(NativeQueries.ex_style(hwnd) & WS_EX_TOOLWINDOW.0, 0);
    assert!(!NativeQueries.minimized(hwnd));
    unsafe {
        let _ = ShowWindow(owned.0, SW_SHOWNOACTIVATE);
    }
    let discovered = find_windows(&filter, 0, &[]);
    let actual = discovered
        .iter()
        .find(|window| window.hwnd == hwnd)
        .expect("owned offscreen window");
    assert_eq!(actual.title, title);
    assert_eq!(actual.process_name, process);
    assert_eq!(actual.rect.w, 300);
    assert!(find_windows(&filter, hwnd, &[])
        .iter()
        .all(|window| window.hwnd != hwnd));
    assert!(find_windows(&filter, 0, &[process])
        .iter()
        .all(|window| window.hwnd != hwnd));
    let long = "a".repeat(9000);
    let text: Vec<u16> = long.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        SetWindowTextW(owned.0, windows::core::PCWSTR(text.as_ptr())).unwrap();
    }
    assert_eq!(get_window_title(hwnd), "a".repeat(8192));
    unsafe {
        SetWindowTextW(owned.0, windows::core::w!("")).unwrap();
    }
    assert_eq!(get_window_title(hwnd), "");
    hide_app_window(hwnd);
    assert!(!NativeQueries.visible(hwnd));
    drop(owned);
    assert_eq!(get_window_title(hwnd), "");
    assert_eq!(NativeQueries.class_name(hwnd), "");
    assert_eq!(NativeQueries.process_id(hwnd), 0);
    assert!(NativeQueries.rect(hwnd).is_none());
    assert!(get_process_name_for_hwnd(hwnd).is_none());
    assert!(get_process_name(0).is_none());
    assert!(get_process_name(u32::MAX).is_none());
    assert!(NativeQueries.read_placement(0).is_err());
    println!("NATIVE DISCOVERY PASS: full Unicode titles, bounded long titles, limited-access process identity, exclusions and closed handles; only owned offscreen HWND manipulated");
}

#[test]
#[ignore = "Queries invalid handles only on an owned private desktop; no input desktop switch"]
fn native_audit_invalid_native_position_and_restore_errors_are_preserved() {
    crate::native_audit::on_private_desktop(|| {
        // HWND 0 is invalid. These failure calls cannot move or restore any window.
        let slot = crate::layout::Slot {
            x: 20,
            y: 30,
            w: 300,
            h: 200,
        };
        let error = NativeQueries.move_to(0, &slot).unwrap_err();
        assert!(!error.is_empty());
        let placement = windows::Win32::UI::WindowsAndMessaging::WINDOWPLACEMENT {
            length: std::mem::size_of::<windows::Win32::UI::WindowsAndMessaging::WINDOWPLACEMENT>()
                as u32,
            ..Default::default()
        };
        let error = NativeQueries.write_placement(0, &placement).unwrap_err();
        assert!(!error.is_empty());
        println!("NATIVE ERROR PASS: native positioning and restoration errors propagated for invalid handles; private desktop only; no windows moved");
    });
}
