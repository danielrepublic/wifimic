# Learnings — wifimic-lan-audio

Conventions, patterns, and successful approaches discovered during work on this plan.

_Auto-scaffolded by /start-work. Append new entries below - never overwrite._

---

## 2026-08-22T07:08:01.8236967Z

- The `.codegraph` Windows junction is correctly ignored by the root `/.codegraph/` rule, while `.omo/` remains trackable.
- `gh repo create wifimic --private --source=. --remote=origin` created the private remote but did not push the existing `main` branch; the explicit `git push -u origin main` was required.
- Repeating the create command failed non-destructively with `GraphQL: Name already exists on this account (createRepository)` and left `origin` and the private repository intact.

## 2026-08-22T07:19:29.6980498Z

- CodeGraph returned no relevant indexed workspace/reference symbols for this repository, so the required micdriver and Rust guidance files were inspected directly.
- The scaffold follows micdriver's resolver 2, edition 2021, Rust 1.97.1, rustfmt, and workspace Clippy `all = "deny"` conventions while intentionally leaving all four members dependency-free.
- The failure probe must copy the four member directories alongside the copied root manifest to reach the deliberately missing `crates/does_not_exist` member; the tracked root manifest hash remained unchanged.

## 2026-08-22T07:54:12.4063885Z

- `wifimic_diagnostics` keeps event records metadata-only: control-plane events carry typed IDs, counters, durations, and classifications, while `Display` emits stable fields without PCM, payload, or sample content.
- Injected `Instant` values make the rate limiter boundary, reset, and 1000-event heartbeat-timeout burst deterministic; the configured one-second gate admits exactly one event from the 0–999 ms burst.
- Rust LSP timed out as expected from prior workspace guidance; package tests, Clippy with `-D warnings`, workspace build, and crate documentation all passed.

## 2026-08-22T08:30:00Z

- Client rotation stores a typed metadata header (`wifimic-diagnostics-v1` plus creation seconds), so age pruning is deterministic with an injected clock and corrupt headers produce typed skip warnings rather than aborting the pass.
- `DiagnosticLogSink` accepts only `wifimic_diagnostics::EventRecord` through `EventSink`; its metadata-only test confirms the serialized output contains no PCM, payload, or sample fields.
- Rotation tests use RAII temp directories with unique process/counter names; the cleanup receipt found no `wifimic-client-logging-*` directories remaining under the Windows temp root.
- Evidence for Task 14: targeted age/size test, corrupt-metadata failure-path test, full client tests, workspace build, and workspace Clippy all passed. Rust LSP diagnostics timed out; Cargo remained authoritative.

## 2026-08-22

- The adaptive client jitter buffer keeps protocol sequence arithmetic in `wifimic_protocol::classify_sequence`, uses an explicit 5 ms playout slot, and bounds resident storage to the derived 41-slot maximum for the 200 ms ceiling.
- Gap, reordered, late, duplicate, and session-mismatch insertion outcomes are typed; the renderer-facing poll result carries either decoded PCM metadata or an explicit gap slot for silence concealment.
- Deterministic injected arrival/planned-playout times make steady, bursty-loss, bursty-late, wraparound, reset, maximal-gap, growth, decay, and ceiling tests reproducible without sockets or sleeps.

## 2026-08-22T09:00:00Z

- The server capture adapter is safest as a small public `CaptureHandle` facade over a private process launcher and injected reader/clock seams: `start` remains the only spawn boundary, `read_frame` fills exactly `PCM_PAYLOAD_BYTES`, and `stop` is idempotent.
- Codegraph indexed the protocol crate but not the server capture symbols, so the required server and micdriver references were inspected directly; this limitation is recorded in task-5 evidence.
- The live SSH smoke was unavailable because `arch-daniel` landed in `/home/daniel` with no wifimic checkout; no real capture child was started. Server-only build, tests, Clippy, and formatting passed.

## 2026-08-22T16:09:53.1634831+08:00

- CodeGraph had no indexed `wifimic_server`/`micdriver_client` network symbols for this checkout, so `apps/wifimic_server/src/network.rs` was created from the task contract; the reference was inspected directly at `C:\Users\Daniel\Documents\opencode\micdriver\apps\micdriver_client\src\network.rs` because the requested `apps/micdriver_client` path does not exist in wifimic.
- The server network seam binds `0.0.0.0:6902`, stores only the fixed typed peer `192.168.0.200`, compares IPv4 addresses exactly, ignores source ports, and returns no datagram before any later consumer for rejected control/audio packets.
- Final focused tests, workspace build, and workspace Clippy passed from a clean detached verification worktree; the repository checkout retained unrelated in-progress capture/client edits unstaged. Rust LSP timed out again, matching the inherited limitation.

