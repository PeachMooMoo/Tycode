use std::path::Path;
use std::sync::OnceLock;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

static TRACE_GUARD: OnceLock<WorkerGuard> = OnceLock::new();

pub fn setup_trace_logging() -> Result<(), Box<dyn std::error::Error>> {
    let trace_dir = Path::new(".tycode");
    if !trace_dir.exists() {
        std::fs::create_dir_all(trace_dir)?;
    }

    let file_appender = tracing_appender::rolling::never(trace_dir, "trace");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let file_layer = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(false);

    let filter = EnvFilter::new("info");

    tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .try_init()
        .map_err(|e| format!("Failed to initialize tracing: {:?}", e))?;

    TRACE_GUARD
        .set(guard)
        .map_err(|_| "Trace logger already initialized")?;

    Ok(())
}

pub fn is_trace_setup() -> bool {
    TRACE_GUARD.get().is_some()
}
