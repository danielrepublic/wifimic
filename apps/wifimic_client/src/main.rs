pub mod jitter;
pub mod logging;

fn main() -> Result<(), logging::LoggingError> {
    let (_diagnostics, _startup_rotation) = logging::initialize_diagnostics()?;
    Ok(())
