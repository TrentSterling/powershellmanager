use serde::{de::DeserializeOwned, Serialize};
use std::{
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

/// The same write/flush/replace sequence is used with real files and failure injection.
trait AtomicFile: Write {
    fn sync(&mut self) -> io::Result<()>;
}

impl AtomicFile for std::fs::File {
    fn sync(&mut self) -> io::Result<()> {
        self.sync_all()
    }
}

trait AtomicFiles {
    fn create(&mut self, path: &Path) -> io::Result<Box<dyn AtomicFile>>;
    fn replace(&mut self, temporary: &Path, destination: &Path) -> io::Result<()>;
    fn remove(&mut self, path: &Path) -> io::Result<()>;
}

struct FileSystem;

impl AtomicFiles for FileSystem {
    fn create(&mut self, path: &Path) -> io::Result<Box<dyn AtomicFile>> {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        Ok(Box::new(file))
    }

    fn replace(&mut self, temporary: &Path, destination: &Path) -> io::Result<()> {
        std::fs::rename(temporary, destination)
    }

    fn remove(&mut self, path: &Path) -> io::Result<()> {
        std::fs::remove_file(path)
    }
}

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

/// Keep the old document intact until a complete, flushed replacement is ready.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    atomic_write_using(&mut FileSystem, path, bytes)
}

fn atomic_write_using(files: &mut dyn AtomicFiles, path: &Path, bytes: &[u8]) -> io::Result<()> {
    for _ in 0..64 {
        let temporary = path.with_extension(format!(
            "{}.{}.tmp",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = match files.create(&temporary) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        };
        // The file closes before cleanup, including early write/sync failures on Windows.
        let result = (|| {
            file.write_all(bytes)?;
            file.sync()?;
            drop(file);
            files.replace(&temporary, path)
        })();
        if result.is_err() {
            let _ = files.remove(&temporary);
        }
        return result;
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "No free temporary settings file",
    ))
}

/// Read candidates in priority order; corrupt or inaccessible files allow fallback.
pub fn load_toml<T: DeserializeOwned + Default>(paths: impl IntoIterator<Item = PathBuf>) -> T {
    for path in paths {
        match std::fs::read_to_string(&path) {
            Ok(content) => match toml::from_str(&content) {
                Ok(value) => {
                    log::info!("Loaded local data from {}", path.display());
                    return value;
                }
                Err(error) => log::warn!("Could not parse {}: {error}", path.display()),
            },
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => log::warn!("Could not read {}: {error}", path.display()),
        }
    }
    T::default()
}

pub fn save_toml(path: &Path, value: &impl Serialize) -> Result<(), String> {
    // Serialize first; invalid values must not create or change any filesystem entries.
    save_serialized(path, toml::to_string_pretty(value))
}

fn save_serialized(
    path: &Path,
    serialized: Result<String, toml::ser::Error>,
) -> Result<(), String> {
    let content = serialized.map_err(|error| format!("Could not serialize local data: {error}"))?;
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
    }
    atomic_write(path, content.as_bytes())
        .map_err(|error| format!("Could not save {}: {error}", path.display()))
}

#[cfg(test)]
#[path = "persistence/tests.rs"]
mod tests;
