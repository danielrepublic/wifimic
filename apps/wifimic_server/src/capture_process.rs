use std::{
    io::{self, Read},
    path::PathBuf,
    process::{Child, ChildStderr, ChildStdout, Command, Stdio},
};

use super::{CaptureError, CaptureLauncher, CaptureProcess, ProcessExit, PINNED_CAPTURE_SOURCE};

const MAX_STDERR_BYTES: usize = 4_096;

pub(super) struct ParecLauncher {
    program: PathBuf,
}

impl ParecLauncher {
    pub(super) fn new(program: PathBuf) -> Self {
        Self { program }
    }
}

impl CaptureLauncher for ParecLauncher {
    fn spawn(&self, arguments: &[&str]) -> Result<Box<dyn CaptureProcess>, CaptureError> {
        let mut child = Command::new(&self.program)
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| CaptureError::Spawn {
                source_name: PINNED_CAPTURE_SOURCE.to_owned(),
                error,
            })?;

        let Some(stdout) = child.stdout.take() else {
            let _ = terminate_child(&mut child);
            return Err(CaptureError::StdoutUnavailable);
        };
        let Some(stderr) = child.stderr.take() else {
            let _ = terminate_child(&mut child);
            return Err(CaptureError::StderrUnavailable);
        };

        Ok(Box::new(ParecProcess {
            child,
            stdout,
            stderr,
        }))
    }
}

struct ParecProcess {
    child: Child,
    stdout: ChildStdout,
    stderr: ChildStderr,
}

impl CaptureProcess for ParecProcess {
    fn read_stdout(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.stdout.read(buffer)
    }

    fn finish_after_eof(&mut self) -> io::Result<ProcessExit> {
        let status = reap_child(&mut self.child)?;
        let stderr = read_bounded(&mut self.stderr)?;
        Ok(ProcessExit {
            success: status.success(),
            exit_code: status.code(),
            stderr,
        })
    }

    fn stop(&mut self) -> io::Result<()> {
        let _ = reap_child(&mut self.child)?;
        let mut discarded_stderr = String::new();
        read_bounded_into(&mut self.stderr, &mut discarded_stderr)
    }
}

impl Drop for ParecProcess {
    fn drop(&mut self) {
        let _ = terminate_child(&mut self.child);
    }
}

fn read_bounded(stderr: &mut impl Read) -> io::Result<String> {
    let mut output = String::new();
    read_bounded_into(stderr, &mut output)?;
    Ok(output)
}

fn read_bounded_into(stderr: &mut impl Read, output: &mut String) -> io::Result<()> {
    let mut buffer = [0_u8; 512];
    let mut retained = Vec::with_capacity(MAX_STDERR_BYTES);

    loop {
        let read = stderr.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        let remaining = MAX_STDERR_BYTES.saturating_sub(retained.len());
        retained.extend_from_slice(&buffer[..read.min(remaining)]);
    }

    output.push_str(&String::from_utf8_lossy(&retained));
    Ok(())
}

fn reap_child(child: &mut Child) -> io::Result<std::process::ExitStatus> {
    if let Some(status) = child.try_wait()? {
        return Ok(status);
    }

    child.kill()?;
    child.wait()
}

fn terminate_child(child: &mut Child) -> io::Result<()> {
    let _ = reap_child(child)?;
    Ok(())
}
