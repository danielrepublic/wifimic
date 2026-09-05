pub mod capture;
mod cli;
pub mod control;
mod diagnostic_capture;
mod doctor;
mod network;
mod status;
mod upgrade;
mod upgrade_native;
#[cfg(test)]
#[path = "upgrade_test_support.rs"]
mod upgrade_test_support;

use std::io;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use wifimic_diagnostics::EventContext;
use wifimic_protocol::{
    decode_calibration, decode_control, encode_audio_frame, encode_calibration, CalibrationPacket,
    ControlMessage, CALIBRATION_PROBE_TAG,
};

use crate::capture::CaptureHandle;
use crate::cli::{parse_command, CliParseError, Command};
use crate::control::{CaptureController, ControlError, ControlPlane};
use crate::diagnostic_capture::LatencyDiagnosticCapture;
use crate::doctor::{run_doctor, NativeCaptureSourceQueries, NativeFirewallQueries};
use crate::status::{run_status, NativeServiceQueries};
use crate::upgrade::{LinuxUpdateAdapter, HEALTH_TIMEOUT};
use wifimic_update::check::{check_update_exit_code, render_check_update, run_check_update};

const WIFIMIC_SERVER_VERSION: &str = env!("WIFIMIC_SERVER_VERSION");
const RECEIVE_POLL_INTERVAL: Duration = Duration::from_millis(1);
/// Interval between server socket bind retries at startup.
const BIND_RETRY_INTERVAL: Duration = Duration::from_secs(2);
/// Maximum bind attempts before giving up to systemd (≈60s: covers boot
/// races and short Wi-Fi flaps without masking real config errors forever).
const BIND_RETRY_ATTEMPTS: u32 = 30;

