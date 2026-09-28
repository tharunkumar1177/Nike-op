//! IPC Handler - Communication with Runner
//!
//! Listens for active-profile configuration updates from Runner via named pipes.

use crate::MacroAppState;
use anyhow::Result;
use edge_optimizer_core::ipc::RunnerToMacroCommand;
use std::sync::{Arc, Mutex};
use tracing::{debug, error, info, warn};

#[cfg(windows)]
use windows::Win32::{Foundation::*, Storage::FileSystem::*, System::Pipes::*};

/// Run the IPC listener that receives config updates from Runner
#[cfg(windows)]
pub fn run_ipc_listener(state: Arc<Mutex<MacroAppState>>) -> Result<()> {
    info!("Starting Macro IPC listener...");

    let pipe_path = edge_optimizer_core::ipc::macro_pipe_name()?;
    let pipe_name: Vec<u16> = pipe_path.encode_utf16().chain(Some(0)).collect();
    let security = edge_optimizer_core::pipe_security::owner_only_attributes()?;

    loop {
        // First-instance creation fails while another process holds the name,
        // and Runner verifies this worker's PID before sending configuration.
        let pipe_handle = unsafe {
            CreateNamedPipeW(
                windows::core::PCWSTR(pipe_name.as_ptr()),
                PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
                PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                8192,
                8192,
                0,
                Some(&security as *const _),
            )
        };

        if pipe_handle.is_invalid() {
            error!(
                "Failed to create {}: {}",
                pipe_path,
                windows::core::Error::from_win32()
            );
            std::thread::sleep(std::time::Duration::from_secs(1));
            continue;
        }

        info!("Macro pipe created, waiting for Runner connection...");

        // Wait for Settings to connect
        unsafe {
            match ConnectNamedPipe(pipe_handle, None) {
                Ok(_) => {
                    info!("Runner connected to Macro pipe");
                }
                Err(e) => {
                    let error_code = e.code().0 as u32;
                    if error_code != ERROR_PIPE_CONNECTED.0 {
                        warn!("ConnectNamedPipe error: {}", e);
                        let _ = CloseHandle(pipe_handle);
                        continue;
                    }
                }
            }
        }

        // Read messages from Settings
        loop {
            let mut buffer = vec![0u8; 1024 * 1024];
            let mut bytes_read = 0u32;

            let read_result =
                unsafe { ReadFile(pipe_handle, Some(&mut buffer), Some(&mut bytes_read), None) };

            match read_result {
                Ok(_) if bytes_read > 0 => {
                    // Deserialize and process message
                    match bincode::deserialize::<RunnerToMacroCommand>(
                        &buffer[..bytes_read as usize],
                    ) {
                        Ok(message) => {
                            debug!("Received IPC message: {:?}", message);
                            process_message(&state, message);
                        }
                        Err(e) => {
                            error!("Failed to deserialize IPC message: {}", e);
                        }
                    }
                }
                Ok(_) => {
                    // No data, pipe might be closing
                    debug!("Empty read from pipe");
                }
                Err(e) => {
                    let error_code = e.code().0 as u32;
                    if error_code == ERROR_BROKEN_PIPE.0 || error_code == ERROR_NO_DATA.0 {
                        info!("Runner disconnected from Macro pipe");
                        break;
                    }
                    error!("ReadFile error: {}", e);
                    break;
                }
            }
        }

        // Cleanup pipe
        unsafe {
            let _ = DisconnectNamedPipe(pipe_handle);
            let _ = CloseHandle(pipe_handle);
        }

        info!("Macro pipe closed, recreating...");
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

/// Process a message from Settings
fn process_message(state: &Arc<Mutex<MacroAppState>>, message: RunnerToMacroCommand) {
    match message {
        RunnerToMacroCommand::ConfigUpdated(config) => {
            info!("Macro config updated: {} macros", config.macros.len());
            let mut state_guard = state.lock().unwrap();
            state_guard.config = config;
            state_guard.config_revision = state_guard.config_revision.wrapping_add(1);
        }
        RunnerToMacroCommand::SetEnabled(enabled) => {
            info!("Macro execution enabled: {}", enabled);
            let mut state_guard = state.lock().unwrap();
            state_guard.enabled = enabled;
        }
        RunnerToMacroCommand::Shutdown => {
            info!("Shutdown requested");
            std::process::exit(0);
        }
    }
}

#[cfg(not(windows))]
pub fn run_ipc_listener(_state: Arc<Mutex<MacroAppState>>) -> Result<()> {
    anyhow::bail!("Macro IPC is only supported on Windows")
}
