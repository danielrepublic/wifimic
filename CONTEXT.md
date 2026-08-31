# wifimic

wifimic streams a Linux PipeWire microphone capture over LAN UDP to a Windows VB-CABLE virtual microphone endpoint, so a fixed peer pair can share one physical mic input.

## Language

**Manual Update**:
A user-triggered action that checks for or installs a release. wifimic deliberately has no background/automatic update mechanism. On both platforms, invoking the `upgrade` command is the user's explicit installation action. When the selected Update Target already matches the installed version, the update makes no mutation and exits normally.
_Avoid_: Auto-update, background update

**Initial Installation**:
The first deployment of wifimic to either peer. The user runs the platform-specific GitHub Release `latest/download` command locally on each peer, which installs the latest published and verified release without a source checkout or development toolchain. It installs `wifimic_server` on Linux and `wifimic_client` on Windows; the Windows installer also adds its install directory to the system PATH and removes a legacy Windows Updater if one exists. Installation immediately starts the platform process. It is complete only after the process, firewall configuration, and a platform-local health check have all succeeded. Re-running the Initial Installation command on an already installed peer is a repair installation: it redeploys `latest`, restores the installation artifacts, starts the process, and health-checks it. If it fails, it removes only artifacts created by that attempt so the user can retry cleanly, without deleting pre-existing artifacts.
_Avoid_: Source installation, development-machine installation

**Update Target**:
The release selected for a Manual Update. `latest` means the newest public release; an explicit version means that published release. Both platforms can target either `latest` or an explicit version, including an older installed version when deliberately selected. On both platforms, `latest` is authoritative: it is installed even when it is older than the installed version.
_Avoid_: Update version (ambiguous between the installed version and the selected release)

**Release Artifact Fingerprint Verification**:
The shared SHA-256 check that confirms a downloaded release archive matches its published checksum manifest before either platform extracts or installs it. It validates only the manifest and archive bytes; downloading, archive-path safety, extraction, and platform-specific error presentation remain outside this verification.
_Avoid_: Archive verification (too broad), platform-specific checksum check

**Platform Command Interface**:
The one-shot command set that both `wifimic_server` (Linux) and `wifimic_client` (Windows) expose: `update` checks the installed and latest versions without mutation; `upgrade [latest|vX.Y.Z]` installs an Update Target; `status` reports local process state; `doctor` checks local installation prerequisites. Without one of these subcommands, each executable retains its normal audio-process behavior.
_Avoid_: check-update (superseded name), platform-specific command vocabulary

**Windows Update Handoff Script**:
The short-lived PowerShell script generated from an embedded template when `wifimic_client upgrade` needs to replace the Windows client executable. The client starts it elevated through UAC, exits, and the script waits for that process to end before performing the verified update transaction, restarting the client, health-checking it, and deleting itself. It replaces the retired `wifimic_client_updater.exe`; it is not a persistent installed program.
_Avoid_: Updater, bootstrapper, persistent update executable

**更新合約 (Update Contract)**:
The shared update transaction both platforms follow: resolve version → download + SHA-256 verify → backup → stop → atomic-swap → start → health-check, with automatic rollback to the prior binary on any failure. A failure before backup makes no mutation; a failure after backup triggers rollback. Its outcomes distinguish a verified rollback from a rollback that could not be fully verified. It is the single concept that Windows `wifimic_client upgrade` and Linux `wifimic_server upgrade` both realize. By design this contract converges into one deep module in the `wifimic_update` crate, which exposes a narrow platform-adapter trait and owns the transaction order plus rollback; Windows (an elevated Windows Update Handoff Script plus Scheduled Task) and Linux (systemd) are the two real adapters behind that seam. Each adapter owns its platform-specific archive handling, process lifecycle, and health check details.
_Avoid_: update workflow (too vague), upgrade path (Linux-only framing), update sequence (too generic)