fn main() -> std::process::ExitCode {
    match run_main() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(MainError::CheckUpdate(error)) => {
            let result = Err(error);
            println!("{}", render_check_update(&result, "wifimic_server"));
            if check_update_exit_code(&result) == 0 {
                std::process::ExitCode::SUCCESS
            } else {
                std::process::ExitCode::FAILURE
            }
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[derive(Debug, thiserror::Error)]
enum MainError {
    #[error("could not initialize logging: {0}")]
    Logging(#[source] io::Error),
    #[error("{0}")]
    Cli(#[from] CliParseError),
    #[error("update check failed: {0}")]
    CheckUpdate(#[from] wifimic_update::UpdateError),
    #[error("upgrade failed: {0}")]
    Upgrade(#[from] wifimic_update::TransactionError),
    #[error("status failed: {0}")]
    Status(#[from] crate::status::StatusError),
    #[error("doctor failed: {0}")]
    Doctor(#[source] io::Error),
    #[error("service failed: {0}")]
    Service(#[source] io::Error),
}

fn run_main() -> Result<(), MainError> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .try_init()
        .map_err(io::Error::other)
        .map_err(MainError::Logging)?;
    match parse_command(std::env::args())? {
        Command::Service {
            calibrate,
            diagnose_latency,
        } => run_service(calibrate, diagnose_latency).map_err(MainError::Service),
        Command::Version => {
            println!("{WIFIMIC_SERVER_VERSION}");
            Ok(())
        }
        Command::Update => {
            let result =
                run_check_update(WIFIMIC_SERVER_VERSION, wifimic_update::discover_latest_tag)?;
            println!("{}", result.render("wifimic_server"));
            Ok(())
        }
        Command::Upgrade { target } => {
            let result = wifimic_update::run_update_transaction(
                &mut LinuxUpdateAdapter,
                target,
                WIFIMIC_SERVER_VERSION,
                HEALTH_TIMEOUT,
            )?;
            match result {
                wifimic_update::TransactionOutcome::NoOp { .. } => {
                    println!("已是最新版本");
                    Ok(())
                }
                wifimic_update::TransactionOutcome::Installed { tag } => {
                    println!("已更新至 {tag}");
                    Ok(())
                }
                wifimic_update::TransactionOutcome::RolledBack { cause } => {
                    println!("更新失敗：{cause}；已還原至先前版本");
                    Err(MainError::Upgrade(*cause))
                }
                wifimic_update::TransactionOutcome::RollbackVerificationFailed { cause } => {
                    println!("更新失敗：{cause}；且無法確認還原狀態");
                    Err(MainError::Upgrade(*cause))
                }
            }
        }
        Command::Status => {
            let report = run_status(&NativeServiceQueries, WIFIMIC_SERVER_VERSION)?;
            println!("{}", report.render());
            Ok(())
        }
        Command::Doctor => {
            let report = run_doctor(
                &NativeServiceQueries,
                &NativeCaptureSourceQueries,
                &NativeFirewallQueries,
                WIFIMIC_SERVER_VERSION,
            );
            print!("{}", report.render());
            if report.all_passed() {
                Ok(())
            } else {
                Err(MainError::Doctor(io::Error::other(
                    "one or more checks failed",
                )))
            }
        }
    }
}

fn run_service(calibrate: bool, diagnose_latency: bool) -> io::Result<()> {
    if calibrate {
        eprintln!("calibration responder enabled");
    }
    if diagnose_latency {
        eprintln!("latency diagnostic capture enabled on the pinned parec source");
        return run_server(LatencyDiagnosticCapture::new());
    }
    run_server(CaptureHandle::new())
}

fn run_server<C>(capture: C) -> io::Result<()>
where
    C: CaptureController,
{
    let mut socket = bind_server_socket()?;
    eprintln!(
        "server listening on {} (environment address {})",
        network::wildcard_bind_address(),
        network::server_bind_address()
    );
    let diagnostics = EventContext::logging(Instant::now());
    let mut control = ControlPlane::new(capture, diagnostics);
    let mut peer: Option<SocketAddr> = None;
    let mut sequence = 0_u32;

    loop {
        // Block only as long as the current state needs: indefinitely while
        // idle, bounded to the pending capture retry while starting, and at
        // the tight poll interval while streaming so the loop returns
        // promptly to the blocking capture-process read that paces audio.
        // This replaces a fixed 1ms poll that spun the idle loop at
        // ~1000 iterations/second even with no client connected.
        socket.set_read_timeout(control.read_timeout(Instant::now(), RECEIVE_POLL_INTERVAL))?;
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

/// Binds the server UDP socket, retrying transient failures in-process.
///
/// A fixed-address bind used to turn a seconds-long Wi-Fi flap
/// (`EADDRNOTAVAIL`) into a process exit, which systemd's
/// `StartLimitBurst=3` then escalated into a permanent `start-limit-hit`.
/// Only address/port conflicts are retried (up to `BIND_RETRY_ATTEMPTS`);
/// any other I/O error (e.g. permission denied) is returned immediately so
/// real misconfigurations stay loud instead of being masked by retries.
fn bind_server_socket() -> io::Result<network::UdpServerSocket> {
    let mut attempts = 0_u32;
    loop {
        match network::UdpServerSocket::bind() {
            Ok(socket) => return Ok(socket),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::AddrNotAvailable | io::ErrorKind::AddrInUse
                ) && attempts + 1 < BIND_RETRY_ATTEMPTS =>
            {
                attempts += 1;
                eprintln!(
                    "server bind attempt {attempts}/{BIND_RETRY_ATTEMPTS} failed ({error}); retrying in {}s",
                    BIND_RETRY_INTERVAL.as_secs()
                );
                std::thread::sleep(BIND_RETRY_INTERVAL);
            }
            Err(error) => return Err(error),
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

#[cfg(test)]
mod tests {
    use wifimic_update::CheckUpdateOutcome;

    #[test]
    fn update_available_renders_server_upgrade_literal() {
        // Hand-construct UpdateAvailable using shared render API
        let outcome = CheckUpdateOutcome::UpdateAvailable {
            current: "v0.1.12".to_owned(),
            latest: "v0.2.0".to_owned(),
        };
        let rendered = outcome.render("wifimic_server");
        // Server upgrade literal present
        assert!(
            rendered.contains("wifimic_server upgrade"),
            "rendered output should contain the server upgrade directive"
        );
        // Client literal absent
        assert!(
            !rendered.contains("wifimic_client"),
            "rendered output should not contain the client binary name"
        );
    }
}
