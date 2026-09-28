//! EdgeOptimizer.Runner - System Tray Orchestrator
//!
//! Responsibilities:
//! - Own the tray icon, context menu, and Direct2D quick flyout
//! - Start and stop the crosshair and macro workers for the active profile
//! - Run user-context cleanup and ask EngineSvc to terminate flyout targets
//! - Relay engine state and results to Settings, which stays optional/on-demand

#![windows_subsystem = "windows"]

mod engine_client;
mod flyout;
mod tracker;

use anyhow::{Context, Result};
use edge_optimizer_core::{
    config,
    crosshair_overlay::{self, OverlayHandle},
    install_layout::{self, SETTINGS_EXE},
    ipc::{GuiToTray, NamedPipeServer, TrayToGui},
    macro_worker::MacroWorkerHandle,
    orchestration::{
        AuthContext, CleanupKind, EngineState, Envelope, IdempotencyCache, OperationResult,
        RunnerToSettingsEvent, SettingsToRunnerCommand,
    },
    pipe_security,
    process::{enumerate_processes, resolve_targets, ProcessTarget},
    profile::Profile,
    state_store::StateStore,
    tray_icon::TrayIconManager,
    user_cleanup,
};
use engine_client::{EngineJob, EngineReply, EngineWorker, TerminationPurpose};
use flyout::{FlyoutAction, FlyoutEvent, FlyoutModel, FlyoutWindow};
use std::process::Command;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};
use tracker::ProcessTracker;
use tray_icon::menu::MenuEvent;
use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent};
use windows::core::PCWSTR;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::*;

const DOUBLE_CLICK_WINDOW: Duration = Duration::from_millis(500);
/// A tray click that dismissed the flyout by taking activation must not
/// reopen it once the double-click window elapses.
const REOPEN_GUARD: Duration = Duration::from_millis(600);
const ENGINE_PING_INTERVAL: Duration = Duration::from_secs(5);

struct SingleInstanceGuard {
    #[cfg(windows)]
    mutex: windows::Win32::Foundation::HANDLE,
}

impl SingleInstanceGuard {
    fn acquire() -> Result<Self> {
        #[cfg(windows)]
        {
            use windows::Win32::Foundation::{WAIT_ABANDONED, WAIT_OBJECT_0, WAIT_TIMEOUT};
            use windows::Win32::System::Threading::{CreateMutexW, WaitForSingleObject};

            let mutex_name: Vec<u16> = "EdgeOptimizer.Runner.SingleInstance\0"
                .encode_utf16()
                .collect();

            let mutex =
                unsafe { CreateMutexW(None, false, windows::core::PCWSTR(mutex_name.as_ptr())) }
                    .context("failed to create runner single-instance mutex")?;

            let wait_result = unsafe { WaitForSingleObject(mutex, 0) };
            if wait_result == WAIT_TIMEOUT {
                unsafe {
                    let _ = windows::Win32::Foundation::CloseHandle(mutex);
                }
                anyhow::bail!("EdgeOptimizer.Runner is already running");
            }
            if wait_result != WAIT_OBJECT_0 && wait_result != WAIT_ABANDONED {
                unsafe {
                    let _ = windows::Win32::Foundation::CloseHandle(mutex);
                }
                anyhow::bail!("failed to acquire single-instance mutex");
            }

            Ok(Self { mutex })
        }

        #[cfg(not(windows))]
        {
            Ok(Self {})
        }
    }
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        #[cfg(windows)]
        unsafe {
            let _ = windows::Win32::System::Threading::ReleaseMutex(self.mutex);
            let _ = windows::Win32::Foundation::CloseHandle(self.mutex);
        }
    }
}

enum UiEvent {
    Tray(TrayIconEvent),
    Menu(MenuEvent),
}

struct PendingClick {
    at: Instant,
    anchor: (i32, i32),
    flyout_was_open: bool,
}

/// Unprivileged workers Runner starts for the active profile.
#[derive(Default)]
struct Workers {
    overlay: Option<OverlayHandle>,
    macros: Option<MacroWorkerHandle>,
}

impl Workers {
    fn stop(&mut self) {
        if let Some(handle) = self.overlay.take() {
            handle.stop();
        }
        if let Some(handle) = self.macros.take() {
            handle.stop();
        }
    }
}

