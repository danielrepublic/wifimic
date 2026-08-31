# wifimic

`v0.1.0` is the first frozen baseline: Linux captures the fixed raw PipeWire microphone and streams it to the Windows VB-CABLE render endpoint at 48 kHz. It has been manually verified to keep normal speech cadence without frame drops. It does not yet suppress the microphone's analogue background noise.

## Install from GitHub

The two machines have fixed peer addresses: Linux `192.168.0.210` and Windows `192.168.0.200`. Install VB-Audio Virtual Cable first on Windows.

### Windows client

Open an Administrator PowerShell in an interactive session, then run:

```powershell
irm https://github.com/danielrepublic/wifimic/releases/latest/download/install-wifimic-windows.ps1 | iex
```

The installer verifies the release zip SHA-256, installs `wifimic_client.exe` to `C:\Program Files\wifimic-client`, registers the interactive logon task, and scopes the UDP 6902 firewall rule to the Linux peer.

#### Manual updates

An already-installed client can check for and apply updates from any PowerShell or Command Prompt window (no Administrator required for the check; `upgrade` will prompt for UAC elevation):

| Command | Effect |
|---------|--------|
| `wifimic_client update` | Queries GitHub and reports whether a newer release is available. Does not install anything. |
| `wifimic_client upgrade` | Downloads, verifies, and installs the latest release. |
| `wifimic_client upgrade vX.Y.Z` | Downloads and installs a specific release tag. |
| `wifimic_client status` | Reports the installed version and scheduled-task state. |
| `wifimic_client doctor` | Runs a one-shot host self-check (VB-CABLE endpoint, network reachability). |

Running `upgrade` launches a short-lived UAC-elevated handoff script that replaces the binary while the normal audio client exits. Once the script completes, the logon task starts the new client automatically. There are no automatic or background updates. For the full upgrade flow and troubleshooting, see [docs/deployment.md](docs/deployment.md).

### Linux server

Run:

```bash
curl -fsSL https://github.com/danielrepublic/wifimic/releases/latest/download/install-wifimic-linux.sh | bash
```

The installer verifies the Linux archive SHA-256, installs the user service, enables linger, and creates the `wifimic_server` command in `/usr/local/bin`. It prompts for `sudo` to install the command entry and the peer-scoped firewall rule.

To install a specific release rather than the latest, download the release installer and pass its tag as the first argument.

#### Manual updates

An already-installed server can check for and apply updates without re-running the install one-liner:

- `wifimic_server update` — queries GitHub for a newer release (reports only, does not install)
- `wifimic_server upgrade [latest|vX.Y.Z]` — downloads, verifies, and installs a release with automatic rollback on failure

These are manual, one-shot commands invoked by the user — not an automatic background update mechanism.

## Verification

Start the Windows client, select `CABLE Output (VB-Audio Virtual Cable)` as the microphone in Discord, and verify normal speech cadence. For detailed operational and network requirements, see [docs/deployment.md](docs/deployment.md).
