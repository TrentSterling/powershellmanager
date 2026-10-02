use serde::Deserialize;
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[cfg(test)]
pub(crate) mod tests;

const RELEASE_API: &str =
    "https://api.github.com/repos/TrentSterling/powershellmanager/releases/latest";
const RELEASE_PAGE: &str = "https://github.com/TrentSterling/powershellmanager/releases/latest";
const MAX_RESPONSE_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateInfo {
    pub latest_version: String,
    pub download_url: String,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

pub(crate) struct ReleaseClient {
    endpoint: String,
    timeout: Duration,
}

impl Default for ReleaseClient {
    fn default() -> Self {
        Self::new(RELEASE_API.into(), Duration::from_secs(10))
    }
}

impl ReleaseClient {
    pub(crate) fn new(endpoint: String, timeout: Duration) -> Self {
        Self { endpoint, timeout }
    }

    fn latest(&self, current: &str) -> Result<Option<UpdateInfo>, String> {
        let response = ureq::get(&self.endpoint)
            .timeout(self.timeout)
            .set("User-Agent", "powershellmanager")
            .set("Accept", "application/vnd.github+json")
            .call()
            .map_err(|error| error.to_string())?;
        // Cap even a chunked response; optional update metadata cannot grow
        // memory without a bound or prevent startup after a malformed reply.
        let mut bytes = Vec::new();
        response
            .into_reader()
            .take(MAX_RESPONSE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        if bytes.len() as u64 > MAX_RESPONSE_BYTES {
            return Err("Release response exceeds 1 MiB".into());
        }
        let release: Release = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
        let latest = release
            .tag_name
            .strip_prefix('v')
            .unwrap_or(&release.tag_name);
        if release.draft || release.prerelease || !version_newer(latest, current) {
            return Ok(None);
        }
        Ok(Some(UpdateInfo {
            latest_version: latest.into(),
            // The server supplies only a version, never a destination to open.
            download_url: RELEASE_PAGE.into(),
        }))
    }
}

pub(crate) fn version_newer(latest: &str, current: &str) -> bool {
    let (Ok(latest), Ok(current)) = (
        semver::Version::parse(latest),
        semver::Version::parse(current),
    ) else {
        return false;
    };
    latest.pre.is_empty() && latest.cmp_precedence(&current).is_gt()
}

pub(crate) fn spawn(
    client: ReleaseClient,
    info: Arc<Mutex<Option<UpdateInfo>>>,
    quitting: Arc<AtomicBool>,
    ctx: egui::Context,
) -> std::io::Result<std::thread::JoinHandle<()>> {
    std::thread::Builder::new()
        .name("psm-update-check".into())
        .spawn(move || {
            if let Err(error) = check(&client, &info, &quitting, &ctx) {
                log::debug!("Update check failed (non-fatal): {error}");
            }
        })
}

fn check(
    client: &ReleaseClient,
    info: &Mutex<Option<UpdateInfo>>,
    quitting: &AtomicBool,
    ctx: &egui::Context,
) -> Result<(), String> {
    if quitting.load(Ordering::Acquire) {
        return Ok(());
    }
    let Some(update) = client.latest(env!("CARGO_PKG_VERSION"))? else {
        return Ok(());
    };
    if quitting.load(Ordering::Acquire) {
        return Ok(());
    }
    *info.lock().map_err(|_| "Update state is unavailable")? = Some(update);
    ctx.request_repaint();
    Ok(())
}
