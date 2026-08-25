# F4 — Scope fidelity evidence (refreshed artifact inventory)

## Verdict

**VERDICT: APPROVE.** The current inventory has **76 artifact rows**. Every
artifact has a permitted source citation, all **22 Must-have** bullets have a
corresponding implementation artifact, and no **6 Must-NOT-have** guardrail is
violated. No silent artifact omission was found. This is a scope-fidelity
verdict, not a replacement for the separate live-QA verdict in F3.

The inventory was regenerated from the current working tree rather than the
previous 73-row snapshot: root support files **4**, crate files **16**, app
files **44**, deployment files **7**, and documentation files **5** (**76
total**). It includes the three newly present private sibling modules:
`apps/wifimic_client/src/render_windows_endpoints.rs`,
`crates/wifimic_protocol/src/latency/calibration.rs`, and
`crates/wifimic_protocol/src/latency/measurement.rs`.

The remaining live results are explicitly recorded as evidence gaps, not as
scope-fidelity failures, because the required implementation artifacts exist:

- `crates/wifimic_protocol/src/latency.rs`,
  `latency/calibration.rs`, and `latency/measurement.rs` contain the
  calibration, tone, timestamp, percentile, and conservative-P95 mechanism,
  with the existing public names re-exported by the parent facade and unit
  tests retained.
  The live five-minute `conservative_p95 <= 200 ms` acceptance run is not
  proven because the required live two-host prerequisites are unavailable.
- The Exit/Restart, reconnect, capture-retry, and heartbeat-timeout behaviors
  exist in code and are unit-tested. Todos 21–24 remain `[~]` because there is
  no live installed Windows client and the remote Linux binary is stale; the
  real two-host run was not completed.

No unrequested product capability was found. The installer-only fixes in
`11ca79c6e4c6d9dd702e8200e5ecfd1d09fbc492` are covered by Task 15's installer
scope (`.omo/plans/wifimic-lan-audio.md:251-258`) and the live bug-fix rationale
(`.omo/notepads/wifimic-lan-audio/issues.md:79-84`): accepting Task Scheduler's
valid omitted empty Arguments node and recursively removing the private stage
directory. This is a Task 15 robustness/cleanup fix, not a new capability.

The three private sibling modules are internal quality refactors explicitly
permitted by Scope L45 (`.omo/plans/wifimic-lan-audio.md:45`) and Decisions D17
and D18 (`.omo/drafts/wifimic-lan-audio.md:88-89`):

- `render_windows_endpoints.rs` is private under `render_windows.rs`; endpoint
  enumeration, exact device selection, and COM cleanup moved behind the same
  `Renderer`, `RenderError`, and `enumerate_render_endpoints` facade
  (`apps/wifimic_client/src/render.rs:166-178`,
  `apps/wifimic_client/src/render_windows.rs:1-40`).
- `latency/calibration.rs` and `latency/measurement.rs` are private child
  modules of `latency.rs`; the parent re-exports the prior calibration,
  measurement, and statistics names (`crates/wifimic_protocol/src/latency.rs:1-13`),
  while `lib.rs` retains the existing public `latency` module and top-level
  calibration re-exports (`crates/wifimic_protocol/src/lib.rs:33-44`).

The renderer and latency working-tree diffs move implementation behind those
facades; they do not add a product path, change the fixed wire contract, or
widen public module visibility.

## Citation key

Every citation in Table A is one of the permitted forms: a line in
`main_draft.md`, or a specific Open-assumption/Decision bullet in
`.omo/drafts/wifimic-lan-audio.md`.

