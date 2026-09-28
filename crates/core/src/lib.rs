//! Edge Optimizer Core Library
//!
//! Shared functionality for all Edge Optimizer processes
//!
//! Architecture:
//! - Runner process owns the system tray (uses tray_icon module)
//! - The WinUI Settings client talks to Runner over the ipc pipe
//! - Workers are resolved beside Runner through install_layout

pub mod common_apps;
pub mod config;
pub mod crosshair_overlay;
pub mod engine_commands;
pub mod engine_ipc;
pub mod flyout;
pub mod image_picker;
pub mod input_recorder;
pub mod install_layout;
pub mod ipc;
pub mod macro_config;
pub mod macro_worker;
pub mod orchestration;
pub mod pipe_security;
pub mod process;
pub mod profile;
pub mod state_store;
pub mod tray_icon; // New minimal tray manager for Runner
pub mod user_cleanup;

pub use crate::input_recorder::InputRecorder;
