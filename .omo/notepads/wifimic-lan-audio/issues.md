# Issues — wifimic-lan-audio

Problems and gotchas encountered during work on this plan.

_Auto-scaffolded by /start-work. Append new entries below - never overwrite._

---

## 2026-08-22T08:45:14.8453280Z

- Todo 8 initially had a truthful `203/EXEC` blocker because `~/.local/bin/wifimic_server` was absent. The pushed repository contained a buildable real server; it was built on `arch-daniel` from a credential-free `origin/main` archive and installed, so the blocker is resolved without a fake executable.

## 2026-08-22

- Rust LSP diagnostics timed out on the six changed server Rust files, matching the inherited workspace limitation; focused tests, workspace build/tests, and workspace Clippy were authoritative and all passed.

## 2026-08-22

- Todo 19 live Ack acceptance is blocked by the active remote ELF being older than the current server control-loop commits. The real Start packet reaches the UFW peer accept rule, but the stale listener returns no Ack within the bounded 5-second receive window. Rebuild/install/restart is intentionally not included because this task forbids changing service state; rerun the ignored harness after a normal deployment refresh.

## 2026-08-22

- Rust LSP diagnostics timed out on the changed client files, matching the inherited workspace limitation; Cargo formatting, tests, build, and Clippy remain the authoritative checks for Todo 13.

## 2026-08-22

- Task 16 was verified with a faithful isolated fake-command harness rather than `arch-daniel`: the Windows checkout is intentionally dirty from orchestration files, and the real service/control smoke requires an approved source at peer `192.168.0.200`; no live update or end-to-end capture claim is made here.

## 2026-08-22

- Verification found and fixed the updater's unsafe localhost smoke default. Missing helper now fails before systemctl discovery/source status; live peer-originated helper execution remains unavailable in this environment.

## 2026-08-22

- Task 12's Rust LSP requests timed out on the client files as expected from the inherited workspace limitation. Cargo focused tests, workspace tests/build, and workspace Clippy all passed; no live hardware claim was substituted.

## 2026-08-22

- Todo 19 live Ack acceptance is blocked by the active remote ELF being older than the current server control-loop commits. The real Start packet reaches the UFW peer accept rule, but the stale listener returns no Ack within the bounded 5-second receive window. Rebuild/install/restart is intentionally not included because this task forbids changing service state; rerun the ignored harness after a normal deployment refresh.
- Task 15's live installation was intentionally not attempted. The script requires the explicit `-AcceptHostMutation` switch plus an elevated interactive Windows session before it can register the task, change the firewall, or copy into Program Files. Read-only inspection found the canonical task and firewall rule absent; deterministic fake tests cover the mutation and rollback paths instead.

## 2026-08-22

- Task 17's live updater run was intentionally not attempted. Read-only inspection found the exact VB-CABLE Input endpoint but no canonical `\wifimic\wifimic-client` task; no Program Files, Scheduled Task, process, or firewall mutation was authorized.

## 2026-08-22

- Task 17 had a real ordering defect: `ResolveRevision` ran before `FetchTags`, so a valid remote-only tag could not be resolved. The updater now keeps dirty-checkout rejection first, fetches within the bounded native operation, then resolves exactly one commit; no live update was attempted.
2026-08-22 Todo 20: Live Start/Ack and `wifimic_client --calibrate` both timed out with Windows error 10060. SSH, the active remote service, `parec`, pinned PipeWire source, peer IP, VB-CABLE endpoint, and UFW scope were present; the remote ELF remains stale relative to the current control loop. Five-minute normal and 60-second saturated runs were not started.
2026-08-22 Todo 20 follow-up: Focused tests now prove stale calibration sequence rejection. The corrected implementation preserves the 494-byte Todo 3 audio contract; the required per-frame cross-host capture timestamp is intentionally unresolved rather than transported through the product wire format.

## 2026-08-22 Todo 21

- Live tray Exit/Restart acceptance is blocked before mutation: the exact canonical Scheduled Task `\wifimic\wifimic-client`, `C:\Program Files\wifimic-client`, and both exact client process identities were absent in the timestamped PowerShell preflight. The active Linux service/listener was present, but no real two-host session could be started.
- No task registration, executable install, firewall change, logout, tray interaction, or remote rebuild/restart was attempted. Task 21 remains unchecked; see `.omo/evidence/task-21-wifimic-lan-audio.log` and its paired failure log.

## 2026-08-22 Todo 23

- The real Linux capture-retry scenario is blocked at its required live-heartbeating-client gate. The fresh local probe found no canonical Windows client task, install, process, or UDP 6902 endpoint; the remote service was active but its journal had zero heartbeat and `CaptureRetry` records.
- The source fault was not injected because doing so without a genuine client would violate the task contract. No remote service restart/rebuild/reinstall or PipeWire mutation was performed; see the paired Task 23 evidence logs.

## 2026-08-22 Todo 22

