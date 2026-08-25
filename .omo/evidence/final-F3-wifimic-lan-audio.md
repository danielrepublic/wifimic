# F3 — Real two-host QA final verification

**Run timestamp:** 2026-08-22, local `+08:00`  
**Method:** fresh read-only Windows PowerShell 5.1 and SSH preflight first; execute the
exact Todo 20–24 live scenarios only if the canonical Windows artifact gate and fresh
Linux deployment gate pass.  No cached output is used as a current result.

## Verdict

**VERDICT: REJECT**

All five live scenarios are **FAIL/BLOCKED at the mechanical preflight gate**.  The
required Windows task, install root, client process, and canonical firewall rule are
absent.  The Linux service is active, but the deployed ELF is stale relative to the
current control-loop source.  VB-CABLE endpoints exist, but that does not establish a
client session or provide a loopback capture tool.  No scenario mutation was authorized
or performed.

## Fresh read-only preflight

### Windows

The preflight queried the canonical Scheduled Task, canonical install root, both exact
client process spellings, local UDP 6902, canonical firewall DisplayName
`wifimic-client` plus its address/port filters, and present VB-CABLE devices.

```text
PREFLIGHT_LOCAL_TIMESTAMP=2026-08-22T23:30:05.8581905+08:00
WINDOWS_HOST=DANIEL-GIGABYTE
WINDOWS_IDENTITY=
daniel-gigabyte\daniel
TASK_\wifimic\wifimic-client=ABSENT
INSTALL_ROOT_EXISTS=False
INSTALL_ROOT=ABSENT
CLIENT_PROCESS_wifimic_client.exe=ABSENT
CLIENT_PROCESS_wifimic-client.exe=ABSENT
LOCAL_UDP_6902=
LOCAL_UDP_6902=NONE
WINDOWS_FIREWALL_RULE_wifimic-client=
FIREWALL_RULE_wifimic-client=ABSENT
VB_CABLE_DEVICES=

Status Class         FriendlyName
------ -----         ------------
OK     AudioEndpoint CABLE Output (VB-Audio Virtual Cable)
OK     AudioEndpoint CABLE Input (VB-Audio Virtual Cable)
OK     MEDIA         VB-Audio Virtual Cable
```

The local prerequisite check also found the known installer fix in the current source
checkout, but did not run the installer or register the canonical task:

```text
LOCAL_SOURCE_HEAD=11ca79c6e4c6d9dd702e8200e5ecfd1d09fbc492|2026-08-22T23:21:43+08:00|fix(deploy): tolerate Task Scheduler Arguments omission and always remove stage directory on cleanup
LOCAL_ARTIFACT=...\target\debug\wifimic_server.exe|2026-08-22T22:02:51.6372188+08:00|6483671|1F425713130B6936F7569D989CB60AC502B87DC687B43F449B79CC1E37741C59
LOCAL_ARTIFACT=...\target\debug\wifimic_client.exe|2026-08-22T22:02:56.8847426+08:00|28304264|EB216BAE928DC28FA283722A19C673733F294C62D55B9BE186B12DD5F2CA2E15
LOOPBACK_CAPTURE_TOOLS=
ffmpeg=ABSENT
ffplay=ABSENT
audacity=ABSENT
sox=ABSENT
pw-record=ABSENT
parec=ABSENT
```

The debug artifacts were not launched as substitutes.

### Linux (`arch-daniel`)

The read-only SSH probe inspected service state, listener, deployed ELF, process,
`parec`, pinned PipeWire source, active firewall backend/rules, and recent journal.

