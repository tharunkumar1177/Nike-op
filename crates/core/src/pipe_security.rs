//! Pipe naming and security descriptors for Runner's and EngineSvc's pipes.
//!
//! Names only separate concurrent sign-ins; access control comes from the
//! security descriptor, `FILE_FLAG_FIRST_PIPE_INSTANCE` on the server, and the
//! client verifying which process or principal serves the pipe.

use anyhow::Result;

/// Protected DACL: deny network logons, allow the pipe owner and SYSTEM only.
pub const OWNER_ONLY_PIPE_SDDL: &str = "D:P(D;;GA;;;NU)(A;;GA;;;OW)(A;;GA;;;SY)";

/// EngineSvc pipe: owned by SYSTEM, network logons denied, interactive users
/// may read and write.
///
/// Read/write also grants interactive users the right to create pipe
/// instances. EngineSvc therefore holds its only instance for its whole
/// lifetime, and clients accept the pipe only when SYSTEM owns it, which a
/// standard user cannot forge.
pub const ENGINE_PIPE_SDDL: &str = "O:SYD:P(D;;GA;;;NU)(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;IU)";

/// Full pipe path for a base name within one Windows session.
pub fn session_pipe_name(base: &str, session_id: u32) -> String {
    format!(r"\\.\pipe\{}-{}", base, session_id)
}

#[cfg(windows)]
pub use windows_impl::{
    current_session_id, engine_pipe_attributes, owner_only_attributes, pipe_client_process_id,
    pipe_client_session_id, pipe_is_owned_by_system, pipe_server_process_id,
};

#[cfg(windows)]
mod windows_impl {
    use super::{ENGINE_PIPE_SDDL, OWNER_ONLY_PIPE_SDDL};
    use anyhow::{Context, Result};
    use std::ffi::c_void;
    use std::sync::OnceLock;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{LocalFree, HANDLE, HLOCAL, PSID};
    use windows::Win32::Security::Authorization::{GetSecurityInfo, SE_KERNEL_OBJECT};
    use windows::Win32::Security::{
        IsWellKnownSid, WinLocalSystemSid, OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR,
        SECURITY_ATTRIBUTES,
    };
    use windows::Win32::System::Pipes::{
        GetNamedPipeClientProcessId, GetNamedPipeClientSessionId, GetNamedPipeServerProcessId,
    };
    use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
    use windows::Win32::System::Threading::GetCurrentProcessId;

    const SDDL_REVISION_1: u32 = 1;

    pub fn current_session_id() -> Result<u32> {
        let mut session_id = 0u32;
        let result = unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &mut session_id) };
        result.context("cannot query the Windows session")?;
        Ok(session_id)
    }

    /// Security attributes carrying the owner-only descriptor.
    pub fn owner_only_attributes() -> Result<SECURITY_ATTRIBUTES> {
        static DESCRIPTOR: OnceLock<usize> = OnceLock::new();
        attributes(OWNER_ONLY_PIPE_SDDL, &DESCRIPTOR)
    }

    /// Security attributes carrying the EngineSvc descriptor.
    pub fn engine_pipe_attributes() -> Result<SECURITY_ATTRIBUTES> {
        static DESCRIPTOR: OnceLock<usize> = OnceLock::new();
        attributes(ENGINE_PIPE_SDDL, &DESCRIPTOR)
    }

    /// The descriptor is converted once and kept for the process lifetime
    /// because pipe servers are recreated after every disconnect.
    fn attributes(sddl: &str, cell: &'static OnceLock<usize>) -> Result<SECURITY_ATTRIBUTES> {
        Ok(SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor(sddl, cell)?,
            bInheritHandle: false.into(),
        })
    }

    fn descriptor(sddl: &str, cell: &'static OnceLock<usize>) -> Result<*mut c_void> {
        if let Some(address) = cell.get() {
            return Ok(*address as *mut c_void);
        }

        let wide: Vec<u16> = sddl.encode_utf16().chain(Some(0)).collect();
        let mut descriptor = PSECURITY_DESCRIPTOR(std::ptr::null_mut());
        let converted = unsafe {
            windows::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(wide.as_ptr()),
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )
        };
        converted.context("invalid pipe security descriptor")?;

        Ok(*cell.get_or_init(|| descriptor.0 as usize) as *mut c_void)
    }

    /// Process ID of the server end of a connected client pipe handle.
    pub fn pipe_server_process_id(pipe: HANDLE) -> Result<u32> {
        let mut process_id = 0u32;
        let result = unsafe { GetNamedPipeServerProcessId(pipe, &mut process_id) };
        result.context("cannot identify the pipe server")?;
        Ok(process_id)
    }

    /// Process ID of the client connected to a server pipe handle.
    pub fn pipe_client_process_id(pipe: HANDLE) -> Result<u32> {
        let mut process_id = 0u32;
        unsafe { GetNamedPipeClientProcessId(pipe, &mut process_id) }
            .context("cannot identify the pipe client")?;
        Ok(process_id)
    }

    /// Windows session of the client connected to a server pipe handle.
    pub fn pipe_client_session_id(pipe: HANDLE) -> Result<u32> {
        let mut session_id = 0u32;
        unsafe { GetNamedPipeClientSessionId(pipe, &mut session_id) }
            .context("cannot identify the pipe client session")?;
        Ok(session_id)
    }

    /// Whether the pipe object behind a handle is owned by LocalSystem.
    pub fn pipe_is_owned_by_system(pipe: HANDLE) -> Result<bool> {
        let mut owner = PSID::default();
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        unsafe {
            GetSecurityInfo(
                pipe,
                SE_KERNEL_OBJECT,
                OWNER_SECURITY_INFORMATION,
                Some(&mut owner),
                None,
                None,
                None,
                Some(&mut descriptor),
            )
        }
        .context("cannot read the pipe owner")?;
        let is_system = unsafe { IsWellKnownSid(owner, WinLocalSystemSid) }.as_bool();
        unsafe {
            let _ = LocalFree(HLOCAL(descriptor.0));
        }
        Ok(is_system)
    }
}

