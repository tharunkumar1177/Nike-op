//! EngineSvc calls on a worker thread, so a slow or absent service never
//! blocks Runner's message pump, tray, or flyout.

use anyhow::{Context, Result};
use edge_optimizer_core::engine_ipc::EnginePipeClient;
use edge_optimizer_core::orchestration::{
    next_request_id, AuthContext, EngineCommand, EngineEvent, EngineState, Envelope,
};
use edge_optimizer_core::process::{ProcessTarget, TerminationReport};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);

/// Why a termination was requested, echoed back with its result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminationPurpose {
    Activation { profile: String },
    Manual,
}

pub enum EngineJob {
    Ping,
    Terminate {
        purpose: TerminationPurpose,
        targets: Vec<ProcessTarget>,
    },
}

pub enum EngineReply {
    Ping(EngineState),
    Terminated {
        purpose: TerminationPurpose,
        result: Result<TerminationReport, String>,
    },
}

pub struct EngineWorker {
    jobs: Sender<EngineJob>,
    replies: Receiver<EngineReply>,
}

impl EngineWorker {
    pub fn spawn() -> Result<Self> {
        let (jobs, job_rx) = mpsc::channel::<EngineJob>();
        let (reply_tx, replies) = mpsc::channel::<EngineReply>();
        std::thread::Builder::new()
            .name("engine-client".into())
            .spawn(move || {
                for job in job_rx {
                    let reply = run(job);
                    if reply_tx.send(reply).is_err() {
                        break;
                    }
                }
            })
            .context("cannot start the engine client thread")?;
        Ok(Self { jobs, replies })
    }

    pub fn submit(&self, job: EngineJob) -> bool {
        self.jobs.send(job).is_ok()
    }

    pub fn try_reply(&self) -> Option<EngineReply> {
        self.replies.try_recv().ok()
    }
}

fn run(job: EngineJob) -> EngineReply {
    match job {
        EngineJob::Ping => EngineReply::Ping(match call(EngineCommand::Ping) {
            Ok(EngineEvent::Pong) => EngineState::Ready,
            Ok(EngineEvent::Error { .. }) => EngineState::Degraded,
            Ok(_) => EngineState::Ready,
            Err(_) => EngineState::Disconnected,
        }),
        EngineJob::Terminate { purpose, targets } => {
            let result = match call(EngineCommand::TerminateTargets { targets }) {
                Ok(EngineEvent::Termination(report)) => Ok(report),
                Ok(EngineEvent::Error { message, .. }) => Err(message),
                Ok(other) => Err(format!("unexpected engine response: {other:?}")),
                Err(error) => Err(format!("{error:#}")),
            };
            EngineReply::Terminated { purpose, result }
        }
    }
}

fn call(command: EngineCommand) -> Result<EngineEvent> {
    let client = EnginePipeClient::connect_default(CONNECT_TIMEOUT)
        .context("EdgeOptimizer_EngineSvc is not reachable")?;
    let request = Envelope::with_request_id(
        "edge-runner",
        AuthContext::RunnerService,
        next_request_id("runner"),
        command,
    );
    client
        .send(&request)
        .context("failed to send command to engine")?;
    let response: Option<Envelope<EngineEvent>> =
        client.recv().context("failed to receive engine response")?;
    response
        .map(|env| env.payload)
        .ok_or_else(|| anyhow::anyhow!("engine disconnected without response"))
}
