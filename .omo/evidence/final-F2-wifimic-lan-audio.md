# F2 — Code quality review

## Verdict

**PASS after remediation.** The original review findings were corrected without changing runtime values, timing, or wire format. The required Cargo gates and the repeated ast-grep scans pass with every previously `V`-flagged site resolved.

Todo 20's calibration implementation is present (`crates/wifimic_protocol/src/latency.rs` and the client control path), but Todo 20's overall acceptance remains `[~]` blocked by the live prerequisites recorded in the notepads. That does not block reviewing the code that is implemented.

## Clippy

Command: `cargo clippy --workspace -- -D warnings`

Exit code: **0 (PASS)**.

The complete captured output is in `.omo/evidence/final-F2-wifimic-lan-audio.log` and is reproduced verbatim here:

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.17s
```

## Literal-scan method and scope

The scan used `ast-grep 0.45.0` structural searches with `--pattern` for `Duration::from_secs($N)`, `Duration::from_millis($N)`, `Duration::from_micros($N)`, and `Duration::from_nanos($N)`, plus `--kind integer_literal --lang rust`. Test-named files/directories and embedded `#[cfg(test)]` modules were excluded. The four requested production source trees contained **27 files**, **212 integer-literal occurrences** on **186 source lines**, and **13 non-test duration-constructor matches** (including variable arguments, which are not magic literals).

The table below groups literals on one source line together; every `file:line` is a separate scan site. `N` means already a named constant, `A` means an exact allow-list or derived wire invariant, `D` means a documented/semantic structural literal rather than a magic product value, and `V` means a genuine unresolved violation requiring a parent-level fix.

The authoritative allow-list from the plan is: protocol `48_000`, `16`, `1`, `6902`, and resolved `SAMPLES_PER_FRAME`/`PCM_PAYLOAD_BYTES`; server 30-second liveness and 5-second capture retry; client 5-second heartbeat/reconnect timing and 2-missed-heartbeat/10-second threshold; jitter 40/200 ms; log rotation 7 days/10 MiB; calibration 20 ms maximum round trip, 30-second recalibration, 5 ms instability warning, ≤2 ms/30-second drift bound, and ≤25 ms conservative error budget; and the session-ordering generator's `+1`.

## Named-constant inventory

These literal occurrences are resolved by being in named constants (or named fixed configuration values), rather than being bare magic values at their use sites:

| Source lines | Literal(s) | Disposition |
|---|---:|---|
| `apps/wifimic_client/src/control.rs:25,27,29,30` | `5; 5; 5; 2` | N — client control constants |
| `apps/wifimic_client/src/control_support.rs:6` | `192,168,0,210` | N — fixed approved peer constant |
| `apps/wifimic_client/src/jitter.rs:13,15,17,19,21,23,27,29` | `40; 200; 3; 20; 20; 5; 41; 1,31` | N — jitter bounds/algorithm constants |
| `apps/wifimic_client/src/logging/errors.rs:7,10` | `7,24,60,60; 10,1024,1024` | N — retention bounds |
| `apps/wifimic_client/src/logging/sink.rs:14` | `1_000` | N — file-name attempt bound |
| `apps/wifimic_client/src/main.rs:21,22` | `4; 1` | N — calibration CLI settings |
| `apps/wifimic_client/src/render.rs:9,10,12` | `100; 1; 2` | N — render wait/channel constants |
| `apps/wifimic_server/src/capture_process.rs:26` | `4_096` | N — bounded stderr constant |
| `apps/wifimic_server/src/capture_types.rs:9` | `7` | N — fixed `parec` argument array length |
| `apps/wifimic_server/src/control.rs:18,19` | `30; 5` | N/A — exact server control timers |
| `apps/wifimic_server/src/network.rs:6` | `6_902` | N/A — fixed UDP port |
| `crates/wifimic_diagnostics/src/sink.rs:22` | `4_096` | N — collector capacity |
| `crates/wifimic_protocol/src/latency.rs:4,6,8,10,12,14,16,18,20,22` | `20_000; 5_000; 30; 25_000; 0x05; 0x06; 14; 30; 1_000; 8_000` | N/A — calibration constants/tags and measurement constants |
| `crates/wifimic_protocol/src/lib.rs:52,54,56,58,60,62,66,69,71,73,75,77,79,82,84,86,88,90,92,94` | `48_000; 16; 1; 5; 1_000; 8; 6_902; 1; 0x00; 0x01; 0x02; 0x03; 0x04; 8; 4; 2; 2; 494; 10; 11` | N/A — protocol wire constants and derived lengths |
| `crates/wifimic_protocol/src/sequence.rs:18` | `1,31` | N — sequence half-range constant |