## 2026-08-22T08:09:42.1821565Z

- Live preflight on `arch-daniel` found `ufw.service` active while both `iptables.service` and `nftables.service` were inactive; `ufw` owns the existing iptables-nft ruleset. The deployment selector used UFW's persistent, peer-scoped allow plus explicit port-scoped deny and deliberately did not enable nftables.service beside the active firewall manager.
- UDP 6902 from `192.168.0.200` incremented the UFW accept counter once; the scoped drop counter remained zero because no third reachable LAN source was available. The third-source limitation is recorded in task-8 failure evidence.
- The user unit parses with the reference hardening intact, but `~/.local/bin/wifimic_server` is not installed yet, so the remote start probe ended in `203/EXEC`; no fake executable was staged to manufacture an active-service result.

## 2026-08-22T08:20:00Z

- The client renderer can be tested independently of the incomplete binary entrypoint by exposing `render` from `apps/wifimic_client/src/lib.rs`; the Windows-targeted library suite passed 9 deterministic tests, with the two hardware tests remaining explicitly ignored by default.
- On this Windows host, live WASAPI endpoint enumeration found `CABLE Input (VB-Audio Virtual Cable)`, and the live renderer accepted and stopped after writing 400 synthetic 1 kHz frames to that exact endpoint.
- The available live test verifies endpoint opening and frame writes, but does not capture `CABLE Output`; no loopback capture tool is installed (`ffmpeg not found`), so the acoustic/payload confirmation remains unverified. Rust LSP timed out again after the focused Cargo checks passed.

## 2026-08-22

- The capture module stays API-compatible when `CaptureHandle` remains the facade and the public types/errors, frame reader, and `parec` process seam move into sibling private modules; `pub(super)` test seams avoid exposing implementation details.
- Measuring pure LOC after extraction gave 102 for `capture.rs`, 88 for `capture_types.rs`, 39 for `capture_reader.rs`, and 114 for `capture_process.rs`, all below the repository's 250-LOC ceiling.

## 2026-08-22

- The jitter buffer's public facade can remain stable while `types.rs`, `buffer.rs`, and `adaptation.rs` own playout data, sequence-aware queue behavior, and target adaptation respectively; private `pub(super)` seams preserve the existing API without widening implementation visibility.
- Refactor measurements are 17 pure LOC for `jitter.rs`, 168 for `jitter/buffer.rs`, 86 for `jitter/types.rs`, 108 for `jitter/adaptation.rs`, and 231 for the unchanged property/scenario tests.
- The previously reported two logging rotation failures did not reproduce during the post-refactor workspace test; no logging code was changed and no issue entry is required.

## 2026-08-22

- The server control plane uses `wifimic_protocol::SessionOrder` as its sole Start high-water primitive; Stop leaves that mark intact, while Streaming supersession swaps the active ID without calling `CaptureHandle::start` again.
- `ControlPlane::handle_datagram` is the tested wire seam: accepted Start/Heartbeat/Stop messages return encoded protocol Acks, while stale, mismatched, and inactive commands return no Ack and emit typed rejection events.
- Deterministic `Instant` inputs and a fake `CaptureController` cover the seven Todo 6 acceptance cases without sleeps; the test AckSink decodes the real control wire response in memory.
- The initial control implementation exceeded the 250 pure-LOC ceiling; moving capture/error helpers to `control_support.rs` and test fixtures to `control_test_support.rs` kept every touched Rust source file below the limit while preserving the public control facade.

## 2026-08-22T09:07:49Z

- Task 9 guide writing: every command in `docs/deployment-linux.md` was cross-checked against the real artifacts — unit `ExecStart=%h/.local/bin/wifimic_server`, firewall selector branches, pinned source from `capture_types.rs`, peer/port from `network.rs` — so no doc-only values were invented.
- A prohibition note can itself leak a forbidden string: the first draft mentioned `/home/daniel/.psw` inside a "do not include this" sentence and the grep gate caught it. Forbidden-string checks must run against the final text, not intent.
- `Test-NetConnection` is TCP-only on Windows PowerShell; UDP probes need a .NET `UdpClient` snippet, with Linux-side firewall counters as the authoritative delivery proof because the server discards datagrams silently.
- Git Bash at `C:\Program Files\Git\bin\bash.exe` provides working `bash -n` syntax checks on this Windows host even though WSL is unavailable; both deploy scripts passed with exit 0.
- UFW example output should avoid hard-coded rule numbers: the allow is inserted at priority 1 but the deny is appended, so numbering shifts with pre-existing rules.

