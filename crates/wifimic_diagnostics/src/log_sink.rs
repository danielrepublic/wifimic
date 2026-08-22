use log::Level;

use super::event::Event;
use super::sink::EventSink;
use super::types::EventRecord;

/// A production sink that forwards structured metadata to the `log` facade.
#[derive(Debug, Default, Clone, Copy)]
pub struct WifimicLogSink;

impl EventSink for WifimicLogSink {
    fn record(&self, record: EventRecord) {
        let level = match record.event {
            Event::SenderSendFailure { .. }
            | Event::PacketGap { .. }
            | Event::MalformedPacket { .. }
            | Event::OverflowEviction { .. }
            | Event::CaptureRetry { .. }
            | Event::HeartbeatTimeout { .. }
            | Event::ControlMessageRejected { .. } => Level::Warn,
            Event::RenderEventTimeout { .. } | Event::JitterBufferLockPoisoned { .. } => {
                Level::Error
            }
            Event::ReorderedRepair { .. }
            | Event::ConnectionTransition { .. }
            | Event::PrefillStart { .. }
            | Event::UnderrunBurst { .. }
            | Event::SessionStarted { .. }
            | Event::SessionStopped { .. } => Level::Info,
        };

        log::log!(target: "wifimic_diagnostics", level, "event={record}");
    }
}

/// Compatibility name matching the source diagnostics crate's log sink.
pub type LogEventSink = WifimicLogSink;
