pub mod cli;
pub mod completions;
pub mod config;
pub mod regen;
pub mod state;
pub mod vless;

#[cfg(target_os = "linux")]
pub mod logs_linux;
#[cfg(target_os = "linux")]
pub mod service_linux;
#[cfg(windows)]
pub mod service_windows;
#[cfg(not(any(target_os = "linux", windows)))]
pub mod service_other;

#[cfg(target_os = "linux")]
pub use service_linux::{maybe_restart, service_action, service_state, sync_tproxy_env};
#[cfg(windows)]
pub use service_windows::{maybe_restart, service_action, service_state, sync_tproxy_env};
#[cfg(not(any(target_os = "linux", windows)))]
pub use service_other::{maybe_restart, service_action, service_state, sync_tproxy_env};
