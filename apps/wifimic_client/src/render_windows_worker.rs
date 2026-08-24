use std::sync::{mpsc::SyncSender, Arc, Mutex, MutexGuard};
use std::time::Duration;

use super::super::fifo::{plan_render_frames, PcmFifo};
use super::super::{RenderConfig, RenderError, MAX_RENDER_QUEUE_FRAMES, MIN_EVENT_WAIT};
use super::stream::RenderStream;

pub(super) type SharedQueue = Arc<Mutex<RenderQueue>>;

pub(super) struct RenderQueue {
    pub(super) fifo: PcmFifo,
    pub(super) shutdown: bool,
    pub(super) failure: Option<String>,
}

impl RenderQueue {
    pub(super) fn new() -> Self {
        Self {
            fifo: PcmFifo::new(MAX_RENDER_QUEUE_FRAMES),
            shutdown: false,
            failure: None,
        }
    }
}

pub(super) fn run(
    config: RenderConfig,
    queue: SharedQueue,
    startup: SyncSender<Result<(), RenderError>>,
) -> Result<(), RenderError> {
    let stream = match RenderStream::open(&config) {
        Ok(stream) => stream,
        Err(error) => {
            if startup.send(Err(error)).is_err() {
                return Err(RenderError::WorkerStartupFailed);
            }
            return Ok(());
        }
    };
    if startup.send(Ok(())).is_err() {
        return match stream.stop() {
            Ok(()) => Err(RenderError::WorkerStartupFailed),
            Err(stop_error) => Err(RenderError::WorkerFailed {
                details: format!("render worker startup channel closed; {stop_error}"),
            }),
        };
    }
    run_event_loop(stream, queue)
}

fn run_event_loop(stream: RenderStream, queue: SharedQueue) -> Result<(), RenderError> {
    let mut scratch = Vec::new();
    loop {
        let wait_timeout_ms =
            duration_to_millis(stream.event_wait_timeout).max(duration_to_millis(MIN_EVENT_WAIT));
        match stream.event.wait_for_event(wait_timeout_ms) {
            Ok(()) => {}
            Err(wasapi::WasapiError::EventTimeout) => {
                let shutting_down = match queue_is_shutting_down(&queue) {
                    Ok(shutting_down) => shutting_down,
                    Err(error) => return fail_worker(&queue, error, &stream),
                };
                if shutting_down {
                    return stream.stop();
                }
                return fail_worker(
                    &queue,
                    RenderError::EventWaitTimeout { wait_timeout_ms },
                    &stream,
                );
            }
            Err(source) => {
                return fail_worker(
                    &queue,
                    RenderError::Wasapi {
                        operation: "wait for render buffer event",
                        source,
                    },
                    &stream,
                );
            }
        }
        let shutting_down = match queue_is_shutting_down(&queue) {
            Ok(shutting_down) => shutting_down,
            Err(error) => return fail_worker(&queue, error, &stream),
        };
        if shutting_down {
            return stream.stop();
        }
        let available_frames = match stream.client.get_available_space_in_frames() {
            Ok(available_frames) => available_frames,
            Err(source) => {
                return fail_worker(
                    &queue,
                    RenderError::Wasapi {
                        operation: "query available render buffer frames",
                        source,
                    },
                    &stream,
                );
            }
        };
        let writable_frames = {
            let state = match lock_queue(&queue) {
                Ok(state) => state,
                Err(error) => return fail_worker(&queue, error, &stream),
            };
            plan_render_frames(available_frames, state.fifo.queued_device_frames())
        };
        if writable_frames == 0 {
            continue;
        }

        {
            let state = match lock_queue(&queue) {
                Ok(state) => state,
                Err(error) => return fail_worker(&queue, error, &stream),
            };
            state.fifo.copy_front(writable_frames, &mut scratch);
        }
        if let Err(source) = stream
            .render_client
            .write_to_device(writable_frames, &scratch, None)
        {
            return fail_worker(
                &queue,
                RenderError::Wasapi {
                    operation: "write queued PCM to render buffer",
                    source,
                },
                &stream,
            );
        }
        let mut state = match lock_queue(&queue) {
            Ok(state) => state,
            Err(error) => return fail_worker(&queue, error, &stream),
        };
        state.fifo.discard_front(writable_frames);
    }
}

fn fail_worker(
    queue: &SharedQueue,
    error: RenderError,
    stream: &RenderStream,
) -> Result<(), RenderError> {
    let details = error.to_string();
    let stop_result = stream.stop();
    let mut state = lock_queue(queue)?;
    state.failure = Some(details);
    state.shutdown = true;
    match stop_result {
        Ok(()) => Err(error),
        Err(stop_error) => Err(RenderError::WorkerFailed {
            details: format!("{error}; {stop_error}"),
        }),
    }
}

fn queue_is_shutting_down(queue: &SharedQueue) -> Result<bool, RenderError> {
    Ok(lock_queue(queue)?.shutdown)
}

fn lock_queue<'a>(queue: &'a SharedQueue) -> Result<MutexGuard<'a, RenderQueue>, RenderError> {
    queue.lock().map_err(|_| RenderError::WorkerStatePoisoned)
}

fn duration_to_millis(duration: Duration) -> u32 {
    u32::try_from(duration.as_millis()).unwrap_or(u32::MAX)
}