```text
PREFLIGHT_REMOTE_TIMESTAMP=2026-08-22T23:30:07,042208044+08:00
REMOTE_HOST=arch-daniel
SERVICE_ACTIVE=active
ActiveState=active
SubState=running
FragmentPath=/home/daniel/.config/systemd/user/wifimic-server.service
ActiveEnterTimestamp=Sat 2026-08-22 16:45:01 CST
MainPID=33601
LISTENER_6902=UNCONN 0 0 0.0.0.0:6902 0.0.0.0:* users:(("wifimic_server",pid=33601,fd=3))
BINARY_STAT=/home/daniel/.local/bin/wifimic_server|2026-08-22 16:44:50.539301747 +0800|450624
BINARY_SHA256=0f7b8036f0e75fe190a30caa11fae51f4cc8c567f18d271572ca4f60c5eab221  /home/daniel/.local/bin/wifimic_server
REMOTE_PROCESS=33601 /home/daniel/.local/bin/wifimic_server
PAREC=/usr/bin/parec
PINNED_SOURCE=57 alsa_input.pci-0000_00_1b.0.analog-stereo PipeWire s32le 2ch 48000Hz SUSPENDED
FIREWALL_BACKENDS=ufw.service=active; iptables.service=inactive; nftables.service=inactive
UFW_STATUS=active; default deny incoming; 6902/udp ALLOW IN 192.168.0.200; 6902/udp DENY IN Anywhere
```

Recent journal output contained service lifecycle records only; no live heartbeat or
`CaptureRetry` event was observed.  A second fresh deployment check recorded:

```text
FRESH_REMOTE_DEPLOYMENT_CHECK=2026-08-22T23:30:53,830784418+08:00
REMOTE_ELF=/home/daniel/.local/bin/wifimic_server|2026-08-22 16:44:50.539301747 +0800|450624
REMOTE_ELF_SHA256=0f7b8036f0e75fe190a30caa11fae51f4cc8c567f18d271572ca4f60c5eab221  /home/daniel/.local/bin/wifimic_server
REMOTE_SERVICE_PID=33601
CONTROL_SOURCE_HEAD=c5dee650d8f6d09a3371a4d1854f462a62506f44|2026-08-22T22:07:40+08:00|refactor(quality): name magic-number constants and preserve typed error sources (F2 findings)
```

The deployed ELF therefore fails the fresh deployment gate: its `16:44:50` mtime and
hash do not match the current local server artifact, and it predates the current
control-loop source head.  No rebuild, install, or service restart was attempted.

## Per-todo exact live QA results

The exact plan scenarios were stopped before lifecycle, network, PipeWire, or process
mutation because the required artifact gates failed.  This is an honest mechanical
FAIL/BLOCKED result, not a simulated pass.

### Todo 20 — real latency and saturated-Wi-Fi characterization

**Result: FAIL (BLOCKED at Windows artifact and fresh Linux deployment gates).**

**Fresh mechanical preflight:** **FAIL** — Windows `2026-08-22T23:30:05.8581905+08:00`
reported the canonical task/install/process/firewall absent; Linux
`2026-08-22T23:30:07,042208044+08:00` reported an active listener but the stale
deployment gate failed.

- Windows gate: **FAIL** — canonical task, install root, exact client process, local UDP
  endpoint, and canonical firewall rule are absent. VB-CABLE is present only as a device.
- Linux gate: **FAIL** — service/listener are present, but the deployed ELF is stale.
- Loopback gate: **FAIL** — no approved Windows loopback capture tool was found.
- `conservative_p95 <= 200 ms`: **FAIL — NOT MEASURED**. No real five-minute run,
  capture-boundary timestamp, or VB-CABLE detection occurred.
- Calibration round trip `<= 20 ms`, 30-second recalibration cadence, raw P50/P95/P99,
  and 60-second saturated-Wi-Fi characterization: **FAIL — NOT MEASURED**.
- No `--calibrate`, Start/Ack probe, tone injection, network saturation, or audio
  capture command was run.

Delta from the original Todo 20 evidence: fresh preflight timestamps are
`23:30:05.8581905` local and `23:30:07.042208044` remote; the current remote ELF
mtime/hash and current control-source head were re-queried; the blocker remains.

### Todo 21 — real tray Exit/Restart lifecycle

**Result: FAIL (BLOCKED at the Windows artifact gate).**

**Fresh mechanical preflight:** **FAIL** — Windows `2026-08-22T23:30:05.8581905+08:00`
reported task/install/process absent; Linux `2026-08-22T23:30:07,042208044+08:00`
reported service/listener active but no current deployed client peer.

