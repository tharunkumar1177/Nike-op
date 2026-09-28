//! Crosshair overlay launcher.
//!
//! Runner owns the worker process: it starts one instance per activation and
//! stops exactly that instance through its process handle.

use crate::install_layout::{self, CROSSHAIR_EXE};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Owned handle to the running crosshair worker.
pub struct OverlayHandle {
    child: Child,
}

impl OverlayHandle {
    /// Stop the worker this handle started.
    pub fn stop(mut self) {
        self.terminate();
    }

    fn terminate(&mut self) {
        if matches!(self.child.try_wait(), Ok(Some(_))) {
            return;
        }
        if let Err(error) = self.child.kill() {
            tracing::warn!("failed to stop crosshair worker: {}", error);
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        tracing::warn!("crosshair worker did not exit within 2 seconds");
    }
}

impl Drop for OverlayHandle {
    fn drop(&mut self) {
        self.terminate();
    }
}

/// Start the crosshair worker for the active profile.
pub fn start_overlay(
    image_path: String,
    x_offset: i32,
    y_offset: i32,
) -> Result<OverlayHandle, String> {
    if !Path::new(&image_path).is_file() {
        return Err(format!("Image not found: {}", image_path));
    }

    let crosshair_exe = install_layout::sibling_executable(CROSSHAIR_EXE)
        .map_err(|error| error.to_string())?;

    tracing::info!(
        "starting crosshair worker {} (offset {}, {})",
        crosshair_exe.display(),
        x_offset,
        y_offset
    );

    let mut command = Command::new(&crosshair_exe);
    command
        .arg(&image_path)
        .arg(x_offset.to_string())
        .arg(y_offset.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    let child = command
        .spawn()
        .map_err(|error| format!("Failed to start crosshair worker: {}", error))?;

    Ok(OverlayHandle { child })
}