The protocol compile-time assertions at `crates/wifimic_protocol/src/lib.rs:99-106` are separately listed below because they are doc/invariant proofs, not production configuration constants.

## Non-const literal dispositions

| Source | Literal(s) | Disposition |
|---|---:|---|
| `apps/wifimic_client/src/control.rs:190` | `0` | D — initial counter state |
| `apps/wifimic_client/src/control.rs:191` | `0` | D — initial transition counter |
| `apps/wifimic_client/src/control.rs:192` | `0` | D — initial malformed-packet counter |
| `apps/wifimic_client/src/control.rs:195` | `0` | D — initial calibration sequence |
| `apps/wifimic_client/src/control.rs:224` | `0` | D — reset missed-heartbeat counter |
| `apps/wifimic_client/src/control.rs:298` | `0` | D — zero PCM silence frame |
| `apps/wifimic_client/src/control.rs:374` | `1` | D — calibration sequence increment |
| `apps/wifimic_client/src/control.rs:383` | `0` | D — Unix-time fallback sentinel |
| `apps/wifimic_client/src/control_logic.rs:24` | `1` | D — malformed-packet counter increment |
| `apps/wifimic_client/src/control_logic.rs:127` | `1` | D — missed-heartbeat counter increment |
| `apps/wifimic_client/src/control_logic.rs:145` | `0` | D — accepted-heartbeat reset |
| `apps/wifimic_client/src/control_logic.rs:149` | `1` | D — transition counter increment |
| `apps/wifimic_client/src/control_logic.rs:169` | `0` | D — heartbeat counter reset |
| `apps/wifimic_client/src/control_logic.rs:171` | `1` | D — reconnect transition increment |
| `apps/wifimic_client/src/control_logic.rs:192` | `0` | D — establishing-state reset |
| `apps/wifimic_client/src/control_logic.rs:214` | `1` | D — unreachable transition increment |
| `apps/wifimic_client/src/control_support.rs:43` | tuple-field `.0` | D — IPv4 tuple-field access |
| `apps/wifimic_client/src/control_support.rs:68` | `0` | D — zero-initialized receive buffer |
| `apps/wifimic_client/src/jitter/adaptation.rs:21` | `0` | D — sequence-distance lower boundary |
| `apps/wifimic_client/src/jitter/adaptation.rs:66` | `0` | D — no anchor means no late frames |
| `apps/wifimic_client/src/jitter/adaptation.rs:70` | `0` | D — wrapping-distance rejection |
| `apps/wifimic_client/src/jitter/adaptation.rs:76` | `0` | D — tolerance means not late |
| `apps/wifimic_client/src/jitter/adaptation.rs:79` | `1` | D — at least one late frame when late-by is positive |
| `apps/wifimic_client/src/jitter/adaptation.rs:83` | `0` | D — stable-frame reset |
| `apps/wifimic_client/src/jitter/adaptation.rs:96` | `0` | D — adverse-frame reset |
| `apps/wifimic_client/src/jitter/adaptation.rs:97` | `1` | D — stable-frame increment |
| `apps/wifimic_client/src/jitter/adaptation.rs:101` | `0` | D — stable-period reset |
| `apps/wifimic_client/src/jitter/buffer.rs:48` | `0` | D — counter initialization |
| `apps/wifimic_client/src/jitter/buffer.rs:49` | `0` | D — counter initialization |
| `apps/wifimic_client/src/jitter/buffer.rs:86` | `0` | D — late-frame predicate boundary |
| `apps/wifimic_client/src/jitter/buffer.rs:104` | `1` | D — a gap always counts at least one adverse frame |
| `apps/wifimic_client/src/jitter/buffer.rs:105` | `0` | D — late-frame predicate boundary |
| `apps/wifimic_client/src/jitter/buffer.rs:116` | `0` | D — late-frame predicate boundary |
| `apps/wifimic_client/src/jitter/buffer.rs:125` | `1` | D — one late/reordered frame |
| `apps/wifimic_client/src/jitter/buffer.rs:156` | `1` | D — next sequence increment |
| `apps/wifimic_client/src/jitter/buffer.rs:184` | `0` | D — adaptive target state reset |
| `apps/wifimic_client/src/jitter/buffer.rs:185` | `0` | D — adverse/stable counter reset |
| `apps/wifimic_client/src/logging/errors.rs:102` | `0` | D — report counter initialization |
| `apps/wifimic_client/src/logging/errors.rs:103` | `0` | D — report counter initialization |
| `apps/wifimic_client/src/logging/errors.rs:104` | `0` | D — report byte-count initialization |
| `apps/wifimic_client/src/logging/rotation.rs:41` | `1` | D — examined-file counter increment |
| `apps/wifimic_client/src/logging/rotation.rs:53` | `0_u64` | D — retained-byte fold identity |
| `apps/wifimic_client/src/logging/rotation.rs:64` | `0_u64` | D — retained-byte fold identity |
| `apps/wifimic_client/src/logging/rotation.rs:136` | `0` | D — empty-header detection |
| `apps/wifimic_client/src/logging/rotation.rs:167` | `1` | D — removed-file counter increment |
| `apps/wifimic_client/src/logging/sink.rs:39` | `0` | D — first log suffix |
| `apps/wifimic_client/src/logging/sink.rs:115` | `0` | D — first-file naming branch |
| `apps/wifimic_client/src/logging/sink.rs:132` | `1` | D — suffix increment |
| `apps/wifimic_client/src/main.rs:31` | `0` | D — OS-selected ephemeral calibration port |
| `apps/wifimic_client/src/main.rs:34` | `0` | D — first calibration sequence |
| `apps/wifimic_client/src/main.rs:74` | `0` | D — Unix-time fallback sentinel |
| `apps/wifimic_client/src/main.rs:90` | `1` | **V — direct 1 ms socket timeout is not named and is outside the allow-list** |
| `apps/wifimic_client/src/render.rs:75` | `0_u8` | D — zero-filled stereo output buffer |
| `apps/wifimic_client/src/render_windows.rs:50` | `0` | **V — raw WASAPI buffer-duration sentinel is a bare undocumented literal** |
| `apps/wifimic_client/src/render_windows.rs:90` | `0_u8` | D — silence prefill buffer |
| `apps/wifimic_client/src/render_windows.rs:149` | `1` | **V — direct 1 ms lower bound duplicates the named render minimum** |
| `apps/wifimic_client/src/render_windows.rs:198` | `0` | D — device-index iteration start |
| `apps/wifimic_client/src/render_windows.rs:250` | `0` | D — device-index iteration start |
| `apps/wifimic_client/src/render_windows.rs:272` | tuple-field `.0`, comparison `0` | D — HRESULT tuple-field access and signed-success boundary |
| `apps/wifimic_client/src/render_windows.rs:273` | tuple-field `.0` | D — HRESULT tuple-field access |
| `apps/wifimic_client/src/tray.rs:144` | `1` | D — Todo 13's documented canonical embedded resource ordinal |
| `apps/wifimic_client/src/tray.rs:204` | `0, 0` | D — documented Win32 `PeekMessageW` filter sentinels |
| `apps/wifimic_server/src/capture_process.rs:108` | `0_u8, 512` | **V — 512-byte stderr read chunk is a bare implementation bound** |
| `apps/wifimic_server/src/capture_process.rs:113` | `0` | D — EOF sentinel |
| `apps/wifimic_server/src/capture_reader.rs:23` | `0_u8` | D — zero-initialized PCM frame |
| `apps/wifimic_server/src/capture_reader.rs:24` | `0_usize` | D — byte-count initialization |
| `apps/wifimic_server/src/capture_reader.rs:28` | `0` | D — EOF result sentinel |
| `apps/wifimic_server/src/control.rs:48` | `0` | D — retry counter initialization |
| `apps/wifimic_server/src/control.rs:49` | `0` | D — malformed-packet counter initialization |
| `apps/wifimic_server/src/control.rs:80` | `1` | D — malformed-packet counter increment |
| `apps/wifimic_server/src/control.rs:181` | `0` | D — first capture attempt |
| `apps/wifimic_server/src/control.rs:185` | `0` | D — retry counter reset |
| `apps/wifimic_server/src/control.rs:257` | `0` | D — successful-start retry reset |
| `apps/wifimic_server/src/control.rs:260` | `1` | D — retry-attempt increment |
| `apps/wifimic_server/src/control.rs:283` | `0` | D — stop/reset retry state |
| `apps/wifimic_server/src/main.rs:22` | `1` | **V — direct 1 ms socket timeout is not named and is outside the allow-list** |
| `apps/wifimic_server/src/main.rs:26` | `0_u32` | D — first audio sequence |
| `apps/wifimic_server/src/main.rs:55` | `0` | D — sequence reset on new session |
| `apps/wifimic_server/src/main.rs:81` | `1` | D — audio sequence increment |
| `apps/wifimic_server/src/main.rs:90` | `0` | D — Unix-time fallback sentinel |
| `apps/wifimic_server/src/network.rs:16` | `192,168,0,200` | D — documented fixed Windows peer address |
| `apps/wifimic_server/src/network.rs:23` | tuple-field `.0` | D — IPv4 tuple-field access |
| `apps/wifimic_server/src/network.rs:64` | `0` | D — zero-initialized datagram buffer |
| `crates/wifimic_diagnostics/src/sink.rs:150` | `1` | **V — direct one-second default limiter duration is not named and is outside the allow-list** |
| `crates/wifimic_protocol/src/audio.rs:34` | `0_u8` | D — zero-filled packet before encoding |
| `crates/wifimic_protocol/src/audio.rs:35` | `0` | D — documented tag offset |
| `crates/wifimic_protocol/src/audio.rs:36` | `1` | D — documented version offset |
| `crates/wifimic_protocol/src/audio.rs:55` | `0` | D — empty datagram length |
| `crates/wifimic_protocol/src/audio.rs:61` | `1` | D — documented version offset |
| `crates/wifimic_protocol/src/audio.rs:89` | `0_u8` | D — zero-filled fixed-width decode buffer |
| `crates/wifimic_protocol/src/audio.rs:91` | `0_u8` | D — zero-filled fixed-width decode buffer |
| `crates/wifimic_protocol/src/audio.rs:93` | `0_u8` | D — zero-filled PCM decode buffer |
| `crates/wifimic_protocol/src/control.rs:66` | `0` | D — empty datagram length |
| `crates/wifimic_protocol/src/control.rs:74` | `1` | D — documented version offset |
| `crates/wifimic_protocol/src/control.rs:101` | `0_u8` | D — zero-filled fixed-width decode buffer |
| `crates/wifimic_protocol/src/latency.rs:85` | `0` | D — empty calibration datagram length |
| `crates/wifimic_protocol/src/latency.rs:93` | `1` | D — documented version offset |
| `crates/wifimic_protocol/src/latency.rs:96` | `1` | D — documented version offset |
| `crates/wifimic_protocol/src/latency.rs:111` | `0_u8, 4` | **V — calibration sequence width is hard-coded instead of using a named protocol width** |
| `crates/wifimic_protocol/src/latency.rs:112` | `2, 6` | **V — calibration packet offsets are bare literals** |
| `crates/wifimic_protocol/src/latency.rs:113` | `0_u8, 8` | **V — calibration timestamp width is hard-coded instead of named** |
| `crates/wifimic_protocol/src/latency.rs:114` | `6, 14` | **V — calibration packet offsets are bare literals** |
| `crates/wifimic_protocol/src/latency.rs:123` | `0_u8, 8` | **V — calibration timestamp width is hard-coded instead of named** |
| `crates/wifimic_protocol/src/latency.rs:124` | `0_u8, 8` | **V — calibration timestamp width is hard-coded instead of named** |
| `crates/wifimic_protocol/src/latency.rs:125` | `14, 22` | **V — calibration packet offsets are bare literals** |
| `crates/wifimic_protocol/src/latency.rs:126` | `22, 30` | **V — calibration packet offsets are bare literals** |
| `crates/wifimic_protocol/src/latency.rs:162` | `2` | D — NTP midpoint/half-round-trip error-bound arithmetic |
| `crates/wifimic_protocol/src/latency.rs:172` | `2` | D — NTP half-round-trip conservative bound |
| `crates/wifimic_protocol/src/latency.rs:267` | `0` | D — empty percentile result |
| `crates/wifimic_protocol/src/latency.rs:270` | `99` | **V — percentile rounding constant is unnamed** |
| `crates/wifimic_protocol/src/latency.rs:271` | `99, 100` | **V — percentile scale/rounding constants are unnamed** |
| `crates/wifimic_protocol/src/latency.rs:272` | `1, 1, 1` | **V — percentile lower-bound/index constants are unnamed** |
| `crates/wifimic_protocol/src/latency.rs:274` | `95` | **V — P95 rank is a bare literal** |
| `crates/wifimic_protocol/src/latency.rs:276` | `50` | **V — P50 rank is a bare literal** |
| `crates/wifimic_protocol/src/latency.rs:278` | `99` | **V — P99 rank is a bare literal** |
| `crates/wifimic_protocol/src/latency.rs:293` | `0` | D — non-negative latency clamp |
| `crates/wifimic_protocol/src/latency.rs:299` | `0_u8` | D — zero-initialized deterministic PCM frame |
| `crates/wifimic_protocol/src/lib.rs:99` | `1` | A — channel-count invariant |
| `crates/wifimic_protocol/src/lib.rs:100` | `1_000` | A — sample-rate derivation invariant |
| `crates/wifimic_protocol/src/lib.rs:101` | `240` | A — resolved `SAMPLES_PER_FRAME` proof |
| `crates/wifimic_protocol/src/lib.rs:102` | `2` | A — resolved bytes-per-sample proof |
| `crates/wifimic_protocol/src/lib.rs:103` | `480` | A — resolved `PCM_PAYLOAD_BYTES` proof |
| `crates/wifimic_protocol/src/lib.rs:104` | `494` | A — documented audio packet-size proof |
| `crates/wifimic_protocol/src/lib.rs:105` | `10` | A — documented control-header-size proof |
| `crates/wifimic_protocol/src/lib.rs:106` | `11` | A — documented Ack-size proof |
| `crates/wifimic_protocol/src/sequence.rs:20` | `1` | D — immediate successor definition |
| `crates/wifimic_protocol/src/sequence.rs:21` | `1` | D — documented in-order successor |
| `crates/wifimic_protocol/src/sequence.rs:22` | `1` | D — documented gap lower boundary |
| `crates/wifimic_protocol/src/sequence.rs:24` | `1` | D — missing-frame derivation |
| `crates/wifimic_protocol/src/session.rs:41` | `1` | A — exact session-ordering generator `+1` allow-list item |
| `crates/wifimic_protocol/src/session.rs:63` | `0` | D — documented required initial high-water mark |