## 2026-08-22

- The Windows client control plane is clearest as a small public facade plus separate state-transition logic and UDP support modules: `control.rs` remains under the 250 pure-LOC ceiling while its public state, transport, jitter, and renderer seams stay stable.
- Deterministic fake transport and renderer tests decode the actual control bytes, inject both `Instant` and epoch-millisecond clocks, and prove that accepted Start Acks—not local UDP send success—authorize Heartbeats and audio delivery.

## 2026-08-22

- Todo 19's ignored harness keeps the live network/firewall scenario separate from the application fallback: it sends real protocol Start bytes from a socket bound to `192.168.0.200`, validates a real Ack when available, and independently inspects the active firewall backend before and after traffic. A Drop guard sends Stop on panic/timeout so a failed live probe does not intentionally leave a session active.
- The live host preflight was literal: `ufw.service=active`, `nftables.service=inactive`, `iptables.service=inactive`; UFW's peer accept counter rose from `1` to `3` while the scoped UDP 6902 drop counter stayed `0`. This proves firewall-path acceptance, not application delivery.
- The installed remote ELF was stale relative to the current control loop: its mtime was `16:44:50 +0800`, while `b0d17cb` wiring the control plane into the UDP loop was committed at `17:15:04 +0800`. The Ack timeout is therefore recorded as a deployment freshness limitation; no rebuild/restart was performed.

## 2026-08-22

- Todo 13 keeps `TrayRuntime`, native `MenuEvent::receiver()`, and the non-blocking Win32 message pump on the same client loop thread that creates the tray icon; the hidden Windows subsystem avoids a console window while the tray remains available.
- The injected seam covers Restart, Exit, unknown IDs, duplicate Exit, Stop errors, and post-Exit render suppression without claiming a real tray click or live network/audio behavior.

## 2026-08-22

- Task 16's updater keeps the source checkout read-only: it accepts exactly one tag or hexadecimal commit, rejects dirty status before fetching/building, stages with `git worktree add --detach`, and bounds fetch, build, systemd, and smoke operations with `timeout`.
- The transaction copies the prior binary, user unit, hashes, and `file` metadata into a private transaction directory before stopping the user service; the candidate is installed with same-directory copy-plus-`mv`, and the EXIT trap restores the binary/unit and proves `systemctl --user is-active` after rollback.
- A complete smoke is stronger than a successful send: the built-in UDP probe checks exact Start, Heartbeat, and Stop Acks, while an injected helper must emit `wifimic-control-smoke: PASS`; the local harness covered good update, deliberate bad-tag health rollback, build failure, invalid tag, prior hash, active service, and staging cleanup.

## 2026-08-22

- The production control smoke cannot default to localhost: the server listens on `192.168.0.210:6902` but rejects every source except the Windows peer `192.168.0.200`. Task 16 now requires an executable absolute peer-helper before any service or checkout mutation.

## 2026-08-22

- Todo 19's ignored harness keeps the live network/firewall scenario separate from the application fallback: it sends real protocol Start bytes from a socket bound to `192.168.0.200`, validates a real Ack when available, and independently inspects the active firewall backend before and after traffic. A Drop guard sends Stop on panic/timeout so a failed live probe does not intentionally leave a session active.
- The live host preflight was literal: `ufw.service=active`, `nftables.service=inactive`, `iptables.service=inactive`; UFW's peer accept counter rose from `1` to `3` while the scoped UDP 6902 drop counter stayed `0`. This proves firewall-path acceptance, not application delivery.
- The installed remote ELF was stale relative to the current control loop: its mtime was `16:44:50 +0800`, while `b0d17cb` wiring the control plane into the UDP loop was committed at `17:15:04 +0800`. The Ack timeout is therefore recorded as a deployment freshness limitation; no rebuild/restart was performed.
- Task 15: exact PnP AudioEndpoint enumeration found both VB-CABLE directions live; matching only `CABLE Input (VB-Audio Virtual Cable)` keeps the render preflight direction-aware and avoids default-device fallback.
