# Windows update moves from a source-build PowerShell script to a self-updater binary

**Status**: accepted

Until v0.2.0, updating an installed Windows client required `deploy/windows/update-wifimic-client.ps1`, which needed a full dev environment on the target machine (`git`, `cargo`, a clean source checkout) to rebuild `wifimic_client.exe` from source before swapping it in — effectively a developer-machine-only update path, unlike the Linux `wifimic_server upgrade` command, which downloads a prebuilt, checksum-verified GitHub release binary with no toolchain required.

v0.2.0 replaces this with `wifimic_client_updater.exe`, a standalone executable installed alongside `wifimic_client.exe` that a user double-clicks to check GitHub for the latest release and install it — mirroring the Linux binary-download model instead of the source-build model. It exists as a separate process because a running executable cannot atomically replace its own file. It carries an embedded UAC manifest (elevation is required to touch the `\wifimic\wifimic-client` Scheduled Task and `C:\Program Files\wifimic-client\`), supports only the latest public release (no `--tag`, unlike `wifimic_server upgrade`), and follows the same stop → atomic-swap → restore → start → health-check rollback contract as both the prior script and the Linux upgrade path.

`deploy/windows/update-wifimic-client.ps1` is deleted outright rather than kept alongside the new binary — no test suite referenced it, and keeping two update mechanisms for the same install would invite drift. `docs/release-process.md` and `docs/deployment.md` must be updated to verify deployments via `wifimic_client_updater.exe` instead of the script; because the updater only installs "latest," the mandatory per-release Windows verification step can no longer pin an exact tag the way the Linux `upgrade --tag` step still can.

On the Linux side, the existing `wifimic_server check-update` command is renamed to `wifimic_server update` in the same release, to match the CLI verb the v0.2.0 spec used loosely; its behavior (check only, no download) is unchanged. `wifimic_server upgrade` (the actual download-and-install command) keeps its name.

## Considered Options

- **Keep the PowerShell script for CI/dev use, ship the exe for end users.** Rejected: no test suite depends on the script today, and running two Windows update mechanisms for the same install surface was judged more likely to drift out of sync than to provide real value.
- **Give the Windows updater a `--tag` flag to match `wifimic_server upgrade`.** Rejected for v0.2.0 to keep the double-click UX simple; this means the release-process.md Windows verification step can only assert against "latest," not a specific just-published tag.