| Key | Permitted source citation |
| --- | --- |
| M3 | `main_draft.md:3` — Linux microphone -> LAN -> Windows VB-Audio Virtual Cable architecture. |
| M7–M8 | `main_draft.md:7-8` — GitHub deployment and `wifimic-server`/`wifimic-client` service names. |
| M9 | `main_draft.md:9` — Linux service remains available and waits for the Windows client to request streaming. |
| M11–M12 | `main_draft.md:11-12` — Windows autostart/background tray plus Restart/Exit lifecycle. |
| M13 | `main_draft.md:13` — private LAN and no encryption requirement. |
| M17–M18 | `main_draft.md:17-18` — fixed Windows/Linux hosts and peer IPs. |
| M20 | `main_draft.md:20` — two-host-specific implementation priority. |
| M24 | `main_draft.md:24` — Traditional Chinese project-session convention. |
| O1 | Draft Open assumption, line 48 — retain 48 kHz mono 16-bit fixed-frame UDP PCM. |
| O2 | Draft Open assumption, line 49 — bidirectional UDP socket on port 6902. |
| O3 | Draft Open assumption, line 50 — interactive Scheduled Task at user logon. |
| O4 | Draft Open assumption, line 51 — explicit VB-CABLE playback endpoint and verification. |
| O5 | Draft Open assumption, line 52 — clone/build deployment from GitHub. |
| O6 | Draft Open assumption, line 54 — fixed peer IPs and no encryption/general discovery. |
| D1 | Draft Decision, line 69 — Rust rebuild/refactor and required PCM/UDP/WASAPI/tray foundations. |
| D2 | Draft Decision, line 70 — client-controlled Linux capture lifecycle. |
| D3 | Draft Decision, line 71 — packet ordering, bounded jitter, sessions, and diagnostics. |
| D4 | Draft Decision, line 72 — fixed packet contract and documented byte invariant. |
| D5 | Draft Decision, line 74 — TDD and agent-executed deployment/two-host QA. |
| D6 | Draft Decision, line 76 — 30-second heartbeat timeout. |
| D7 | Draft Decision, line 77 — 48 kHz, 16-bit mono transport/renderer contract. |
| D8 | Draft Decision, line 78 — typed capture failure and five-second retry while heartbeat is valid. |
| D9 | Draft Decision, line 79 — journald and bounded Windows diagnostic logs with no raw audio. |
| D10 | Draft Decision, line 81 — application-measured P95 <= 200 ms target. |
| D11 | Draft Decision, line 82 — adaptive 40–200 ms jitter buffer. |
| D12 | Draft Decision, line 83 — pinned Linux capture source and no substitution. |
| D13 | Draft Decision, line 84 — UDP 6902 firewall and application peer allow-lists. |
| D14 | Draft Decision, line 85 — explicit manual tag/commit updates with rollback. |
| D15 | Draft Decision, line 86 — Exit stops the current process/session but leaves autostart enabled. |
| D16 | Draft Decision, line 87 — client stays alive and retries after server outage. |
| D17 | Draft Decision, line 88 — explicit state machines, typed errors, cohesive modules, lints, and tests. |
| D18 | Draft Decision, line 89 — preserve the agreed external contract without product scope expansion. |
| D19 | Draft Decision, line 90 — Linux user-level systemd mechanism for always-on service behavior. |
| D20 | Draft Decision, line 75 — create the canonical private GitHub repository. |

## Table A — Complete implementation-artifact traceability

The inventory covers the root workspace support files (`Cargo.toml`,
`Cargo.lock`, `rust-toolchain.toml`, and `rustfmt.toml`), all four Cargo
packages/binary targets, every current source/test module in the `crates/` and
`apps/` trees (including the three private siblings added by the refactors),
every deploy file, and every file in `docs/`. The three `docs/agents/` files are
retained repository-governance documents, not additional product capabilities;
they are included so the document inventory is exhaustive.

