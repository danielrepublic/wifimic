# Decisions — wifimic-lan-audio

Architectural choices and rationales discovered during work on this plan.

_Auto-scaffolded by /start-work. Append new entries below - never overwrite._

---

## 2026-08-22T07:35:44.8208649Z

- The wifimic wire contract uses a leading tag plus version: audio is `0x00 | version | big-endian u64 session | big-endian u32 wrapping sequence | 480-byte signed-16-bit little-endian PCM`; Start/Heartbeat/Stop are 10-byte tagged control messages and Ack is 11 bytes with the acknowledged control tag appended.
- The reference project's 484-byte PCM value is not carried forward: the named invariant test proves `48,000 Hz × 5 ms = 240 samples`, and `240 × 2 bytes/sample = 480 bytes`; no four-byte trailer is specified.
- Session acceptance is strict high-water ordering (`candidate > last accepted`), while the in-process generator uses the injected-clock formula `max(current_epoch_ms, last_issued + 1)` and returns a typed exhaustion error at `u64::MAX`.

## 2026-08-22T08:45:14.8453280Z

- Treat active `ufw.service` as the selected packet-filtering manager on `arch-daniel`; keep iptables/nftables services inactive and refuse any mixed-backend state. The final UDP 6902 rule remains exactly peer `192.168.0.200` allow plus anywhere deny.
- A real server binary may be staged from the pushed repository snapshot when the host toolchain can build it; use a temporary credential-free source archive, verify the resulting ELF and active user service, then remove all staging artifacts.

## 2026-08-22

- The production loop gives the UDP socket a bounded 100 ms read timeout so `ControlPlane::advance` can enforce liveness and retry timers without a separate timer thread; the control module itself owns only the named 30-second and 5-second intervals.

## 2026-08-22

- The updater does not change firewall, capture, or service-unit configuration during a successful update. It snapshots the existing user unit only so an interrupted or failed transaction can restore the exact prior config atomically before restarting `wifimic-server`.
- The default smoke uses the real fixed wire constants (version 1; Start `0x01`, Heartbeat `0x02`, Stop `0x03`, Ack `0x04`) and supports an explicit helper for a peer-originated probe when the server's source-IP gate makes a local probe impossible.

## 2026-08-22

- Remove the built-in localhost control probe from the updater. A peer-originated `WIFIMIC_CONTROL_SMOKE_HELPER` is mandatory, receives the explicit server host/port, is validated as an executable absolute path before mutation, is bounded, and must report the complete Ack exchange marker.

## 2026-08-22

- Fix the peer smoke destination to `192.168.0.210:6902` rather than allowing environment overrides; this prevents accidentally turning the truthful peer check back into localhost health.

## 2026-08-22

- Client reachability is a typed transition: Start Ack establishes the pending ID, matching Heartbeat Acks reset the missed counter, and two missed Heartbeat Acks enter Unreachable. A retry always mints a new ID with the shared monotonic generator; no rejected or unacknowledged ID is reused.
- The client trusts exactly IPv4 `192.168.0.210` for inbound Ack and audio datagrams, while accepting any source port from that IP. Filtering occurs before protocol state or jitter/render mutation.

## 2026-08-22

- Todo 13 uses the canonical resource ordinal `1`, labels `Restart`/`Exit`, and tooltip `wifimic-client`; no extra tray capabilities or service/IPC layer is introduced.
- Restart dispatches directly to `ControlPlane::restart(now, epoch_ms)` and never calls Stop first. Exit calls `ControlPlane::stop(now)` before marking the current process run for shutdown, including when Stop returns a typed error; the Scheduled Task remains outside this process's lifecycle.
- Task 15 uses an explicit `-AcceptHostMutation` safety gate for real installation; `-TestMode` and `-DryRun` select only injected fake operations. Existing canonical artifacts are updated only when their ownership signatures match, otherwise the installer refuses before mutation; owned prior artifacts are captured and restored on failure.

## 2026-08-22

- Task 17 keeps the installer isolated and changes no firewall state. It pins the canonical `C:\Program Files\wifimic-client\wifimic_client.exe` and `\wifimic\wifimic-client` identities, validates the exact VB-CABLE Input endpoint, and requires `-AcceptHostMutation` for native task/file operations.
- Revision resolution is performed after the clean-checkout gate and before tag fetching; the resolved commit, detached worktree, prior executable bytes/hash, prior task XML/enabled state, and cleanup roots belong to one rollback transaction.
