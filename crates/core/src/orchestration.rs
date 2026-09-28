use crate::process::{ProcessTarget, TerminationOutcome, TerminationReport};
use crate::profile::Profile;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Shared by the Settings/Runner envelope and the EngineSvc envelope; the
/// WinUI client's Bincode codecs check the same value.
pub const PROTOCOL_VERSION: u16 = 3;

static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuthContext {
    InteractiveUser,
    RunnerService,
    ScheduledTask,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope<T> {
    pub protocol_version: u16,
    pub client_id: String,
    pub request_id: String,
    pub timestamp_unix_ms: u64,
    pub auth_context: AuthContext,
    pub payload: T,
}

impl<T> Envelope<T> {
    pub fn new(client_id: impl Into<String>, auth_context: AuthContext, payload: T) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            client_id: client_id.into(),
            request_id: next_request_id("edge"),
            timestamp_unix_ms: now_unix_ms(),
            auth_context,
            payload,
        }
    }

    pub fn with_request_id(
        client_id: impl Into<String>,
        auth_context: AuthContext,
        request_id: impl Into<String>,
        payload: T,
    ) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            client_id: client_id.into(),
            request_id: request_id.into(),
            timestamp_unix_ms: now_unix_ms(),
            auth_context,
            payload,
        }
    }

    pub fn respond<U>(
        &self,
        client_id: impl Into<String>,
        auth_context: AuthContext,
        payload: U,
    ) -> Envelope<U> {
        Envelope {
            protocol_version: self.protocol_version,
            client_id: client_id.into(),
            request_id: self.request_id.clone(),
            timestamp_unix_ms: now_unix_ms(),
            auth_context,
            payload,
        }
    }
}

pub fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}

pub fn next_request_id(prefix: &str) -> String {
    let counter = REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{}-{}-{}", prefix, now_unix_ms(), counter)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CleanupKind {
    RecycleBin,
    BrowserCache,
}

