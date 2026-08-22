use std::{
    collections::VecDeque,
    io::{self, Read},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Instant,
};

pub(super) use super::super::ProcessExit;
use super::super::{CaptureClock, CaptureError, CaptureHandle, CaptureLauncher, CaptureProcess};

pub(super) fn test_handle(launcher: TestLauncher, clock: Box<dyn CaptureClock>) -> CaptureHandle {
    CaptureHandle::with_test_components(Box::new(launcher), clock)
}

pub(super) struct TestLauncher {
    spawn_count: Arc<AtomicUsize>,
    process: Mutex<Option<FakeProcess>>,
    arguments: Arc<Mutex<Vec<Vec<String>>>>,
}

impl TestLauncher {
    pub(super) fn new(spawn_count: Arc<AtomicUsize>, process: FakeProcess) -> Self {
        Self {
            spawn_count,
            process: Mutex::new(Some(process)),
            arguments: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl CaptureLauncher for TestLauncher {
    fn spawn(&self, arguments: &[&str]) -> Result<Box<dyn CaptureProcess>, CaptureError> {
        self.spawn_count.fetch_add(1, Ordering::SeqCst);
        self.arguments
            .lock()
            .expect("test argument mutex must not be poisoned")
            .push(
                arguments
                    .iter()
                    .map(|argument| (*argument).to_owned())
                    .collect(),
            );
        let process = self
            .process
            .lock()
            .expect("test process mutex must not be poisoned")
            .take()
            .ok_or(CaptureError::NotRunning)?;
        Ok(Box::new(process))
    }
}

pub(super) struct FakeProcess {
    reader: ChunkedReader,
    exit: ProcessExit,
    stopped: Option<Arc<AtomicBool>>,
}

impl FakeProcess {
    pub(super) fn empty() -> Self {
        Self::from_reader(ChunkedReader::new(Vec::new(), []), ProcessExit::success())
    }

    pub(super) fn from_reader(reader: ChunkedReader, exit: ProcessExit) -> Self {
        Self {
            reader,
            exit,
            stopped: None,
        }
    }

    pub(super) fn with_stop_signal(stopped: Arc<AtomicBool>) -> Self {
        Self {
            reader: ChunkedReader::new(Vec::new(), []),
            exit: ProcessExit::success(),
            stopped: Some(stopped),
        }
    }
}

impl CaptureProcess for FakeProcess {
    fn read_stdout(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.reader.read(buffer)
    }

    fn finish_after_eof(&mut self) -> io::Result<ProcessExit> {
        Ok(self.exit.clone())
    }

    fn stop(&mut self) -> io::Result<()> {
        if let Some(stopped) = &self.stopped {
            stopped.store(true, Ordering::SeqCst);
        }
        Ok(())
    }
}

pub(super) struct ChunkedReader {
    bytes: Vec<u8>,
    cursor: usize,
    chunk_sizes: VecDeque<usize>,
}

impl ChunkedReader {
    pub(super) fn new(bytes: Vec<u8>, chunk_sizes: impl IntoIterator<Item = usize>) -> Self {
        Self {
            bytes,
            cursor: 0,
            chunk_sizes: chunk_sizes.into_iter().collect(),
        }
    }
}

impl io::Read for ChunkedReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.cursor == self.bytes.len() {
            return Ok(0);
        }
        let requested = self
            .chunk_sizes
            .pop_front()
            .unwrap_or(buffer.len())
            .min(buffer.len());
        let available = self.bytes.len() - self.cursor;
        let read = requested.min(available);
        buffer[..read].copy_from_slice(&self.bytes[self.cursor..self.cursor + read]);
        self.cursor += read;
        Ok(read)
    }
}

pub(super) struct SequenceClock {
    values: Mutex<VecDeque<Instant>>,
}

impl SequenceClock {
    pub(super) fn new(values: VecDeque<Instant>) -> Self {
        Self {
            values: Mutex::new(values),
        }
    }
}

impl CaptureClock for SequenceClock {
    fn now(&self) -> Instant {
        self.values
            .lock()
            .expect("test clock mutex must not be poisoned")
            .pop_front()
            .unwrap_or_else(Instant::now)
    }
}

impl ProcessExit {
    pub(super) fn success() -> Self {
        Self {
            success: true,
            exit_code: Some(0),
            stderr: String::new(),
        }
    }

    pub(super) fn failure(stderr: &str) -> Self {
        Self {
            success: false,
            exit_code: Some(1),
            stderr: stderr.to_owned(),
        }
    }
}
