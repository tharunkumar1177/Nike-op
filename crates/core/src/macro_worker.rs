use crate::ipc::RunnerToMacroCommand;
use crate::macro_config::MacroConfig;
use anyhow::{Context, Result};
use std::process::Child;
use std::time::{Duration, Instant};

#[cfg(windows)]
use crate::install_layout::{self, MACRO_EXE};
#[cfg(windows)]
use std::process::Command;
#[cfg(windows)]
use windows::Win32::{Foundation::*, Storage::FileSystem::*};

#[cfg(windows)]
pub struct MacroWorkerHandle {
    child: Child,
    pipe_handle: HANDLE,
}

#[cfg(windows)]
impl MacroWorkerHandle {
    pub fn start(config: MacroConfig) -> Result<Self> {
        let executable = install_layout::sibling_executable(MACRO_EXE)?;

        let mut child = Command::new(&executable)
            .spawn()
            .with_context(|| format!("failed to start {:?}", executable))?;
        let pipe_handle = match connect_with_timeout(child.id(), Duration::from_secs(3)) {
            Ok(handle) => handle,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        };
        let handle = Self { child, pipe_handle };
        handle.send(&RunnerToMacroCommand::ConfigUpdated(config))?;
        handle.send(&RunnerToMacroCommand::SetEnabled(true))?;
        Ok(handle)
    }

    fn send(&self, command: &RunnerToMacroCommand) -> Result<()> {
        let data = bincode::serialize(command).context("failed to serialize Macro command")?;
        if data.len() > 1024 * 1024 {
            anyhow::bail!("Macro command exceeds 1 MiB");
        }
        let mut written = 0;
        unsafe {
            WriteFile(self.pipe_handle, Some(&data), Some(&mut written), None)
                .context("failed to write Macro command")?;
            let _ = FlushFileBuffers(self.pipe_handle);
        }
        if written as usize != data.len() {
            anyhow::bail!("Macro worker accepted only part of the command");
        }
        Ok(())
    }

    pub fn stop(mut self) {
        let _ = self.send(&RunnerToMacroCommand::Shutdown);
        let deadline = Instant::now() + Duration::from_millis(500);
        while Instant::now() < deadline {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(windows)]
impl Drop for MacroWorkerHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.pipe_handle);
        }
        if !matches!(self.child.try_wait(), Ok(Some(_))) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

/// Connect to the worker's pipe and confirm the server is the child Runner started,
/// so a pipe pre-created under the same name by another process is never trusted.
#[cfg(windows)]
fn connect_with_timeout(worker_pid: u32, timeout: Duration) -> Result<HANDLE> {
    let pipe_name: Vec<u16> = crate::ipc::macro_pipe_name()?
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let started = Instant::now();
    while started.elapsed() < timeout {
        let result = unsafe {
            CreateFileW(
                windows::core::PCWSTR(pipe_name.as_ptr()),
                (FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0).into(),
                FILE_SHARE_NONE,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                HANDLE::default(),
            )
        };
        if let Ok(handle) = result {
            if !handle.is_invalid() {
                return match crate::pipe_security::pipe_server_process_id(handle) {
                    Ok(server_pid) if server_pid == worker_pid => Ok(handle),
                    Ok(server_pid) => {
                        unsafe {
                            let _ = CloseHandle(handle);
                        }
                        anyhow::bail!(
                            "Macro pipe is served by process {} instead of worker {}",
                            server_pid,
                            worker_pid
                        )
                    }
                    Err(error) => {
                        unsafe {
                            let _ = CloseHandle(handle);
                        }
                        Err(error)
                    }
                };
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    anyhow::bail!("timed out connecting to Macro worker");
}

#[cfg(not(windows))]
pub struct MacroWorkerHandle;

#[cfg(not(windows))]
impl MacroWorkerHandle {
    pub fn start(_config: MacroConfig) -> Result<Self> {
        anyhow::bail!("Macro worker is only available on Windows")
    }

    pub fn stop(self) {}
}