| # | Artifact | Kind / implemented behavior | Source citation | Citation result |
| ---: | --- | --- | --- | --- |
| 1 | `Cargo.toml` | Independent Rust workspace root and four intended members. | D1; D20 | PASS |
| 1a | `Cargo.lock` | Reproducible workspace dependency resolution for the four intended packages. | O5; D1 | PASS |
| 1b | `rustfmt.toml` | Workspace formatting policy supporting the planned Rust code-quality contract. | D17 | PASS |
| 2 | `rust-toolchain.toml` | Pinned Rust toolchain for the two-host build contract. | O5; D1 | PASS |
| 3 | `crates/wifimic_protocol/Cargo.toml` | `wifimic_protocol` Cargo crate manifest. | D1; D4 | PASS |
| 4 | `crates/wifimic_protocol/src/lib.rs` | Protocol constants, exports, and fixed 480-byte PCM / 494-byte audio-datagram contract. | O1; O2; D4; D7 | PASS |
| 5 | `crates/wifimic_protocol/src/audio.rs` | Audio packet encode/decode for session, sequence, and PCM payloads. | O1; D3; D4; D7 | PASS |
| 6 | `crates/wifimic_protocol/src/control.rs` | Start/Heartbeat/Stop/Ack control wire messages on the same UDP socket. | O2; D2; D3 | PASS |
| 7 | `crates/wifimic_protocol/src/latency.rs` | Public latency facade, retained tests, and re-exports for calibration, tone, timestamp math, percentile, and conservative-P95 behavior. | D10; D17; D18 | PASS |
| 7a | `crates/wifimic_protocol/src/latency/calibration.rs` | Private calibration packet, NTP-style timestamp math, clock tracker, and calibration errors moved behind the existing facade. | D10; D17; D18 | PASS |
| 7b | `crates/wifimic_protocol/src/latency/measurement.rs` | Private latency statistics, application-latency, and deterministic-tone helpers moved behind the existing facade. | D10; D17; D18 | PASS |
| 8 | `crates/wifimic_protocol/src/sequence.rs` | Wrapping packet-order classification. | D3 | PASS |
| 9 | `crates/wifimic_protocol/src/session.rs` | Session ordering and monotonic session-ID primitives. | D2; D3 | PASS |
| 10 | `crates/wifimic_protocol/tests/wire_contract.rs` | Protocol byte-size, wire round-trip, and rejection contract tests. | D4; D5 | PASS |
| 11 | `crates/wifimic_diagnostics/Cargo.toml` | `wifimic_diagnostics` Cargo crate manifest. | D1; D3 | PASS |
| 12 | `crates/wifimic_diagnostics/src/lib.rs` | Diagnostics crate public API. | D3; D5 | PASS |
| 13 | `crates/wifimic_diagnostics/src/event.rs` | Typed event classifications including capture retry, heartbeat timeout, sessions, and clock warnings. | D3; D8; D9 | PASS |
| 14 | `crates/wifimic_diagnostics/src/log_sink.rs` | Metadata-only production log sink. | D3; D9 | PASS |
| 15 | `crates/wifimic_diagnostics/src/sink.rs` | Structured event sink, bounded collector, and rate limiter. | D3; D5; D9 | PASS |
| 16 | `crates/wifimic_diagnostics/src/types.rs` | Typed metadata event records with no raw PCM field. | D3; D9; D17 | PASS |
| 17 | `apps/wifimic_server/Cargo.toml` | `wifimic_server` binary package manifest. | M7–M9; D1 | PASS |
| 18 | `apps/wifimic_server/src/main.rs` | Persistent Linux server process and control/audio event loop. | M7; M9; M17–M18; D19 | PASS |
| 19 | `apps/wifimic_server/src/capture.rs` | On-demand `CaptureHandle` for the pinned `parec` capture path. | M3; D2; D8; D12 | PASS |
| 20 | `apps/wifimic_server/src/capture_types.rs` | Pinned source/arguments, PCM frame type, acquisition timestamp, and typed capture errors. | D7; D8; D12 | PASS |
| 21 | `apps/wifimic_server/src/capture_reader.rs` | Fixed-frame stdout reader and capture-boundary timestamp production. | D7; D8; D10; D12 | PASS |
| 22 | `apps/wifimic_server/src/capture_process.rs` | Typed `parec` launcher/process lifecycle and bounded stderr diagnostics. | D8; D9; D12 | PASS |
| 23 | `apps/wifimic_server/src/control.rs` | Idle/Starting/Streaming state machine, session supersession, timeout, Ack, and five-second capture retry. | D2; D6; D8; D15; D16 | PASS |
| 24 | `apps/wifimic_server/src/control_support.rs` | Typed control/capture error boundary and control timing helpers. | D8; D17 | PASS |
| 25 | `apps/wifimic_server/src/network.rs` | Single configured Windows-peer IP filter for UDP control/audio. | M17–M18; O6; D13 | PASS |
| 26 | `apps/wifimic_server/src/control_tests.rs` | Unit coverage for session lifecycle, Ack, retry, timeout, stale IDs, and supersession. | D2; D5; D6; D8 | PASS |
| 27 | `apps/wifimic_server/src/control_test_support.rs` | Deterministic control-plane test doubles and clocks. | D5; D17 | PASS |
| 28 | `apps/wifimic_server/src/capture_tests.rs` | Unit coverage for frame capture, source failure, and stop behavior. | D5; D8; D12 | PASS |
| 29 | `apps/wifimic_server/src/capture_test_support.rs` | Fake process, chunked stdout, and acquisition-clock support for capture tests. | D5; D12; D17 | PASS |
| 30 | `apps/wifimic_server/tests/lan_audio_acceptance.rs` | Ignored live LAN acceptance-test entry point. | D5; M3; M17–M18 | PASS |
| 31 | `apps/wifimic_server/tests/lan_audio_acceptance/session.rs` | Live session/control acceptance helpers. | D2; D5; D6 | PASS |
| 32 | `apps/wifimic_server/tests/lan_audio_acceptance/firewall.rs` | Live firewall/peer-scope acceptance helpers. | D13; D5 | PASS |
| 33 | `apps/wifimic_server/tests/lan_audio_acceptance/ssh.rs` | Remote Linux-host acceptance transport/helpers. | O5; D5; D19 | PASS |
| 34 | `apps/wifimic_client/Cargo.toml` | `wifimic_client` binary package manifest. | M8; M11–M12; D1 | PASS |
| 35 | `apps/wifimic_client/src/main.rs` | Interactive Windows client process and render/control/tray orchestration. | M8; M11–M12; D2; D16 | PASS |
| 36 | `apps/wifimic_client/src/lib.rs` | Client module boundary and shared public crate surface. | D1; D17; D18 | PASS |
| 37 | `apps/wifimic_client/src/control.rs` | Start/heartbeat/stop, Ack reachability, fresh-session reconnect, and peer filtering. | O2; O6; D2; D13; D15; D16 | PASS |
| 38 | `apps/wifimic_client/src/control_logic.rs` | Client-side session/reconnect state and audio-to-jitter/render flow. | D2; D3; D11; D16 | PASS |
| 39 | `apps/wifimic_client/src/control_support.rs` | Typed client control errors and deterministic transport/test seams. | D17; D18 | PASS |
| 40 | `apps/wifimic_client/src/control_tests.rs` | Unit coverage for Ack reachability, reconnect, peer filtering, and Exit semantics. | D5; D13; D15; D16 | PASS |
| 41 | `apps/wifimic_client/src/tray.rs` | Hidden-by-default tray menu, Restart dispatch, Exit stop-then-shutdown, and render suppression. | M11–M12; D15; D18 | PASS |
| 42 | `apps/wifimic_client/src/tray_tests.rs` | Unit coverage for Restart/Exit menu behavior. | M12; D5; D15 | PASS |
| 43 | `apps/wifimic_client/src/render.rs` | Renderer abstraction and explicit endpoint configuration. | O4; D7 | PASS |
| 44 | `apps/wifimic_client/src/render_windows.rs` | WASAPI render facade and VB-CABLE stream lifecycle. | M3; O4; D7; D17; D18 | PASS |
| 44a | `apps/wifimic_client/src/render_windows_endpoints.rs` | Private exact endpoint enumeration/selection and COM-apartment cleanup extracted from the renderer facade. | O4; D17; D18 | PASS |
| 45 | `apps/wifimic_client/src/render_non_windows.rs` | Non-Windows unsupported-platform seam preserving workspace build shape. | D1; D18 | PASS |
| 46 | `apps/wifimic_client/src/render_tests.rs` | Endpoint-selection and renderer contract tests. | O4; D5 | PASS |
| 47 | `apps/wifimic_client/src/jitter.rs` | Adaptive jitter-buffer module boundary. | D3; D11 | PASS |
| 48 | `apps/wifimic_client/src/jitter/buffer.rs` | Sequence-aware bounded FIFO, gap concealment, reorder repair, and adaptive target delay. | D3; D11 | PASS |
| 49 | `apps/wifimic_client/src/jitter/adaptation.rs` | Growth/decay policy constrained to 40–200 ms. | D11 | PASS |
| 50 | `apps/wifimic_client/src/jitter/types.rs` | Typed frame-insert and playout classifications. | D3; D11; D17 | PASS |
| 51 | `apps/wifimic_client/src/jitter/tests.rs` | Steady, loss, late-frame, ceiling, and recovery tests. | D5; D11 | PASS |
| 52 | `apps/wifimic_client/src/logging.rs` | Windows diagnostic logging facade and lifecycle initialization. | D9 | PASS |
| 53 | `apps/wifimic_client/src/logging/errors.rs` | Typed logging/rotation errors. | D9; D17 | PASS |
| 54 | `apps/wifimic_client/src/logging/rotation.rs` | Seven-day/10 MiB bounded retention and pruning. | D9; D17 | PASS |
| 55 | `apps/wifimic_client/src/logging/sink.rs` | Windows file sink for structured metadata events. | D9 | PASS |
| 56 | `apps/wifimic_client/src/logging/tests.rs` | Log retention and no-raw-audio tests. | D5; D9 | PASS |
| 57 | `apps/wifimic_client/build.rs` | Windows tray resource build integration. | M11–M12; D1 | PASS |
| 58 | `apps/wifimic_client/assets/tray-icon.rc` | Embedded tray icon resource declaration. | M11–M12 | PASS |
| 59 | `apps/wifimic_client/assets/tray-icon.ico` | Embedded tray icon asset. | M11–M12 | PASS |
| 60 | `deploy/systemd/wifimic-server.service` | Hardened Linux user-level `wifimic-server` service. | M7; M9; D19 | PASS |
| 61 | `deploy/linux/update-wifimic-server.sh` | Explicit-tag/commit Linux build, health check, and automatic rollback. | O5; D14; D19 | PASS |
| 62 | `deploy/linux/wifimic-server-firewall.sh` | Linux firewall installation/application helper for the fixed peer and UDP 6902. | M17–M18; D13 | PASS |
| 63 | `deploy/linux/wifimic-server-iptables.sh` | Active-iptables backend rule path scoped to the Windows peer. | M17–M18; D13 | PASS |
| 64 | `deploy/linux/wifimic-server.nft` | nftables DROP-by-default plus approved-peer UDP 6902 rule. | M17–M18; D13 | PASS |
| 65 | `deploy/windows/install-wifimic-client.ps1` | Program Files install, canonical Scheduled Task, canonical firewall rule, and VB-CABLE preflight. | M8; M11–M12; O3; O4; O6 | PASS |
| 66 | `deploy/windows/update-wifimic-client.ps1` | Explicit-tag/commit Windows build, health check, task restart, and rollback. | O5; D14; D18 | PASS |
| 67 | `docs/deployment.md` | Traditional Chinese end-to-end two-machine clone/install/update/rollback guide. | M7–M8; M24; O5; D14 | PASS |
| 68 | `docs/deployment-linux.md` | Traditional Chinese Linux user-service, pinned-source, firewall, and peer-allow-list guide. | M7; M9; M24; D13; D19 | PASS |
| 69 | `docs/agents/domain.md` | Retained repository domain-documentation governance. | D17; D20 | PASS |
| 70 | `docs/agents/triage-labels.md` | Retained repository issue-triage vocabulary governance. | D20; D17 | PASS |
| 71 | `docs/agents/issue-tracker.md` | Retained repository issue-tracker integration guidance. | D20; D5 | PASS |

