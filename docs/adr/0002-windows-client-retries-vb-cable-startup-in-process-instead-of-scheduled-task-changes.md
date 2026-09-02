# Windows client now retries opening VB-CABLE render endpoint in-process for bounded 60-second window (2-second interval) at startup, instead of changing Scheduled Task

**Status**: accepted

Modern Standby wake re-triggers the LogonTrigger before the audio subsystem has finished re-initializing. This causes the single-attempt open to fail, and the windowless client exits silently.

Empirical root cause:
- Historical diagnostic log files were consistently truncated at just the file header.
- Timestamps closely tracked Windows wake-from-Modern-Standby system events.
- Scheduled Task's last recorded result was a non-zero exit code.
- Manual un-triggered launch always succeeded once the audio subsystem had already settled.

The client now implements a bounded in-process retry loop during startup. It is given a total budget of 60 seconds to successfully open the render endpoint, attempting a reconnection every 2 seconds (approximately 30 attempts). This is implemented via an injectable seam (`retry_bounded`) to allow for deterministic testing. If the budget is exhausted, the client emits a `Event::RenderStartupRetryExhausted` diagnostic event. The Scheduled Task XML and installer are left unchanged.

## Consequences

- Startup latency is unchanged on an already-warm login, as the retry loop's first attempt still succeeds immediately.
- A genuinely broken installation (e.g. VB-CABLE uninstalled) now waits within the bounded 60-second retry window before reporting failure and exiting, instead of failing instantly. This trade-off is accepted because the failure is now visible via the diagnostic event, whereas today's instant failure is silent.

## Considered Options

- **Scheduled Task `Delay` trigger setting.** Rejected: a fixed guess that either wastes time on every already-warm login or is still too short on a slow wake; the adaptive in-process retry dominates it in both directions.
- **Scheduled Task `RestartOnFailure`.** Rejected for this fix: the in-process retry already covers the readiness race; noted as a possible future defense-in-depth addition, explicitly out of scope here.
- **Windows Service.** Rejected: contradicts the project's existing "no system-level service" design decision (reference CONTEXT.md Manual Update / Windows Update Handoff Script terms and ADR-0001 consequences which both assume no persistent system service).