struct Runner {
    state_store: StateStore,
    tray: TrayIconManager,
    pipe_server: NamedPipeServer,
    settings_connected: bool,
    engine_state: EngineState,
    idempotency: IdempotencyCache,
    workers: Workers,
    engine: EngineWorker,
    ping_in_flight: bool,
    last_ping: Option<Instant>,
    flyout: Option<FlyoutWindow>,
    tracker: ProcessTracker,
    session_id: u32,
    /// In-memory projection of `state.db` that the flyout reads.
    profiles: Vec<Profile>,
    active_profile: Option<String>,
    flyout_status: Option<String>,
    terminations_in_flight: u32,
    pending_click: Option<PendingClick>,
    last_click: Option<Instant>,
    should_exit: bool,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    tracing::info!("EdgeOptimizer.Runner starting...");

    let _instance_guard = match SingleInstanceGuard::acquire() {
        Ok(guard) => guard,
        Err(e) => {
            tracing::warn!("{}", e);
            return Ok(());
        }
    };

    let mut state_store =
        StateStore::open_default().context("failed to open Runner state store")?;
    let legacy_config = config::load_config();
    let legacy_profiles = config::get_data_directory()
        .ok()
        .and_then(|directory| edge_optimizer_core::profile::load_profiles(&directory).ok())
        .unwrap_or_default();
    if state_store.import_legacy_if_empty(
        &legacy_profiles,
        legacy_config.active_profile.as_deref(),
        legacy_config.overlay_visible,
    )? {
        tracing::info!(
            "migrated legacy JSON state into {}",
            state_store.path().display()
        );
    }
    let startup_state = state_store
        .load_snapshot()
        .context("failed to load startup state")?;
    let tray = TrayIconManager::new(startup_state.active_profile.clone())
        .context("failed to create tray icon manager")?;
    let pipe_server = NamedPipeServer::new().context("failed to create named pipe server")?;
    let session_id =
        pipe_security::current_session_id().context("failed to read the Windows session")?;

    let (ui_tx, ui_rx) = mpsc::channel::<UiEvent>();
    let tray_tx = ui_tx.clone();
    let menu_tx = ui_tx;
    TrayIconEvent::set_event_handler(Some(move |event| {
        let _ = tray_tx.send(UiEvent::Tray(event));
    }));
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = menu_tx.send(UiEvent::Menu(event));
    }));

    let flyout = match FlyoutWindow::new() {
        Ok(flyout) => Some(flyout),
        Err(error) => {
            tracing::warn!(
                "quick flyout unavailable; tray clicks open Settings instead: {:#}",
                error
            );
            None
        }
    };

    let mut runner = Runner {
        state_store,
        tray,
        pipe_server,
        settings_connected: false,
        engine_state: EngineState::Starting,
        idempotency: IdempotencyCache::default(),
        workers: Workers::default(),
        engine: EngineWorker::spawn()?,
        ping_in_flight: false,
        last_ping: None,
        flyout,
        tracker: ProcessTracker::new(session_id),
        session_id,
        profiles: startup_state.profiles,
        active_profile: startup_state.active_profile,
        flyout_status: None,
        terminations_in_flight: 0,
        pending_click: None,
        last_click: None,
        should_exit: false,
    };
    runner.run(&ui_rx);
    runner.shutdown();

    tracing::info!("EdgeOptimizer.Runner shutdown complete");
    Ok(())
}

impl Runner {
    fn run(&mut self, ui_rx: &Receiver<UiEvent>) {
        while !self.should_exit {
            if !pump_messages() {
                break;
            }
            while let Ok(event) = ui_rx.try_recv() {
                match event {
                    UiEvent::Tray(event) => self.handle_tray_event(event),
                    UiEvent::Menu(event) => self.handle_menu_event(event),
                }
            }
            self.handle_flyout_events();
            self.handle_engine_replies();
            self.fire_pending_click();
            self.poll_settings();
            self.schedule_ping();

            let flyout_has_events = self.flyout.as_ref().is_some_and(FlyoutWindow::has_events);
            if !self.should_exit && !flyout_has_events {
                let timeout = if self.pending_click.is_some() {
                    20
                } else {
                    200
                };
                unsafe {
                    let _ = MsgWaitForMultipleObjects(None, false, timeout, QS_ALLINPUT);
                }
            }
        }
    }

    fn shutdown(&mut self) {
        self.hide_flyout();
        self.tracker.clear();
        self.workers.stop();
        self.flyout = None;
    }

