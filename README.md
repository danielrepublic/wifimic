# wifimic

`v0.1.0` is the first frozen baseline: Linux captures the fixed raw PipeWire microphone and streams it to the Windows VB-CABLE render endpoint at 48 kHz. It has been manually verified to keep normal speech cadence without frame drops. It does not yet suppress the microphone's analogue background noise.

## Install from GitHub

The two machines have fixed peer addresses: Linux `192.168.0.210` and Windows `192.168.0.200`. Install VB-Audio Virtual Cable first on Windows.

### Windows client

Open an Administrator PowerShell in an interactive session, then run:

```powershell
irm https://github.com/danielrepublic/wifimic/releases/latest/download/install-wifimic-windows.ps1 | iex
```

The bootstrap verifies the release zip SHA-256, then invokes the bundled compiled installer to install `wifimic_client.exe` to `C:\Program Files\wifimic-client`, register the interactive logon task, and scope the UDP 6902 firewall rule to the Linux peer.

#### Manual updates

An already-installed client can check for and apply updates without re-running the install one-liner:

- `wifimic_client check-update` — queries GitHub for a newer release (reports only, does not install)
- `wifimic_client upgrade [--tag vX.Y.Z]` — downloads, verifies, and installs a release with automatic rollback on failure
- The tray `檢查更新…` (“Check for Updates…”) item — prompts for confirmation, then starts the update with UAC elevation

These are manual, one-shot update paths; users do not need to invoke the bundled installer directly.

### Linux server

Run:

```bash
curl -fsSL https://github.com/danielrepublic/wifimic/releases/latest/download/install-wifimic-linux.sh | bash
```

The installer verifies the Linux archive SHA-256, installs the user service, enables linger, and prompts for `sudo` only to install the peer-scoped firewall rule.

To install a specific release rather than the latest, download the release installer and pass its tag as the first argument.

#### Manual updates

An already-installed server can check for and apply updates without re-running the install one-liner:

- `wifimic_server check-update` — queries GitHub for a newer release (reports only, does not install)
- `wifimic_server upgrade [--tag vX.Y.Z]` — downloads, verifies, and installs a release with automatic rollback on failure

These are manual, one-shot commands invoked by the user — not an automatic background update mechanism.

## Verification

Start the Windows client, select `CABLE Output (VB-Audio Virtual Cable)` as the microphone in Discord, and verify normal speech cadence. For detailed operational and network requirements, see [docs/deployment.md](docs/deployment.md).