- Automatic reconnect acceptance is blocked before mutation: `\\wifimic\\wifimic-client`, `C:\\Program Files\\wifimic-client`, both exact client process spellings, and both `Get-Command` lookups were absent. The only client executable found was the uninstalled build artifact `target\\debug\\wifimic_client.exe`.
- The active remote listener remains the stale ELF from Todo 19; the prior real Start/Ack attempt timed out with Windows error 10060. No live session existed, so the exact 15-second interruption and 20-second resume assertion were not attempted.
- Wi-Fi was verified `Up`, local UDP 6902 had no endpoint, and no temporary adapter/firewall mutation was performed or left behind. See both Task 22 evidence files.

## 2026-08-22 Todo 24

- The real 30-second heartbeat-timeout acceptance is blocked at read-only preflight: the canonical
  `\\wifimic\\wifimic-client` task, `C:\\Program Files\\wifimic-client` install, and both exact
  `wifimic_client.exe` process identities were absent at
  `2026-08-22T21:29:04.4467076+08:00`.
- The remote service/listener was active at `2026-08-22T21:29:05,152186091+08:00`, but no live
  stream existed to terminate. Consequently the required last-heartbeat-to-Idle elapsed time,
  capture stop, fresh-session recovery, and server survival during the scenario are all unobserved
  and remain FAIL rather than simulated.

## 2026-08-22 Task Scheduler round-trip fix

- The first real Windows installer run exposed that Task Scheduler drops an empty `<Arguments />` element when `Export-ScheduledTask` round-trips the registered XML. `ConvertTo-WifimicTaskDefinition` previously treated that valid omission as malformed XML and aborted immediately after `schtasks.exe /Create` succeeded. The installer now uses an Arguments-only optional XML lookup and resolves a missing node to `''`; all mandatory task nodes retain strict missing-node validation.

## 2026-08-22 Native cleanup follow-up

- The same failed native install left the copied executable in its private `.wifimic-stage-<guid>` directory because cleanup used `RemoveDirectoryIfEmpty` even though the stage directory is intentionally non-empty. The installer now uses a dedicated recursive `RemoveDirectory` operation for stage cleanup while preserving `RemoveDirectoryIfEmpty` for the install root. The two fixes together cover the live failure: Task Scheduler's valid empty-Arguments omission is accepted, and private staging state is removed on every completion or rollback path.

## 2026-08-23 FirewallContractMismatch

- The user's elevated install reached `SetFirewall`, then failed at `deploy/windows/install-wifimic-client.ps1:516-518` with `wifimic-client installer failed: [FirewallContractMismatch] The installed firewall rule did not match the exact peer-scoped UDP contract.` This proves rule creation returned before read-back verification rejected the signature.
- The expected contract remained Name/DisplayName `wifimic-client`, UDP, local port `6902`, remote peer `192.168.0.210/32`, Profile `Any`, inbound, Allow. The old comparison required literal equality for the remote-address string.
- Root cause: Windows can read back the semantically equivalent single-host peer as `192.168.0.210` without the `/32` suffix. The non-elevated diagnostic shell reported `IS_ADMIN=False`; its throwaway `New-NetFirewallRule` probe stopped with Windows System Error 5 before creation, so no native filter fields are claimed from this shell. The user's elevated red transcript remains the native evidence.
- Fix: `Test-WifimicFirewallSignature` now uses a pure address-list normalizer that trims comma-separated/array values, canonicalizes only IPv4 single hosts and explicit `/32` values to one `/32` token, and requires exactly one normalized peer. Any, LocalSubnet, broader CIDRs, ranges, extra addresses, wrong peers, and wrong identity/protocol/port/direction/action remain rejected.
- Deterministic regression evidence: the old comparator exited `1` after rejecting `192.168.0.210`; the fixed helper exited `0` while accepting the equivalent scalar/array representation and rejecting all broad/extra/incorrect address and contract cases. PowerShell parser, DryRun, and all six TestMode rollback points also passed with private roots removed.
- The exact elevated native install command was not rerun because the available shell was not administrator-elevated; canonical task, firewall rule, and install root were absent after the attempt.

## 2026-08-23 Todo 22 rerun

- The canonical task/install and refreshed Linux listener were present, but the current PowerShell token reported `IS_ADMIN=False` at `2026-08-23T01:54:50.5519820+08:00`. A scoped temporary UDP 6902 firewall block could not be safely created without elevation, so the autonomous reconnect run stopped before starting the task or mutating host state.
- The no-op postcondition remained clean: task `Ready`/enabled, client absent, canonical inbound UDP Allow rule unchanged for peer `192.168.0.210`, temporary rule count `0`, Linux service PID `35138` active, listener active, exact `parec` absent, and pinned source `SUSPENDED`. Todo 22 remains blocked with no session IDs or interruption timing.

## 2026-08-23 Linux deployment refresh

