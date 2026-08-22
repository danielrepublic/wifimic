use std::io::{self, Read};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const SSH_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug)]
pub(super) struct CommandOutput {
    status: ExitStatus,
    stdout: String,
    stderr: String,
}

fn collect_output(child: &mut Child, status: ExitStatus) -> io::Result<CommandOutput> {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    if let Some(mut pipe) = child.stdout.take() {
        pipe.read_to_end(&mut stdout)?;
    }
    if let Some(mut pipe) = child.stderr.take() {
        pipe.read_to_end(&mut stderr)?;
    }
    Ok(CommandOutput {
        status,
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
    })
}

pub(super) fn run_ssh(target: &str, remote_command: &str) -> io::Result<CommandOutput> {
    let mut child = Command::new("ssh")
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectionAttempts=1",
            "-o",
            "ConnectTimeout=5",
            "-o",
            "ServerAliveInterval=2",
            "-o",
            "ServerAliveCountMax=2",
            target,
            remote_command,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let deadline = Instant::now() + SSH_TIMEOUT;
    loop {
        if let Some(status) = child.try_wait()? {
            return collect_output(&mut child, status);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("ssh command exceeded {SSH_TIMEOUT:?}"),
            ));
        }
        thread::sleep(Duration::from_millis(50));
    }
}

pub(super) fn require_success(label: &str, output: CommandOutput) -> String {
    assert!(
        output.status.success(),
        "{label} failed: stdout={} stderr={}",
        output.stdout,
        output.stderr
    );
    output.stdout
}

pub(super) fn remote_state(target: &str, command: &str) -> String {
    let output = run_ssh(target, command).expect("bounded SSH state probe must start");
    require_success(command, output).trim().to_owned()
}