- Streaming before lifecycle: **FAIL** — no canonical client process/task exists.
- Exit and Linux Idle transition `<= 2 s`: **FAIL — NOT MEASURED**; no Exit was sent.
- Windows process exit and Scheduled Task `Ready`: **FAIL — NOT MEASURED**; task is
  absent, not `Ready`.
- Logon auto-start and streaming resume `<= 10 s`: **FAIL — NOT MEASURED**; no task
  or session exists.
- Mid-session Restart keeping process/task alive and resuming with a fresh session
  `<= 10 s`: **FAIL — NOT MEASURED**; no real tray process exists.

No task registration, installer run, logout/logon, tray interaction, or remote
Start/Ack command was run.

### Todo 22 — exact 15-second interruption and automatic reconnect

**Result: FAIL (BLOCKED at the Windows artifact and fresh Linux deployment gates).**

**Fresh mechanical preflight:** **FAIL** — Windows `2026-08-22T23:30:05.8581905+08:00`
reported no client process or local UDP endpoint; Linux
`2026-08-22T23:30:07,042208044+08:00` reported the stale deployed ELF.

- Real stream before interruption: **FAIL** — no client process or local UDP 6902
  endpoint exists.
- Exactly 15-second interruption: **FAIL — NOT ATTEMPTED**; there was no real stream
  and no adapter/firewall mutation was authorized.
- Resume `<= 20 s` after restoration: **FAIL — NOT MEASURED**.
- Fresh strictly-greater session ID, server supersession without capture restart, and
  both-process survival: **FAIL — NOT MEASURED**.
- The 5-second client retry cadence and the server's `<30 s` session-timeout distinction
  were not exercised.

Wi-Fi/network state was observed only; no adapter was disabled and no temporary firewall
  rule was created.

### Todo 23 — pinned capture-source missing/retry

**Result: FAIL (BLOCKED at the live-heartbeating-client gate).**

**Fresh mechanical preflight:** **FAIL** — Windows `2026-08-22T23:30:05.8581905+08:00`
reported no canonical client/task/endpoint; Linux `2026-08-22T23:30:07,042208044+08:00`
reported the pinned source and service, but no live heartbeat and no fresh deployment.

- Running heartbeat-valid client: **FAIL** — canonical task, install root, exact
  process, and local UDP endpoint are absent; recent journal has no heartbeat event.
- Reversible source fault injection: **FAIL — NOT ATTEMPTED**; the source was not
  muted, suspended, renamed, or replaced.
- Typed retry cadence `approximately 5 s`: **FAIL — NOT MEASURED**; no `Starting`
  session and no `CaptureRetry` record exists in the fresh recent journal.
- Zero fallback to another source: **FAIL — NOT MEASURED**; no capture attempt ran.
- Source restoration and automatic resumption: **FAIL — NOT MEASURED**.
- The pinned source was observed read-only as
  `alsa_input.pci-0000_00_1b.0.analog-stereo ... SUSPENDED`; no audio configuration
  mutation occurred.

### Todo 24 — heartbeat-timeout auto-stop

**Result: FAIL (BLOCKED at the Windows artifact and fresh Linux deployment gates).**

**Fresh mechanical preflight:** **FAIL** — Windows `2026-08-22T23:30:05.8581905+08:00`
reported no client/task/session; Linux `2026-08-22T23:30:07,042208044+08:00` reported
PID 33601 active only at preflight with the stale deployed ELF.

- Live streaming session and last-heartbeat timestamp: **FAIL — NOT OBSERVED**.
- Client termination without Stop: **FAIL — NOT ATTEMPTED**; no real client process
  existed and no substitute was killed.
- Heartbeat timeout `30–35 s` inclusive: **FAIL — NOT MEASURED**; there is no real
  last-heartbeat-to-Idle/capture-stop interval.
- Fresh session after timeout: **FAIL — NOT MEASURED**.
- Server survival across the timeout scenario: **FAIL — NOT MEASURED**; PID 33601
  was observed alive only during preflight, not across an unexecuted scenario.