**Table A result: PASS.** All **76** inventoried rows have a permitted source
citation; **uncited artifacts: 0**. The `docs/agents/` rows are explicitly
classified as retained repository-governance documentation, not new product
capabilities. The three private refactor siblings are explicitly cited under
D17/D18 and are not omitted from the inventory.

## Table B — Every `Scope / Must have` bullet and its implementation

The plan has 22 literal bullet items when nested bullets are counted. The
rows below preserve that nesting as individual requirements rather than
silently collapsing it into the nine top-level bullets. `Artifact coverage`
answers the F4 scope question; it does not turn a missing live acceptance run
into a passing measurement.

| ID | Plan bullet | Specific satisfying artifact(s) | Artifact coverage / acceptance status |
| --- | --- | --- | --- |
| S1 | L25 — New independent `wifimic` workspace with `wifimic_protocol`, `wifimic_diagnostics`, `wifimic_server`, and `wifimic_client`, in the new private GitHub repository. | `Cargo.toml`; the four package manifests; the four named targets; Todo 1 repository setup. | **PASS — artifact present.** |
| S2 | L26 — Fixed 48 kHz mono 16-bit PCM protocol, one bidirectional UDP socket/host on port 6902, with the reference 484-vs-480 discrepancy resolved by a documented byte-size proof. | `crates/wifimic_protocol/src/lib.rs`, `audio.rs`, `control.rs`, `sequence.rs`, `tests/wire_contract.rs`. The implemented contract is 480 PCM bytes (240 samples × 2) and 494 total audio-datagram bytes. | **PASS — resolved contract and unit proof present.** |
| S3 | L27 — Persistent always-running Linux user-level `wifimic-server`, idle until a Windows client is active. | `apps/wifimic_server/src/main.rs`; `deploy/systemd/wifimic-server.service`. | **PASS — implementation present.** |
| S4 | L28 — Capture pinned to `alsa_input.pci-0000_00_1b.0.analog-stereo` via mono `parec`, with no silent substitution. | `capture.rs`, `capture_types.rs`, `capture_reader.rs`, `capture_process.rs`; `docs/deployment-linux.md`. | **PASS — pinned-source implementation and tests present.** |
| S5 | L29 — Valid Start begins streaming; capture stops after 30 seconds without a valid heartbeat while the service remains alive. | `apps/wifimic_server/src/control.rs`, `control_tests.rs`, `main.rs`. | **PASS — code and unit tests present.** |
| S6 | L30 — Typed capture failure, five-second retry while heartbeat remains valid, same-session resume, and no fallback source. | `control.rs`, `capture*.rs`, `capture_tests.rs`, `wifimic_diagnostics/src/event.rs`. | **PASS — code and unit tests present.** |
| S7 | L31 — Linux application peer allow-list plus host firewall scoped to the one Windows peer and UDP 6902. | `apps/wifimic_server/src/network.rs`; `deploy/linux/wifimic-server-firewall.sh`, `wifimic-server-iptables.sh`, `wifimic-server.nft`; acceptance tests. | **PASS — both application and firewall artifacts present.** |
| S8 | L32 — Windows interactive LogonTrigger/InteractiveToken Scheduled Task at `C:\Program Files\wifimic-client`, not a Session-0 service. | `deploy/windows/install-wifimic-client.ps1`; `apps/wifimic_client/src/main.rs`, `tray.rs`. | **PASS — installer and client artifacts present.** |
| S9 | L33 — Canonical task `\wifimic\wifimic-client`, firewall DisplayName `wifimic-client`, and install directory. | `deploy/windows/install-wifimic-client.ps1`; `docs/deployment.md`. | **PASS — canonical identifiers implemented.** |
| S10 | L34 — Symmetric Windows incoming peer allow-list and host firewall scoped to Linux `192.168.0.210`/UDP 6902. | `apps/wifimic_client/src/control.rs`, `control_logic.rs`; `install-wifimic-client.ps1`. | **PASS — client filtering and installer scope present.** |
| S11 | L35 — Hidden-by-default tray icon with Restart and Exit. | `tray.rs`, `tray_tests.rs`, `build.rs`, `assets/tray-icon.rc`, `assets/tray-icon.ico`. | **PASS — implementation and unit tests present.** |
| S12 | L36 — Restart sends a fresh Start/heartbeat stream. | `apps/wifimic_client/src/control.rs`, `control_logic.rs`, `tray.rs`, `control_tests.rs`. | **PASS — implementation and tests present.** |
| S13 | L37 — Exit sends Stop, terminates only the current run, and leaves the Scheduled Task enabled for next logon. | `tray.rs`, `tray_tests.rs`, `control.rs`, `install-wifimic-client.ps1`. | **PASS — implementation and tests present.** |
| S14 | L38 — Unreachable server leaves the tray alive, retries every five seconds, and resumes automatically. | `control.rs`, `control_logic.rs`, `control_tests.rs`. | **PASS — implementation and unit tests present.** Live reconnection proof is separately blocked under S20. |
| S15 | L39 — WASAPI renders to the explicitly enumerated and verified VB-CABLE playback endpoint, with clear failure if absent. | `render.rs`, `render_windows.rs`, private `render_windows_endpoints.rs`, `render_non_windows.rs`, `render_tests.rs`. | **PASS — implementation and endpoint tests present.** |
| S16 | L40 — Adaptive jitter target starts at 40 ms, grows to 200 ms, and decays back without exceeding bounds. | `jitter.rs`, `jitter/buffer.rs`, `jitter/adaptation.rs`, `jitter/types.rs`, `jitter/tests.rs`. | **PASS — implementation and property-style unit coverage present.** |
| S17 | L41 — Windows logs are raw-audio-free and capped at seven days/10 MiB; Linux diagnostics use journald. | `logging.rs`, `logging/errors.rs`, `logging/rotation.rs`, `logging/sink.rs`, `logging/tests.rs`; `wifimic_diagnostics`; `docs/deployment-linux.md`. | **PASS — implementation and tests present.** |
| S18 | L42 — GitHub clone/build deployment on both hosts, explicit tag/commit updates, health validation, automatic rollback, no background self-update. | `deploy/linux/update-wifimic-server.sh`, `deploy/windows/install-wifimic-client.ps1`, `deploy/windows/update-wifimic-client.ps1`, `docs/deployment.md`, `docs/deployment-linux.md`. | **PASS — deployment artifacts and documentation present.** |
| S19 | L43 — Verified application-measured one-way P95 <= 200 ms over real UDP 6902, using the clarified capture-boundary-to-VB-CABLE boundary and conservative error budget. | `crates/wifimic_protocol/src/latency.rs`, private `latency/calibration.rs`, private `latency/measurement.rs`; protocol latency tests; server/client calibration integration points. | **ARTIFACT PRESENT; LIVE EVIDENCE GAP.** The calibration and measurement mechanisms exist behind the unchanged `latency` facade, but the live five-minute P95 run and `conservative_p95 <= 200 ms` result are `[~]` blocked on external prerequisites. This does not indicate a missing artifact or silent scope reduction. |
| S20 | L44 — TDD for protocol/control/jitter plus happy/failure agent QA for every implementation todo and a final two-host wave. | Protocol, diagnostics, server, client unit/test modules; `apps/wifimic_server/tests/lan_audio_acceptance/*`; `.omo/evidence/task-1` through `task-24` artifacts. | **ARTIFACTS PRESENT; LIVE EVIDENCE GAP.** Unit/test and ignored live-harness artifacts exist, but the final live wave remains incomplete because Todo 20 and Todos 21–24 are `[~]`. |
| S21 | L45 — Improved code quality: explicit state machines, typed errors, cohesive modules, strict lints, and behavior-locking tests without added product capability. | `control.rs` state machines; typed error modules; private `render_windows_endpoints.rs`, `latency/calibration.rs`, and `latency/measurement.rs`; split server/client modules; workspace lint configuration; unit/integration tests. | **PASS for implemented artifact coverage.** The private sibling extraction is an internal quality refactor permitted by L45 and D17/D18; the external lifecycle, public facades, and fixed wire contract remain unchanged. |
| S22 | L46 — Deployment-facing instructions are Traditional Chinese; source identifiers/comments remain English. | `docs/deployment.md`; `docs/deployment-linux.md`. | **PASS — required deployment docs present.** |

