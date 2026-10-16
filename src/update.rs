use velopack::sources::GithubSource;
use velopack::{UpdateCheck, UpdateManager};

const REPO_URL: &str = "https://github.com/wsp-space/e220-controller";

/// Downloads the newest release so Velopack applies it on the next launch.
/// Returns the downloaded version, or `None` when the app was not installed
/// by Velopack (e.g. `cargo run`), is up to date, or the check failed.
pub fn download_latest() -> Option<String> {
    let manager = UpdateManager::new(GithubSource::new(REPO_URL, None, false), None, None).ok()?;
    let UpdateCheck::UpdateAvailable(update) = manager.check_for_updates().ok()? else {
        return None;
    };
    manager.download_updates(&update, None).ok()?;
    Some(update.TargetFullRelease.Version)
}
