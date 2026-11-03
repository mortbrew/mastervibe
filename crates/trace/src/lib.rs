//! Terminal UI for ./trace.

pub mod app;
pub mod audio;
pub mod ui;

#[cfg(test)]
mod screen_tests;

pub use app::{App, Flow, Screen};
pub use ui::{capture, render_plain, Shot, ShotCell};
