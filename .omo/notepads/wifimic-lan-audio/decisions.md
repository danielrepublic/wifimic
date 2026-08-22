# Decisions — wifimic-lan-audio

Architectural choices and rationales discovered during work on this plan.

_Auto-scaffolded by /start-work. Append new entries below - never overwrite._

---

## 2026-08-22T07:35:44.8208649Z

- The wifimic wire contract uses a leading tag plus version: audio is `0x00 | version | big-endian u64 session | big-endian u32 wrapping sequence | 480-byte signed-16-bit little-endian PCM`; Start/Heartbeat/Stop are 10-byte tagged control messages and Ack is 11 bytes with the acknowledged control tag appended.
- The reference project's 484-byte PCM value is not carried forward: the named invariant test proves `48,000 Hz × 5 ms = 240 samples`, and `240 × 2 bytes/sample = 480 bytes`; no four-byte trailer is specified.
- Session acceptance is strict high-water ordering (`candidate > last accepted`), while the in-process generator uses the injected-clock formula `max(current_epoch_ms, last_issued + 1)` and returns a typed exhaustion error at `u64::MAX`.
