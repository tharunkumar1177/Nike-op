//! Transitional Bincode pipe between EngineSvc and its clients.
//!
//! Runner and Settings are the only accepted clients. The server derives each
//! client's identity from the pipe; clients accept the server only when the
//! pipe is owned by SYSTEM.

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::time::Duration;

#[cfg(windows)]
use crate::pipe_security;
#[cfg(windows)]
use windows::Win32::{Foundation::*, Storage::FileSystem::*, System::Pipes::*};

/// Base name only; `EdgeOptimizer.Settings.WinUI` connects to the same pipe.
pub const ENGINE_PIPE_BASE: &str = "EdgeOptimizerEngineIPC";
pub const ENGINE_PIPE_NAME: &str = r"\\.\pipe\EdgeOptimizerEngineIPC";
pub const MAX_ENGINE_MESSAGE_BYTES: usize = 64 * 1024;

/// A connected client as identified by the operating system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineClientIdentity {
    pub process_id: u32,
    pub session_id: u32,
    pub image_path: String,
}

#[cfg(windows)]
pub struct EnginePipeServer {
    pipe_handle: HANDLE,
}

#[cfg(windows)]
impl EnginePipeServer {
    /// Create the only instance of the engine pipe.
    ///
    /// Fails if any process already owns the name, so EngineSvc never shares
    /// the pipe with a squatter.
    pub fn new(pipe_name: &str) -> Result<Self> {
        let pipe_name_wide: Vec<u16> = pipe_name.encode_utf16().chain(Some(0)).collect();
        let security = pipe_security::engine_pipe_attributes()?;

        unsafe {
            let pipe_handle = CreateNamedPipeW(
                windows::core::PCWSTR(pipe_name_wide.as_ptr()),
                PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
                PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                MAX_ENGINE_MESSAGE_BYTES as u32,
                MAX_ENGINE_MESSAGE_BYTES as u32,
                0,
                Some(&security as *const _),
            );

            if pipe_handle.is_invalid() {
                anyhow::bail!(
                    "failed to create {}: {}",
                    pipe_name,
                    windows::core::Error::from_win32()
                )
            }

            Ok(Self { pipe_handle })
        }
    }

    pub fn new_default() -> Result<Self> {
        Self::new(ENGINE_PIPE_NAME)
    }

    pub fn wait_for_client(&self) -> Result<()> {
        unsafe {
            match ConnectNamedPipe(self.pipe_handle, None) {
                Ok(_) => Ok(()),
                Err(e) if e.code() == ERROR_PIPE_CONNECTED.to_hresult() => Ok(()),
                Err(e) => Err(anyhow::anyhow!("ConnectNamedPipe failed: {}", e)),
            }
        }
    }

    /// Identity of the connected client, read from the pipe and the client
    /// process rather than from any message.
    pub fn client_identity(&self) -> Result<EngineClientIdentity> {
        let process_id = pipe_security::pipe_client_process_id(self.pipe_handle)?;
        let session_id = pipe_security::pipe_client_session_id(self.pipe_handle)?;
        let image_path = crate::process::process_image_path(process_id)?;
        Ok(EngineClientIdentity {
            process_id,
            session_id,
            image_path,
        })
    }

    pub fn recv<T: DeserializeOwned>(&self) -> Result<Option<T>> {
        read_message(self.pipe_handle)
    }

    pub fn send<T: Serialize>(&self, message: &T) -> Result<()> {
        write_message(self.pipe_handle, message)
    }

    pub fn disconnect(&self) {
        unsafe {
            let _ = FlushFileBuffers(self.pipe_handle);
            let _ = DisconnectNamedPipe(self.pipe_handle);
        }
    }
}

#[cfg(windows)]
impl Drop for EnginePipeServer {
    fn drop(&mut self) {
        unsafe {
            let _ = DisconnectNamedPipe(self.pipe_handle);
            let _ = CloseHandle(self.pipe_handle);
        }
    }
}

#[cfg(windows)]
pub struct EnginePipeClient {
    pipe_handle: HANDLE,
}

#[cfg(windows)]
impl EnginePipeClient {
    /// Connect and verify that SYSTEM owns the pipe.
    pub fn connect(pipe_name: &str, timeout: Duration) -> Result<Self> {
        let pipe_name_wide: Vec<u16> = pipe_name.encode_utf16().chain(Some(0)).collect();
        let start = std::time::Instant::now();
        let mut delay_ms = 50u64;

        let client = loop {
            let handle = unsafe {
                CreateFileW(
                    windows::core::PCWSTR(pipe_name_wide.as_ptr()),
                    FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0,
                    FILE_SHARE_NONE,
                    None,
                    OPEN_EXISTING,
                    FILE_ATTRIBUTE_NORMAL,
                    HANDLE::default(),
                )
            };
            match handle {
                Ok(h) if !h.is_invalid() => break Self { pipe_handle: h },
                _ if start.elapsed() > timeout => anyhow::bail!(
                    "timed out connecting to engine pipe after {:?} ({})",
                    timeout,
                    pipe_name
                ),
                _ => {
                    std::thread::sleep(Duration::from_millis(delay_ms));
                    delay_ms = (delay_ms * 2).min(500);
                }
            }
        };

        let mode = PIPE_READMODE_MESSAGE;
        unsafe { SetNamedPipeHandleState(client.pipe_handle, Some(&mode as *const _), None, None) }
            .context("cannot switch the engine pipe to message mode")?;
        if !pipe_security::pipe_is_owned_by_system(client.pipe_handle)? {
            anyhow::bail!("the engine pipe is not owned by SYSTEM; refusing to use it");
        }
        Ok(client)
    }