### Genuine literal violations

The original `V` rows were corrected in the F2 remediation commit. The exact numeric values remain unchanged; each use site now refers to a named constant or derived offset/rank.

## F2 remediation

The four original duration-constructor patterns and the `integer_literal` Rust scan were rerun with ast-grep 0.45.0 over the four production source trees. The previously flagged sites are resolved as follows:

1. Client and server 1 ms receive polls use `RECEIVE_POLL_INTERVAL`.
2. WASAPI's zero shared-buffer-duration sentinel uses the documented `WASAPI_DEFAULT_BUFFER_DURATION_HNS` constant.
3. The render wait lower bound reuses `super::MIN_EVENT_WAIT`.
4. The stderr reader uses `STDERR_READ_CHUNK_BYTES`.
5. The diagnostics standard limiter uses `DEFAULT_RATE_LIMIT_INTERVAL`.
6. Calibration decoding uses named prefix, field-width, and derived slice-bound constants; the existing packet layout is unchanged.
7. Percentile rounding, scale, lower-bound/index, and P50/P95/P99 rank values use named constants.

The scan found no remaining bare literal at any former `V` site. The typed-error review also passes: tray operation failures retain their boxed typed sources, and `run_calibration` returns `CalibrationCliError` with dedicated filtered-peer, probe-reply, and sequence-mismatch variants plus typed transport/protocol/calibration wrappers.

