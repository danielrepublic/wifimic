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

- Task 16 was verified with a faithful isolated fake-command harness rather than `arch-daniel`: the Windows checkout is intentionally dirty from orchestration files, and the real service/control smoke requires an approved source at peer `192.168.0.200`; no live update or end-to-end capture claim is made here.

## 2026-08-22

- Task 12's Rust LSP requests timed out on the client files as expected from the inherited workspace limitation. Cargo focused tests, workspace tests/build, and workspace Clippy all passed; no live hardware claim was substituted.

## 2026-08-22

- Todo 19 live Ack acceptance is blocked by the active remote ELF being older than the current server control-loop commits. The real Start packet reaches the UFW peer accept rule, but the stale listener returns no Ack within the bounded 5-second receive window. Rebuild/install/restart is intentionally not included because this task forbids changing service state; rerun the ignored harness after a normal deployment refresh.
