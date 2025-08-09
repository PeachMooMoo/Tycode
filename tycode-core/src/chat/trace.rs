// Trace logging is not available in WASM builds
// This module provides stub implementations for compatibility

pub fn setup_trace_logging() -> Result<(), Box<dyn std::error::Error>> {
    // No-op in WASM or when tracing dependencies aren't available
    Ok(())
}

pub fn is_trace_setup() -> bool {
    // Always return false in WASM or when tracing isn't available
    false
}
