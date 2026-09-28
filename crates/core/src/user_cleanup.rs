//! Cleanup that Runner executes in the signed-in user's context.
//!
//! These operations are user-specific, so they never run in the privileged
//! engine, whose SYSTEM environment resolves to the wrong profile.

use crate::orchestration::{CleanupKind, OperationResult};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Run one cleanup kind for the current user.
pub fn run_cleanup(request_id: String, cleanup_kind: CleanupKind) -> OperationResult {
    let outcome = match cleanup_kind {
        CleanupKind::RecycleBin => clear_recycle_bin(),
        CleanupKind::BrowserCache => clear_browser_cache(),
    };
    match outcome {
        Ok(summary) => OperationResult {
            request_id,
            success: true,
            summary,
            ..OperationResult::default()
        },
        Err(error) => OperationResult::error(
            request_id,
            format!("{} cleanup failed: {:#}", cleanup_kind.as_str(), error),
        ),
    }
}

/// Per-item outcome of clearing a set of directories.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct CleanupTally {
    pub folders: usize,
    pub removed: usize,
    pub skipped: usize,
}

/// Chrome and Edge default-profile cache folders under a local app-data root.
pub fn browser_cache_paths(local_app_data: &Path) -> Vec<PathBuf> {
    let chrome = local_app_data.join(r"Google\Chrome\User Data\Default");
    let edge = local_app_data.join(r"Microsoft\Edge\User Data\Default");
    vec![
        chrome.join("Cache"),
        chrome.join("Code Cache"),
        edge.join("Cache"),
        edge.join("Code Cache"),
    ]
}

/// Remove the contents of each existing directory, keeping the directory itself.
///
/// Links are removed without being followed, and entries that cannot be
/// removed (typically files held open by a running browser) are counted as
/// skipped instead of failing the whole operation.
pub fn clear_directory_contents(directories: &[PathBuf]) -> CleanupTally {
    let mut tally = CleanupTally::default();
    for directory in directories {
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        tally.folders += 1;
        for entry in entries {
            let Ok(entry) = entry else {
                tally.skipped += 1;
                continue;
            };
            match remove_entry(&entry.path()) {
                Ok(()) => tally.removed += 1,
                Err(_) => tally.skipped += 1,
            }
        }
    }
    tally
}

fn remove_entry(path: &Path) -> std::io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.is_dir() {
        fs::remove_dir_all(path)
    } else if metadata.file_type().is_symlink() {
        fs::remove_file(path).or_else(|_| fs::remove_dir(path))
    } else {
        fs::remove_file(path)
    }
}

fn clear_browser_cache() -> Result<String> {
    let local_app_data = std::env::var_os("LOCALAPPDATA")
        .context("LOCALAPPDATA is not set")?;
    let tally = clear_directory_contents(&browser_cache_paths(Path::new(&local_app_data)));
    Ok(format!(
        "Browser cache: removed {} item(s), skipped {} in use, across {} folder(s)",
        tally.removed, tally.skipped, tally.folders
    ))
}

#[cfg(windows)]
fn clear_recycle_bin() -> Result<String> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Shell::SHEmptyRecycleBinW;

    const SHERB_NOCONFIRMATION: u32 = 0x1;
    const SHERB_NOPROGRESSUI: u32 = 0x2;
    const SHERB_NOSOUND: u32 = 0x4;
    // The shell reports E_UNEXPECTED when every Recycle Bin is already empty.
    const E_UNEXPECTED: u32 = 0x8000_FFFF;

    let flags = SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND;
    match unsafe { SHEmptyRecycleBinW(HWND::default(), PCWSTR::null(), flags) } {
        Ok(()) => Ok("Recycle Bin emptied".to_string()),
        Err(error) if error.code().0 as u32 == E_UNEXPECTED => {
            Ok("Recycle Bin was already empty".to_string())
        }
        Err(error) => Err(anyhow::anyhow!("SHEmptyRecycleBinW failed: {}", error)),
    }
}

#[cfg(not(windows))]
fn clear_recycle_bin() -> Result<String> {
    anyhow::bail!("Recycle Bin cleanup is only supported on Windows")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_root(label: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "edge-optimizer-cleanup-{label}-{}-{suffix}",
            std::process::id()
        ))
    }

    #[test]
    fn browser_cache_paths_stay_under_the_given_root() {
        // Verifies every cache folder resolves inside the caller-supplied user root, never a SYSTEM profile.
        let root = Path::new(r"C:\Users\fixture\AppData\Local");
        let paths = browser_cache_paths(root);
        assert_eq!(paths.len(), 4);
        assert!(paths.iter().all(|path| path.starts_with(root)));
        let names: Vec<_> = paths.iter().filter_map(|path| path.file_name()).collect();
        assert_eq!(names, ["Cache", "Code Cache", "Cache", "Code Cache"]);
    }

    #[test]
    fn clears_contents_but_keeps_folders_and_ignores_missing_ones() {
        // Verifies files and nested folders are removed from disposable fixtures while roots survive.
        let root = temporary_root("contents");
        let cache = root.join("Cache");
        fs::create_dir_all(cache.join("nested")).unwrap();
        fs::write(cache.join("entry.bin"), b"fixture").unwrap();
        fs::write(cache.join("nested").join("inner.bin"), b"fixture").unwrap();
        let missing = root.join("Missing");

        let tally = clear_directory_contents(&[cache.clone(), missing]);

        assert_eq!(
            tally,
            CleanupTally {
                folders: 1,
                removed: 2,
                skipped: 0,
            }
        );
        assert!(cache.is_dir());
        assert_eq!(fs::read_dir(&cache).unwrap().count(), 0);
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[test]
    fn files_held_open_are_skipped_not_fatal() {
        // Verifies an exclusively opened fixture file is reported as skipped while other entries are removed.
        use std::os::windows::fs::OpenOptionsExt;

        let root = temporary_root("locked");
        fs::create_dir_all(&root).unwrap();
        let locked_path = root.join("locked.bin");
        fs::write(&locked_path, b"fixture").unwrap();
        fs::write(root.join("free.bin"), b"fixture").unwrap();
        let locked = fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&locked_path)
            .unwrap();

        let tally = clear_directory_contents(std::slice::from_ref(&root));

        assert_eq!(tally.removed, 1);
        assert_eq!(tally.skipped, 1);
        drop(locked);
        let _ = fs::remove_dir_all(root);
    }
}
