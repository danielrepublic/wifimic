pub mod control;
pub mod jitter;
pub mod logging;
pub mod render;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (_diagnostics, _startup_rotation) = logging::initialize_diagnostics()?;
    #[cfg(target_os = "windows")]
    run_windows_client()?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn run_windows_client() -> Result<(), Box<dyn std::error::Error>> {
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    use control::{ControlPlane, InboundOutcome, UdpClientSocket};
    use render::{RenderConfig, Renderer};

    let origin = Instant::now();
    let socket = UdpClientSocket::bind()?;
    socket.set_read_timeout(Some(Duration::from_millis(1)))?;
    let renderer = Renderer::open(RenderConfig::vb_cable())?;
    let mut control = ControlPlane::new(socket, renderer, origin);
    let epoch_ms = || {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
    };
    control.start(origin, epoch_ms()?)?;

    loop {
        let now = Instant::now();
        match control.receive_once(now) {
            Ok(Some(InboundOutcome::DroppedUnapprovedSource))
            | Ok(Some(InboundOutcome::IgnoredAck { .. }))
            | Ok(Some(InboundOutcome::IgnoredAudio { .. }))
            | Ok(Some(InboundOutcome::IgnoredControl))
            | Ok(Some(InboundOutcome::StartAck { .. }))
            | Ok(Some(InboundOutcome::HeartbeatAck { .. }))
            | Ok(Some(InboundOutcome::AudioQueued { .. }))
            | Ok(None) => {}
            Err(control::ControlError::Transport(error))
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                ) => {}
            Err(control::ControlError::Protocol(_)) => {}
            Err(error) => return Err(error.into()),
        }
        control.advance(now, epoch_ms()?)?;
        let _ = control.render_ready(now)?;
    }
}