    fn handle_tray_event(&mut self, event: TrayIconEvent) {
        let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = event
        else {
            return;
        };
        let now = Instant::now();
        let is_double_click = self
            .last_click
            .is_some_and(|last| now.duration_since(last) < DOUBLE_CLICK_WINDOW);
        if is_double_click {
            self.pending_click = None;
            self.last_click = None;
            self.hide_flyout();
            self.open_settings();
            return;
        }
        self.last_click = Some(now);
        let flyout_was_open = self
            .flyout
            .as_ref()
            .is_some_and(|flyout| flyout.is_visible() || flyout.hidden_within(REOPEN_GUARD));
        self.pending_click = Some(PendingClick {
            at: now,
            anchor: flyout::cursor_position(),
            flyout_was_open,
        });
    }

    fn fire_pending_click(&mut self) {
        let Some(click) = self
            .pending_click
            .take_if(|click| click.at.elapsed() >= DOUBLE_CLICK_WINDOW)
        else {
            return;
        };
        if click.flyout_was_open {
            self.hide_flyout();
        } else {
            self.show_flyout(click.anchor);
        }
    }

    fn handle_menu_event(&mut self, event: MenuEvent) {
        if event.id == self.tray.menu_item_settings {
            self.open_settings();
        } else if event.id == self.tray.menu_item_docs {
            let _ = open::that("https://github.com/yourusername/EdgeOptimizer#readme");
        } else if event.id == self.tray.menu_item_bug_report {
            let _ = open::that("https://github.com/yourusername/EdgeOptimizer/issues/new");
        } else if event.id == self.tray.menu_item_exit {
            self.request_exit();
        }
    }

    fn request_exit(&mut self) {
        self.send_settings(&TrayToGui::Exit);
        self.should_exit = true;
    }

    // --- Quick flyout -----------------------------------------------------

    fn flyout_visible(&self) -> bool {
        self.flyout.as_ref().is_some_and(FlyoutWindow::is_visible)
    }

    fn hide_flyout(&self) {
        if let Some(flyout) = &self.flyout {
            flyout.hide();
        }
    }

    fn show_flyout(&mut self, anchor: (i32, i32)) {
        if self.flyout.is_none() {
            self.open_settings();
            return;
        }
        self.reload_profiles();
        self.sync_tracker_names();
        self.tracker.rescan();
        let model = self.flyout_model();
        if let Some(flyout) = &self.flyout {
            flyout.show_at(anchor, model);
        }
    }

    fn refresh_flyout(&self) {
        if let Some(flyout) = &self.flyout {
            if flyout.is_visible() {
                flyout.set_model(self.flyout_model());
            }
        }
    }

    fn flyout_model(&self) -> FlyoutModel {
        FlyoutModel {
            profiles: self.profiles.iter().map(|p| p.name.clone()).collect(),
            active_profile: self.active_profile.clone(),
            processes: self.tracker.rows(),
            termination_available: self.engine_state == EngineState::Ready,
            busy: self.terminations_in_flight > 0,
            status: self.flyout_status.clone(),
        }
    }

    fn handle_flyout_events(&mut self) {
        let events = match &self.flyout {
            Some(flyout) => flyout.take_events(),
            None => return,
        };
        for event in events {
            match event {
                FlyoutEvent::Tick => {
                    if self.tracker.tick() {
                        self.refresh_flyout();
                    }
                }
                FlyoutEvent::Hidden => self.tracker.clear(),
                FlyoutEvent::Action(action) => self.handle_flyout_action(action),
            }
        }
    }

    fn handle_flyout_action(&mut self, action: FlyoutAction) {
        let Some(action) = flyout::validate_action(&self.flyout_model(), &action) else {
            self.flyout_status = Some("That item changed. Showing the current state.".into());
            self.tracker.rescan();
            self.refresh_flyout();
            return;
        };
        match action {
            FlyoutAction::Activate(name) => {
                self.hide_flyout();
                self.activate_from_flyout(&name);
            }
            FlyoutAction::Deactivate => {
                self.hide_flyout();
                self.deactivate();
            }
            FlyoutAction::Terminate(key) => {
                let targets = self.tracker.targets(Some(key));
                self.request_termination(targets, TerminationPurpose::Manual);
            }
            FlyoutAction::TerminateAll => {
                let targets = self.tracker.targets(None);
                self.request_termination(targets, TerminationPurpose::Manual);
            }
            FlyoutAction::OpenSettings => {
                self.hide_flyout();
                self.open_settings();
            }
            FlyoutAction::ExitRunner => {
                self.hide_flyout();
                self.request_exit();
            }
        }
    }

