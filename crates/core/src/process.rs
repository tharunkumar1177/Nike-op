//! Process identity, protected-process policy, and validated termination.
//!
//! Windows reuses PIDs, so a PID alone never identifies a process. A process is
//! identified by its PID together with its creation time, and termination
//! re-opens the PID, re-checks that pair and the image name, and terminates
//! through the same handle. An open handle keeps the process object alive, so
//! the PID cannot be recycled between validation and termination.

use serde::{Deserialize, Serialize};

/// Upper bound on targets in one termination request.
pub const MAX_TERMINATION_TARGETS: usize = 128;
/// Upper bound, in UTF-8 bytes, on a requested image file name.
pub const MAX_IMAGE_NAME_BYTES: usize = 255;

/// Processes that are never listed as targets or terminated, compared after
/// [`normalize_process_name`]. Security products are included because no
/// component may disable them.
const PROTECTED_PROCESSES: &[&str] = &[
    "audiodg",
    "csrss",
    "ctfmon",
    "dwm",
    "edgeoptimizer.settings.winui",
    "edgeoptimizer_crosshair",
    "edgeoptimizer_enginesvc",
    "edgeoptimizer_macro",
    "edgeoptimizer_runner",
    "explorer",
    "fontdrvhost",
    "lsaiso",
    "lsass",
    "memory compression",
    "msmpeng",
    "mssense",
    "nissrv",
    "registry",
    "securityhealthservice",
    "services",
    "sihost",
    "smss",
    "svchost",
    "system",
    "wininit",
    "winlogon",
];

/// PIDs of the System Idle Process and the System process.
const RESERVED_PIDS: [u32; 2] = [0, 4];

/// Normalize a process name for comparison: trimmed, ASCII-lowercased, and
/// without a trailing `.exe`.
pub fn normalize_process_name(name: &str) -> String {
    let lower = name.trim().to_ascii_lowercase();
    lower
        .strip_suffix(".exe")
        .unwrap_or(&lower)
        .trim()
        .to_string()
}

/// Whether a name refers to a protected process in any accepted spelling.
pub fn would_be_protected(process_name: &str) -> bool {
    let normalized = normalize_process_name(process_name);
    PROTECTED_PROCESSES.contains(&normalized.as_str())
}

/// Whether two process names refer to the same executable.
pub fn names_match(left: &str, right: &str) -> bool {
    let left = normalize_process_name(left);
    !left.is_empty() && left == normalize_process_name(right)
}

/// One specific process instance, identified by PID and creation time.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProcessTarget {
    pub pid: u32,
    /// Creation time as a Windows FILETIME (100 ns intervals since 1601).
    pub creation_time: u64,
    /// Image file name, for example `discord.exe`.
    pub image_name: String,
}

/// Per-target result of a termination request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminationOutcome {
    Terminated,
    /// The process had already exited.
    NotRunning,
    /// The PID now belongs to a different process than the one requested.
    IdentityChanged,
    Protected,
    /// Windows marks the process as critical.
    Critical,
    /// The process runs in a different Windows session than the requester.
    OutsideSession,
    AccessDenied,
    /// The request itself was malformed.
    Invalid,
    Failed {
        code: u32,
    },
}

