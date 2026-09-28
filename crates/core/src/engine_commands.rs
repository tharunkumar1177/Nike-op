//! Pure Engine command routing separated from Windows side effects.

use crate::orchestration::{
    CleanupKind, EngineToRunnerEvent, Envelope, OperationResult, RunnerToEngineCommand,
};
use crate::process::KillReport;

pub trait EngineOperations {
    fn kill_processes(&mut self, processes: &[String]) -> KillReport;
    fn run_cleanup(&mut self, request_id: &str, cleanup_kind: CleanupKind) -> OperationResult;
}

pub fn dispatch_engine_command(
    request: &Envelope<RunnerToEngineCommand>,
    operations: &mut impl EngineOperations,
) -> EngineToRunnerEvent {
    match &request.payload {
        RunnerToEngineCommand::Ping => EngineToRunnerEvent::Pong,
        RunnerToEngineCommand::GetCapabilities => EngineToRunnerEvent::Capabilities {
            cleanup_kinds: vec![CleanupKind::RecycleBin, CleanupKind::BrowserCache],
            supports_process_kill: true,
        },
        RunnerToEngineCommand::KillProcesses { processes } => {
            let report = operations.kill_processes(processes);
            EngineToRunnerEvent::Result(OperationResult::from_kill_report(
                request.request_id.clone(),
                report,
            ))
        }
        RunnerToEngineCommand::ApplyProfile { profile } => {
            let report = operations.kill_processes(&profile.processes_to_kill);
            let mut result = OperationResult::from_kill_report(request.request_id.clone(), report);
            result.summary = format!("profile={} {}", profile.name, result.summary);
            EngineToRunnerEvent::Result(result)
        }
        RunnerToEngineCommand::RunCleanup { cleanup_kind } => EngineToRunnerEvent::Result(
            operations.run_cleanup(&request.request_id, cleanup_kind.clone()),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestration::{AuthContext, Envelope};
    use crate::profile::create_profile;

    #[derive(Default)]
    struct FakeOperations {
        killed_inputs: Vec<Vec<String>>,
        cleanup_inputs: Vec<CleanupKind>,
    }

    impl EngineOperations for FakeOperations {
        fn kill_processes(&mut self, processes: &[String]) -> KillReport {
            self.killed_inputs.push(processes.to_vec());
            KillReport {
                killed: processes.to_vec(),
                failed: Vec::new(),
                not_found: Vec::new(),
                blocklist_skipped: Vec::new(),
            }
        }

        fn run_cleanup(&mut self, request_id: &str, cleanup_kind: CleanupKind) -> OperationResult {
            self.cleanup_inputs.push(cleanup_kind);
            OperationResult {
                request_id: request_id.to_string(),
                success: true,
                summary: "fake cleanup".into(),
                ..OperationResult::default()
            }
        }
    }

    fn request(payload: RunnerToEngineCommand) -> Envelope<RunnerToEngineCommand> {
        Envelope::with_request_id("test", AuthContext::Unknown, "request-7", payload)
    }

    #[test]
    fn ping_and_capabilities_are_pure_responses() {
        // Verifies discovery commands return without invoking any operating-system operation.
        let mut operations = FakeOperations::default();
        assert!(matches!(
            dispatch_engine_command(&request(RunnerToEngineCommand::Ping), &mut operations),
            EngineToRunnerEvent::Pong
        ));
        assert!(matches!(
            dispatch_engine_command(
                &request(RunnerToEngineCommand::GetCapabilities),
                &mut operations
            ),
            EngineToRunnerEvent::Capabilities {
                supports_process_kill: true,
                ..
            }
        ));
        assert!(operations.killed_inputs.is_empty());
        assert!(operations.cleanup_inputs.is_empty());
    }

    #[test]
    fn process_intents_are_delegated_to_the_injected_fake() {
        // Verifies kill and apply-profile commands route fixture names without touching real processes.
        let mut operations = FakeOperations::default();
        let names = vec!["fixture.exe".to_string()];
        let result = dispatch_engine_command(
            &request(RunnerToEngineCommand::KillProcesses {
                processes: names.clone(),
            }),
            &mut operations,
        );
        assert!(matches!(result, EngineToRunnerEvent::Result(result) if result.success));

        let mut profile = create_profile("Gaming".into());
        profile.processes_to_kill = names.clone();
        let result = dispatch_engine_command(
            &request(RunnerToEngineCommand::ApplyProfile { profile }),
            &mut operations,
        );
        assert!(matches!(
            result,
            EngineToRunnerEvent::Result(result)
                if result.summary.starts_with("profile=Gaming")
        ));
        assert_eq!(operations.killed_inputs, vec![names.clone(), names]);
    }

    #[test]
    fn cleanup_intent_is_delegated_to_the_injected_fake() {
        // Verifies cleanup routing returns the fake result without deleting any user data.
        let mut operations = FakeOperations::default();
        let result = dispatch_engine_command(
            &request(RunnerToEngineCommand::RunCleanup {
                cleanup_kind: CleanupKind::BrowserCache,
            }),
            &mut operations,
        );
        assert!(matches!(
            result,
            EngineToRunnerEvent::Result(result)
                if result.success && result.request_id == "request-7"
        ));
        assert_eq!(operations.cleanup_inputs, vec![CleanupKind::BrowserCache]);
    }
}
