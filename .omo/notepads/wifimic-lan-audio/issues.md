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

- The first real Windows installer run exposed that Task Scheduler drops an empty `<Arguments />` element when `Export-ScheduledTask` round-trips the registered XML. `ConvertTo-WifimicTaskDefinition` previously treated that valid omission as malformed XML and aborted immediately after `schtasks.exe /Create` succeeded. The installer now uses an Arguments-only optional XML lookup and resolves a missing node to ''; all mandatory task nodes retain strict missing-node validation.

## 2026-08-22 Native cleanup follow-up

- The same failed native install left the copied executable in its private `.wifimic-stage-<guid>` directory because cleanup used `RemoveDirectoryIfEmpty` even though the stage directory is intentionally non-empty. The installer now uses a dedicated recursive `RemoveDirectory` operation for stage cleanup while preserving `RemoveDirectoryIfEmpty` for the install root. The two fixes together cover the live failure: Task Scheduler's valid empty-Arguments omission is accepted, and private staging state is removed on every completion or rollback path.
