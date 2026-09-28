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
pub const ENGINE_EXE: &str = "EdgeOptimizer_EngineSvc.exe";

/// Executables EngineSvc accepts as pipe clients.
pub const ENGINE_CLIENT_EXES: [&str; 2] = [RUNNER_EXE, SETTINGS_EXE];

/// Whether an image path is one of `allowed` directly inside `directory`.
///
/// Comparison is case-insensitive, as Windows paths are.
pub fn is_sibling_image(image: &Path, directory: &Path, allowed: &[&str]) -> bool {
    let (Some(parent), Some(file_name)) = (image.parent(), image.file_name()) else {
        return false;
    };
    let Some(file_name) = file_name.to_str() else {
        return false;
    };
    let normalize = |path: &Path| {
        path.to_string_lossy()
            .trim_end_matches(['\\', '/'])
            .to_lowercase()
    };
    let has_traversal = image
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir));
    !has_traversal
        && !normalize(directory).is_empty()
        && normalize(parent) == normalize(directory)
        && allowed
            .iter()
            .any(|name| name.eq_ignore_ascii_case(file_name))
}

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
        for name in [
            RUNNER_EXE,
            SETTINGS_EXE,
            CROSSHAIR_EXE,
            MACRO_EXE,
            ENGINE_EXE,
        ] {
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

    #[test]
    fn engine_accepts_only_runner_and_settings_beside_it() {
        // Verifies EngineSvc peers must be the installed Runner or Settings image in its own directory.
        let directory = Path::new(r"C:\Program Files\Edge Optimizer");
        let accepted = [
            r"C:\Program Files\Edge Optimizer\EdgeOptimizer_Runner.exe",
            r"c:\program files\edge optimizer\edgeoptimizer.settings.winui.EXE",
        ];
        for image in accepted {
            assert!(
                is_sibling_image(Path::new(image), directory, &ENGINE_CLIENT_EXES),
                "rejected {image:?}"
            );
        }
        let rejected = [
            r"C:\Program Files\Edge Optimizer\EdgeOptimizer_Macro.exe",
            r"C:\Users\Player\Downloads\EdgeOptimizer_Runner.exe",
            r"C:\Program Files\Edge Optimizer\sub\EdgeOptimizer_Runner.exe",
            r"C:\Program Files\Edge Optimizer\sub\..\EdgeOptimizer_Runner.exe",
            r"C:\Program Files\Edge Optimizer Evil\EdgeOptimizer_Runner.exe",
            "EdgeOptimizer_Runner.exe",
            "",
        ];
        for image in rejected {
            assert!(
                !is_sibling_image(Path::new(image), directory, &ENGINE_CLIENT_EXES),
                "accepted {image:?}"
            );
        }
        assert!(!is_sibling_image(
            Path::new("EdgeOptimizer_Runner.exe"),
            Path::new(""),
            &ENGINE_CLIENT_EXES
        ));
    }
}
