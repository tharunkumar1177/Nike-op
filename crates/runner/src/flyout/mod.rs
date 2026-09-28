//! Runner-owned quick flyout, drawn with Direct2D and DirectWrite.

pub mod model;
mod render;
mod window;

pub use model::{validate_action, FlyoutAction, FlyoutModel, ProcessKey, ProcessRow};
pub use window::{cursor_position, FlyoutEvent, FlyoutWindow};
