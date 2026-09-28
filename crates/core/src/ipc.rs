//! Inter-Process Communication between Settings and Runner processes
//! Uses Windows Named Pipes for cross-process communication

use crate::macro_config::MacroConfig;
use crate::orchestration::{Envelope, RunnerToSettingsEvent, SettingsToRunnerCommand};
use crate::pipe_security;
use crate::profile::Profile;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
#[cfg(windows)]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(windows)]
use windows::Win32::{Foundation::*, Storage::FileSystem::*, System::Pipes::*};

/// Base name of the Settings <-> Runner pipe; the full path is per session.
/// `EdgeOptimizer.Settings.WinUI` derives the same name.
pub const SETTINGS_PIPE_BASE: &str = "EdgeOptimizerIPC";
const MAX_SETTINGS_MESSAGE_BYTES: usize = 1024 * 1024;

/// Base name of the Runner <-> Macro worker pipe; the full path is per session.
pub const MACRO_PIPE_BASE: &str = "EdgeOptimizerMacroIPC";

/// Settings <-> Runner pipe path for the caller's session.
pub fn settings_pipe_name() -> Result<String> {
    pipe_security::current_session_pipe_name(SETTINGS_PIPE_BASE)
}

/// Runner <-> Macro worker pipe path for the caller's session.
pub fn macro_pipe_name() -> Result<String> {
    pipe_security::current_session_pipe_name(MACRO_PIPE_BASE)
}

/// Messages from Settings to Runner
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GuiToTray {
    /// Request the authoritative state snapshot from Runner.
    RequestState,
    /// Update profiles list
    ProfilesUpdated(Vec<Profile>),
    /// Active profile changed
    ActiveProfileChanged(Option<String>),
    /// Overlay visibility changed
    OverlayVisibilityChanged(bool),
    /// Request tray to exit
    Shutdown,
    /// Versioned orchestration command envelope
    Orchestration(Envelope<SettingsToRunnerCommand>),
    /// Request a read-only process list for the System Tweaks UI.
    RequestProcessSnapshot,
}

