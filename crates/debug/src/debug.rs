// Re-export tracing macros
pub use tracing::{error, info, trace, warn as warning};

/// Initialize the debug logging system
/// This should be called once at the start of the application if verbose logging is desired
pub fn init() {
    use tracing_subscriber::{EnvFilter, fmt};

    fmt()
        .with_env_filter(EnvFilter::new(
            "craft=trace,manager=trace,pipeline=trace,resolver=trace,package=trace",
        ))
        .with_target(false)
        .init();
}
