use std::sync::{Arc, Mutex};
use std::time::Instant;

use wifimic_diagnostics::{EventCollector, EventContext};
use wifimic_protocol::{decode_control, encode_control, ControlMessage};

use crate::capture::{CaptureError, CapturedFrame};
use crate::control::{CaptureController, ControlPlane};

#[derive(Debug, Default)]
pub(super) struct FakeCaptureState {
    pub(super) starts: usize,
    pub(super) stops: usize,
    pub(super) start_results: Vec<bool>,
}

#[derive(Clone, Debug)]
pub(super) struct FakeCapture {
    state: Arc<Mutex<FakeCaptureState>>,
}

impl FakeCapture {
    pub(super) fn new(start_results: Vec<bool>) -> (Self, Arc<Mutex<FakeCaptureState>>) {
        let state = Arc::new(Mutex::new(FakeCaptureState {
            start_results,
            ..FakeCaptureState::default()
        }));
        (
            Self {
                state: state.clone(),
            },
            state,
        )
    }

    pub(super) fn starts(state: &Arc<Mutex<FakeCaptureState>>) -> usize {
        state
            .lock()
            .expect("fake capture state is not poisoned")
            .starts
    }

    pub(super) fn stops(state: &Arc<Mutex<FakeCaptureState>>) -> usize {
        state
            .lock()
            .expect("fake capture state is not poisoned")
            .stops
    }
}

impl CaptureController for FakeCapture {
    fn start(&mut self) -> Result<(), CaptureError> {
        let mut state = self
            .state
            .lock()
            .expect("fake capture state is not poisoned");
        state.starts += 1;
        if state.start_results.first().copied().unwrap_or(true) {
            if !state.start_results.is_empty() {
                state.start_results.remove(0);
            }
            Ok(())
        } else {
            state.start_results.remove(0);
            Err(CaptureError::EndpointNotFound {
                source_name: "fake-source".to_owned(),
                stderr: "fake start failure".to_owned(),
            })
        }
    }

    fn stop(&mut self) -> Result<(), CaptureError> {
        self.state
            .lock()
            .expect("fake capture state is not poisoned")
            .stops += 1;
        Ok(())
    }

    fn read_frame(&mut self) -> Result<CapturedFrame, CaptureError> {
        Ok(CapturedFrame {
            pcm: [0; wifimic_protocol::PCM_PAYLOAD_BYTES],
            acquired_at: Instant::now(),
        })
    }
}

pub(super) fn plane(
    start_results: Vec<bool>,
) -> (
    ControlPlane<FakeCapture>,
    Arc<Mutex<FakeCaptureState>>,
    EventCollector,
    Instant,
) {
    let (capture, state) = FakeCapture::new(start_results);
    let origin = Instant::now();
    let collector = EventCollector::new();
    let diagnostics = EventContext::new(origin, collector.clone());
    (
        ControlPlane::new(capture, diagnostics),
        state,
        collector,
        origin,
    )
}

pub(super) fn command(
    plane: &mut ControlPlane<FakeCapture>,
    message: ControlMessage,
    now: Instant,
) -> Option<ControlMessage> {
    let packet = encode_control(&message);
    plane
        .handle_datagram(&packet, now)
        .expect("test control command must not fail")
        .map(|ack| decode_control(&ack).expect("control Ack must decode"))
}

#[derive(Debug, Default)]
pub(super) struct AckSink {
    pub(super) messages: Vec<ControlMessage>,
}

impl AckSink {
    pub(super) fn record(&mut self, ack: Option<ControlMessage>) {
        if let Some(ack) = ack {
            self.messages.push(ack);
        }
    }
}
