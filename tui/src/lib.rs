//! TUI library crate for xiaozhi_linux_rs
//!
//! Provides a terminal user interface with an animated cartoon pixel face
//! that reflects the system state (idle, listening, speaking, thinking),
//! a subtitle display, and a log viewer panel.

mod app;
mod face;

pub use app::{TuiApp, TuiCommand, TuiState};
pub use face::FaceState;
