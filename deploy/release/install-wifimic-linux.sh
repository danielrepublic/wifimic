#!/usr/bin/env bash
set -Eeuo pipefail

readonly REPOSITORY='danielrepublic/wifimic'
readonly ASSET_NAME='wifimic-linux-x86_64.tar.gz'
readonly SERVICE_NAME='wifimic-server'

tag="${1:-}"
release_segment='latest/download'
if [[ -n "$tag" ]]; then
    release_segment="download/$tag"
fi
asset_base="https://github.com/${REPOSITORY}/releases/${release_segment}"
temporary_root="$(mktemp -d "${TMPDIR:-/tmp}/wifimic-release.XXXXXX")"

cleanup() {
    rm -rf -- "$temporary_root"
}
trap cleanup EXIT

require_command() {
    command -v "$1" >/dev/null 2>&1 || {
        printf 'wifimic install failed: required command is absent: %s\n' "$1" >&2
        exit 1
    }
}

require_command curl
require_command sha256sum
require_command tar
require_command systemctl
require_command parec
require_command sudo

archive_path="$temporary_root/$ASSET_NAME"
checksum_path="$temporary_root/$ASSET_NAME.sha256"
curl --fail --location --silent --show-error "$asset_base/$ASSET_NAME" --output "$archive_path"
curl --fail --location --silent --show-error "$asset_base/$ASSET_NAME.sha256" --output "$checksum_path"

(
    cd -- "$temporary_root"
    sha256sum --check --status "$(basename -- "$checksum_path")"
) || {
    printf 'wifimic install failed: checksum verification failed for %s\n' "$ASSET_NAME" >&2
    exit 1
}

stage_path="$temporary_root/stage"
mkdir -p -- "$stage_path"
tar --extract --gzip --file "$archive_path" --directory "$stage_path"

server_binary="$stage_path/wifimic_server"
unit_file="$stage_path/wifimic-server.service"
firewall_script="$stage_path/wifimic-server-firewall.sh"
for required_file in "$server_binary" "$unit_file" "$firewall_script"; do
    [[ -f "$required_file" ]] || {
        printf 'wifimic install failed: verified archive is missing %s\n' "$(basename -- "$required_file")" >&2
        exit 1
    }
done

install -Dm755 "$server_binary" "$HOME/.local/bin/wifimic_server"
install -Dm644 "$unit_file" "${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/wifimic-server.service"
sudo loginctl enable-linger "$USER"
systemctl --user daemon-reload
systemctl --user enable --now "$SERVICE_NAME"
sudo bash "$firewall_script"
systemctl --user is-active --quiet "$SERVICE_NAME"
printf 'wifimic Linux server installed from %s.\n' "${tag:-the latest GitHub release}"