    fn active_profile_config(&self) -> Option<&Profile> {
        let name = self.active_profile.as_deref()?;
        self.profiles
            .iter()
            .find(|profile| profile.name.eq_ignore_ascii_case(name))
    }

    fn sync_tracker_names(&mut self) {
        let names = self
            .active_profile_config()
            .map(|profile| profile.processes_to_kill.clone())
            .unwrap_or_default();
        self.tracker.set_names(&names);
    }

    fn reload_profiles(&mut self) {
        match self.state_store.load_snapshot() {
            Ok(snapshot) => {
                self.profiles = snapshot.profiles;
                self.active_profile = snapshot.active_profile;
            }
            Err(error) => {
                tracing::warn!("cannot reload profiles for the flyout: {:#}", error);
                self.flyout_status = Some(format!("Profiles could not be read: {error:#}"));
            }
        }
    }

    fn activate_from_flyout(&mut self, name: &str) {
        let Some(profile) = self
            .profiles
            .iter()
            .find(|profile| profile.name.eq_ignore_ascii_case(name))
            .cloned()
        else {
            return;
        };
        if let Err(error) = self.state_store.save_active_profile(&profile) {
            tracing::warn!("cannot persist activation of {}: {:#}", profile.name, error);
            self.flyout_status = Some(format!("{} was not activated: {error:#}", profile.name));
            return;
        }
        self.active_profile = Some(profile.name.clone());
        self.tray.set_active_profile(Some(profile.name.clone()));
        self.send_settings(&TrayToGui::ActivateProfile(profile.name.clone()));

        let result = self.start_profile_workers(&profile, "runner-flyout".into());
        self.flyout_status = Some(format!("{}: {}", profile.name, result.summary));
        self.sync_tracker_names();

        let targets = match enumerate_processes() {
            Ok(processes) => resolve_targets(
                &processes,
                &profile.processes_to_kill,
                self.session_id,
                std::process::id(),
            ),
            Err(error) => {
                tracing::warn!("cannot enumerate processes for activation: {:#}", error);
                Vec::new()
            }
        };
        if !targets.is_empty() {
            self.request_termination(
                targets,
                TerminationPurpose::Activation {
                    profile: profile.name.clone(),
                },
            );
        }
    }

    fn deactivate(&mut self) {
        self.workers.stop();
        match self.state_store.set_active_profile(None) {
            Ok(()) => {
                self.active_profile = None;
                self.tray.set_active_profile(None);
                self.send_settings(&TrayToGui::DeactivateProfile);
                self.flyout_status = Some("Profile deactivated.".into());
            }
            Err(error) => {
                tracing::warn!("cannot persist deactivation: {:#}", error);
                self.flyout_status = Some(format!("Could not deactivate: {error:#}"));
            }
        }
        self.sync_tracker_names();
    }

    fn request_termination(&mut self, targets: Vec<ProcessTarget>, purpose: TerminationPurpose) {
        if targets.is_empty() {
            self.flyout_status = Some("None of those apps are running now.".into());
        } else if self
            .engine
            .submit(EngineJob::Terminate { purpose, targets })
        {
            self.terminations_in_flight += 1;
            self.flyout_status = Some("Asking the engine service to close apps…".into());
        } else {
            self.flyout_status =
                Some("The engine client stopped. Restart Edge Optimizer to close apps.".into());
        }
        self.refresh_flyout();
    }

    // --- EngineSvc ------------------------------------------------------------

    fn schedule_ping(&mut self) {
        let due = self
            .last_ping
            .is_none_or(|last| last.elapsed() >= ENGINE_PING_INTERVAL);
        if !self.ping_in_flight && due && self.engine.submit(EngineJob::Ping) {
            self.ping_in_flight = true;
            self.last_ping = Some(Instant::now());
        }
    }

