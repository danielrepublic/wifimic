#[path = "render_windows_endpoints.rs"]
mod endpoints;

#[path = "render_windows_stream.rs"]
mod stream;

#[path = "render_windows_worker.rs"]
mod worker;

pub use endpoints::enumerate_render_endpoints;

use std::sync::{mpsc, Arc};
use std::thread::{self, JoinHandle};

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
        let queue = Arc::new(std::sync::Mutex::new(worker::RenderQueue::new()));
        let (startup_sender, startup_receiver) = mpsc::sync_channel(1);
        let worker_queue = Arc::clone(&queue);
        let handle = thread::Builder::new()
            .name("wifimic-wasapi-render".to_owned())
            .spawn(move || worker::run(config, worker_queue, startup_sender))
            .map_err(|source| RenderError::WorkerSpawn { source })?;
        match startup_receiver.recv() {
            Ok(Ok(())) => Ok(Self {
                queue,
                worker: Some(handle),
                started: true,
            }),
            Ok(Err(error)) => match handle.join() {
                Ok(_) => Err(error),
                Err(_) => Err(RenderError::WorkerPanicked),
            },
            Err(_) => match handle.join() {
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