- `arch-daniel` had no persistent `wifimic` checkout under the searched home/common roots; the deployable source was staged temporarily from committed `origin/main` revision `ad65dfda2d6baa4c872c8ed75a9020fc45f97a44` (ancestor `b0d17cb` control-loop wiring), without local uncommitted files. The temporary checkout/build path was `/tmp/wifimic-server-deploy-20260823-011023/source`.
- Pinned Rust `1.97.1` built `wifimic_server` successfully. Prior binary: SHA-256 `0f7b8036f0e75fe190a30caa11fae51f4cc8c567f18d271572ca4f60c5eab221`, mtime `2026-08-22 16:44:50.539301747 +0800`; installed binary: SHA-256 `b6483703b21637946dc1590a2cb9ccf558957e04116e4428cb9a74a12661f162`, mtime `2026-08-23 01:11:59.056140684 +0800`.
- Only `wifimic-server` was restarted. At `2026-08-23T01:17:50+08:00`, `systemctl --user is-active` was `active`, UDP `0.0.0.0:6902` was listening, exact `parec` was absent after Stop, and the pinned source `alsa_input.pci-0000_00_1b.0.analog-stereo` remained the only matching source and was `SUSPENDED`.
- The real Windows peer `192.168.0.200` completed Start Ack (`0x01`), Heartbeat Ack (`0x02`, one interleaved valid audio frame handled), Stop Ack (`0x03`), and post-Stop Heartbeat `NoAck` at `2026-08-23T01:16:46.8267027+08:00`–`01:16:48.1699321+08:00`; the streaming process command used the pinned source with no fallback. No latency or Tasks 20–24 claim is made.
- The existing ignored repository helper reached Start Ack but failed its cleanup receive with Windows error 10040 because its 64-byte buffer cannot consume the legitimate 494-byte audio frame before Stop Ack. It was not weakened or modified; the interleaving-aware real PowerShell probe is the deployment evidence. UFW remained peer-scoped (`ufw.service=active`, other firewall services inactive); no firewall or Windows install state changed.

## 2026-08-23 Todo 21 live rerun

- The canonical installation and task are now present: `schtasks.exe /Run /TN '\\wifimic\\wifimic-client'` launched `C:\Program Files\wifimic-client\wifimic_client.exe`, and the refreshed Linux service produced the pinned `parec` capture process. This is real task/capture evidence, not a mock.
- Native tray acceptance remains blocked by the Windows session: UI Automation exposed no `wifimic-client` element, PyWinAuto/PyWin32 were unavailable, `SetCursorPos` returned `False` with Win32 error 203, and `SendInput(Win+B)` returned 0. No real Restart or Exit action was claimed.
- The installed client's live diagnostics files contained only the metadata header, so they did not provide Start/Heartbeat/Stop Ack or session-ID records. `pktmon` could not be enabled because its filter operations returned access denied. Cleanup Stop Acks were recorded separately as cleanup evidence only.
- A precision canonical-task retrigger observed Linux capture at `2026-08-23T01:43:54,217706963+08:00`, less than one second after the `01:43:53.2370838+08:00` task trigger. This proves task-triggered capture recovery, but not post-tray-Exit logon semantics.
- Final cleanup left the canonical task `Ready`/enabled, no exact client process, Linux service active, listener on UDP 6902 active, exact `parec` absent, and the pinned source `SUSPENDED`.

## 2026-08-23 Todo 23 live retry attempt

- A real canonical task/client/server session was established at `2026-08-23T02:01:21.3615817+08:00`: task `Running`, client PID `33092`, client UDP `0.0.0.0:6902`, Linux service PID `35138`, and exact pinned `parec` PID `35533`. The capture process remained alive beyond two minutes, which operationally proves heartbeats continued; session ID and packet-level heartbeat timestamps were not observable because production diagnostics/journald were empty and no packet-capture utility was installed.
- The only least-invasive reversible source mutation attempted was `pactl suspend-source alsa_input.pci-0000_00_1b.0.analog-stereo 1` at `2026-08-23T02:04:25.1104952+08:00`. It returned `0`, but the active source stayed `RUNNING`, exact `parec` stayed alive, and the server PID stayed stable. Restore with `pactl suspend-source ... 0` also returned `0` and left the source/process unchanged.
- Todo 23 is blocked by the current capture boundary, not by the client gate: `CaptureHandle::start` only spawns `parec`, so a missing source cannot yield its typed start error. Endpoint absence is detected after spawn by `read_frame`, outside the `ControlState::Starting` retry path; forcing source/card removal would risk the read-error/server-exit path rather than prove the required 5-second retries. No card-profile removal or other invasive mutation was attempted.
- No `CaptureRetry` event, two retry timestamps, or automatic-resumption timestamp was observed. No alternate source was selected; the exact pinned `--device` argument remained in every process observation. Cleanup returned task `Ready`/enabled, client absent, service active at PID `35138`, no `parec`, source `SUSPENDED`, mute `no`, 60% volume, and the same default source as baseline. See the appended 2026-08-23 sections in both Task 23 evidence logs.

## 2026-08-23 Todo 24 live acceptance

- The canonical task and installed client were exercised for real. The first session ran as exact
  Program Files PID `31228` with UDP `0.0.0.0:6902`; Linux ran pinned `parec` PID `35992` under
  unchanged server MainPID `35138`. Only `Stop-Process` against that exact executable was used
  for the crash-like timeout trigger at `2026-08-23T02:14:28.2578729+08:00`; no tray Exit or
  explicit Stop was used beforehand.
