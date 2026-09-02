#[path = "render_windows_endpoints.rs"]
mod endpoints;

#[path = "render_windows_stream.rs"]
mod stream;

#[path = "render_windows_worker.rs"]
mod worker;

pub use endpoints::enumerate_render_endpoints;

use std::sync::{mpsc, Arc};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use wifimic_protocol::AudioFrame;

use super::{RenderConfig, RenderError};

/// A verified shared-mode, event-driven WASAPI render stream.
pub struct Renderer {
    queue: worker::SharedQueue,
    worker: Option<JoinHandle<Result<(), RenderError>>>,
    started: bool,
}

impl Renderer {
    /// Starts a dedicated event-driven worker for the exact configured endpoint.
    pub fn open(config: RenderConfig) -> Result<Self, RenderError> {
        Self::open_impl(config, None)
    }

    /// Starts the worker with a bounded startup-receive window so a stalled
    /// `RenderStream::open` cannot block the caller's retry budget.
    ///
    /// Consumed only by the binary target's startup-retry path (`main.rs`),
    /// which re-declares this module; the library target alone never calls it.
    #[allow(dead_code)]
    pub(crate) fn open_with_startup_timeout(
        config: RenderConfig,
        startup_timeout: Duration,
    ) -> Result<Self, RenderError> {
        Self::open_impl(config, Some(startup_timeout))
    }

    fn open_impl(
        config: RenderConfig,
        startup_timeout: Option<Duration>,
    ) -> Result<Self, RenderError> {
        let queue = Arc::new(std::sync::Mutex::new(worker::RenderQueue::new()));
        let (startup_sender, startup_receiver) = mpsc::sync_channel(1);
        let worker_queue = Arc::clone(&queue);
        let handle = thread::Builder::new()
            .name("wifimic-wasapi-render".to_owned())
            .spawn(move || worker::run(config, worker_queue, startup_sender))
            .map_err(|source| RenderError::WorkerSpawn { source })?;
        let startup_result = match startup_timeout {
            Some(timeout) => startup_receiver.recv_timeout(timeout),
            None => startup_receiver
                .recv()
                .map_err(|_| mpsc::RecvTimeoutError::Disconnected),
        };
        match startup_result {
            Ok(Ok(())) => Ok(Self {
                queue,
                worker: Some(handle),
                started: true,
            }),
            Ok(Err(error)) => match handle.join() {
                Ok(_) => Err(error),
                Err(_) => Err(RenderError::WorkerPanicked),
            },
            Err(mpsc::RecvTimeoutError::Timeout) => {
                drop(handle);
                Err(worker_startup_timed_out(&queue, startup_timeout))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => match handle.join() {
                Ok(_) => Err(RenderError::WorkerStartupFailed),
                Err(_) => Err(RenderError::WorkerPanicked),
            },
        }
    }

    /// Enqueues one protocol-owned mono PCM frame without waiting for WASAPI.
    pub fn render_frame(&mut self, frame: &AudioFrame) -> Result<(), RenderError> {
        if !self.started {
            return Err(RenderError::WorkerStopped);
        }
        let mut state = self
            .queue
            .lock()
            .map_err(|_| RenderError::WorkerStatePoisoned)?;
        if let Some(details) = &state.failure {
            return Err(RenderError::WorkerFailed {
                details: details.clone(),
            });
        }
        state.fifo.push(frame)?;
        Ok(())
    }

    /// Requests worker shutdown and joins the event thread.
    pub fn stop(&mut self) -> Result<(), RenderError> {
        if !self.started {
            return Ok(());
        }
        self.started = false;
        let shutdown_result = self
            .queue
            .lock()
            .map(|mut state| {
                state.shutdown = true;
            })
            .map_err(|_| RenderError::WorkerStatePoisoned);
        let worker_result = match self.worker.take() {
            Some(handle) => match handle.join() {
                Ok(result) => result,
                Err(_) => Err(RenderError::WorkerPanicked),
            },
            None => Ok(()),
        };
        shutdown_result.and(worker_result)
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        if self.started {
            self.started = false;
            if let Ok(mut state) = self.queue.lock() {
                state.shutdown = true;
            }
            if let Some(handle) = self.worker.take() {
                let _ = handle.join();
            }
        }
    }
}

/// Stops a worker whose startup result exceeded its bounded receive window.
fn worker_startup_timed_out(
    queue: &worker::SharedQueue,
    startup_timeout: Option<Duration>,
) -> RenderError {
    let _ = queue.lock().map(|mut state| state.shutdown = true);
    RenderError::WorkerStartupTimedOut {
        startup_timeout_ms: startup_timeout.map_or(u32::MAX, |timeout| {
            u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX)
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_startup_timeout_sets_shutdown_without_a_join_handle() {
        // Given
        let queue = Arc::new(std::sync::Mutex::new(worker::RenderQueue::new()));
        let (_sender, receiver) = mpsc::sync_channel::<()>(1);

        // When
        let receive_result = receiver.recv_timeout(Duration::ZERO);
        let error = worker_startup_timed_out(&queue, Some(Duration::ZERO));

        // Then
        assert!(matches!(
            receive_result,
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        assert!(matches!(
            error,
            RenderError::WorkerStartupTimedOut {
                startup_timeout_ms: 0
            }
        ));
        assert!(queue.lock().expect("test queue lock").shutdown);
    }
}
