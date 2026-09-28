//! EdgeOptimizer.EngineSvc - privileged optimization worker
//!
//! Performs only machine-level operations. User-specific cleanup runs in
//! Runner's interactive-user context and is not part of this protocol.

// #![windows_subsystem = "windows"]

use anyhow::{Context, Result};
use edge_optimizer_core::engine_commands::{dispatch_engine_command, EngineOperations};
use edge_optimizer_core::engine_ipc::EnginePipeServer;
use edge_optimizer_core::orchestration::{
    AuthContext, EngineToRunnerEvent, Envelope, IdempotencyCache, RunnerToEngineCommand,
};
use edge_optimizer_core::process::{kill_processes, KillReport};

fn main() -> Result<()> {
    tracing_subscriber::fmt().init();
    tracing::info!("EdgeOptimizer.EngineSvc starting");

    let server = EnginePipeServer::new_default().context("failed to create engine pipe server")?;
    let mut idempotency = IdempotencyCache::default();

    loop {
        server
            .wait_for_client()
            .context("failed waiting for runner connection")?;
        tracing::info!("Runner connected to engine pipe");

        loop {
            let request: Option<Envelope<RunnerToEngineCommand>> =
                server.recv().context("failed receiving engine request")?;

            let Some(request) = request else {
                tracing::info!("Runner disconnected from engine pipe");
                server.disconnect();
                break;
            };

            let response_payload = if !idempotency.check_and_insert(&request.request_id) {
                EngineToRunnerEvent::Ack {
                    message: format!("Duplicate request ignored: {}", request.request_id),
                }
            } else {
                dispatch_engine_command(&request, &mut SystemEngineOperations)
            };

            let response =
                request.respond("edge-engine", AuthContext::RunnerService, response_payload);
            if let Err(e) = server.send(&response) {
                tracing::warn!("failed to send engine response: {}", e);
                server.disconnect();
                break;
            }
        }
    }
}

struct SystemEngineOperations;

impl EngineOperations for SystemEngineOperations {
    fn kill_processes(&mut self, processes: &[String]) -> KillReport {
        kill_processes(processes)
    }
}
