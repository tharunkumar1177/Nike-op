//! Per-session pipe naming and owner-only pipe security for Runner's pipes.
//!
//! The name only separates concurrent sign-ins; access control comes from the
//! security descriptor, `FILE_FLAG_FIRST_PIPE_INSTANCE` on the server, and the
//! client verifying which process serves the pipe.

use anyhow::Result;

/// Protected DACL: deny network logons, allow the pipe owner and SYSTEM only.
pub const OWNER_ONLY_PIPE_SDDL: &str = "D:P(D;;GA;;;NU)(A;;GA;;;OW)(A;;GA;;;SY)";

/// Full pipe path for a base name within one Windows session.
pub fn session_pipe_name(base: &str, session_id: u32) -> String {
    format!(r"\\.\pipe\{}-{}", base, session_id)
}

#[cfg(windows)]
pub use windows_impl::{current_session_id, owner_only_attributes, pipe_server_process_id};

#[cfg(windows)]
mod windows_impl {
    use super::OWNER_ONLY_PIPE_SDDL;
    use anyhow::{Context, Result};
    use std::ffi::c_void;
    use std::sync::OnceLock;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
    use windows::Win32::System::Pipes::GetNamedPipeServerProcessId;
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
    ///
    /// The descriptor is converted once and kept for the process lifetime
    /// because pipe servers are recreated after every disconnect.
    pub fn owner_only_attributes() -> Result<SECURITY_ATTRIBUTES> {
        Ok(SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: owner_only_descriptor()?,
            bInheritHandle: false.into(),
        })
    }

    fn owner_only_descriptor() -> Result<*mut c_void> {
        static DESCRIPTOR: OnceLock<usize> = OnceLock::new();
        if let Some(address) = DESCRIPTOR.get() {
            return Ok(*address as *mut c_void);
        }

        let sddl: Vec<u16> = OWNER_ONLY_PIPE_SDDL.encode_utf16().chain(Some(0)).collect();
        let mut descriptor = PSECURITY_DESCRIPTOR(std::ptr::null_mut());
        let converted = unsafe {
            windows::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(sddl.as_ptr()),
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )
        };
        converted.context("invalid pipe security descriptor")?;

        Ok(*DESCRIPTOR.get_or_init(|| descriptor.0 as usize) as *mut c_void)
    }

    /// Process ID of the server end of a connected client pipe handle.
    pub fn pipe_server_process_id(pipe: HANDLE) -> Result<u32> {
        let mut process_id = 0u32;
        let result = unsafe { GetNamedPipeServerProcessId(pipe, &mut process_id) };
        result.context("cannot identify the pipe server")?;
        Ok(process_id)
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
        let allowed: Vec<&str> = OWNER_ONLY_PIPE_SDDL
            .split('(')
            .filter_map(|ace| ace.strip_prefix("A;;GA;;;"))
            .map(|sid| sid.trim_end_matches(')'))
            .collect();
        assert_eq!(allowed, ["OW", "SY"]);
        assert_eq!(OWNER_ONLY_PIPE_SDDL.matches("(A;").count(), 2);
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