## Table C — Must-NOT-have guardrail check

The six literal guardrail bullets were checked against the complete Table A
inventory and the implementation/test source. No artifact exists whose only
explanation would be an out-of-scope product capability.

| ID | Plan bullet | Check basis | Result |
| --- | --- | --- | --- |
| N1 | L49 — No encryption/authentication infrastructure, traversal, discovery, multi-client fan-out, or general-purpose networking. | Fixed private-LAN/no-encryption contract (M13; O6); four-package inventory and fixed-peer/port implementation. | **PASS — no violation found.** |
| N2 | L50 — No web UI and no VB-CABLE driver bundling/automation. | VB-CABLE is only enumerated/verified as an installed endpoint (O4); no web/UI or driver-install artifact appears in Table A. | **PASS — no violation found.** |
| N3 | L51 — No Session-0 service, separate service/UI IPC pair, or pre-logon streaming. | Interactive Logon Scheduled Task and tray contract (O3; M11–M12); installer/client artifacts use the signed-in session. | **PASS — no violation found.** |
| N4 | L52 — No Opus/RTP/WebRTC adoption or stereo wire-format expansion. | Fixed 48 kHz mono 16-bit PCM decision (O1; D7); protocol/audio artifacts retain that contract. | **PASS — no violation found.** |
| N5 | L53 — No background self-update or automatic capture/render fallback. | Manual tag/commit update with rollback (D14); pinned source/endpoint behavior (O4; D12); no fallback/update daemon artifact. | **PASS — no violation found.** |
| N6 | L54 — No new product capability beyond the approved draft/plan decisions. | Complete cited inventory, quality-refactor boundary (D17; D18), and specific installer-fix rationale above. | **PASS — no scope expansion found.** |