- The capture process disappeared at the first no-`parec` sample
  `2026-08-23T02:14:57,711149956+08:00`, with the pinned source IDLE and then SUSPENDED. The
  server remained active at PID `35138` with `NRestarts=0`. The literal kill-to-capture-stop
  interval was `29.453277056` seconds, but the production surfaces exposed neither packet-level
  heartbeat timestamps nor an explicit `ControlState::Idle` record. Todo 24 therefore remains
  blocked on the truthful 30–35-second last-heartbeat-to-Idle measurement rather than claiming
  an inferred interval.
- A fresh canonical task session recovered normally at `2026-08-23T02:16:46.7887126+08:00` as
  client PID `34604`, UDP 6902, pinned `parec` PID `36366`, and source `RUNNING`. A `finally`
  cleanup guard ended the task and used the exact-client `Stop-Process` fallback; final state was
  task `Ready`/enabled, client and UDP endpoint absent, service active at unchanged PID `35138`,
  no `parec`, pinned source SUSPENDED, and default source unchanged. No temporary firewall rule,
  source mutation, deployment change, or artifact was created.

## 2026-08-23 F3 independent rerun

- The strict F3 rerun performed only read-only host checks after comparing the original `task-20` through `task-24` evidence with the 2026-08-23 live attempts. All five required live QA verdicts remain **BLOCKED**: Todo 20 has no real five-minute normal latency run, conservative P95, or 60-second saturated characterization; Todo 21 has no actual tray Restart or Exit action; Todo 22 has no exact elevated 15-second interruption or fresh-session recovery within 20 seconds; Todo 23 has no typed `CaptureRetry` cadence/resumption while heartbeats remain valid because source suspension did not make the source unavailable; Todo 24 has no direct last-heartbeat-to-Idle measurement in the 30–35-second window, and the `29.453277056`-second kill-to-stop proxy is insufficient.
- Current read-only receipt at `2026-08-23T02:26:25+08:00` confirms cleanup: canonical task `Ready`, client process and local UDP 6902 absent, Linux service active at stable PID `35138`, listener present, exact `parec` absent, pinned source `SUSPENDED`, UFW peer allow/default deny intact, and no temporary firewall rule or other scenario residue. Persistent installed artifacts/rules are expected deployment state, not a mutation from this rerun.
- Final F3 remains **REJECT**. Cargo/unit tests and task-trigger/capture observations remain implementation or partial-live proof only and are not promoted to live acceptance passes.

## 2026-08-23 Todo 22 elevated retry

- The required `Start-Process powershell.exe -Verb RunAs -Wait -ArgumentList ...` helper was actually attempted. The full helper invocation returned `System.InvalidOperationException` before creating its transcript; a second minimal RunAs probe hung for 120 seconds with no child output while unattended UAC consent remained pending.
- No elevated task start, real UDP probe, temporary firewall block, packet capture, Start Ack/session ID, interruption, or recovery was performed after the elevation blocker. No temporary rule or transcript/helper remained.
- A pre-existing canonical task/client session observed before this retry was ended without tray interaction. Final cleanup confirmed task `Ready`/enabled, client absent, server PID `35138` active, exact `parec` absent, and the pinned source `SUSPENDED`. Todo 22 remains blocked by unattended UAC consent, not by the default token check alone.

## 2026-08-23 Final F1 refresh

- The current F1 crosswalk contains exactly 28 rows: 22 literal Scope Must-have bullets and 6 literal Must-NOT-have bullets. It reconciles the prior stale pre-install evidence with the current native Windows install (`Ready` task, Program Files executable, exact VB-CABLE endpoint, peer-scoped firewall) and the refreshed Linux real Start/Heartbeat/Stop Ack smoke with post-Stop `NoAck`.
- F1 remains rejected because blocked live acceptance cannot be promoted from implementation evidence: Todo 20 lacks the approved VB-CABLE latency loopback and all required percentiles; Todo 21 lacks native tray automation; Todo 22 lacks an elevated temporary network interruption; Todo 23's reversible source suspension did not induce a capture-start failure; and Todo 24 lacks direct heartbeat/Idle timestamps (the observed `29.453277056`-second kill-to-capture-stop interval is not the required 30–35-second last-heartbeat interval).
- F2 and F4 remain approved independently. The exact current blockers are recorded as F1 rows M5, M6, M11–M14, M19, and M20; all six Must-NOT-have guardrails mechanically pass, including no encryption, web UI, Session-0 service, fallback target, or automatic update.

## 2026-08-23 Native latency diagnostic

