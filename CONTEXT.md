# wifimic

wifimic streams a Linux PipeWire microphone capture over LAN UDP to a Windows VB-CABLE virtual microphone endpoint, so a fixed peer pair can share one physical mic input.

## Language

**Manual Update**:
A user-triggered action that checks for or installs a newer release. wifimic deliberately has no background/automatic update mechanism — every update, on either platform, is invoked by a human.
_Avoid_: Auto-update, background update

**`wifimic_server update`** (Linux, v0.2.0+):
The one-shot CLI command that checks GitHub for a newer public release tag and reports the result without downloading or mutating anything. Renamed from `check-update` in v0.2.0.
_Avoid_: check-update (superseded name)

**`wifimic_server upgrade`** (Linux):
The one-shot CLI command that downloads, verifies, and atomically installs a release binary, restarting the systemd user service and automatically rolling back to the prior binary on any failure. Accepts an optional `--tag vX.Y.Z`; defaults to the latest public release.

**Updater** (Windows, v0.2.0+):
`wifimic_client_updater.exe` — a standalone executable, separate from `wifimic_client.exe`, that a user double-clicks to check GitHub for the latest public release and install it. It exists because a running process cannot atomically replace its own executable file. Installed into `C:\Program Files\wifimic-client\` alongside the client during initial install, so the user can re-launch it locally at any time. Supports only the latest release (no explicit `--tag`). Carries an embedded UAC manifest (`requireAdministrator`) so a double-click prompts for elevation, and follows the same stop-task → atomic swap → restore-task → start-task → health-check rollback contract as the Linux `upgrade` path.
_Avoid_: wifi-client.exe (typo in early drafts), wifimic-client-updater.exe (hyphenated form from early drafts — the shipped binary keeps the repo's underscore executable-naming convention)

**Console Wait-for-Keypress Convention**:
The CLI-style pattern this repo uses for one-shot Windows tools that a user launches by double-click: keep the console window, print human-readable status text (checking / up to date / updating / done / failed), and wait for a keypress before the window closes. Used by `wifimic_client_updater.exe` instead of a GUI dialog or toast notification, to stay consistent with the existing `wifimic_server` CLI and deployment scripts.
