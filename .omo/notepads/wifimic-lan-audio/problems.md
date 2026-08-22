# Problems — wifimic-lan-audio

Unresolved blockers and technical debt discovered during work on this plan.

_Auto-scaffolded by /start-work. Append new entries below - never overwrite._

---

## 2026-08-22

- The Todo 19 live Ack scenario cannot pass against the currently active remote ELF: the host firewall accepts the real peer packet, but the installed binary predates the current control-plane loop. This remains an explicit deployment limitation rather than a reason to weaken the harness or mutate the remote service during this task.

## 2026-08-22

- No unresolved Todo 13 implementation blocker remains. Real tray clicks, Scheduled Task state, VB-CABLE/network delivery, and live Ack behavior are intentionally deferred to later Wave 5/deployment acceptance; this task claims only the injected menu seam and bounded startup smoke.

## 2026-08-22

- No live `arch-daniel` updater run was attempted: its required peer-originated control smoke cannot be honestly substituted by a localhost datagram because the production server accepts only `192.168.0.200`.

## 2026-08-22

- The live peer helper is still an external prerequisite: no helper that can originate from `192.168.0.200` is available here, so live update success remains intentionally unclaimed.

## 2026-08-22

- No unresolved Task 12 implementation blocker remains. Live two-host control/audio behavior is intentionally deferred to the later deployment and Wave 5 acceptance todos; this task's evidence makes no hardware or firewall claim.

## 2026-08-22

- The Todo 19 live Ack scenario cannot pass against the currently active remote ELF: the host firewall accepts the real peer packet, but the installed binary predates the current control-plane loop. This remains an explicit deployment limitation rather than a reason to weaken the harness or mutate the remote service during this task.
- No unresolved Task 15 implementation blocker remains. Live Scheduled Task/firewall installation and post-install `Ready` inspection remain unclaimed because the safe acceptance boundary was not authorized; the read-only host check confirmed both canonical artifacts are absent and the fake rollback receipts are clean.

## 2026-08-22

- No unresolved Task 17 implementation blocker remains. Native Windows task/binary mutation was not exercised because the canonical task is absent and host acceptance was not supplied; PowerShell parse plus the deterministic injected-operation rollback matrix are authoritative for this checkout.

## 2026-08-22

- The Task 17 remote-only revision defect is resolved and locked by a fake ref-availability gate plus fetch-failure receipt. Native Scheduled Task/update/audio behavior remains unclaimed because the canonical task is absent and host mutation was not accepted.
2026-08-22 Todo 20: External prerequisites still block truthful acceptance evidence: the approved peer does not receive a live Ack/calibration reply, and the available Windows loopback capture tooling is absent. Do not rebuild/restart the remote service or claim raw/conservative P50/P95/P99 values until deployment is refreshed and the real VB-CABLE loopback can be captured.
2026-08-22 Todo 20: Rust `lsp_diagnostics` was requested for every changed Rust file but the daemon timed out (first request after 30 s, remaining requests MCP timeout). Cargo test/build/Clippy passed and are the authoritative verification gates.
2026-08-22 Todo 20 correction: The previous implementation's 502-byte timestamped audio packet and pre-read timestamp claim were invalid against Todo 3. Both were removed. With the fixed 494-byte wire contract, no live raw/conservative latency statistics can be computed until a separately approved timestamp/detection mechanism exists; Todo 20 remains blocked by this limitation and the stale remote service.
2026-08-22 Todo 20 follow-up verification: `git diff --check`, formatting, workspace tests, build, and Clippy passed. Rust-LSP timed out again for all changed Rust files. No files were staged, and no commit or push was performed.

## 2026-08-22 Todo 21

- The real lifecycle cannot be verified on this host because the canonical `\wifimic\wifimic-client` task and `C:\Program Files\wifimic-client` installation are absent, and no exact `wifimic_client.exe` process is running. The active remote listener alone is insufficient evidence for streaming, Idle timing, process exit, logon restart, or Restart session rotation.
- The required corrective mutation would be unauthorized under this task and would require an explicit reversible cleanup plan. Do not install/register the Windows artifacts or rebuild/restart the remote ELF merely to manufacture a Task 21 pass.

## 2026-08-22 Todo 22

- The real network-interruption scenario cannot start because there is no installed/running Windows client and the active server deployment is stale relative to the current control-plane loop. Do not install/register the client or rebuild/restart the remote ELF merely to manufacture a reconnect pass.
- Without a live pre-interruption Ack/session, it is impossible to truthfully assert a 15-second interruption, process survival, restoration within 20 seconds, a strictly-greater resumed `session_id`, server supersession without capture restart, or post-interruption revert. The task remains blocked rather than simulated.

## 2026-08-22 Todo 24

- No truthful live timeout evidence can be produced while the exact Windows client/task artifacts
  are absent and the active Linux ELF remains the stale deployment identified by Todo 19. Do not
  install/register the client, kill a substitute process, rebuild/restart the service, or claim a
  30–35 second transition from Todo 6's fake-clock tests.
