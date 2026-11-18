pub mod cli;
pub mod completions;
pub mod config;
pub mod mesh;
pub mod regen;
pub mod state;

#[cfg(target_os = "linux")]
pub mod logs_linux;
#[cfg(target_os = "linux")]
pub mod service_linux;
#[cfg(not(target_os = "linux"))]
pub mod service_other;

#[cfg(target_os = "linux")]
pub use service_linux::{install_config, maybe_restart, service_action, service_state};
#[cfg(not(target_os = "linux"))]
pub use service_other::{install_config, maybe_restart, service_action, service_state};
