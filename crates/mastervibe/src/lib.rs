//! Terminal UI for mastervibe.
//!
//! The rules live in [`mastervibe_core`]. This crate draws the run and sends
//! probes, buys, sells, rerolls, and advances back through [`Run`]. It never
//! reads the secret.

mod app;
mod art;
mod format;
mod ui;

pub use app::{App, Flow, Screen};
pub use ui::{draw, render_screen};

#[cfg(test)]
mod screen_tests;
