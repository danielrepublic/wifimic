# F1 — Final plan-compliance audit (current evidence)

Run date: 2026-08-23. This audit re-reads the live plan at
`.omo/plans/wifimic-lan-audio.md`, `main_draft.md`,
`.omo/drafts/wifimic-lan-audio.md`, all Task 1–24 evidence files, the current
notepads, and final F2/F4 evidence. It uses the current Windows/Linux state and
does not reuse the prior F1 result where the literal result changed.

No source, deployment script, plan checkbox, or git history was modified.

## Scope enumeration and coverage

The literal Scope contains **22 Must-have bullets** (including nested bullets)
and **6 Must-NOT-have bullets**, therefore this crosswalk has exactly **28 rows**.

```powershell
$lines=Get-Content -LiteralPath '.omo/plans/wifimic-lan-audio.md'
$must=@(); for($i=23;$i -le 45;$i++){if($lines[$i] -match '^\s*-\s'){$must += $lines[$i]}}
$mustNot=@(); for($i=47;$i -le 53;$i++){if($lines[$i] -match '^\s*-\s'){$mustNot += $lines[$i]}}
"MUST_HAVE_BULLETS=$($must.Count)"
"MUST_NOT_HAVE_BULLETS=$($mustNot.Count)"
"CROSSWALK_ROWS=$($must.Count+$mustNot.Count)"
```

Literal result:

```text
MUST_HAVE_BULLETS=22
MUST_NOT_HAVE_BULLETS=6
CROSSWALK_ROWS=28
```

## Current mechanical evidence

### Cargo, repository, and diff gates

Commands run from the repository root:

```text
gh repo view --json name,visibility,defaultBranchRef
cargo metadata --no-deps --format-version 1
cargo fmt --all -- --check
cargo test --workspace
cargo build --workspace
cargo clippy --workspace -- -D warnings
git diff --check
```

Literal results:

```text
gh: {"defaultBranchRef":{"name":"main"},"name":"wifimic","visibility":"PRIVATE"}
workspace_members=wifimic_protocol,wifimic_diagnostics,wifimic_server,wifimic_client
cargo fmt: exit 0 (only existing use_small_diffs warnings)
cargo test: 113 passed, 0 failed, 6 ignored; exit 0
cargo build: exit 0
cargo clippy --workspace -- -D warnings: exit 0
git diff --check: GIT_DIFF_CHECK=PASS
```

The ignored tests are not promoted to live passes: four client VB-CABLE tests,
one Linux PipeWire test, and one two-host/firewall test remained ignored. The
corresponding current task evidence is in `.omo/evidence/task-1` through
`.omo/evidence/task-19`, and the current Wave-5 attempts are in
`.omo/evidence/task-20` through `task-24`.

### Current Windows native-install state

The current read-only PowerShell run at
`2026-08-23T02:29:06.1980583+08:00` returned:

```text
TASK_PRESENT=True
TaskPath : \wifimic\
TaskName : wifimic-client
State    : Ready
INSTALL_ROOT_EXISTS=True
CLIENT_EXE_EXISTS=True
Name=wifimic-client DisplayName=wifimic-client Enabled=True Direction=Inbound Action=Allow Profile=Any
Protocol=UDP LocalPort=6902 RemotePort=Any
LocalAddress=Any RemoteAddress=192.168.0.210
VB-CABLE: Status=OK CABLE Input (VB-Audio Virtual Cable)
VB-CABLE: Status=OK CABLE Output (VB-Audio Virtual Cable)
```

The exported task XML was mechanically checked:

```text
TASK_XML_LOGON_TRIGGER_COUNT=1
TASK_XML_LOGON_TYPE=InteractiveToken
TASK_XML_EXECUTE=C:\Program Files\wifimic-client\wifimic_client.exe
TASK_XML_ARGUMENTS_PRESENT=False
TASK_XML_CONTRACT=PASS
POWERSHELL_PARSE=PASS
GIT_BASH_SHELL_EXIT_CODES=0,0,0
```