- Added the self-contained invocation `wifimic_client.exe --diagnose-latency --duration-secs 300`; the duration is optional and defaults to 300 seconds, so `--duration-secs 60` is the saturated-Wi-Fi characterization form.
- Parseable output uses `latency_sample sequence=<u32> raw_latency_us=<u64>` and one final `latency_stats raw_p50_us=<u64> raw_p95_us=<u64> raw_p99_us=<u64> conservative_p95_us=<u64> conservative_p95_margin_us=25000` line.
- The capture implementation selects the exact `CABLE Output (VB-Audio Virtual Cable)` friendly name in `Direction::Capture`, with no default-device fallback, and reuses `CABLE Input (VB-Audio Virtual Cable)` for rendering. Real hardware execution was intentionally not attempted in this implementation task, so no new live endpoint enumeration result is claimed.
- The existing 494-byte audio frame contract is unchanged. Because that contract carries no Linux capture-acquisition timestamp, the diagnostic's calibrated anchor reports the native Windows VB-CABLE loopback leg; an exact server-capture-to-Windows-onset attribution still requires a separately approved timestamp transport mechanism and is not claimed here.

## 2026-08-23 Server production wiring defects

- `apps/wifimic_server/src/main.rs:20-30` constructed `EventContext::logging` without registering a `log` backend, so `wifimic_diagnostics::WifimicLogSink`'s `log!` calls were silently discarded even though the systemd unit already routed stderr to journald. `env_logger` is now initialized with an `info` default before the context is constructed, while preserving `RUST_LOG` overrides.
- `apps/wifimic_server/src/control.rs:129-148` previously converted a mid-stream `read_frame` failure into `ControlError::CaptureRead`; `main.rs:81-85` then propagated it and terminated the service. The read boundary now reuses `schedule_capture_retry` at `control.rs:262-284`, emits the existing typed `CaptureRetry`, changes only `Streaming` to `Starting`, and schedules the unchanged 5-second retry without clearing session or heartbeat metadata.
- `apps/wifimic_server/src/control_tests.rs:124-202` adds deterministic streaming-failure coverage for swallowed retry transition, same-session retry recovery, and heartbeat preservation through the failure cycle.

## 2026-08-23 Todo 21 second retry — deeper tray automation

- Three escalating native automation methods exercised against the real installed client (PID 23628/24968) and canonical task:
  1. **UIA overflow flyout + icon Invoke**: Chevron ("Show hidden icons") invoked successfully; flyout `TopLevelWindowForOverflowXamlIsland` appeared; `wifimic-client` icon found (AutomationId=NotifyItemIcon, Rect=3301,1882,60,60); `InvokePattern.Invoke()` succeeded — **but no context menu appeared; Restart/Exit menu items not found**. The Invoke is a left-click; the `tray-icon`/`muda` crate expects a right-click for context menu.
  2. **Shell_NotifyIconGetRect + SendInput right-click**: `EnumWindows`/`EnumThreadWindows` for client PID returned **zero windows** — the crate's hidden message-only window (parent `HWND_MESSAGE`) is not enumerable by standard APIs. No HWND to pass to `Shell_NotifyIconGetRect`.
  3. **WM_CONTEXTMENU/WM_RBUTTONDOWN via PostMessage**: Same enumeration failure — no candidate window to post messages to.
- Remote AF_PACKET capture (65,535-byte buffer) ran concurrently with all attempts: continuous audio frames (494 bytes, TAG=0x00) with session IDs `1787424119882` and `1787424682907`; no Restart/Exit control packets correlated with automation.
- **Root cause**: The `tray-icon` crate's message-only window is invisible to `EnumWindows`; UIA `InvokePattern` on the overflow icon performs a left-click, not the right-click the crate requires for context menu. Without a native right-click at the exact icon rectangle (from `Shell_NotifyIconGetRect`) or an IPC test seam, the real Restart/Exit cannot be reached.
- All temporary scripts (`.debug-task21-*.ps1`) removed; no injected assemblies, helper processes, or packet filters remain. Final state clean: task `Ready`/enabled, no client process, Linux capture `SUSPENDED`.
- Todo 21 remains **UNCHECKED** — no verifiable native tray Restart or Exit click achieved.

## 2026-08-23 Server wiring line-reference correction

- After the F2 LOC-safe extraction, the retry/read implementation is at `apps/wifimic_server/src/control_support.rs:96-146`, and the two focused regressions are at `apps/wifimic_server/src/control_capture_retry_tests.rs:14-80`; the earlier entry's pre-extraction ranges remain historical evidence only.

## 2026-08-23 Todo 23/24 redeploy rerun

- `wifimic_server` was rebuilt on `arch-daniel` from exact `origin/main` descendant `d1496fd5e6cee94173f437e7e3ae0db3e9261747` (`a98a173` ancestor), installed as SHA-256 `243b2a3072146bcf9d776be607f661cd122101cd5197d72499baaa6888b58628`, and restarted with UDP 6902 listening. Typed diagnostics became visible in journald, including `SessionStarted`, `CaptureRetry`, `HeartbeatTimeout`, and `SessionStopped`.
- Todo 23's safest profile-off attempt removed the source from the list but left the existing `parec` stream alive. Destroying only the pinned PipeWire source node and its dangling exact `parec` stream produced real `CaptureRetry` records at approximately five-second cadence while server PID `37703` survived and no alternate source appeared. The canonical Windows client then exited at the failure boundary; journald recorded `heartbeat ... inactive_session`, so the required heartbeat-valid automatic resumption was not observed. Todo 23 remains **BLOCKED/FAIL**, not promoted from the partial retry proof.
- Todo 24's final canonical run killed only Program Files PID `32884`. Journald recorded `HeartbeatTimeout elapsed_since_heartbeat_ms=30001` at `03:45:40.956075` followed by `SessionStopped reason=heartbeat_timeout` at `03:45:40.957112`. The authoritative interval is `30.001 s`; server PID `38464` survived, a fresh canonical session recovered, and final cleanup matched the exact audio/task baseline. Todo 24 is **PASS**.