The active service alone is not evidence of timeout behavior.

## Cleanup and residual host state

No cleanup action was needed because no mutation occurred.

```text
Windows task \wifimic\wifimic-client: ABSENT before/after
Windows install root C:\Program Files\wifimic-client: ABSENT before/after
Windows exact client processes: ABSENT before/after
Windows local UDP 6902: NONE before/after
Windows firewall DisplayName wifimic-client: ABSENT before/after
VB-CABLE devices: present (Input and Output); not changed
Linux wifimic-server: active, PID 33601; not restarted
Linux UDP 6902 listener: present; not changed
Linux pinned source: present, observed SUSPENDED; not changed
Linux UFW: active, peer-scoped 6902/udp allow and default deny; not changed
Temporary adapter/firewall/PipeWire state: none created
```

No canonical task was registered, no file was copied into Program Files, no firewall
rule was altered, Wi-Fi was not disabled, PipeWire was not fault-injected, the remote
service was not rebuilt/restarted, and no substitute/debug process was launched or
killed.  This wave wrote only this evidence file; plan/source/boulder files were not
edited.

## External blockers

1. An elevated, explicitly authorized canonical Windows installation is still required
   to create the task, install the client, and create the canonical firewall rule.
2. The current Linux control-loop build must be deployed and restarted through the
   approved deployment workflow; the live ELF is stale.
3. An approved Windows VB-CABLE loopback capture/detection mechanism is required for
   Todo 20's real latency gate.

Because all five live scenarios did not pass, the required final verdict is **REJECT**.

---

## Independent F3 rerun — 2026-08-23

**Review method:** The Todo 20–24 QA and acceptance text was reread from the plan. The
original `task-20` through `task-24` happy/failure evidence and this file were compared
against the latest live evidence. Only safe read-only cleanup/state queries were run for
this review. A blocked scenario was not repeated merely to manufacture a result.

### Current read-only cleanup/state receipt

Local timestamp: `2026-08-23T02:26:25.1437615+08:00`.

```text
TASK=Ready; TaskPath=\wifimic\; TaskName=wifimic-client
TASK_ACTION=C:\Program Files\wifimic-client\wifimic_client.exe
TASK_WORKING_DIRECTORY=C:\Program Files\wifimic-client
CLIENT_PROCESSES=ABSENT
LOCAL_UDP_6902=ABSENT
FIREWALL_RULE_wifimic-client=present; Enabled=True; Direction=Inbound; Action=Allow; Profile=Any
FIREWALL_PORT=UDP; LocalPort=6902; RemotePort=Any
FIREWALL_ADDRESS=LocalAddress=Any; RemoteAddress=192.168.0.210
VB_CABLE_INPUT=OK; CABLE Input (VB-Audio Virtual Cable)
```

Remote timestamp: `2026-08-23T02:26:25,854035782+08:00`.

```text
SERVICE_ACTIVE=active
ActiveState=active
SubState=running
MainPID=35138
NRestarts=0
LISTENER_6902=0.0.0.0:6902 owned by wifimic_server pid 35138
PAREC=ABSENT
PINNED_SOURCE=alsa_input.pci-0000_00_1b.0.analog-stereo ... SUSPENDED
FIREWALL_SERVICES=ufw active; iptables inactive; nftables inactive
UFW=default deny incoming; 6902/udp ALLOW IN 192.168.0.200; 6902/udp DENY IN Anywhere
```

This confirms no scenario-induced client process, UDP session, `parec`, temporary
firewall block, adapter change, or source mutation remains. The installed canonical
task/binary and peer-scoped firewall rule are expected persistent deployment state,
not residue created by this review.

### Exact thresholds used

- **Todo 20 acceptance:** “the normal-condition measurement report shows
  `conservative_p95 ≤ 200ms` ... over the full 5-minute real run”; its happy QA
  scenario requires raw and conservative P50/P95/P99 values, the calibration bound,
  and 30-second recalibration cadence. Its separate failure characterization requires
  at least 60 seconds of deliberate saturation and zero sustained underrun bursts
  longer than 1 second.