### Live-acceptance qualification for S19/S20 and Todos 21–24

The underlying application behaviors are not being marked absent:

- Todo 20's calibration and conservative-statistics mechanism is implemented
  in `crates/wifimic_protocol/src/latency.rs`,
  `latency/calibration.rs`, and `latency/measurement.rs`, and unit-tested. The
  acceptance measurement itself is `[~]` because the external live two-host
  prerequisites are unavailable.
- Todo 21's Exit/Restart behavior is implemented and unit-tested, but the
  real installed Windows Scheduled Task/client sequence was not observed.
- Todo 22's fresh-session reconnect behavior is implemented and unit-tested,
  but the real 15-second interruption/20-second recovery run was not
  observed.
- Todo 23's pinned-source capture retry behavior is implemented and
  unit-tested, but the real-host source fault-injection/resume run was not
  observed.
- Todo 24's 30-second heartbeat-timeout behavior is implemented and
  unit-tested, but the real-host 30–35-second timing run was not observed.

The blocking prerequisites are the inherited external constraints: no live
installed Windows client in the required elevated session, a stale remote
Linux binary requiring redeployment, and the optional live loopback/calibration
setup. These facts prevent claiming the plan's live-verified Must-have
behaviors from code and unit tests alone.

**Table B result:** artifact coverage is complete for all 22 Must-have bullets;
S19 and S20 have live-evidence gaps, but neither is missing its required
artifact. Table C found no Must-NOT-have violation. Consequently the F4
scope-fidelity verdict is **APPROVE**, while F3 independently remains
**REJECT** for the unexecuted live scenarios.
