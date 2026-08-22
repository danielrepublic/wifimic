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
