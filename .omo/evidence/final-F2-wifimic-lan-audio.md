# F2 — Final independent code-quality verification

## Verdict

**APPROVE.** All F2 criteria pass on the current working tree. The two
previous module-size failures are closed by the renderer and latency
extractions. Cargo gates, the exact magic-number scan, typed-error/state
machine review, wire-layout/endpoint preservation review, and the 250 pure-LOC
threshold all pass.

## Mechanical gates

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | **PASS**, exit 0; Cargo emitted 8 repeated `use_small_diffs` unknown-option warnings only |
| `cargo test --workspace` | **PASS**, exit 0; 113 passed, 0 failed, 6 ignored |
| `cargo build --workspace` | **PASS**, exit 0 |
| `cargo clippy --workspace -- -D warnings` | **PASS**, exit 0 |
| `git diff --check` | **PASS**, exit 0 |
| `git diff --check c5dee65^ HEAD` | **PASS**, exit 0; historical F2 diff check also rerun |

The literal command transcript is in
`.omo/evidence/final-F2-wifimic-lan-audio.log`.

### Executed versus ignored tests

Executed tests were **113 passed, 0 failed**:

- `wifimic_client` library: 32 passed, 2 ignored.
- `wifimic_client` binary: 40 passed, 2 ignored.
- `wifimic_diagnostics`: 5 passed, 0 ignored.
- `wifimic_protocol`: 7 passed, 0 ignored.
- `wifimic_protocol/tests/wire_contract.rs`: 11 passed, 0 ignored.
- `wifimic_server`: 18 passed, 1 ignored.
- `wifimic_server/tests/lan_audio_acceptance.rs`: 0 passed, 1 ignored.
- Doc-tests: 0 passed, 0 failed, 0 ignored.

Ignored tests were **not counted as executed passes**:

- 4 client live VB-CABLE renderer tests (two live tests in each client test
  binary).
- 1 server live PipeWire capture test.
- 1 live two-host/firewall acceptance test.

No live host mutation, hardware acceptance, or end-to-end audio claim is made.

## Refactor preservation review

- `apps/wifimic_client/src/render_windows.rs` retains `Renderer`, its public
  methods, `RenderError` usage, and the exact renderer write/wait/stop flow.
  `render_windows_endpoints.rs` contains the moved COM apartment cleanup,
  exact endpoint selection, and endpoint enumeration bodies. The facade's
  `pub use endpoints::enumerate_render_endpoints` preserves the public name and
  re-export. The exact endpoint tests passed, including no-fallback and typed
  missing-endpoint cases; live endpoint tests remained ignored.
- `crates/wifimic_protocol/src/latency.rs` retains the prior public latency
  names through explicit `pub use` re-exports from `calibration` and
  `measurement`. The moved calibration code preserves the same tags, packet
  lengths, field widths, offsets, big-endian encoding, timestamp math, tracker
  behavior, and typed errors. The calibration wire round-trip and all protocol
  wire-contract tests passed.
- The refactor diff is extraction-only: no public name was removed, no UDP
  wire layout was changed, and no endpoint fallback or selection behavior was
  changed.

## Deterministic magic-number scan

The exact F2 scan was rerun with **ast-grep 0.45.0** using Rust
`integer_literal` AST nodes over:

`apps/wifimic_client/src`, `apps/wifimic_server/src`,
`crates/wifimic_diagnostics/src`, and `crates/wifimic_protocol/src`.

Test-named files/directories and every `#[cfg(test)]` item span were excluded.
The exact scan result was:

```text
AST_ROWS_TOTAL=616
PRODUCTION_INTEGER_ROWS=198
NAMED_CONSTANT_OR_DOC_CONTEXT_ROWS=85
NON_CONST_ROWS=113
FLAGGED_FINDINGS=0
NON_CONST_DISTINCT_VALUES=0,1,2,168,192,200
```

The non-constant remainder contains only structural `0`/`1`/`2` values and
the documented fixed peer-address components (`192`, `168`, `200`). The exact
allow-list was not weakened: protocol `48_000`, `16`, `1`, `6902`, resolved
`SAMPLES_PER_FRAME`/`PCM_PAYLOAD_BYTES`; server 30-second/5-second timing;
client 5-second/5-second, two-missed/10-second threshold; jitter 40/200 ms;
rotation 7 days/10 MiB; calibration 20,000 us/30 seconds/5,000 us, the
documented ≤2 ms per 30 seconds and ≤25 ms bounds; and session-ordering `+1`.

The former flagged literal patterns were structurally absent (**PASS** for
each):

- direct `Duration::from_millis(1)` and `Duration::from_secs(1)`;
- direct `buffer_duration_hns: 0`;
- direct `[0_u8; 512]` capture stderr allocation;
- direct calibration `[0_u8; 4]`, `[0_u8; 8]`, `2..6`, `6..14`, `14..22`,
  and `22..30` offsets;
- direct percentile `saturating_add(99)`, `/ 100`, and `rank.max(1)` sites.

## Typed errors, state machines, and scope review

- `apps/wifimic_client/src/main.rs:11-24` uses the typed
  `CalibrationCliError` enum with typed transport, protocol, and calibration
  sources plus dedicated peer/probe/sequence variants. **PASS.**
- `apps/wifimic_client/src/tray.rs:120-128` retains `TrayError` with a boxed
  `#[source]` error for heterogeneous tray operations. **PASS.**
- `apps/wifimic_client/src/control.rs:33-44` retains the explicit
  `Establishing`, `Streaming`, `Unreachable`, and `Stopped` lifecycle.
- `apps/wifimic_server/src/control_support.rs:9-15` retains the explicit
  `Idle`, `Starting`, and `Streaming` lifecycle; `:38-70` retains typed
  `ControlError` variants and typed error sources. **PASS.**
- No stringly-typed error variant or new reference-project subsystem was
  introduced. The refactor adds no product capability or scope creep.

## 250 pure-LOC review

The deterministic F2 counter excludes blank lines, comment lines, test-only
`#[cfg(test)]` spans, and the renderer's `#[path]` module directive; it counts
production code only. Every changed production Rust file and both prior F2
failures were measured:

| File | Physical lines | Pure production LOC | Prior F2 pure LOC | Result |
|---|---:|---:|---:|---|
| `apps/wifimic_client/src/render_windows.rs` | 196 | **175** | 264 (**previous FAIL**) | **PASS** |
| `apps/wifimic_client/src/render_windows_endpoints.rs` | 104 | **97** | new | **PASS** |
| `crates/wifimic_protocol/src/latency.rs` | 108 (14 production before tests) | **12** | 294 (**previous FAIL**) | **PASS** |
| `crates/wifimic_protocol/src/latency/calibration.rs` | 255 | **226** | new | **PASS** |
| `crates/wifimic_protocol/src/latency/measurement.rs` | 79 | **68** | new | **PASS** |

The prior failing files are now below the unchanged 250-LOC ceiling: renderer
264 → 175 and latency 294 → 12. The extracted siblings are also below the
ceiling.

## LSP limitation

Rust `lsp_diagnostics` was requested for all five changed Rust files. The LSP
daemon timed out (the first request after 30 seconds; the remaining requests
returned MCP timeout). This is recorded as an environment limitation only:
all Cargo gates remained clean, and no LSP diagnostic was available to
override the passing compiler, test, build, Clippy, formatting, and diff
results.

**VERDICT: APPROVE**