/// Messages from Runner to Settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrayToGui {
    /// Authoritative state loaded from Runner's SQLite database.
    StateSnapshot {
        profiles: Vec<Profile>,
        active_profile: Option<String>,
        overlay_visible: bool,
    },
    /// User selected a profile from tray
    ActivateProfile(String),
    /// User deactivated profile from tray
    DeactivateProfile,
    /// User toggled overlay from tray
    ToggleOverlay,
    /// User requested to open settings/GUI
    OpenSettings,
    /// User single-clicked tray icon - show flyout window
    ShowFlyout,
    /// User clicked away or toggled - hide flyout window
    HideFlyout,
    /// User double-clicked tray icon - bring main window to front
    BringMainToFront,
    /// User requested exit
    Exit,
    /// Versioned orchestration event envelope
    OrchestrationEvent(Envelope<RunnerToSettingsEvent>),
    /// Read-only process metrics; process selection remains profile state.
    ProcessSnapshot(Vec<ProcessSnapshotEntry>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessSnapshotEntry {
    pub name: String,
    pub cpu_percent: f32,
    pub memory_kb: u64,
}

/// Named Pipe Server (Runner side)
/// Receives messages from Settings and sends messages to Settings
#[cfg(windows)]
#[allow(dead_code)]
pub struct NamedPipeServer {
    pipe_handle: HANDLE,
    connected: AtomicBool,
}

#[cfg(windows)]
#[allow(dead_code)]
impl NamedPipeServer {
    /// Create the session's Settings pipe server (Runner side).
    ///
    /// Fails if any process already owns the pipe name, so Runner never shares
    /// an instance with a squatter.
    pub fn new() -> Result<Self> {
        let pipe_path = settings_pipe_name()?;
        let pipe_name: Vec<u16> = pipe_path.encode_utf16().chain(Some(0)).collect();
        let security = pipe_security::owner_only_attributes()?;

        unsafe {
            let pipe_handle = CreateNamedPipeW(
                windows::core::PCWSTR(pipe_name.as_ptr()),
                PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
                PIPE_TYPE_MESSAGE
                    | PIPE_READMODE_MESSAGE
                    | PIPE_NOWAIT
                    | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                8192,
                8192,
                0,
                Some(&security as *const _),
            );

            if pipe_handle.is_invalid() {
                anyhow::bail!(
                    "Failed to create {}: {}",
                    pipe_path,
                    windows::core::Error::from_win32()
                );
            }

            tracing::info!("Named pipe server created: {}", pipe_path);

            Ok(Self {
                pipe_handle,
                connected: AtomicBool::new(false),
            })
        }
    }

    /// Accept a pending Settings connection without blocking Runner's UI loop.
    pub fn try_accept(&self) -> Result<bool> {
        if self.connected.load(Ordering::Acquire) {
            return Ok(true);
        }

        unsafe {
            match ConnectNamedPipe(self.pipe_handle, None) {
                Ok(_) => {
                    tracing::info!("Client connected to named pipe");
                    self.connected.store(true, Ordering::Release);
                    Ok(true)
                }
                Err(error) => match error.code().0 as u32 {
                    code if code == ERROR_PIPE_CONNECTED.0 => {
                        tracing::info!("Client already connected to named pipe");
                        self.connected.store(true, Ordering::Release);
                        Ok(true)
                    }
                    code if code == ERROR_PIPE_LISTENING.0 || code == ERROR_NO_DATA.0 => Ok(false),
                    _ => Err(anyhow::anyhow!("ConnectNamedPipe failed: {}", error)),
                },
            }
        }
    }

    /// Try to receive a message (non-blocking)
    pub fn try_recv(&self) -> Result<Option<GuiToTray>> {
        if !self.connected.load(Ordering::Acquire) {
            return Ok(None);
        }
        let mut buffer = vec![0u8; MAX_SETTINGS_MESSAGE_BYTES];
        let mut bytes_read = 0u32;

        unsafe {
            match ReadFile(
                self.pipe_handle,
                Some(&mut buffer),
                Some(&mut bytes_read),
                None,
            ) {
                Ok(_) => {
                    if bytes_read == 0 {
                        return Ok(None);
                    }

                    let message: GuiToTray = bincode::deserialize(&buffer[..bytes_read as usize])
                        .context("Failed to deserialize GuiToTray message")?;

                    Ok(Some(message))
                }
                Err(e) => {
                    let error_code = e.code().0 as u32;
                    if error_code == ERROR_NO_DATA.0 || error_code == ERROR_PIPE_LISTENING.0 {
                        return Ok(None); // No data available
                    }
                    if error_code == ERROR_BROKEN_PIPE.0 {
                        let _ = DisconnectNamedPipe(self.pipe_handle);
                        self.connected.store(false, Ordering::Release);
                        return Err(anyhow::anyhow!("Settings disconnected"));
                    }
                    Err(anyhow::anyhow!("ReadFile failed: {}", e))
                }
            }
        }
    }

    /// Send a message to Settings
    pub fn send(&self, message: &TrayToGui) -> Result<()> {
        if !self.connected.load(Ordering::Acquire) {
            anyhow::bail!("Settings is not connected");
        }
        let data = bincode::serialize(message).context("Failed to serialize TrayToGui message")?;
        if data.len() > MAX_SETTINGS_MESSAGE_BYTES {
            anyhow::bail!("Settings response exceeds 1 MiB");
        }

        let mut bytes_written = 0u32;

        unsafe {
            WriteFile(
                self.pipe_handle,
                Some(&data),
                Some(&mut bytes_written),
                None,
            )
            .context("WriteFile failed")?;

            let _ = FlushFileBuffers(self.pipe_handle);
        }

        Ok(())
    }
}

#[cfg(windows)]
impl Drop for NamedPipeServer {
    fn drop(&mut self) {
        unsafe {
            let _ = DisconnectNamedPipe(self.pipe_handle);
            let _ = CloseHandle(self.pipe_handle);
        }
        tracing::info!("Named pipe server closed");
    }
}

/// Transitional Runner-to-Macro worker protocol. Runner owns this worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RunnerToMacroCommand {
    ConfigUpdated(MacroConfig),
    SetEnabled(bool),
    Shutdown,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestration::{AuthContext, Envelope, SettingsToRunnerCommand};
    use crate::profile::create_profile;

    #[test]
    fn settings_state_request_round_trips_without_opening_a_pipe() {
        // Verifies the transitional state request can be encoded and decoded entirely in memory.
        let encoded = bincode::serialize(&GuiToTray::RequestState).unwrap();
        let decoded: GuiToTray = bincode::deserialize(&encoded).unwrap();
        assert!(matches!(decoded, GuiToTray::RequestState));
    }

    #[test]
    fn runner_state_snapshot_round_trips_without_opening_a_pipe() {
        // Verifies authoritative profile and activation state survive transitional serialization.
        let message = TrayToGui::StateSnapshot {
            profiles: vec![create_profile("Gaming".into())],
            active_profile: Some("Gaming".into()),
            overlay_visible: true,
        };
        let encoded = bincode::serialize(&message).unwrap();
        let decoded: TrayToGui = bincode::deserialize(&encoded).unwrap();
        assert!(matches!(
            decoded,
            TrayToGui::StateSnapshot {
                profiles,
                active_profile: Some(active),
                overlay_visible: true,
            } if profiles.len() == 1 && active == "Gaming"
        ));
    }

    #[test]
    fn orchestration_envelope_preserves_command_correlation() {
        // Verifies a settings command retains its request ID across the current IPC message wrapper.
        let envelope = Envelope::with_request_id(
            "settings",
            AuthContext::InteractiveUser,
            "request-88",
            SettingsToRunnerCommand::OpenFlyout,
        );
        let encoded = bincode::serialize(&GuiToTray::Orchestration(envelope)).unwrap();
        let decoded: GuiToTray = bincode::deserialize(&encoded).unwrap();
        assert!(matches!(
            decoded,
            GuiToTray::Orchestration(Envelope {
                request_id,
                payload: SettingsToRunnerCommand::OpenFlyout,
                ..
            }) if request_id == "request-88"
        ));
    }

    #[test]
    fn process_snapshot_round_trips_without_enumerating_live_processes() {
        // Verifies process metrics cross the transitional pipe using deterministic fixture data only.
        let message = TrayToGui::ProcessSnapshot(vec![ProcessSnapshotEntry {
            name: "fixture.exe".into(),
            cpu_percent: 1.5,
            memory_kb: 2048,
        }]);
        let encoded = bincode::serialize(&message).unwrap();
        let decoded: TrayToGui = bincode::deserialize(&encoded).unwrap();
        assert!(matches!(
            decoded,
            TrayToGui::ProcessSnapshot(entries)
                if entries.len() == 1 && entries[0].name == "fixture.exe"
        ));
    }

    #[test]
    fn macro_worker_configuration_round_trips() {
        // Verifies Runner and the Macro worker share the same activation-time configuration representation.
        let command = RunnerToMacroCommand::ConfigUpdated(Default::default());
        let encoded = bincode::serialize(&command).unwrap();
        let decoded: RunnerToMacroCommand = bincode::deserialize(&encoded).unwrap();
        assert!(matches!(
            decoded,
            RunnerToMacroCommand::ConfigUpdated(config) if config.macros.is_empty()
        ));
    }
}