impl CleanupKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            CleanupKind::RecycleBin => "recycle-bin",
            CleanupKind::BrowserCache => "browser-cache",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SettingsToRunnerCommand {
    ActivateProfile {
        profile: Profile,
        game_session_id: Option<String>,
    },
    OpenFlyout,
    OpenSettings,
    RequestOptimization {
        profile: Profile,
        game_session_id: Option<String>,
    },
    RequestCleanup {
        cleanup_kind: CleanupKind,
    },
    PreviewImpact {
        profile: Profile,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EngineState {
    Starting,
    Ready,
    Degraded,
    Disconnected,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OperationResult {
    pub request_id: String,
    pub success: bool,
    pub summary: String,
    pub killed: Vec<String>,
    pub failed: Vec<String>,
    pub not_found: Vec<String>,
    pub skipped: Vec<String>,
}

impl OperationResult {
    pub fn from_termination(request_id: String, report: &TerminationReport) -> Self {
        let mut result = Self {
            request_id,
            success: report.is_success(),
            summary: report.summary(),
            ..Self::default()
        };
        for entry in &report.outcomes {
            let label = format!("{} ({})", entry.target.image_name, entry.target.pid);
            match entry.outcome {
                TerminationOutcome::Terminated => result.killed.push(label),
                TerminationOutcome::NotRunning | TerminationOutcome::IdentityChanged => {
                    result.not_found.push(label)
                }
                TerminationOutcome::Protected
                | TerminationOutcome::Critical
                | TerminationOutcome::OutsideSession => result.skipped.push(label),
                TerminationOutcome::AccessDenied
                | TerminationOutcome::Invalid
                | TerminationOutcome::Failed { .. } => result.failed.push(label),
            }
        }
        result
    }

    pub fn error(request_id: String, summary: impl Into<String>) -> Self {
        Self {
            request_id,
            success: false,
            summary: summary.into(),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RunnerToSettingsEvent {
    EngineState(EngineState),
    OptimizationResult(OperationResult),
    CleanupResult(OperationResult),
    UserActionRequired { reason: String },
    Ack { message: String },
}

/// Commands EngineSvc accepts from its authenticated clients, Runner and
/// Settings. Variant order is the Bincode tag the WinUI codec writes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EngineCommand {
    /// Terminate specific process instances; never names or patterns.
    TerminateTargets {
        targets: Vec<ProcessTarget>,
    },
    GetCapabilities,
    Ping,
}

/// EngineSvc responses. Variant order is the Bincode tag the WinUI codec reads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EngineEvent {
    Ack {
        message: String,
    },
    Error {
        code: String,
        recoverable: bool,
        message: String,
    },
    Capabilities {
        supports_process_termination: bool,
    },
    Pong,
    Termination(TerminationReport),
}

#[derive(Debug, Default)]
pub struct IdempotencyCache {
    seen: HashSet<String>,
}

impl IdempotencyCache {
    pub fn check_and_insert(&mut self, request_id: &str) -> bool {
        self.seen.insert(request_id.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_ids_are_unique_enough() {
        // Verifies sequential requests receive distinct correlation identifiers.
        let a = next_request_id("test");
        let b = next_request_id("test");
        assert_ne!(a, b);
    }

    #[test]
    fn idempotency_cache_detects_duplicates() {
        // Verifies duplicate request identifiers are rejected without executing work.
        let mut cache = IdempotencyCache::default();
        assert!(cache.check_and_insert("req-1"));
        assert!(!cache.check_and_insert("req-1"));
    }

    #[test]
    fn response_preserves_request_and_protocol_version() {
        // Verifies responses retain the caller's correlation ID and protocol version.
        let request = Envelope::with_request_id(
            "settings",
            AuthContext::InteractiveUser,
            "request-42",
            SettingsToRunnerCommand::OpenSettings,
        );
        let response = request.respond("runner", AuthContext::RunnerService, EngineState::Ready);
        assert_eq!(response.request_id, "request-42");
        assert_eq!(response.protocol_version, PROTOCOL_VERSION);
    }

    fn fixture_target(pid: u32, name: &str) -> ProcessTarget {
        ProcessTarget {
            pid,
            creation_time: 0x1122_3344_5566_7788,
            image_name: name.into(),
        }
    }

    fn fixture_envelope<T>(payload: T) -> Envelope<T> {
        Envelope {
            protocol_version: PROTOCOL_VERSION,
            client_id: "c".into(),
            request_id: "r".into(),
            timestamp_unix_ms: 5,
            auth_context: AuthContext::InteractiveUser,
            payload,
        }
    }

    /// Envelope header for `fixture_envelope`, as Bincode lays it out.
    fn fixture_header() -> Vec<u8> {
        let mut bytes = vec![3, 0];
        bytes.extend([1, 0, 0, 0, 0, 0, 0, 0, b'c']);
        bytes.extend([1, 0, 0, 0, 0, 0, 0, 0, b'r']);
        bytes.extend([5, 0, 0, 0, 0, 0, 0, 0]);
        bytes.extend([0, 0, 0, 0]);
        bytes
    }

    fn fixture_target_bytes() -> Vec<u8> {
        let mut bytes = vec![0x04, 0x03, 0x02, 0x01];
        bytes.extend([0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11]);
        bytes.extend([5, 0, 0, 0, 0, 0, 0, 0]);
        bytes.extend(b"a.exe");
        bytes
    }

    #[test]
    fn operation_result_maps_every_termination_outcome() {
        // Verifies safe fixture outcomes map into the orchestration result lists without terminating a process.
        let entry = |pid, outcome| crate::process::TargetOutcome {
            target: fixture_target(pid, "fixture.exe"),
            outcome,
        };
        let report = TerminationReport {
            outcomes: vec![
                entry(1, TerminationOutcome::Terminated),
                entry(2, TerminationOutcome::AccessDenied),
                entry(3, TerminationOutcome::IdentityChanged),
                entry(4, TerminationOutcome::Critical),
            ],
        };
        let result = OperationResult::from_termination("request-1".into(), &report);
        assert!(!result.success);
        assert_eq!(result.killed, ["fixture.exe (1)"]);
        assert_eq!(result.failed, ["fixture.exe (2)"]);
        assert_eq!(result.not_found, ["fixture.exe (3)"]);
        assert_eq!(result.skipped, ["fixture.exe (4)"]);
    }

    #[test]
    fn engine_terminate_request_matches_the_winui_wire_layout() {
        // Verifies the exact bytes the WinUI EngineProtocol codec writes; both sides assert the same vector.
        let envelope = fixture_envelope(EngineCommand::TerminateTargets {
            targets: vec![fixture_target(0x0102_0304, "a.exe")],
        });
        let mut expected = fixture_header();
        expected.extend([0, 0, 0, 0]);
        expected.extend([1, 0, 0, 0, 0, 0, 0, 0]);
        expected.extend(fixture_target_bytes());
        assert_eq!(bincode::serialize(&envelope).unwrap(), expected);
    }

    #[test]
    fn engine_termination_response_matches_the_winui_wire_layout() {
        // Verifies the response bytes the WinUI codec decodes, including a tagged outcome with a payload.
        let envelope = fixture_envelope(EngineEvent::Termination(TerminationReport {
            outcomes: vec![crate::process::TargetOutcome {
                target: fixture_target(0x0102_0304, "a.exe"),
                outcome: TerminationOutcome::Failed { code: 5 },
            }],
        }));
        let mut expected = fixture_header();
        expected.extend([4, 0, 0, 0]);
        expected.extend([1, 0, 0, 0, 0, 0, 0, 0]);
        expected.extend(fixture_target_bytes());
        expected.extend([8, 0, 0, 0, 5, 0, 0, 0]);
        assert_eq!(bincode::serialize(&envelope).unwrap(), expected);
        let decoded: Envelope<EngineEvent> = bincode::deserialize(&expected).unwrap();
        assert!(matches!(
            decoded.payload,
            EngineEvent::Termination(report) if !report.is_success()
        ));
    }

    #[test]
    fn auth_context_is_only_serialized_claim_data() {
        // Verifies AuthContext round-trips as message data without conferring authorization behavior.
        let encoded = bincode::serialize(&AuthContext::RunnerService).unwrap();
        let decoded: AuthContext = bincode::deserialize(&encoded).unwrap();
        assert_eq!(decoded, AuthContext::RunnerService);
    }
}