## 2026-08-23 Todo 20 native latency diagnostic acceptance

- Built the clean detached `origin/main` worktree at exact revision `d1496fd5e6cee94173f437e7e3ae0db3e9261747` with `cargo build --release -p wifimic_client`; no source, canonical install, Scheduled Task, or firewall mutation was made.
- The refreshed `arch-daniel` peer was reachable and `wifimic-server` was active with UDP 6902 listening. A shared-host coordination issue initially exposed other canonical installed-client launches and orphaned `parec` children; those exact processes were stopped/cleaned before the authoritative runs without touching the task definition.
- The authoritative normal run was the full `--duration-secs 300` continuous run from `2026-08-23T03:32:01+08:00` to `03:37:02+08:00`. It produced 300 literal `latency_sample` lines and `latency_stats raw_p50_us=206593 raw_p95_us=213773 raw_p99_us=215458 conservative_p95_us=238773 conservative_p95_margin_us=25000`. The exact gate `conservative_p95_us <= 200000` is **FAIL** (`238773 > 200000`); no threshold adjustment or rounding was used.
- The first saturated attempt deliberately loaded the link before calibration and failed with the exact tool error `RoundTripTooLong { round_trip_us: 30940, maximum_us: 20000 }` after samples at 19,231 us and 18,970 us. This was recorded as an environment/load interaction, not silently retried.
- The authoritative saturated characterization started the 4 GiB SSH `/dev/urandom` transfer after the diagnostic session was ready, ran the 60-second diagnostic to completion, and produced 60 samples plus `latency_stats raw_p50_us=209411 raw_p95_us=215553 raw_p99_us=216118 conservative_p95_us=240553 conservative_p95_margin_us=25000`. It is descriptive only and is not held to the normal 200 ms gate.
- The saturated output cannot prove “zero sustained underrun bursts longer than one second” because this native diagnostic does not expose `wifimic_diagnostics` jitter-buffer event counts.
- Methodology remains a deliberate scope caveat: calibrated WASAPI render-to-WASAPI capture loopback translated to Linux time, not a literal Linux `parec`/`CaptureAdapter` acquisition timestamp to Windows render-boundary measurement, because the immutable 494-byte wire contract carries no Linux capture timestamp. F1/F3 must judge whether this is a defensible M19 Scope interpretation or partial/characterization evidence.
- Final cleanup at `2026-08-23T03:43:13+08:00`: no temporary or installed client, no local UDP 6902 endpoint, no transfer SSH process, and no remote `parec`; Linux service remained active at PID `38464`, UDP 6902 remained listening, the pinned source was `SUSPENDED`, and the journal recorded explicit `session_stopped` events for both authoritative runs. Full literal outputs are appended to both Task 20 evidence logs.

## 2026-08-23 Corrected Todo 20 diagnostic path

- The previous diagnostic methodology was invalid because `apps/wifimic_client/src/latency_diagnostic_windows.rs` rendered `deterministic_tone_frame` directly through `control.renderer()`, measuring only local WASAPI render-to-VB-CABLE loopback capture. It never traversed the server capture boundary, UDP transport, client jitter buffer, or normal receive/render scheduling, so the recorded 206–215 ms values were not Wi-Fi/protocol latency.
- `apps/wifimic_server/src/main.rs:29-38,40-105` now selects a synthetic source only for `--diagnose-latency` and sends it through the unchanged `ControlPlane::next_audio_frame` and UDP loop. `apps/wifimic_server/src/diagnostic_capture.rs:40-100` implements the existing `CaptureController` seam without constructing `CaptureHandle` or spawning `parec`; it emits `wifimic_protocol::latency::deterministic_tone_frame(sequence)` on the derived 5 ms cadence, timestamps the real acquisition `Instant`, and prints `latency_capture sequence=... acquired_at_unix_us=...`.
- `apps/wifimic_server/src/control_support.rs:20-26,103-123` forwards the assigned wire sequence to the capture substitute solely for diagnostic correlation; the production `CaptureHandle` implementation keeps the default no-op hook and its spawn/read path is unchanged.
- `apps/wifimic_client/src/latency_diagnostic_windows.rs:84-131,151-169` now establishes the normal session, calls `receive_once`, `advance`, and `render_ready`, and observes the sequence only after the real renderer seam succeeds. `measure_tone` at `:172-211` waits for the tone arriving through UDP/jitter/render, then prints `latency_onset sequence=... client_onset_us=... server_onset_us=... clock_offset_us=...`; it no longer synthesizes or locally renders test frames. The translation helper is `apps/wifimic_client/src/latency_diagnostic.rs:81-84`.
- The external correlation is now truthful and wire-compatible: for matching sequence numbers, `raw_latency_us = latency_onset.server_onset_us - latency_capture.acquired_at_unix_us`. The immutable 494-byte `AudioFrame` remains unchanged; no capture timestamp crosses the product protocol. `--diagnose-latency` on the Linux server must run before `wifimic_client.exe --diagnose-latency --duration-secs 300` (or `60` for the later saturated run).

