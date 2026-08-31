# Windows update moves from a self-updater binary to a client command with a transient handoff script

**Status**: accepted

Until v0.2.0, updating an installed Windows client required a PowerShell updater script, which needed a full development environment on the target machine (`git`, `cargo`, and a clean source checkout) to rebuild `wifimic_client.exe` from source before swapping it in. v0.2.0 replaced that developer-only path with `wifimic_client_updater.exe`, a separate installed executable that could replace the client while it was stopped.

The self-updater binary is now retired. Windows instead installs `wifimic_client` onto the system PATH and gives it the same one-shot command interface as Linux: `update`, `upgrade [latest|vX.Y.Z]`, `status`, and `doctor`. `update` is non-mutating; `upgrade` is the explicit installation action and supports both `latest` and an explicit version, including deliberate downgrade.

Windows cannot reliably replace a running executable. Therefore `wifimic_client upgrade` generates an embedded, short-lived PowerShell handoff script, starts it elevated through UAC, and exits. The script waits for the client process to end, performs the verified transaction, restarts the scheduled client task, health-checks it, and deletes itself. It is not a persistent update executable. A failure before backup makes no mutation; a later failure rolls back the client and task state. The GitHub Release `latest/download` installer remains the first-installation and repair-installation path; it removes any legacy `wifimic_client_updater.exe`.

Both platforms resolve an explicit Update Target the same way: no target or `latest` selects the latest public release, while `vX.Y.Z` selects that published release. `latest` is authoritative even if it is older than the installed version; equal targets are no-ops. The shared transaction and rollback logic belongs in `wifimic_update`; Windows and Linux provide their platform-specific adapters.

## Consequences

- Windows and Linux documentation and release verification use the same command vocabulary.
- The Windows release archive and repair installer no longer ship or preserve the legacy Updater.
- The transient handoff script is generated from an embedded template; it is not fetched as executable code from GitHub. The Release artifacts it installs remain SHA-256 verified.
- The primary shared tests exercise `wifimic_update` through fake adapters. Windows and Linux retain focused tests for their platform-specific adapters.

## Considered Options

- **Keep `wifimic_client_updater.exe`.** Rejected: it gives Windows a different command surface and creates a second persistent executable with its own self-update problem.
- **Install a permanent third bootstrapper.** Rejected: it adds another persistent program that would eventually require self-update.
- **Fetch a transient handoff script from GitHub at update time.** Rejected: the embedded template avoids introducing a second remote-code acquisition path; only the verified Release artifacts are downloaded.