The absent `<Arguments />` node is a valid Task Scheduler round-trip, not an
install failure; the installer fix and the user's native install are recorded
in `.omo/notepads/wifimic-lan-audio/issues.md:79-94`. The current Task 21
evidence independently records the exact installed action and final `Ready`
state at `.omo/evidence/task-21-wifimic-lan-audio.log:115-138,306-331`.

### Current Linux deployment and smoke state

The current read-only SSH check returned:

```text
SERVICE=active
ActiveState=active
SubState=running
MainPID=35138
LISTENER=0.0.0.0:6902 owned by wifimic_server
FIREWALL_SERVICES=ufw.service active; iptables.service inactive; nftables.service inactive
UFW=6902/udp ALLOW IN 192.168.0.200; 6902/udp DENY IN Anywhere
SOURCE=alsa_input.pci-0000_00_1b.0.analog-stereo ... 48000Hz SUSPENDED
```

The refreshed deployment then produced the real peer Start Ack (`0x01`),
Heartbeat Ack (`0x02`, with an interleaved valid audio frame), Stop Ack (`0x03`),
and post-Stop Heartbeat `NoAck`; the service remained active and the exact
`parec --device=alsa_input.pci-0000_00_1b.0.analog-stereo` path was used. This
current smoke is recorded in `.omo/notepads/wifimic-lan-audio/issues.md:101-107`.
It supersedes the earlier Task 19 timeout against the stale ELF; the old failed
attempt remains truthfully recorded at
`.omo/evidence/task-19-wifimic-lan-audio-failure.log:18-43`.

### Current Wave-5 reconciliation

| Evidence item | Current literal status |
| --- | --- |
| Native Windows install | **PASS** — canonical task is `Ready`/enabled, Program Files executable exists, exact VB-CABLE endpoint exists, and the firewall rule is peer-scoped to Linux UDP 6902. |
| Linux Start/Heartbeat/Stop smoke | **PASS** — real peer Acks and post-Stop `NoAck` observed; service remains active. |
| Todo 20 | **BLOCKED/FAIL** — no five-minute normal run, no saturated run, no raw/conservative percentiles; see `.omo/evidence/task-20-wifimic-lan-audio.log:17-23,49-53` and failure log `:15-28`. |
| Todo 21 | **BLOCKED/FAIL** — real task/capture start succeeded, but native tray Restart/Exit automation was unavailable; see `.omo/evidence/task-21-wifimic-lan-audio.log:191-222` and failure log `:63-85`. |
| Todo 22 | **BLOCKED/FAIL** — current token reported `IS_ADMIN=False`, so the temporary elevated 6902 block and exact 15-second interruption were not run; see `.omo/evidence/task-22-wifimic-lan-audio.log:138-238` and failure log `:68-113`. |
| Todo 23 | **BLOCKED/FAIL** — source suspension was a no-op (`RUNNING` remained `RUNNING`), so no typed start failure, retry cadence, or resumption was observed; see `.omo/evidence/task-23-wifimic-lan-audio.log:214-307` and failure log `:72-122`. |
| Todo 24 | **BLOCKED/FAIL** — crash-like kill stopped capture and preserved the server, but heartbeat/Idle timestamps were unavailable; the observed kill-to-stop interval was `29.453277056 s`, not the required 30–35-second last-heartbeat-to-Idle measurement; see `.omo/evidence/task-24-wifimic-lan-audio.log:180-210,277-303` and failure log `:55-137`. |
| F2 | **APPROVED** — `.omo/evidence/final-F2-wifimic-lan-audio.md:3-9,147`; current log has `FAILURES=0` and `VERDICT: APPROVE` at `:88-90`. |
| F4 | **APPROVED** — `.omo/evidence/final-F4-wifimic-lan-audio.md:3-9,272-276`; artifact coverage is not substituted for missing live acceptance. |

