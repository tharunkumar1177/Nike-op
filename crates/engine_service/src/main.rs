//! EdgeOptimizer.EngineSvc - privileged process-termination worker
//!
//! Serves the installed Runner and Settings executables only. Every request
//! is authorized from the pipe peer's verified identity, and every target is
//! re-validated against its live PID immediately before termination.

// #![windows_subsystem = "windows"]

use anyhow::{Context, Result};
use edge_optimizer_core::engine_commands::{
    dispatch_engine_command, EngineOperations, VerifiedClient,
};
use edge_optimizer_core::engine_ipc::{EngineClientIdentity, EnginePipeServer};
use edge_optimizer_core::install_layout::{self, ENGINE_CLIENT_EXES};
use edge_optimizer_core::orchestration::{
    AuthContext, EngineCommand, EngineEvent, Envelope, IdempotencyCache,
};
use edge_optimizer_core::process::{ProcessTarget, TerminationOutcome};
use std::path::{Path, PathBuf};

fn main() -> Result<()> {
    tracing_subscriber::fmt().init();
    tracing::info!("EdgeOptimizer.EngineSvc starting");

    let install_directory = install_directory()?;
    let server = EnginePipeServer::new_default().context("failed to create engine pipe server")?;
    let mut idempotency = IdempotencyCache::default();

    loop {
        server
            .wait_for_client()
            .context("failed waiting for an engine client")?;

        let client = match authenticate(&server, &install_directory) {
            Ok(client) => client,
            Err(error) => {
                tracing::warn!("rejected engine client: {:#}", error);
                server.disconnect();
                continue;
            }
        };
        tracing::info!(
            "engine client connected: pid={} session={} image={}",
            client.process_id,
            client.session_id,
            client.image_path
        );
        let verified = VerifiedClient {
            process_id: client.process_id,
            session_id: client.session_id,
        };

        loop {
            let request: Option<Envelope<EngineCommand>> = match server.recv() {
                Ok(request) => request,
                Err(error) => {
                    tracing::warn!("malformed engine request: {:#}", error);
                    None
                }
            };

            let Some(request) = request else {
                server.disconnect();
                break;
            };

            let response_payload = if !idempotency.check_and_insert(&request.request_id) {
                EngineEvent::Ack {
                    message: format!("Duplicate request ignored: {}", request.request_id),
                }
            } else {
                dispatch_engine_command(&request, &verified, &mut SystemEngineOperations)
            };
            if let EngineEvent::Termination(report) = &response_payload {
                tracing::info!(
                    "termination request {} from pid {}: {}",
                    request.request_id,
                    client.process_id,
                    report.summary()
                );
            }

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

fn install_directory() -> Result<PathBuf> {
    let current = std::env::current_exe().context("failed to resolve the engine executable")?;
    current
        .parent()
        .map(Path::to_path_buf)
        .context("the engine executable has no parent directory")
}

/// Accept only the Runner or Settings image installed beside this service.
fn authenticate(
    server: &EnginePipeServer,
    install_directory: &Path,
) -> Result<EngineClientIdentity> {
    let client = server.client_identity()?;
    if !install_layout::is_sibling_image(
        Path::new(&client.image_path),
        install_directory,
        &ENGINE_CLIENT_EXES,
    ) {
        anyhow::bail!(
            "client image {} is not an installed Edge Optimizer client",
            client.image_path
        );
    }
    Ok(client)
}

struct SystemEngineOperations;

impl EngineOperations for SystemEngineOperations {
    fn terminate(&mut self, target: &ProcessTarget, client_session: u32) -> TerminationOutcome {
        #[cfg(windows)]
        {
            edge_optimizer_core::process::terminate_target(target, client_session)
        }
        #[cfg(not(windows))]
        {
            let _ = (target, client_session);
            TerminationOutcome::Failed { code: 0 }
        }
    }
}