- **Todo 21 acceptance:** the timestamped sequence must observe streaming active,
  real tray Exit, Linux `Idle` within 2 seconds, process exit, task `Ready`, next
  logon auto-start, and streaming resumed within 10 seconds; the Restart scenario
  must keep the process/task alive and re-establish within 10 seconds.
- **Todo 22 acceptance:** an exactly 15-second interruption must resume within 20
  seconds of restoration through a fresh strictly-greater session ID, without tray
  interaction, while capture remains uninterrupted and the interruption mechanism is
  reverted.
- **Todo 23 acceptance:** repeated typed `CaptureRetry` events must occur every
  approximately 5 seconds while the source is unavailable, with zero fallback and
  automatic resumption while client heartbeats remain valid.
- **Todo 24 acceptance:** the elapsed time from the last received heartbeat to
  `Idle`/capture stop must be **30–35 seconds inclusive**; the tray Exit path is not
  valid for this scenario.

### Per-todo independent comparison

#### Todo 20 — application-level latency and saturated-Wi-Fi characterization

**VERDICT: BLOCKED.**

- **Original task evidence:** `task-20-wifimic-lan-audio.log` records Start/Ack and
  calibration timeout, no five-minute normal run, no 60-second saturated run, and no
  raw or conservative latency statistics. Its local Cargo evidence is explicitly
  implementation proof only.
- **Current rerun evidence:** the current state query shows the deployed artifacts are
  present and cleanup is clean, but no latency run was executed. The evidence already
  records no approved VB-CABLE loopback capture tool and no valid capture-boundary
  timestamp path.
- **Threshold result:** **BLOCKED — no real 5-minute normal measurement, no real
  60-second saturated characterization, no raw P50/P95/P99, and no conservative P95.**
  Ping, capture-process presence, and unit/Cargo tests are not latency evidence.
- **Cleanup receipt:** **PASS — no Todo 20 mutation or audio run was attempted; the
  current read-only state is clean.**

#### Todo 21 — real tray Exit/Restart lifecycle

**VERDICT: BLOCKED.**

- **Original task evidence:** the first run was blocked by absent artifacts. The
  2026-08-23 rerun cleared the artifact gate and proved scheduled-task launch and
  active pinned capture, but native UI automation could not perform either real tray
  action. No Restart session ID, tray Exit Stop transition, process exit caused by
  Exit, or post-Exit logon transition was observed. Cleanup Stop probes were expressly
  not tray Exit evidence.
- **Current rerun evidence:** at `02:26:25+08:00`, the canonical task is `Ready`,
  the exact client process and local UDP 6902 endpoint are absent, and Linux is idle
  with no `parec`. This is a clean post-run state, not evidence that Exit or Restart
  occurred.
- **Threshold result:** **BLOCKED — actual Restart and Exit tray actions were not
  performed; therefore the ≤2-second Exit-to-Idle, task-Ready-after-Exit, logon
  auto-start/≤10-second resume, and Restart fresh-session checks are unverified.**
- **Cleanup receipt:** **PASS — the attempted task-trigger/capture run was cleaned
  with the bounded Stop probe/process cleanup; task is `Ready`, client/UDP/`parec`
  are absent, service PID is 35138, and the source is `SUSPENDED`.**

#### Todo 22 — exact 15-second interruption and automatic reconnect

**VERDICT: BLOCKED.**

- **Original task evidence:** the 2026-08-22 run had no installed/running client. The
  2026-08-23 rerun found the canonical artifacts but stopped at the mandatory
  elevated-interruption gate because `IS_ADMIN=False`; no temporary UDP block was
  created, no 15-second interval ran, and no recovery Ack/session ID was observed.
- **Current rerun evidence:** the canonical task is `Ready`, client and UDP endpoint
  are absent, the Linux service/listener is healthy, and UFW contains only the
  persistent peer allow/default deny signature. There is no temporary interruption
  rule to remove.