Blocked live acceptance is not represented as PASS below, even where a unit test,
static artifact, proxy observation, or stale/pre-refresh result passed.

## Complete Scope crosswalk — exactly 28 rows

`PASS` means the current evidence satisfies that Scope bullet. A live Wave-5
requirement remains `FAIL — BLOCKED` when the required live observation is absent;
implementation/unit tests are cited as supporting evidence but do not replace it.

| ID | Scope bullet (literal plan line) | Current verification and result |
| --- | --- | --- |
| M1 | L25 — A new, independent `wifimic` Rust workspace (crates `wifimic_protocol`, `wifimic_diagnostics`; binaries `wifimic_server`, `wifimic_client`) inside a new private GitHub repository created under the authenticated account. | `cargo metadata` listed exactly the four workspace members; `gh repo view` returned `name=wifimic`, `visibility=PRIVATE`, `defaultBranch=main`. **PASS.** |
| M2 | L26 — Fixed 48 kHz mono, 16-bit PCM; one bidirectional UDP socket per host on port 6902 carrying control and audio; the 484-byte invariant is re-derived/documented with a byte-size proof test. | `cargo test --workspace` passed; Task 3’s named `pcm_payload_bytes_matches_frame_duration_and_sample_format`, control round-trip, and rejection tests passed (`.omo/evidence/task-3-wifimic-lan-audio.log:18-31`). Current static required-artifact scan passed (`STATIC_REQUIRED_ARTIFACTS=PASS`). **PASS.** |
| M3 | L27 — Linux `wifimic-server` is a persistent, always-running user-level systemd service on `arch-daniel` that stays idle until a Windows client is active. | Current SSH: `SERVICE=active`, `SubState=running`, stable `MainPID=35138`, UDP 6902 listener; current deployment smoke observed active capture only during the real client session and post-Stop idle while service remained active (`issues.md:101-107`). **PASS.** |
| M4 | L28 — Capture is pinned to `alsa_input.pci-0000_00_1b.0.analog-stereo` via `parec --channels=1`; the server never silently substitutes another source. | Current live process evidence shows exact `parec --channels=1 --device=alsa_input.pci-0000_00_1b.0.analog-stereo` and no alternate source (`task-23-wifimic-lan-audio.log:209-212,230-247`); Task 5 typed no-fallback tests pass (`task-5-wifimic-lan-audio.log:19-26`). **PASS.** |
| M5 | L29 — On valid Start, capture begins; after 30 seconds with no valid client heartbeat, capture stops while the service remains running. | Task 6 injected-clock tests pass, but Todo 24’s required live last-heartbeat-to-Idle measurement is blocked. Only kill-to-first-no-`parec` was observed at `29.453277056 s`; it is explicitly not the required heartbeat-to-Idle interval (`task-24-wifimic-lan-audio.log:205-210,277-291`). **FAIL — BLOCKED live acceptance.** |
| M6 | L30 — If capture/`parec` fails while heartbeat is valid, log a typed error, retry every 5 seconds, resume the same session, and never fall back. | Unit/state-machine evidence passes (`task-6-wifimic-lan-audio.log:9-16`), and the live run kept the exact pinned source with no fallback; however, source suspension did not induce the required start failure, so no `CaptureRetry` events, two retry timestamps, or resumption timestamp exist (`task-23-wifimic-lan-audio.log:251-273`). **FAIL — BLOCKED live acceptance.** |
| M7 | L31 — Linux UDP control/audio accepts only the configured Windows peer IP, and the host firewall is scoped to that peer and UDP 6902. | Task 7 application allow-list tests pass (`task-7-wifimic-lan-audio.log:11-18`); current SSH/UFW output is peer Allow plus Anywhere Deny, with UFW sole active backend; refreshed smoke received real peer traffic (`issues.md:101-107`). **PASS.** |
| M8 | L32 — Windows client is an interactive Scheduled Task (`LogonTrigger` + `InteractiveToken`), installed at `C:\Program Files\wifimic-client`, not a Session-0 service. | Current task XML check: one `LogonTrigger`, `InteractiveToken`, exact Program Files executable, `TASK_XML_CONTRACT=PASS`; current task/root/executable are present and Ready. **PASS.** |
| M9 | L33 — Canonical naming is identical everywhere: task `\wifimic\wifimic-client`, firewall DisplayName `wifimic-client`, install directory `C:\Program Files\wifimic-client`. | Current task/action and firewall read-back show the exact task path/name, DisplayName, and Program Files executable; current static artifact scan passed. **PASS.** |
| M10 | L34 — Windows UDP control/audio accepts only the configured Linux peer IP, and the Windows host firewall is scoped to that peer and UDP 6902. | Current firewall read-back: inbound Allow, UDP local port 6902, `RemoteAddress=192.168.0.210`; client peer-filter tests pass (`task-12-wifimic-lan-audio.log:29-39`). The installer’s semantic single-host read-back accepts the Windows-normalized no-`/32` form (`issues.md:87-94`). **PASS.** |
| M11 | L35 — A hidden-by-default tray icon is visible in the tray and has Restart and Exit menu items. | Tray implementation and injected menu tests pass (`task-13-wifimic-lan-audio.log:5-22`), and a real tray window existed, but UI Automation exposed no menu and `SetCursorPos`/`SendInput` were blocked (`task-21-wifimic-lan-audio.log:191-209`). **FAIL — BLOCKED native tray acceptance; no real menu interaction is promoted to PASS.** |
| M12 | L36 — Restart re-sends a fresh start request/heartbeat stream. | Unit tests prove fresh IDs and Ack-gated heartbeats (`task-12-wifimic-lan-audio.log:29-39`), but Todo 21’s real Restart action and session ID were unobserved because tray automation was unavailable (`task-21-wifimic-lan-audio.log:211-222`). **FAIL — BLOCKED live acceptance.** |
| M13 | L37 — Exit immediately sends Stop and terminates only the current run; the Scheduled Task remains enabled for the next logon. | Tray unit tests prove Stop-before-shutdown (`task-13-wifimic-lan-audio.log:7-8,24-28`), and cleanup Stop Acks were observed, but they were cleanup probes rather than tray Exit (`task-21-wifimic-lan-audio.log:224-240`). No real tray Exit/logon sequence was observed. **FAIL — BLOCKED live acceptance.** |
| M14 | L38 — If Linux is unreachable, the tray remains alive, retries the start/heartbeat handshake every 5 seconds, and resumes automatically without Restart. | Client unit tests prove two missed Acks and fresh-ID retries (`task-12-wifimic-lan-audio.log:29-35`), but Todo 22 could not create the required temporary elevated network block because `IS_ADMIN=False`; no interruption or automatic recovery was observed (`task-22-wifimic-lan-audio.log:208-238`). **FAIL — BLOCKED live acceptance.** |
| M15 | L39 — WASAPI renders to the exact enumerated VB-Audio Virtual Cable playback endpoint and fails clearly if it is missing, without default-device fallback. | Exact endpoint is present in current native preflight; Task 10 opened/wrote 400 synthetic frames to `CABLE Input (VB-Audio Virtual Cable)` and typed missing-endpoint/no-fallback tests passed (`task-10-wifimic-lan-audio.log:4-22`; failure log `:4-11`). CABLE Output loopback payload capture is unavailable, but that limitation belongs to M19 latency, not endpoint selection. **PASS.** |
| M16 | L40 — Adaptive jitter starts at 40 ms, grows toward 200 ms under loss/late bursts, and decays toward 40 ms when stable. | Task 11 growth/decay and ceiling tests pass (`task-11-wifimic-lan-audio.log:5-19`; failure log `:5-15`); workspace tests pass. **PASS.** |
| M17 | L41 — Windows metadata-only diagnostics rotate under `%LOCALAPPDATA%\wifimic-client`, capped at 7 days or 10 MiB; Linux diagnostics use journald. | Task 14 age/size and corrupt-metadata tests pass (`task-14-wifimic-lan-audio.log:3-15`; failure log `:3-7`); diagnostics no-audio-content tests pass (`task-4-wifimic-lan-audio.log:11-18`). Current service is a user systemd service and client logs contained only the metadata header during live runs. **PASS.** |
| M18 | L42 — Both-host clone/build deployment, explicit tag/commit updates, health validation, automatic rollback to last good state, and no background/automatic self-update. | Task 16/17 deterministic rollback matrices and Task 18 command cross-checks pass (`task-16-wifimic-lan-audio.log:6-27`; `task-17-wifimic-lan-audio.log:18-44`; `task-18-wifimic-lan-audio.log:42-93`). Current Linux refresh and native Windows install also completed successfully; no automatic updater artifact was found. **PASS.** |
| M19 | L43 — Real UDP 6902 application-measured one-way latency over the clarified capture-acquisition-to-VB-CABLE boundary, with conservative P95 ≤ 200 ms. | Todo 20 has no five-minute normal run, no 60-second saturated characterization, no raw P50/P95/P99, and no conservative P95 (`task-20-wifimic-lan-audio.log:17-23,49-53`). Required VB-CABLE loopback tooling/measurement remains unavailable. **FAIL — BLOCKED live latency acceptance.** |
| M20 | L44 — TDD for protocol/control/jitter, happy/failure agent QA for every implementation todo, and a final two-host end-to-end verification wave. | Cargo/TDD and Tasks 1–19 evidence pass, but current live Todos 20–24 are respectively blocked (Todo 20 latency, Todo 21 tray, Todo 22 elevated interruption, Todo 23 fault injection, Todo 24 exact heartbeat timestamps). **FAIL — final live acceptance wave incomplete.** |
| M21 | L45 — Code quality is improved with explicit state-machine boundaries, typed errors, cohesive modules, strict lints, behavior tests, and no added product capability beyond this plan. | **Cited judgment PASS:** F2 is approved with clean Cargo/Clippy/diff gates, typed errors, explicit client/server states, and zero magic-number findings (`final-F2-wifimic-lan-audio.md:3-9,104-145`); F4 independently inventories the refactor siblings and finds no scope expansion (`final-F4-wifimic-lan-audio.md:34-58,192-196`). This is the plan’s permitted judgment-based row, not an uncited assertion. **PASS (judgment).** |
| M22 | L46 — Deployment-facing instructions are Traditional Chinese; source identifiers/comments use English. | Task 9 and Task 18 checks report zero forbidden credential path, Traditional Chinese prose, and matching source/script values (`task-9-wifimic-lan-audio.log:27-69`; `task-18-wifimic-lan-audio.log:70-84`). **PASS.** |
| N1 | L49 — No encryption, authentication infrastructure, Internet/NAT traversal, mDNS/zeroconf discovery, multi-client fan-out, or configurable general-purpose networking beyond the two fixed hosts/6902. | Current product scan over `apps`/`crates` found `OUT_OF_SCOPE_NETWORK_SECURITY_TOKENS=0` and `OUT_OF_SCOPE_NETWORK_SCAN=PASS`; fixed peer/port behavior is covered by Task 7 and the plan’s no-encryption decision. **PASS.** |
| N2 | L50 — No web UI and no VB-CABLE driver bundling/automation; the user installs VB-CABLE manually. | Current `apps`/`crates`/`deploy`/`docs` scan found `WEB_UI_OR_DRIVER_AUTOMATION_TOKENS=0` and `WEB_UI_DRIVER_SCAN=PASS`; the installer only enumerates/verifies the installed endpoint. **PASS.** |
| N3 | L51 — No Windows Session-0 service, separate service+tray-IPC pair, or pre-logon/unattended audio streaming. | Current interactive-only scan found `SESSION0_IPC_TOKENS=0` and `INTERACTIVE_ONLY_SCAN=PASS`; current task XML is `LogonTrigger` + `InteractiveToken`, and the client is a tray process in the signed-in session. **PASS.** |
| N4 | L52 — No Opus/RTP/WebRTC adoption and no stereo wire-format expansion; protocol remains 48 kHz mono PCM. | Current codec scan found `OUT_OF_SCOPE_CODEC_TOKENS=0` and `CODEC_SCAN=PASS`; Task 3’s protocol tests prove the fixed mono wire contract (`task-3-wifimic-lan-audio.log:18-31`). **PASS.** |
| N5 | L53 — No automatic/background self-update and no automatic fallback to another Linux capture source or Windows render endpoint; failures retry the same pinned target. | Current update scan found `AUTO_UPDATE_TOKENS=0` and `AUTO_UPDATE_SCAN=PASS`; Task 5/10 typed failure tests and current live process evidence preserve the exact pinned source/endpoint (`task-5-wifimic-lan-audio.log:19-26`; `task-10-wifimic-lan-audio-failure.log:4-11`; `task-23-wifimic-lan-audio.log:265-273`). **PASS.** |
| N6 | L54 — No new product capability beyond `main_draft.md` plus the recorded decisions; internal-quality refactors must not change the external lifecycle/deployment contract. | **Cited judgment plus mechanical boundary PASS:** F4 reports all 76 inventoried artifacts cited, zero uncited artifacts, and no scope expansion (`final-F4-wifimic-lan-audio.md:3-9,192-196,272-276`); F2 records extraction-only public-facade/wire-layout preservation (`final-F2-wifimic-lan-audio.md:47-64`). **PASS.** |

