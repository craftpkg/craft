// Re-export tracing macros
pub use tracing::{error, info, trace, warn as warning};

const WORKSPACE_CRATES: [&str; 11] = [
    "craft", "manager", "actor", "pipeline", "resolver", "package", "network", "tarball",
    "process", "lockfile", "fsutil",
];

/// Initialize the debug logging system
/// This should be called once at the start of the application if debug logging is desired
pub fn init() {
    use tracing_subscriber::{EnvFilter, fmt};

    let directives = WORKSPACE_CRATES
        .iter()
        .map(|crate_name| format!("{crate_name}=trace"))
        .collect::<Vec<_>>()
        .join(",");

    fmt()
        .with_env_filter(EnvFilter::new(directives))
        .with_target(false)
        .init();
}