    pub fn connect_default(timeout: Duration) -> Result<Self> {
        Self::connect(ENGINE_PIPE_NAME, timeout)
    }

    pub fn send<T: Serialize>(&self, message: &T) -> Result<()> {
        write_message(self.pipe_handle, message)
    }

    pub fn recv<T: DeserializeOwned>(&self) -> Result<Option<T>> {
        read_message(self.pipe_handle)
    }
}

#[cfg(windows)]
impl Drop for EnginePipeClient {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.pipe_handle);
        }
    }
}

/// Read one message; messages larger than the bound fail instead of being
/// truncated or reassembled.
#[cfg(windows)]
fn read_message<T: DeserializeOwned>(pipe: HANDLE) -> Result<Option<T>> {
    let mut buffer = vec![0u8; MAX_ENGINE_MESSAGE_BYTES];
    let mut bytes_read = 0u32;
    match unsafe {
        ReadFile(
            pipe,
            Some(buffer.as_mut_slice()),
            Some(&mut bytes_read),
            None,
        )
    } {
        Ok(()) if bytes_read == 0 => Ok(None),
        Ok(()) => {
            let message = bincode::deserialize::<T>(&buffer[..bytes_read as usize])
                .context("failed to deserialize engine pipe payload")?;
            Ok(Some(message))
        }
        Err(e)
            if e.code() == ERROR_BROKEN_PIPE.to_hresult()
                || e.code() == ERROR_PIPE_NOT_CONNECTED.to_hresult() =>
        {
            Ok(None)
        }
        Err(e) if e.code() == ERROR_MORE_DATA.to_hresult() => {
            anyhow::bail!("engine pipe message exceeds {MAX_ENGINE_MESSAGE_BYTES} bytes")
        }
        Err(e) => Err(anyhow::anyhow!("ReadFile failed: {}", e)),
    }
}

#[cfg(windows)]
fn write_message<T: Serialize>(pipe: HANDLE, message: &T) -> Result<()> {
    let data = bincode::serialize(message).context("failed to serialize engine pipe payload")?;
    if data.len() > MAX_ENGINE_MESSAGE_BYTES {
        anyhow::bail!("engine pipe message exceeds {MAX_ENGINE_MESSAGE_BYTES} bytes");
    }
    let mut bytes_written = 0u32;
    unsafe { WriteFile(pipe, Some(data.as_slice()), Some(&mut bytes_written), None) }
        .context("WriteFile failed")?;
    Ok(())
}

#[cfg(not(windows))]
pub struct EnginePipeServer;
#[cfg(not(windows))]
pub struct EnginePipeClient;

#[cfg(not(windows))]
impl EnginePipeServer {
    pub fn new(_pipe_name: &str) -> Result<Self> {
        anyhow::bail!("engine pipe server is only supported on windows")
    }

    pub fn new_default() -> Result<Self> {
        anyhow::bail!("engine pipe server is only supported on windows")
    }

    pub fn wait_for_client(&self) -> Result<()> {
        anyhow::bail!("engine pipe server is only supported on windows")
    }

    pub fn client_identity(&self) -> Result<EngineClientIdentity> {
        anyhow::bail!("engine pipe server is only supported on windows")
    }

    pub fn recv<T: DeserializeOwned>(&self) -> Result<Option<T>> {
        anyhow::bail!("engine pipe server is only supported on windows")
    }

    pub fn send<T: Serialize>(&self, _message: &T) -> Result<()> {
        anyhow::bail!("engine pipe server is only supported on windows")
    }

    pub fn disconnect(&self) {}
}

#[cfg(not(windows))]
impl EnginePipeClient {
    pub fn connect(_pipe_name: &str, _timeout: Duration) -> Result<Self> {
        anyhow::bail!("engine pipe client is only supported on windows")
    }

    pub fn connect_default(_timeout: Duration) -> Result<Self> {
        anyhow::bail!("engine pipe client is only supported on windows")
    }

    pub fn send<T: Serialize>(&self, _message: &T) -> Result<()> {
        anyhow::bail!("engine pipe client is only supported on windows")
    }

    pub fn recv<T: DeserializeOwned>(&self) -> Result<Option<T>> {
        anyhow::bail!("engine pipe client is only supported on windows")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipe_name_is_derived_from_the_shared_base() {
        // Verifies the fixed path matches the base name the WinUI client connects to.
        assert_eq!(ENGINE_PIPE_NAME, format!(r"\\.\pipe\{ENGINE_PIPE_BASE}"));
    }
}
