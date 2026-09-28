//! Pure EngineSvc command routing separated from Windows side effects.

use crate::orchestration::{EngineCommand, EngineEvent, Envelope, PROTOCOL_VERSION};
use crate::process::{
    validate_target, ProcessTarget, TargetOutcome, TerminationOutcome, TerminationReport,
    MAX_TERMINATION_TARGETS,
};

/// Identity EngineSvc derived from the pipe itself, never from the message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedClient {
    pub process_id: u32,
    pub session_id: u32,
}

pub trait EngineOperations {
    /// Re-validate and terminate one target that passed request validation.
    fn terminate(&mut self, target: &ProcessTarget, client_session: u32) -> TerminationOutcome;
}

pub fn dispatch_engine_command(
    request: &Envelope<EngineCommand>,
    client: &VerifiedClient,
    operations: &mut impl EngineOperations,
) -> EngineEvent {
    if request.protocol_version != PROTOCOL_VERSION {
        return EngineEvent::Error {
            code: "unsupported-protocol".into(),
            recoverable: false,
            message: format!(
                "protocol {} is not supported; expected {}",
                request.protocol_version, PROTOCOL_VERSION
            ),
        };
    }
    match &request.payload {
        EngineCommand::Ping => EngineEvent::Pong,
        EngineCommand::GetCapabilities => EngineEvent::Capabilities {
            supports_process_termination: true,
        },
        EngineCommand::TerminateTargets { targets } => {
            if targets.is_empty() || targets.len() > MAX_TERMINATION_TARGETS {
                return EngineEvent::Error {
                    code: "invalid-request".into(),
                    recoverable: false,
                    message: format!(
                        "a request must name between 1 and {MAX_TERMINATION_TARGETS} targets"
                    ),
                };
            }
            let mut outcomes: Vec<TargetOutcome> = Vec::with_capacity(targets.len());
            for target in targets {
                let outcome = if outcomes.iter().any(|seen| seen.target.pid == target.pid) {
                    TerminationOutcome::Invalid
                } else if target.pid == client.process_id {
                    TerminationOutcome::Protected
                } else {
                    match validate_target(target) {
                        Ok(()) => operations.terminate(target, client.session_id),
                        Err(outcome) => outcome,
                    }
                };
                outcomes.push(TargetOutcome {
                    target: target.clone(),
                    outcome,
                });
            }
            EngineEvent::Termination(TerminationReport { outcomes })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestration::{AuthContext, Envelope};

    #[derive(Default)]
    struct FakeOperations {
        terminated: Vec<(ProcessTarget, u32)>,
    }

    impl EngineOperations for FakeOperations {
        fn terminate(&mut self, target: &ProcessTarget, client_session: u32) -> TerminationOutcome {
            self.terminated.push((target.clone(), client_session));
            TerminationOutcome::Terminated
        }
    }

    const CLIENT: VerifiedClient = VerifiedClient {
        process_id: 900,
        session_id: 2,
    };

    fn request(payload: EngineCommand) -> Envelope<EngineCommand> {
        Envelope::with_request_id("test", AuthContext::Unknown, "request-7", payload)
    }

    fn target(pid: u32, name: &str) -> ProcessTarget {
        ProcessTarget {
            pid,
            creation_time: 42,
            image_name: name.into(),
        }
    }

    fn terminate(targets: Vec<ProcessTarget>, operations: &mut FakeOperations) -> EngineEvent {
        dispatch_engine_command(
            &request(EngineCommand::TerminateTargets { targets }),
            &CLIENT,
            operations,
        )
    }

    fn outcomes(event: EngineEvent) -> Vec<TerminationOutcome> {
        match event {
            EngineEvent::Termination(report) => {
                report.outcomes.into_iter().map(|o| o.outcome).collect()
            }
            other => panic!("unexpected event {other:?}"),
        }
    }

    #[test]
    fn ping_and_capabilities_are_pure_responses() {
        // Verifies discovery commands return without invoking any operating-system operation.
        let mut operations = FakeOperations::default();
        assert!(matches!(
            dispatch_engine_command(&request(EngineCommand::Ping), &CLIENT, &mut operations),
            EngineEvent::Pong
        ));
        assert!(matches!(
            dispatch_engine_command(
                &request(EngineCommand::GetCapabilities),
                &CLIENT,
                &mut operations
            ),
            EngineEvent::Capabilities {
                supports_process_termination: true,
            }
        ));
        assert!(operations.terminated.is_empty());
    }

    #[test]
    fn valid_targets_reach_the_fake_with_the_verified_session() {
        // Verifies the pipe-derived session, not any serialized claim, is passed to termination.
        let mut operations = FakeOperations::default();
        let result = terminate(vec![target(10, "game.exe")], &mut operations);
        assert_eq!(outcomes(result), [TerminationOutcome::Terminated]);
        assert_eq!(operations.terminated, [(target(10, "game.exe"), 2)]);
    }

    #[test]
    fn protected_and_malformed_targets_never_reach_operations() {
        // Verifies mixed-case, extensionless, padded, path-bearing, and reserved targets are refused first.
        let mut operations = FakeOperations::default();
        let result = terminate(
            vec![
                target(11, "WINLOGON"),
                target(12, " lsass.EXE "),
                target(13, r"C:\Windows\explorer.exe"),
                target(4, "game.exe"),
                target(CLIENT.process_id, "game.exe"),
                ProcessTarget {
                    creation_time: 0,
                    ..target(14, "game.exe")
                },
            ],
            &mut operations,
        );
        assert_eq!(
            outcomes(result),
            [
                TerminationOutcome::Protected,
                TerminationOutcome::Protected,
                TerminationOutcome::Invalid,
                TerminationOutcome::Protected,
                TerminationOutcome::Protected,
                TerminationOutcome::Invalid,
            ]
        );
        assert!(operations.terminated.is_empty());
    }

    #[test]
    fn duplicate_pids_are_executed_once() {
        // Verifies a repeated PID in one request cannot trigger a second termination attempt.
        let mut operations = FakeOperations::default();
        let result = terminate(
            vec![target(20, "game.exe"), target(20, "game.exe")],
            &mut operations,
        );
        assert_eq!(
            outcomes(result),
            [TerminationOutcome::Terminated, TerminationOutcome::Invalid]
        );
        assert_eq!(operations.terminated.len(), 1);
    }

    #[test]
    fn empty_oversized_and_wrong_version_requests_are_rejected() {
        // Verifies request bounds and protocol versioning are enforced before any target is examined.
        let mut operations = FakeOperations::default();
        assert!(matches!(
            terminate(Vec::new(), &mut operations),
            EngineEvent::Error { ref code, .. } if code == "invalid-request"
        ));
        let oversized = (0..=MAX_TERMINATION_TARGETS as u32)
            .map(|pid| target(pid + 100, "game.exe"))
            .collect();
        assert!(matches!(
            terminate(oversized, &mut operations),
            EngineEvent::Error { ref code, .. } if code == "invalid-request"
        ));
        let mut old = request(EngineCommand::TerminateTargets {
            targets: vec![target(30, "game.exe")],
        });
        old.protocol_version = PROTOCOL_VERSION - 1;
        assert!(matches!(
            dispatch_engine_command(&old, &CLIENT, &mut operations),
            EngineEvent::Error { ref code, .. } if code == "unsupported-protocol"
        ));
        assert!(operations.terminated.is_empty());
    }
}
