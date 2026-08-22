pub mod capture;
pub mod control;
mod diagnostic_capture;
mod network;

use std::io;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use wifimic_diagnostics::EventContext;
use wifimic_protocol::{
    decode_calibration, decode_control, encode_audio_frame, encode_calibration, CalibrationPacket,
    ControlMessage, CALIBRATION_PROBE_TAG,
};

use crate::capture::CaptureHandle;
use crate::control::{CaptureController, ControlError, ControlPlane};
use crate::diagnostic_capture::LatencyDiagnosticCapture;

const RECEIVE_POLL_INTERVAL: Duration = Duration::from_millis(1);

fn main() -> std::io::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .try_init()
        .map_err(io::Error::other)?;
    if std::env::args().any(|argument| argument == "--calibrate") {
        eprintln!("calibration responder enabled");
    }
    if std::env::args().any(|argument| argument == "--diagnose-latency") {
        eprintln!("latency diagnostic capture enabled on the pinned parec source");
        return run_server(LatencyDiagnosticCapture::new());
    }
    run_server(CaptureHandle::new())
}

fn run_server<C>(capture: C) -> io::Result<()>
where
    C: CaptureController,
{
    let mut socket = network::UdpServerSocket::bind()?;
    socket.set_read_timeout(Some(RECEIVE_POLL_INTERVAL))?;
    let diagnostics = EventContext::logging(Instant::now());
    let mut control = ControlPlane::new(capture, diagnostics);
    let mut peer: Option<SocketAddr> = None;
    let mut sequence = 0_u32;

    loop {
        match socket.receive_once() {
            Ok(Some(datagram)) => {
                if datagram.payload.first() == Some(&CALIBRATION_PROBE_TAG) {
                    if let Ok(CalibrationPacket::Probe {
                        sequence,
                        t1_client_send_us,
                    }) = decode_calibration(&datagram.payload)
                    {
                        let t2_server_receive_us = unix_micros();
                        let t3_server_send_us = unix_micros();
                        let reply = encode_calibration(CalibrationPacket::Reply {
                            sequence,
                            t1_client_send_us,
                            t2_server_receive_us,
                            t3_server_send_us,
                        });
                        socket.send_to(&reply, datagram.source)?;
                    }
                    continue;
                }
                let message = decode_control(&datagram.payload).ok();
                match control.handle_datagram(&datagram.payload, Instant::now()) {
                    Ok(Some(ack)) => {
                        socket.send_to(&ack, datagram.source)?;
                        if matches!(message, Some(ControlMessage::Start { .. })) {
                            peer = Some(datagram.source);
                            sequence = 0;
                        }
                        if matches!(message, Some(ControlMessage::Stop { .. })) {
                            peer = None;
                        }
                    }
                    Ok(None) => {}
                    Err(ControlError::Protocol(_) | ControlError::UnexpectedAck { .. }) => {}
                    Err(error) => return Err(io::Error::other(error)),
                }
            }
            Ok(None) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                ) => {}
            Err(error) => return Err(error),
        }
        control.advance(Instant::now()).map_err(io::Error::other)?;
        if let Some(destination) = peer {
            let now = Instant::now();
            if let Some(frame) = control
                .next_audio_frame(sequence, now)
                .map_err(io::Error::other)?
            {
                socket.send_to(&encode_audio_frame(&frame), destination)?;
                sequence = sequence.wrapping_add(1);
            }
        }
    }
}

fn unix_micros() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
        })
}