    fn handle_engine_replies(&mut self) {
        while let Some(reply) = self.engine.try_reply() {
            match reply {
                EngineReply::Ping(state) => {
                    self.ping_in_flight = false;
                    self.set_engine_state(state);
                    self.refresh_flyout();
                }
                EngineReply::Terminated { purpose, result } => {
                    self.terminations_in_flight = self.terminations_in_flight.saturating_sub(1);
                    let prefix = match &purpose {
                        TerminationPurpose::Activation { profile } => format!("{profile}: "),
                        TerminationPurpose::Manual => String::new(),
                    };
                    let message = match &result {
                        Ok(report) => {
                            tracing::info!("termination result: {}", report.summary());
                            format!("{prefix}{}", report.summary())
                        }
                        Err(error) => {
                            tracing::warn!("termination failed: {}", error);
                            format!("{prefix}apps were not closed. {error}")
                        }
                    };
                    self.flyout_status = Some(message.clone());
                    self.send_runner_event(RunnerToSettingsEvent::Ack { message });
                    if self.flyout_visible() {
                        self.tracker.rescan();
                    }
                    self.refresh_flyout();
                }
            }
        }
    }

    fn set_engine_state(&mut self, next: EngineState) {
        if self.engine_state == next {
            return;
        }
        self.engine_state = next.clone();
        tracing::info!("Engine state -> {:?}", next);
        self.send_runner_event(RunnerToSettingsEvent::EngineState(next));
    }

    // --- Settings ---------------------------------------------------------------

    fn send_settings(&mut self, message: &TrayToGui) {
        if !self.settings_connected {
            return;
        }
        if let Err(error) = self.pipe_server.send(message) {
            tracing::warn!("failed to send to Settings: {}", error);
            self.settings_connected = false;
        }
    }

    fn send_runner_event(&mut self, event: RunnerToSettingsEvent) {
        let envelope = Envelope::new("edge-runner", AuthContext::RunnerService, event);
        self.send_settings(&TrayToGui::OrchestrationEvent(envelope));
    }

    fn open_settings(&mut self) {
        if self.settings_connected {
            if self.pipe_server.send(&TrayToGui::BringMainToFront).is_ok() {
                return;
            }
            self.settings_connected = false;
        }
        if let Err(error) = spawn_settings_window() {
            tracing::warn!("cannot open Settings: {:#}", error);
        }
    }

    fn poll_settings(&mut self) {
        if !self.settings_connected {
            match self.pipe_server.try_accept() {
                Ok(true) => {
                    self.settings_connected = true;
                    self.send_runner_event(RunnerToSettingsEvent::EngineState(
                        self.engine_state.clone(),
                    ));
                }
                Ok(false) => {}
                Err(error) => tracing::warn!("failed to accept Settings IPC: {}", error),
            }
        }

        match self.pipe_server.try_recv() {
            Ok(Some(message)) => {
                self.settings_connected = true;
                if let Err(error) = self.handle_settings_message(message) {
                    tracing::warn!("Settings request failed: {:#}", error);
                    self.send_runner_event(RunnerToSettingsEvent::UserActionRequired {
                        reason: format!("Runner could not complete the request: {error:#}"),
                    });
                }
            }
            Ok(None) => {}
            Err(error) => {
                tracing::warn!("error reading Settings IPC: {}", error);
                self.settings_connected = false;
            }
        }
    }

    fn handle_settings_message(&mut self, message: GuiToTray) -> Result<()> {
        match message {
            GuiToTray::RequestState => {
                let snapshot = self.state_store.load_snapshot()?;
                self.profiles = snapshot.profiles.clone();
                self.active_profile = snapshot.active_profile.clone();
                self.send_settings(&TrayToGui::StateSnapshot {
                    profiles: snapshot.profiles,
                    active_profile: snapshot.active_profile,
                    overlay_visible: snapshot.overlay_visible,
                });
            }
            GuiToTray::ProfilesUpdated(profiles) => {
                self.state_store.save_profiles(&profiles)?;
                self.reload_profiles();
                self.sync_tracker_names();
                self.refresh_flyout();
            }
            GuiToTray::ActiveProfileChanged(active) => {
                self.state_store.set_active_profile(active.as_deref())?;
                self.tray.set_active_profile(active.clone());
                self.active_profile = active;
                self.sync_tracker_names();
                self.refresh_flyout();
            }
            GuiToTray::OverlayVisibilityChanged(visible) => {
                self.state_store.set_overlay_visible(visible)?;
            }
            GuiToTray::Shutdown => self.should_exit = true,
            GuiToTray::Orchestration(envelope) => {
                if !self.idempotency.check_and_insert(&envelope.request_id) {
                    self.send_runner_event(RunnerToSettingsEvent::Ack {
                        message: format!("Duplicate request ignored: {}", envelope.request_id),
                    });
                    return Ok(());
                }
                self.process_orchestration_command(envelope)?;
            }
        }
        Ok(())
    }