## 2026-08-23 Todo 20 real capture-boundary correction

- Replaced the diagnostic-only synthetic capture source with a thin `LatencyDiagnosticCapture` wrapper around the existing `CaptureHandle`; `--diagnose-latency` therefore starts the unchanged pinned `parec --device=alsa_input.pci-0000_00_1b.0.analog-stereo` path and continues through the existing ControlPlane, UDP, jitter-buffer, render, and VB-CABLE capture path.
- `CapturedFrame` now exposes `acquired_at_unix_us`, sampled immediately after the exact 480-byte stdout frame is produced by `parec`, before ControlPlane or UDP send. The diagnostic wrapper pairs that timestamp with the sequence assigned by `CaptureController::set_sequence` and emits `latency_capture` records without changing the 494-byte audio datagram.
- Deterministic capture and diagnostic tests cover the injected read-boundary timestamp and sequence/timestamp correlation; they are not live two-host evidence. No live tone routing, normal five-minute run, or saturated 60-second characterization was attempted here, so Todo 20 remains pending live acceptance.

## 2026-08-23T04:53:00+08:00 Corrected Todo 20 live redeploy/run attempt
- Rebuilt `wifimic_server` on `arch-daniel` from clean detached checkout HEAD `1d7de3eed653086efef5bff2896b79316cb57221` (exact requested commit), after confirming `git -C <checkout> rev-parse HEAD`; installed SHA-256 `dffb24117ba9167d36ebd5e01cb396fcacb634fb194b620f534deb6a418e7c26`, mtime `2026-08-23 04:42:43.995428402 +0800`, and restarted only the normal `wifimic-server` unit. The fresh Windows client was built from exact `1d7de3e` because the canonical Program Files binary hash `0FFE17E96009ADF61C3C840B38D2482FC8BA390B79AC2950EF2595E6B997093D` was stale; fresh hash was `B59752EDD8D13415C5531C760C64F62F5A0701F05C0C8AF9D1607D5DC8403204`.
- The corrected normal invocation `--diagnose-latency --duration-secs 300` completed calibration and Start/session establishment but failed on the first real-audio measurement with `Windows(ToneOnsetTimeout { timeout_ms: 250 })`; it produced zero `latency_onset` records and therefore no truthful raw percentiles. Linux nevertheless emitted 5994 `latency_capture` records from real `parec`, sequences `0..5993`; the watcher observed PID `39335` with the exact pinned `--device=alsa_input.pci-0000_00_1b.0.analog-stereo` argument.
- The saturated invocation launched a concurrent 4 GiB `/dev/urandom` SSH transfer, but `--duration-secs 60` failed during calibration with `RoundTripTooLong { round_trip_us: 22492, maximum_us: 20000 }`; it emitted no onset/capture records and did not produce a 60-second characterization. Transfer/client/diagnostic processes were cleaned up.
- Correlation is blocked, not estimated: normal `0` client onset / `0` matched pairs; saturated `0` client onset / `0` matched pairs. No P50/P95/P99 or conservative P95 is claimed. Full raw attempted output is appended to the Task 20 evidence log; failure details and literal client errors are appended to its failure log.
- Correction: the final remote `sha256sum /home/daniel/.local/bin/wifimic_server` is `ddfb24117ba9167d36ebd5e01cb396fcacb634fb194b620f534deb6a418e7c26`; the earlier `dffb...` text was a transcription typo.

## 2026-08-23 Calibration probe retry

- The saturated Todo 20 run aborted when one of four calibration probes returned `RoundTripTooLong { round_trip_us: 22492, maximum_us: 20000 }`; `calibrate_socket` propagated the first bad sample instead of retrying that probe sequence.
- The fix extracts the per-probe exchange behind an injected transport/clock seam, retries only `RoundTripTooLong` with a fresh timestamp and probe send on each attempt, and bounds each sequence to the plan's 10 attempts. Exhaustion still returns the existing typed calibration error.
- Deterministic client tests cover recovery on a later good attempt and typed failure after 10 consecutive long round trips.

## 2026-08-23 Todo 20 corrected live redeploy and tone-injection follow-up

