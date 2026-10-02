use super::*;
use std::{cell::RefCell, rc::Rc};

struct Folder(PathBuf);
impl Folder {
    fn new(label: &str) -> Self {
        let path = PathBuf::from("target/storage-tests").join(format!(
            "{label}-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Failure {
    None,
    Create,
    Write,
    Sync,
    Replace,
}

struct BufferedFile {
    bytes: Rc<RefCell<Vec<u8>>>,
    events: Rc<RefCell<Vec<&'static str>>>,
    fail: bool,
    fail_sync: bool,
}
impl Write for BufferedFile {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.events.borrow_mut().push("write");
        if self.fail {
            return Err(io::Error::other("write failed"));
        }
        // Force write_all to handle short writes, just as it must with a real writer.
        let count = bytes.len().min(3);
        self.bytes.borrow_mut().extend_from_slice(&bytes[..count]);
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl Drop for BufferedFile {
    fn drop(&mut self) {
        self.events.borrow_mut().push("close");
    }
}

impl AtomicFile for BufferedFile {
    fn sync(&mut self) -> io::Result<()> {
        self.events.borrow_mut().push("sync");
        if self.fail_sync {
            return Err(io::Error::other("sync failed"));
        }
        Ok(())
    }
}

struct Files {
    failure: Failure,
    collisions: usize,
    cleanup_fails: bool,
    events: Rc<RefCell<Vec<&'static str>>>,
    bytes: Rc<RefCell<Vec<u8>>>,
    original: Vec<u8>,
    attempted: Vec<PathBuf>,
    removed: Vec<PathBuf>,
}
impl Files {
    fn new(failure: Failure) -> Self {
        Self {
            failure,
            collisions: 0,
            cleanup_fails: false,
            events: Rc::new(RefCell::new(Vec::new())),
            bytes: Rc::new(RefCell::new(Vec::new())),
            original: b"original".to_vec(),
            attempted: Vec::new(),
            removed: Vec::new(),
        }
    }
}
impl AtomicFiles for Files {
    fn create(&mut self, path: &Path) -> io::Result<Box<dyn AtomicFile>> {
        self.events.borrow_mut().push("create");
        self.attempted.push(path.to_owned());
        if self.collisions > 0 {
            self.collisions -= 1;
            return Err(io::ErrorKind::AlreadyExists.into());
        }
        if self.failure == Failure::Create {
            return Err(io::Error::other("create failed"));
        }
        Ok(Box::new(BufferedFile {
            bytes: self.bytes.clone(),
            events: self.events.clone(),
            fail: self.failure == Failure::Write,
            fail_sync: self.failure == Failure::Sync,
        }))
    }
    fn replace(&mut self, temporary: &Path, _: &Path) -> io::Result<()> {
        self.events.borrow_mut().push("replace");
        assert_eq!(self.attempted.last().unwrap(), temporary);
        if self.failure == Failure::Replace {
            return Err(io::Error::other("replace failed"));
        }
        self.original = self.bytes.borrow().clone();
        Ok(())
    }
    fn remove(&mut self, path: &Path) -> io::Result<()> {
        self.events.borrow_mut().push("remove");
        self.removed.push(path.to_owned());
        if self.cleanup_fails {
            Err(io::Error::other("cleanup failed"))
        } else {
            Ok(())
        }
    }
}

#[test]
fn atomic_commit_flushes_complete_short_writes_and_closes_before_replacing() {
    let mut files = Files::new(Failure::None);
    atomic_write_using(&mut files, Path::new("settings.toml"), b"new document").unwrap();
    assert_eq!(files.original, b"new document");
    assert_eq!(
        *files.events.borrow(),
        ["create", "write", "write", "write", "write", "sync", "close", "replace"]
    );
    assert!(files.removed.is_empty());
}

#[test]
fn atomic_failures_keep_the_original_close_before_cleanup_and_return_the_primary_error() {
    for (failure, expected) in [
        (Failure::Create, "create failed"),
        (Failure::Write, "write failed"),
        (Failure::Sync, "sync failed"),
        (Failure::Replace, "replace failed"),
    ] {
        for cleanup_fails in [false, true] {
            let mut files = Files::new(failure);
            files.cleanup_fails = cleanup_fails;
            let error = atomic_write_using(&mut files, Path::new("settings.toml"), b"replacement")
                .unwrap_err();
            assert_eq!(error.to_string(), expected);
            assert_eq!(files.original, b"original");
            let events = files.events.borrow();
            if failure == Failure::Create {
                assert_eq!(*events, ["create"]);
                assert!(files.removed.is_empty());
            } else {
                assert!(
                    events.iter().position(|event| *event == "close").unwrap()
                        < events.iter().position(|event| *event == "remove").unwrap()
                );
                assert_eq!(files.removed, files.attempted);
            }
        }
    }
}

#[test]
fn temporary_collisions_retry_without_modifying_unowned_files_and_are_bounded() {
    let mut files = Files::new(Failure::None);
    files.collisions = 2;
    atomic_write_using(&mut files, Path::new("settings.toml"), b"replacement").unwrap();
    assert_eq!(files.attempted.len(), 3);
    assert_eq!(
        files
            .attempted
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        3
    );
    assert!(files.removed.is_empty());
    files = Files::new(Failure::None);
    files.collisions = 100;
    assert_eq!(
        atomic_write_using(&mut files, Path::new("settings.toml"), b"replacement")
            .unwrap_err()
            .kind(),
        io::ErrorKind::AlreadyExists
    );
    assert_eq!(files.attempted.len(), 64);
    assert_eq!(files.original, b"original");
    assert!(files.removed.is_empty());
}

#[test]
fn concurrent_real_writers_leave_one_complete_document_and_no_temporary_files() {
    let folder = Folder::new("concurrent");
    let path = folder.0.join("settings.toml");
    let payloads: Vec<_> = (0..16).map(|index| vec![index; 16 * 1024]).collect();
    let workers: Vec<_> = payloads
        .iter()
        .map(|payload| {
            let path = path.clone();
            let payload = payload.clone();
            std::thread::spawn(move || atomic_write(&path, &payload))
        })
        .collect();
    for worker in workers {
        worker.join().unwrap().unwrap();
    }
    assert!(payloads.contains(&std::fs::read(path).unwrap()));
    assert_eq!(std::fs::read_dir(&folder.0).unwrap().count(), 1);
}

#[test]
fn real_storage_roundtrips_config_and_recovers_from_each_failed_read_candidate() {
    let folder = Folder::new("read");
    let path = folder.0.join("nested/config.toml");
    let mut config = crate::config::Config::default();
    config.defaults.gap = 17;
    save_toml(&path, &config).unwrap();
    let missing = folder.0.join("missing.toml");
    let corrupt = folder.0.join("corrupt.toml");
    let invalid_utf8 = folder.0.join("invalid-utf8.toml");
    std::fs::write(&corrupt, b"[defaults\ngap='invalid'").unwrap();
    std::fs::write(&invalid_utf8, [0xff, 0xfe]).unwrap();
    let candidates = [
        missing.clone(),
        folder.0.clone(),
        invalid_utf8.clone(),
        corrupt.clone(),
        path,
    ];
    let loaded: crate::config::Config = load_toml(candidates);
    assert_eq!(loaded.defaults.gap, 17);
    let fallback: crate::config::Config = load_toml([
        missing,
        folder.0.clone(),
        invalid_utf8,
        corrupt,
        folder.0.join("also-missing.toml"),
    ]);
    assert_eq!(fallback.defaults.gap, 4);
    let empty: crate::config::Config = load_toml([]);
    assert_eq!(empty.defaults.target, "all");
}

struct Refuse;

#[test]
fn a_filename_without_a_parent_roundtrips_without_changing_the_working_directory() {
    let cwd = std::env::current_dir().unwrap();
    let filename = format!(
        ".psm-owned-storage-{}-{}.toml",
        std::process::id(),
        NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
    );
    let path = Path::new(&filename);
    assert!(!path.exists());
    let config = crate::config::Config::default();
    save_toml(path, &config).unwrap();
    let loaded: crate::config::Config = load_toml([path.to_owned()]);
    assert_eq!(loaded.defaults.target, config.defaults.target);
    assert_eq!(loaded.defaults.gap, config.defaults.gap);
    assert_eq!(std::env::current_dir().unwrap(), cwd);
    std::fs::remove_file(path).unwrap();
}

impl Serialize for Refuse {
    fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom("isolated serialization failure"))
    }
}

#[test]
fn serialization_parent_and_commit_failures_preserve_existing_data() {
    let folder = Folder::new("write-errors");
    let path = folder.0.join("settings.toml");
    std::fs::write(&path, b"original").unwrap();
    assert!(save_toml(&path, &Refuse).unwrap_err().contains("serialize"));
    assert_eq!(std::fs::read(&path).unwrap(), b"original");
    let uncreated = folder.0.join("uncreated/settings.toml");
    assert!(save_toml(&uncreated, &Refuse).is_err());
    assert!(!uncreated.parent().unwrap().exists());
    assert!(
        save_toml(&path.join("child.toml"), &crate::config::Config::default())
            .unwrap_err()
            .contains("create")
    );
    assert!(save_toml(&folder.0, &crate::config::Config::default())
        .unwrap_err()
        .contains("save"));
    assert_eq!(std::fs::read_dir(&folder.0).unwrap().count(), 1);
    assert!(atomic_write(&folder.0.join("absent/settings.toml"), b"replacement").is_err());
}