## State machines, errors, and complexity

- **Explicit state machines: PASS.** The server has `ControlState::{Idle, Starting, Streaming}` in `apps/wifimic_server/src/control_support.rs`; the client has `ClientState::{Establishing, Streaming, Unreachable, Stopped}`; tray shutdown has a separate `ClientRunState`. Transitions are centralized in the control logic rather than represented by loosely coupled booleans.
- **Typed errors: PASS after remediation.** Tray operation failures now store `detail: Box<dyn std::error::Error + Send + Sync>` with `#[source]`, and the calibration CLI returns a dedicated `CalibrationCliError` rather than string-derived errors.
- **Reference-project complexity: no separate unnecessary module/capability was identified.** The production implementation is split into cohesive protocol, capture, control, jitter, render, logging, and tray modules; the Todo 20 calibration code is in-scope work rather than an unrequested capability. This qualitative observation does not offset the concrete `V` findings above.

Because the literal and typed-error findings are resolved and all requested Cargo gates pass, the overall F2 result is **PASS**.

## Verification

- `cargo fmt --all -- --check`: **PASS**
- `cargo test --workspace`: **PASS** — 32 + 40 + 5 + 7 + 11 + 18 tests passed; 6 hardware/live tests remained ignored by their existing prerequisites.
- `cargo build --workspace`: **PASS**
- `cargo clippy --workspace -- -D warnings`: **PASS**
