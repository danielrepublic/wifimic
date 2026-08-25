#!/usr/bin/env bash
set -Eeuo pipefail

readonly REMOTE_SSH_TARGET='daniel@192.168.0.200'
readonly SSH_CONNECT_TIMEOUT_SECONDS=10
readonly DEFAULT_WINDOWS_SMOKE_EXE='C:\Users\Daniel\Documents\opencode\wifimic\target\release\wifimic_control_smoke.exe'
readonly WINDOWS_SMOKE_EXE="${WIFIMIC_WINDOWS_SMOKE_EXE:-$DEFAULT_WINDOWS_SMOKE_EXE}"

if (( $# != 2 )); then
    printf 'usage: %s HOST PORT\n' "${BASH_SOURCE[0]}" >&2
    exit 2
fi

command -v ssh >/dev/null 2>&1 || {
    printf '%s\n' 'wifimic control smoke helper: ssh is required' >&2
    exit 1
}
command -v base64 >/dev/null 2>&1 || {
    printf '%s\n' 'wifimic control smoke helper: base64 is required' >&2
    exit 1
}

encode_for_remote() {
    printf '%s' "$1" | base64 | tr -d '\n'
}

readonly REMOTE_EXE_B64="$(encode_for_remote "$WINDOWS_SMOKE_EXE")"
readonly SERVER_HOST_B64="$(encode_for_remote "$1")"
readonly SERVER_PORT_B64="$(encode_for_remote "$2")"

printf '%s\n' \
    "\$exe = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('$REMOTE_EXE_B64'))" \
    "\$serverHost = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('$SERVER_HOST_B64'))" \
    "\$serverPort = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('$SERVER_PORT_B64'))" \
    '& $exe $serverHost $serverPort' \
    'exit $LASTEXITCODE' |
    ssh \
        -o BatchMode=yes \
        -o StrictHostKeyChecking=yes \
        -o "ConnectTimeout=$SSH_CONNECT_TIMEOUT_SECONDS" \
        "$REMOTE_SSH_TARGET" \
        powershell.exe -NoProfile -NonInteractive -Command -