/// Pipe path for a base name in the caller's own session.
#[cfg(windows)]
pub fn current_session_pipe_name(base: &str) -> Result<String> {
    Ok(session_pipe_name(base, current_session_id()?))
}

#[cfg(not(windows))]
pub fn current_session_pipe_name(_base: &str) -> Result<String> {
    anyhow::bail!("named pipes are only supported on Windows")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allowed_aces(sddl: &str) -> Vec<(&str, &str)> {
        sddl.split('(')
            .filter_map(|ace| ace.strip_prefix("A;;"))
            .filter_map(|ace| {
                let (rights, rest) = ace.split_once(";;;")?;
                Some((rights, rest.trim_end_matches(')')))
            })
            .collect()
    }

    #[test]
    fn session_pipe_names_are_distinct_per_session() {
        // Verifies two concurrent sign-ins never resolve to the same pipe path.
        assert_eq!(
            session_pipe_name("EdgeOptimizerIPC", 3),
            r"\\.\pipe\EdgeOptimizerIPC-3"
        );
        assert_ne!(
            session_pipe_name("EdgeOptimizerIPC", 1),
            session_pipe_name("EdgeOptimizerIPC", 2)
        );
    }

    #[test]
    fn descriptor_is_protected_denies_network_first_and_grants_only_owner_and_system() {
        // Verifies the DACL cannot inherit broader access and orders the network deny before any allow.
        assert!(OWNER_ONLY_PIPE_SDDL.starts_with("D:P(D;;GA;;;NU)"));
        assert_eq!(
            allowed_aces(OWNER_ONLY_PIPE_SDDL),
            [("GA", "OW"), ("GA", "SY")]
        );
    }

    #[test]
    fn engine_descriptor_is_system_owned_and_limits_interactive_users_to_read_write() {
        // Verifies EngineSvc's pipe is SYSTEM-owned, denies network logons first, and never grants users full access.
        assert!(ENGINE_PIPE_SDDL.starts_with("O:SYD:P(D;;GA;;;NU)"));
        assert_eq!(
            allowed_aces(ENGINE_PIPE_SDDL),
            [("GA", "SY"), ("GA", "BA"), ("GRGW", "IU")]
        );
        assert!(!ENGINE_PIPE_SDDL.contains("WD"));
        assert!(!ENGINE_PIPE_SDDL.contains("AU"));
    }

    #[cfg(windows)]
    #[test]
    fn owner_only_descriptor_converts_and_is_reused() {
        // Verifies Windows accepts the SDDL and repeated callers share one process-lifetime descriptor.
        let first = owner_only_attributes().unwrap();
        let second = owner_only_attributes().unwrap();
        assert!(!first.lpSecurityDescriptor.is_null());
        assert_eq!(first.lpSecurityDescriptor, second.lpSecurityDescriptor);
        let name = current_session_pipe_name("EdgeOptimizerIPC").unwrap();
        assert!(name.starts_with(r"\\.\pipe\EdgeOptimizerIPC-"));
    }
}