- `origin/main` resolved to exact commit `52085d99c9b5bd6ea39e907d154ae1fdf9b14045` (`fix(client): retry long calibration round trips`). A detached Linux bundle checkout verified `git rev-parse HEAD` at that exact commit and built `wifimic_server` with Rust `1.97.1`; the installed `/home/daniel/.local/bin/wifimic_server` is SHA-256 `ddfb24117ba9167d36ebd5e01cb396fcacb634fb194b620f534deb6a418e7c26`, mtime `2026-08-23 05:16:06.814829502 +0800`. The Windows Program Files binary was stale (`0FFE17E...997093D`), so it was rebuilt from the same exact commit and installed as SHA-256 `5DEEAB7D57FA709314BED965D28DB85E41C07640C5E43BAC586EBB55500777BB`.
- Existing tooling was sufficient: `/usr/bin/ffmpeg`, `/usr/bin/pw-play`, `/usr/bin/pw-cat`, `/usr/bin/pw-link`, `/usr/bin/pw-record`, `/usr/bin/parec`, `/usr/bin/pw-cli`, and `/usr/bin/pw-dump` were present; no package was installed. The pinned node was inspected as PipeWire node 43 with `input ports: 0/0`, `output ports: 2/129`, and output ports `alsa_input.pci-0000_00_1b.0.analog-stereo:capture_FL/FR`. A direct `pw-link pw-play:output_FL alsa_input...:capture_FL` attempt returned `failed to link ports: No such file or directory` because both are output ports. The topology-valid graph injection used the live pinned-source `parec` stream's `parec:input_FL/FR` ports while preserving the original `alsa_input...:capture_FL/FR -> parec:input_FL/FR` links; `pw-link -l` showed both active links during injection, and an independent raw probe measured `mean_volume: -22.7 dB`.
- A real Start/Ack receiver bound to `192.168.0.200` while the injection was active received 1,594 494-byte audio frames for one session, with 1,593 non-silent frames and representative `max_abs` values approximately 3,500–7,400. This isolates the remaining failure to the Windows diagnostic receive/render/onset observation rather than Linux tone generation, pinned `parec`, or UDP payload silence.
- The authoritative normal invocations did not complete 300 seconds. The full literal client/server/tone output is appended to `.omo/evidence/task-20-wifimic-lan-audio.log`; attempts ended with `ToneOnsetTimeout` or `NoRenderedToneFrame`, so no full-run P50/P95/P99 or conservative P95 is claimed. A partial non-acceptance debug sample had only three matched pairs: raw P50 `573748 us`, raw P95 `1086731 us`, raw P99 `1086731 us`, conservative P95 `1111731 us`; these values are explicitly not an acceptance result.
- The valid saturated attempt did start a PowerShell background `ssh arch-daniel dd if=/dev/urandom bs=1M count=4096 status=none` transfer. Calibration completed with accepted round trips `19176`, `19540`, `19452`, and `19358 us`, demonstrating the retry fix under load, but the first measurement ended with `ToneOnsetTimeout` before 60 seconds. No saturated percentile or underrun claim is made. Failure details are appended to `.omo/evidence/task-20-wifimic-lan-audio-failure.log`.

## 2026-08-23 Windows latency diagnostic render/capture correlation fix

- Root cause of `NoRenderedToneFrame`: `run` called `service_control` immediately before `measure_tone` and discarded its `RenderOutcome::Audio` sequence. The first real frame could therefore reach `ControlPlane::render_ready` and the verified `SequenceRenderer` seam before the 250 ms measurement loop began; a later WASAPI loopback onset then found energy but saw `first_rendered_sequence = None`. The jitter buffer's first playout is intentionally delayed by its 40 ms floor and can adapt to 200 ms, while `render_ready` advances only one 5 ms slot per control poll.
- The capture side independently drains only packets currently available at `discard_available`; WASAPI may deliver a queued packet after that drain. The old code treated the first render observed in the same loop as the only evidence and could not distinguish a queued/stale onset from a frame rendered just before the loop.
- Fixed only the client diagnostic correlation path. `service_control` now returns a timestamped `RenderedSequence`; `run` carries the immediately preceding successful real render into `measure_tone`; and `RenderCorrelation` retains a bounded set of in-measure render events, selecting the newest real render at or before the detected loopback onset. If no real render event can explain the onset, the existing typed `NoRenderedToneFrame` error remains the precise result.
- Files changed: `apps/wifimic_client/src/latency_diagnostic.rs` (bounded correlation state plus deterministic tests) and `apps/wifimic_client/src/latency_diagnostic_windows.rs` (render evidence propagation and onset matching). The Linux `LatencyDiagnosticCapture`/pinned `CaptureHandle`/`parec` path, UDP receive/jitter/render path, WASAPI `CaptureStream`, 494-byte audio frame, and `latency_onset` output format are unchanged; no local synthetic renderer bypass was added.
- Focused regression coverage passes for render evidence carried across the measurement boundary, future render exclusion, and missing-render rejection. No live rerun was performed here; Todo 20 remains partial, and no live latency values or acceptance verdicts are claimed. The remaining `ToneOnsetTimeout` limitation requires the orchestrator's real PipeWire-injected/CABLE run to determine whether the external pulse falls outside the fixed 250 ms capture window after this correlation fix.