    /// Settings terminates its own selections through EngineSvc before it
    /// sends activation, so this path only starts workers and persists state.
    fn process_orchestration_command(
        &mut self,
        envelope: Envelope<SettingsToRunnerCommand>,
    ) -> Result<()> {
        match envelope.payload {
            SettingsToRunnerCommand::ActivateProfile { profile, .. }
            | SettingsToRunnerCommand::RequestOptimization { profile, .. } => {
                let result = self.start_profile_workers(&profile, envelope.request_id);
                if result.success {
                    self.state_store.save_active_profile(&profile)?;
                    self.tray.set_active_profile(Some(profile.name.clone()));
                    self.reload_profiles();
                    self.sync_tracker_names();
                    self.refresh_flyout();
                }
                self.send_runner_event(RunnerToSettingsEvent::OptimizationResult(result));
            }
            SettingsToRunnerCommand::RequestCleanup { cleanup_kind } => {
                let event = request_cleanup(envelope.request_id, cleanup_kind);
                self.send_runner_event(event);
            }
            SettingsToRunnerCommand::PreviewImpact { profile } => {
                let message = format!(
                    "Preview: {} process(es) selected for optimization",
                    profile.processes_to_kill.len()
                );
                self.send_runner_event(RunnerToSettingsEvent::Ack { message });
            }
            SettingsToRunnerCommand::OpenFlyout => {
                self.show_flyout(flyout::cursor_position());
            }
            SettingsToRunnerCommand::OpenSettings => {
                self.send_settings(&TrayToGui::BringMainToFront);
            }
        }
        Ok(())
    }

    fn start_profile_workers(&mut self, profile: &Profile, request_id: String) -> OperationResult {
        let mut success = true;
        let mut parts: Vec<String> = Vec::new();

        if let Some(handle) = self.workers.overlay.take() {
            handle.stop();
        }
        if profile.overlay_enabled {
            match &profile.crosshair_image_path {
                Some(path) => match crosshair_overlay::start_overlay(
                    path.clone(),
                    profile.crosshair_x_offset,
                    profile.crosshair_y_offset,
                ) {
                    Ok(handle) => {
                        self.workers.overlay = Some(handle);
                        parts.push("crosshair=on".into());
                    }
                    Err(error) => {
                        success = false;
                        parts.push(format!("crosshair_error={error}"));
                    }
                },
                None => parts.push("crosshair=skipped(no-image)".into()),
            }
        } else {
            parts.push("crosshair=off".into());
        }

        if let Some(handle) = self.workers.macros.take() {
            handle.stop();
        }
        if profile
            .macros
            .macros
            .iter()
            .any(|macro_def| macro_def.enabled)
        {
            match MacroWorkerHandle::start(profile.macros.clone()) {
                Ok(handle) => {
                    self.workers.macros = Some(handle);
                    parts.push("macros=on".into());
                }
                Err(error) => {
                    success = false;
                    parts.push(format!("macro_error={error}"));
                }
            }
        } else {
            parts.push("macros=off".into());
        }

        OperationResult {
            request_id,
            success,
            summary: format!("profile={} {}", profile.name, parts.join(" | ")),
            ..OperationResult::default()
        }
    }
}

fn pump_messages() -> bool {
    unsafe {
        let mut msg = MSG::default();
        while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
            if msg.message == WM_QUIT {
                return false;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    true
}

/// User-specific cleanup runs here, in the signed-in user's context.
fn request_cleanup(request_id: String, cleanup_kind: CleanupKind) -> RunnerToSettingsEvent {
    RunnerToSettingsEvent::CleanupResult(user_cleanup::run_cleanup(request_id, cleanup_kind))
}

fn spawn_settings_window() -> Result<()> {
    if bring_existing_settings_to_front() {
        return Ok(());
    }
    let target = install_layout::sibling_executable(SETTINGS_EXE)?;
    Command::new(&target)
        .spawn()
        .context("failed to spawn Settings process")?;
    Ok(())
}

fn bring_existing_settings_to_front() -> bool {
    unsafe {
        let settings_title: Vec<u16> = "Edge Optimizer\0".encode_utf16().collect();
        let hwnd = FindWindowW(None, PCWSTR(settings_title.as_ptr()));

        if hwnd != HWND::default() {
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            let _ = SetForegroundWindow(hwnd);
            let _ = BringWindowToTop(hwnd);
            return true;
        }
        false
    }
}