impl TerminationOutcome {
    pub fn describe(&self) -> &'static str {
        match self {
            TerminationOutcome::Terminated => "closed",
            TerminationOutcome::NotRunning => "already closed",
            TerminationOutcome::IdentityChanged => "skipped (process changed)",
            TerminationOutcome::Protected => "skipped (protected)",
            TerminationOutcome::Critical => "skipped (critical)",
            TerminationOutcome::OutsideSession => "skipped (other session)",
            TerminationOutcome::AccessDenied => "failed (access denied)",
            TerminationOutcome::Invalid => "rejected (invalid request)",
            TerminationOutcome::Failed { .. } => "failed",
        }
    }

    /// Whether the outcome means the requested process could not be closed
    /// for a reason other than safety policy or it already being gone.
    pub fn is_failure(&self) -> bool {
        matches!(
            self,
            TerminationOutcome::AccessDenied
                | TerminationOutcome::Invalid
                | TerminationOutcome::Failed { .. }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetOutcome {
    pub target: ProcessTarget,
    pub outcome: TerminationOutcome,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminationReport {
    pub outcomes: Vec<TargetOutcome>,
}

impl TerminationReport {
    pub fn count(&self, predicate: impl Fn(&TerminationOutcome) -> bool) -> usize {
        self.outcomes
            .iter()
            .filter(|entry| predicate(&entry.outcome))
            .count()
    }

    pub fn is_success(&self) -> bool {
        !self.outcomes.iter().any(|entry| entry.outcome.is_failure())
    }

    pub fn summary(&self) -> String {
        if self.outcomes.is_empty() {
            return "No selected apps were running.".to_string();
        }
        let closed = self.count(|o| *o == TerminationOutcome::Terminated);
        let gone = self.count(|o| {
            matches!(
                o,
                TerminationOutcome::NotRunning | TerminationOutcome::IdentityChanged
            )
        });
        let skipped = self.count(|o| {
            matches!(
                o,
                TerminationOutcome::Protected
                    | TerminationOutcome::Critical
                    | TerminationOutcome::OutsideSession
            )
        });
        let failed = self.count(TerminationOutcome::is_failure);
        format!("Closed {closed}, already closed {gone}, skipped {skipped}, failed {failed}.")
    }
}

/// A running process as seen by enumeration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunningProcess {
    pub pid: u32,
    /// Zero when the creation time could not be read; such a process is
    /// never turned into a target.
    pub creation_time: u64,
    pub image_name: String,
    pub session_id: u32,
}

/// Request-level validation that needs no operating-system access.
pub fn validate_target(target: &ProcessTarget) -> Result<(), TerminationOutcome> {
    if RESERVED_PIDS.contains(&target.pid) {
        return Err(TerminationOutcome::Protected);
    }
    let name = target.image_name.trim();
    if target.creation_time == 0
        || name.is_empty()
        || target.image_name.len() > MAX_IMAGE_NAME_BYTES
        || target
            .image_name
            .chars()
            .any(|c| matches!(c, '\\' | '/' | ':') || c.is_control())
    {
        return Err(TerminationOutcome::Invalid);
    }
    if would_be_protected(name) {
        return Err(TerminationOutcome::Protected);
    }
    Ok(())
}

/// Compare what the PID resolves to now against the requested identity.
pub fn verify_identity(
    expected: &ProcessTarget,
    actual_creation_time: u64,
    actual_image_path: &str,
) -> Result<(), TerminationOutcome> {
    let actual_name = actual_image_path
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(actual_image_path);
    if would_be_protected(actual_name) {
        return Err(TerminationOutcome::Protected);
    }
    if actual_creation_time != expected.creation_time
        || !names_match(actual_name, &expected.image_name)
    {
        return Err(TerminationOutcome::IdentityChanged);
    }
    Ok(())
}

/// Turn selected process names into concrete targets in one session.
///
/// Protected names, processes with an unknown creation time, and the calling
/// process are never returned.
pub fn resolve_targets(
    processes: &[RunningProcess],
    selected_names: &[String],
    session_id: u32,
    exclude_pid: u32,
) -> Vec<ProcessTarget> {
    let wanted: Vec<String> = selected_names
        .iter()
        .map(|name| normalize_process_name(name))
        .filter(|name| !name.is_empty() && !PROTECTED_PROCESSES.contains(&name.as_str()))
        .collect();
    let mut targets: Vec<ProcessTarget> = Vec::new();
    for process in processes {
        if process.session_id != session_id
            || process.creation_time == 0
            || process.pid == exclude_pid
            || RESERVED_PIDS.contains(&process.pid)
            || would_be_protected(&process.image_name)
        {
            continue;
        }
        let normalized = normalize_process_name(&process.image_name);
        if !wanted.contains(&normalized) || targets.iter().any(|t| t.pid == process.pid) {
            continue;
        }
        targets.push(ProcessTarget {
            pid: process.pid,
            creation_time: process.creation_time,
            image_name: process.image_name.clone(),
        });
    }
    targets
}

#[cfg(windows)]
pub use os::{enumerate_processes, process_image_path, terminate_target, WatchedProcess};

#[cfg(windows)]
mod os {
    use super::{
        validate_target, verify_identity, ProcessTarget, RunningProcess, TerminationOutcome,
    };
    use anyhow::{Context, Result};
    use windows::core::PWSTR;
    use windows::Win32::Foundation::{
        CloseHandle, BOOL, ERROR_ACCESS_DENIED, ERROR_INVALID_PARAMETER, FILETIME, HANDLE,
        WAIT_OBJECT_0,
    };
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
    use windows::Win32::System::Threading::{
        GetProcessTimes, IsProcessCritical, OpenProcess, QueryFullProcessImageNameW,
        TerminateProcess, WaitForSingleObject, PROCESS_ACCESS_RIGHTS, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
    };

    struct OwnedHandle(HANDLE);

    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }

    fn open(pid: u32, access: PROCESS_ACCESS_RIGHTS) -> windows::core::Result<OwnedHandle> {
        unsafe { OpenProcess(access, false, pid) }.map(OwnedHandle)
    }

    fn creation_time(handle: &OwnedHandle) -> Option<u64> {
        let mut created = FILETIME::default();
        let mut exited = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        unsafe { GetProcessTimes(handle.0, &mut created, &mut exited, &mut kernel, &mut user) }
            .ok()?;
        let value = (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime);
        (value != 0).then_some(value)
    }

    fn image_path(handle: &OwnedHandle) -> Option<String> {
        let mut buffer = [0u16; 1024];
        let mut length = buffer.len() as u32;
        unsafe {
            QueryFullProcessImageNameW(
                handle.0,
                PROCESS_NAME_WIN32,
                PWSTR(buffer.as_mut_ptr()),
                &mut length,
            )
        }
        .ok()?;
        Some(String::from_utf16_lossy(&buffer[..length as usize]))
    }

    fn session_of(pid: u32) -> Option<u32> {
        let mut session = 0u32;
        unsafe { ProcessIdToSessionId(pid, &mut session) }.ok()?;
        Some(session)
    }

    fn has_exited(handle: &OwnedHandle) -> bool {
        unsafe { WaitForSingleObject(handle.0, 0) == WAIT_OBJECT_0 }
    }

    /// Full image path of a process, for authenticating pipe peers.
    pub fn process_image_path(pid: u32) -> Result<String> {
        let handle = open(pid, PROCESS_QUERY_LIMITED_INFORMATION)
            .with_context(|| format!("cannot open process {pid}"))?;
        image_path(&handle).with_context(|| format!("cannot read the image path of process {pid}"))
    }

    /// Enumerate processes with the identity needed to target them later.
    pub fn enumerate_processes() -> Result<Vec<RunningProcess>> {
        let snapshot = OwnedHandle(
            unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }
                .context("cannot snapshot running processes")?,
        );
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut processes = Vec::new();
        let mut more = unsafe { Process32FirstW(snapshot.0, &mut entry) }.is_ok();
        while more {
            let length = entry
                .szExeFile
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(entry.szExeFile.len());
            let pid = entry.th32ProcessID;
            if let Some(session_id) = session_of(pid) {
                let creation_time = open(pid, PROCESS_QUERY_LIMITED_INFORMATION)
                    .ok()
                    .and_then(|handle| creation_time(&handle))
                    .unwrap_or(0);
                processes.push(RunningProcess {
                    pid,
                    creation_time,
                    image_name: String::from_utf16_lossy(&entry.szExeFile[..length]),
                    session_id,
                });
            }
            more = unsafe { Process32NextW(snapshot.0, &mut entry) }.is_ok();
        }
        Ok(processes)
    }

    /// Terminate one target after re-validating it against the live PID.
    ///
    /// `required_session` is the Windows session of the authenticated
    /// requester; processes in any other session are never terminated.
    pub fn terminate_target(target: &ProcessTarget, required_session: u32) -> TerminationOutcome {
        if let Err(outcome) = validate_target(target) {
            return outcome;
        }
        if target.pid == std::process::id() {
            return TerminationOutcome::Protected;
        }
        let handle = match open(
            target.pid,
            PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
        ) {
            Ok(handle) => handle,
            Err(error) if error.code() == ERROR_INVALID_PARAMETER.to_hresult() => {
                return TerminationOutcome::NotRunning
            }
            Err(error) if error.code() == ERROR_ACCESS_DENIED.to_hresult() => {
                return TerminationOutcome::AccessDenied
            }
            Err(error) => {
                return TerminationOutcome::Failed {
                    code: error.code().0 as u32,
                }
            }
        };
        let (Some(created), Some(path)) = (creation_time(&handle), image_path(&handle)) else {
            return TerminationOutcome::IdentityChanged;
        };
        if let Err(outcome) = verify_identity(target, created, &path) {
            return outcome;
        }
        if has_exited(&handle) {
            return TerminationOutcome::NotRunning;
        }
        if session_of(target.pid) != Some(required_session) {
            return TerminationOutcome::OutsideSession;
        }
        let mut critical = BOOL(0);
        if unsafe { IsProcessCritical(handle.0, &mut critical) }.is_err() || critical.as_bool() {
            return TerminationOutcome::Critical;
        }
        match unsafe { TerminateProcess(handle.0, 1) } {
            Ok(()) => TerminationOutcome::Terminated,
            Err(error) if error.code() == ERROR_ACCESS_DENIED.to_hresult() => {
                TerminationOutcome::AccessDenied
            }
            Err(error) => TerminationOutcome::Failed {
                code: error.code().0 as u32,
            },
        }
    }

    /// A process pinned by an open handle so its exit is observed without
    /// re-enumeration and its PID cannot be recycled while watched.
    pub struct WatchedProcess {
        handle: OwnedHandle,
        pub target: ProcessTarget,
    }

    impl WatchedProcess {
        /// Watch a target only if its PID still belongs to the same process.
        pub fn open(target: ProcessTarget) -> Option<Self> {
            let handle = open(
                target.pid,
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            )
            .ok()?;
            (creation_time(&handle) == Some(target.creation_time))
                .then_some(Self { handle, target })
        }

        pub fn has_exited(&self) -> bool {
            has_exited(&self.handle)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(pid: u32, name: &str) -> ProcessTarget {
        ProcessTarget {
            pid,
            creation_time: 1_000,
            image_name: name.into(),
        }
    }

    fn running(pid: u32, name: &str, session_id: u32) -> RunningProcess {
        RunningProcess {
            pid,
            creation_time: 1_000 + u64::from(pid),
            image_name: name.into(),
            session_id,
        }
    }

    #[test]
    fn test_normalize_process_name() {
        // Verifies executable names normalize casing, whitespace, and optional extensions consistently.
        assert_eq!(normalize_process_name("notepad.exe"), "notepad");
        assert_eq!(normalize_process_name("Notepad.exe"), "notepad");
        assert_eq!(normalize_process_name("NOTEPAD.EXE"), "notepad");
        assert_eq!(normalize_process_name("notepad"), "notepad");
        assert_eq!(normalize_process_name("  Notepad.ExE  "), "notepad");
        assert_eq!(normalize_process_name(" notepad .exe"), "notepad");
    }

    #[test]
    fn protected_names_cannot_bypass_the_blocklist() {
        // Verifies protected Windows names cannot bypass the blocklist through alternate spelling forms.
        for name in [
            "csrss.exe",
            "CSRSS.EXE",
            "csrss",
            " svchost.exe ",
            "Explorer.exe",
            "MsMpEng.exe",
            "EdgeOptimizer_EngineSvc.exe",
            "Memory Compression",
        ] {
            assert!(would_be_protected(name), "{name:?} was not protected");
        }
        assert!(!would_be_protected("notepad.exe"));
        assert!(!would_be_protected("chrome.exe"));
    }

    #[test]
    fn names_match_ignores_case_extension_and_padding_but_not_empty_names() {
        // Verifies profile names and discovered images compare with the same normalization.
        assert!(names_match("Discord", " discord.EXE "));
        assert!(!names_match("discord", "discordptb.exe"));
        assert!(!names_match("", ".exe"));
    }

    #[test]
    fn validate_target_rejects_reserved_protected_and_malformed_requests() {
        // Verifies request-level checks run before any process is opened.
        assert_eq!(validate_target(&target(4321, "game.exe")), Ok(()));
        assert_eq!(
            validate_target(&target(0, "game.exe")),
            Err(TerminationOutcome::Protected)
        );
        assert_eq!(
            validate_target(&target(4, "game.exe")),
            Err(TerminationOutcome::Protected)
        );
        for name in ["LSASS", "lsass.exe", " Lsass.Exe "] {
            assert_eq!(
                validate_target(&target(10, name)),
                Err(TerminationOutcome::Protected)
            );
        }
        for name in [
            "",
            "   ",
            r"C:\Windows\game.exe",
            "dir/game.exe",
            "C:game.exe",
            "game\u{0}.exe",
        ] {
            assert_eq!(
                validate_target(&target(10, name)),
                Err(TerminationOutcome::Invalid),
                "{name:?}"
            );
        }
        let oversized = "a".repeat(MAX_IMAGE_NAME_BYTES + 1);
        assert_eq!(
            validate_target(&target(10, &oversized)),
            Err(TerminationOutcome::Invalid)
        );
        let mut unknown_time = target(10, "game.exe");
        unknown_time.creation_time = 0;
        assert_eq!(
            validate_target(&unknown_time),
            Err(TerminationOutcome::Invalid)
        );
    }

    #[test]
    fn verify_identity_detects_pid_reuse_and_renamed_images() {
        // Verifies a recycled PID or a different image never passes identity validation.
        let expected = target(77, "Discord.exe");
        assert_eq!(
            verify_identity(&expected, 1_000, r"C:\Apps\discord.exe"),
            Ok(())
        );
        assert_eq!(
            verify_identity(&expected, 2_000, r"C:\Apps\discord.exe"),
            Err(TerminationOutcome::IdentityChanged)
        );
        assert_eq!(
            verify_identity(&expected, 1_000, r"C:\Apps\notepad.exe"),
            Err(TerminationOutcome::IdentityChanged)
        );
        assert_eq!(
            verify_identity(&expected, 1_000, r"C:\Windows\System32\winlogon.exe"),
            Err(TerminationOutcome::Protected)
        );
    }

    #[test]
    fn resolve_targets_matches_every_instance_in_the_session_only() {
        // Verifies name selection expands to each same-session instance and excludes unsafe processes.
        let processes = [
            running(100, "chrome.exe", 1),
            running(101, "Chrome.exe", 1),
            running(102, "chrome.exe", 2),
            running(103, "svchost.exe", 1),
            running(104, "discord.exe", 1),
            RunningProcess {
                creation_time: 0,
                ..running(105, "chrome.exe", 1)
            },
            running(106, "chrome.exe", 1),
        ];
        let selected = [" CHROME ".to_string(), "svchost".to_string(), String::new()];
        let targets = resolve_targets(&processes, &selected, 1, 106);
        let pids: Vec<u32> = targets.iter().map(|t| t.pid).collect();
        assert_eq!(pids, [100, 101]);
        assert_eq!(targets[1].creation_time, 1_101);
        assert_eq!(targets[1].image_name, "Chrome.exe");
    }

    #[test]
    fn report_summary_and_success_classify_every_outcome() {
        // Verifies safety skips are not failures while access and malformed-request errors are.
        let entry = |outcome| TargetOutcome {
            target: target(9, "game.exe"),
            outcome,
        };
        let mut report = TerminationReport {
            outcomes: vec![
                entry(TerminationOutcome::Terminated),
                entry(TerminationOutcome::NotRunning),
                entry(TerminationOutcome::Protected),
            ],
        };
        assert!(report.is_success());
        assert_eq!(
            report.summary(),
            "Closed 1, already closed 1, skipped 1, failed 0."
        );
        report
            .outcomes
            .push(entry(TerminationOutcome::Failed { code: 5 }));
        assert!(!report.is_success());
        assert_eq!(
            TerminationReport::default().summary(),
            "No selected apps were running."
        );
    }
}
