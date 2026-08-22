# Problems — wifimic-lan-audio

Unresolved blockers and technical debt discovered during work on this plan.

_Auto-scaffolded by /start-work. Append new entries below - never overwrite._

---

## 2026-08-22

- The Todo 19 live Ack scenario cannot pass against the currently active remote ELF: the host firewall accepts the real peer packet, but the installed binary predates the current control-plane loop. This remains an explicit deployment limitation rather than a reason to weaken the harness or mutate the remote service during this task.

## 2026-08-22

- No live `arch-daniel` updater run was attempted: its required peer-originated control smoke cannot be honestly substituted by a localhost datagram because the production server accepts only `192.168.0.200`.

## 2026-08-22

- The live peer helper is still an external prerequisite: no helper that can originate from `192.168.0.200` is available here, so live update success remains intentionally unclaimed.

## 2026-08-22

- No unresolved Task 12 implementation blocker remains. Live two-host control/audio behavior is intentionally deferred to the later deployment and Wave 5 acceptance todos; this task's evidence makes no hardware or firewall claim.

## 2026-08-22

- The Todo 19 live Ack scenario cannot pass against the currently active remote ELF: the host firewall accepts the real peer packet, but the installed binary predates the current control-plane loop. This remains an explicit deployment limitation rather than a reason to weaken the harness or mutate the remote service during this task.