## Precise live-acceptance blockers and scope reductions

The current evidence leaves these reductions explicit:

1. **Latency loopback unavailable:** no approved Windows VB-CABLE loopback
   capture/detection path exists, so Todo 20 cannot produce the required raw or
   conservative P95. No proxy timing, unit test, or stale statistic is used as a
   latency pass (M19).
2. **Tray automation unavailable:** UI Automation exposed no tray menu; PyWinAuto
   and PyWin32 were unavailable, and `SetCursorPos`/`SendInput` were rejected.
   Cleanup Stop probes are not tray Exit, so Restart/Exit/logon semantics remain
   blocked (M11–M13).
3. **Elevated network interruption unavailable:** the current token reported
   `IS_ADMIN=False`; the exact temporary UDP 6902 block could not be created and
   removed, so Todo 22 has no 15-second interruption or 20-second recovery result
   (M14).
4. **Capture-retry fault injection unavailable:** reversible source suspension
   returned success but did not make the pinned source unavailable; no typed start
   failure, 5-second retry series, or automatic resumption can be claimed (M6).
5. **Exact heartbeat timestamp visibility unavailable:** production diagnostics,
   journald, and available observers exposed no packet heartbeat or explicit
   `ControlState::Idle` timestamps. The `29.453277056 s` kill-to-capture-stop
   observation is retained but cannot satisfy the 30–35-second last-heartbeat
   window (M5).

The resolved current facts are not blockers: native Windows install is PASS,
Linux real Start/Heartbeat/Stop smoke is PASS, F2 is approved, and F4 is approved.

## Coverage and verdict

Coverage: **28/28 rows present**; **20 PASS** (14 Must-have + 6 Must-NOT-have),
**8 FAIL/BLOCKED** (M5, M6, M11, M12, M13, M14, M19, M20).

Approval is not permitted while any Scope Must-have row remains unsatisfied by
current evidence. The implementation artifacts and unit tests do not convert the
blocked live acceptance rows into passes.

VERDICT: REJECT
