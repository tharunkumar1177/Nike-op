//! Install-directory layout shared by Runner and its workers.
//!
//! Every executable ships in one directory, so siblings are resolved only
//! relative to the running image and never from the current directory or PATH.

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

pub const RUNNER_EXE: &str = "EdgeOptimizer_Runner.exe";
pub const SETTINGS_EXE: &str = "EdgeOptimizer.Settings.WinUI.exe";
pub const CROSSHAIR_EXE: &str = "EdgeOptimizer_Crosshair.exe";
pub const MACRO_EXE: &str = "EdgeOptimizer_Macro.exe";

/// Resolve an executable that must exist beside the running image.
pub fn sibling_executable(file_name: &str) -> Result<PathBuf> {
    let current = std::env::current_exe().context("failed to resolve the running executable")?;
    let directory = current
        .parent()
        .context("the running executable has no parent directory")?;
    let path = sibling_in(directory, file_name)?;
    if !path.is_file() {
        bail!(
            "{} was not found beside {}; reinstall the application",
            file_name,
            current.display()
        );
    }
    Ok(path)
}

/// Join a bare executable file name onto an install directory.
///
/// Rejects anything that could escape the directory or name a non-executable.
pub fn sibling_in(directory: &Path, file_name: &str) -> Result<PathBuf> {
    let candidate = Path::new(file_name);
    let is_bare_name = candidate
        .file_name()
        .is_some_and(|name| name == candidate.as_os_str());
    let has_exe_extension = candidate
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"));

    if file_name.is_empty() || file_name.trim() != file_name || !is_bare_name || !has_exe_extension
    {
        bail!("invalid sibling executable name: {:?}", file_name);
    }
    Ok(directory.join(candidate))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_bare_executable_names() {
        // Verifies shipped executable names resolve directly inside the install directory.
        let directory = Path::new(r"C:\Program Files\Edge Optimizer");
        for name in [RUNNER_EXE, SETTINGS_EXE, CROSSHAIR_EXE, MACRO_EXE] {
            assert_eq!(sibling_in(directory, name).unwrap(), directory.join(name));
        }
        assert!(sibling_in(directory, "Mixed.EXE").is_ok());
    }

    #[test]
    fn rejects_names_that_escape_or_are_not_executables() {
        // Verifies traversal, absolute, drive-relative, padded, empty, and non-.exe names are refused.
        let directory = Path::new(r"C:\Program Files\Edge Optimizer");
        for name in [
            "",
            " EdgeOptimizer_Macro.exe",
            "EdgeOptimizer_Macro.exe ",
            r"..\EdgeOptimizer_Macro.exe",
            "sub/EdgeOptimizer_Macro.exe",
            r"sub\EdgeOptimizer_Macro.exe",
            r"C:\Windows\System32\cmd.exe",
            "C:cmd.exe",
            "..",
            "EdgeOptimizer_Macro",
            "EdgeOptimizer_Macro.dll",
        ] {
            assert!(sibling_in(directory, name).is_err(), "accepted {name:?}");
        }
    }
}