- **Threshold result:** **BLOCKED — no exact elevated 15-second interruption and no
  fresh-session recovery within 20 seconds were observed.** Capture continuity,
  strictly-greater session supersession, process survival, and no-tray recovery remain
  unverified.
- **Cleanup receipt:** **PASS — the interruption branch was not entered; no adapter,
  temporary firewall, service, or process mutation was left behind.** This cleanup
  result is not a reconnect pass.

#### Todo 23 — pinned source missing and typed retry

**VERDICT: BLOCKED.**

- **Original task evidence:** the 2026-08-22 run lacked a real heartbeating client. The
  2026-08-23 run did establish a real client/server capture session, but
  `pactl suspend-source ... 1` returned 0 while the source stayed `RUNNING`, the same
  `parec` stayed alive, and no typed `CaptureRetry` or resumption timestamps appeared.
- **Current rerun evidence:** no client session is active; the pinned source is
  `SUSPENDED` as the normal idle state. No new fault injection was performed.
- **Threshold result:** **BLOCKED — no typed `CaptureRetry` cadence and no automatic
  resumption while valid heartbeats remained were observed.** A source-suspend command
  returning 0 is not source unavailability and is not retry evidence. The exact pinned
  source argument/no-fallback observation does not prove the missing-source path.
- **Cleanup receipt:** **PASS — the attempted reversible command was restored; task is
  `Ready`, client/UDP/`parec` are absent, service PID 35138 is active, and pinned
  source name/mute/volume/default-source state matches the baseline.**

#### Todo 24 — 30–35-second heartbeat timeout

**VERDICT: BLOCKED.**

- **Original task evidence:** the 2026-08-22 run lacked a live session. The
  2026-08-23 rerun then performed the real crash-like kill and observed capture stop,
  stable server PID, and fresh-session recovery. It measured `29.453277056` seconds
  from kill request to first no-`parec` sample, but had no packet-level last-heartbeat
  timestamp and no direct `ControlState::Idle` timestamp.
- **Current rerun evidence:** the post-cleanup state query confirms service PID 35138
  remains active with `NRestarts=0`, no `parec`, pinned source `SUSPENDED`, no client,
  and no local UDP 6902 endpoint.
- **Threshold result:** **BLOCKED — the required direct last-heartbeat-to-Idle
  measurement in the inclusive 30–35-second window is absent.** The
  `29.453277056`-second kill-to-stop proxy is insufficient and is not substituted as
  a PASS. Fresh-session recovery and server survival do not replace this threshold.
- **Cleanup receipt:** **PASS — the fresh session was ended and the exact client
  fallback cleaned; task is `Ready`, client/UDP/`parec` are absent, server PID 35138
  is stable, and the pinned source is `SUSPENDED`.**

### Cleanup and remaining host state

Every attempted scenario has a cleanup receipt marked **PASS** for cleanup only:

| Todo | Scenario mutation attempted | Cleanup receipt | Remaining scenario residue |
| --- | --- | --- | --- |
| 20 | No latency/saturation mutation; read-only preflight only | PASS | None |
| 21 | Canonical task start/capture; tray actions unavailable | PASS | None; task is expected `Ready`/enabled |
| 22 | No interruption because elevated firewall operation was unavailable | PASS | None; no temporary rule |
| 23 | Reversible source-suspend request was ineffective, then restore/task cleanup | PASS | None; source is baseline `SUSPENDED` |
| 24 | Exact-client crash-like kill plus fresh-session recovery | PASS | None; no client/UDP/`parec` |

No host is left altered by this F3 rerun. The expected persistent deployment state remains:
the canonical Windows task/binary and peer-scoped `wifimic-client` firewall rule, the
active Linux `wifimic-server` service/listener at stable PID 35138, and the pre-existing
UFW peer allow/default deny rules. No temporary process, task run, firewall block,
adapter change, PipeWire/source mutation, or streaming session remains active.

Cargo/unit and implementation evidence remains useful for implementation proof only; it
does not satisfy any of the blocked live thresholds above. Since all five required live
QA scenarios lack at least one mandatory acceptance observation, F3 cannot approve.

VERDICT: REJECT
