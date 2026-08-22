#!/usr/bin/env bash
set -euo pipefail

readonly PEER_IP="192.168.0.200"
readonly UDP_PORT="6902"
readonly SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly IPTABLES_RULES="/etc/iptables/iptables.rules"

die() {
    printf 'wifimic firewall setup aborted: %s\n' "$1" >&2
    exit 1
}

require_root() {
    [[ "${EUID}" -eq 0 ]] || die 'run as root (for example: sudo bash deploy/linux/wifimic-server-firewall.sh)'
}

is_active() {
    systemctl is-active --quiet "$1"
}

apply_nft_rules() {
    nft delete table inet wifimic_server 2>/dev/null || true
    nft -f "$SCRIPT_DIR/wifimic-server.nft"
    nft list chain inet wifimic_server input | grep -Fq 'ip saddr 192.168.0.200 udp dport 6902 counter' || die 'nftables peer accept rule was not present after apply'
    nft list chain inet wifimic_server input | grep -Fq 'udp dport 6902 counter drop' || die 'nftables scoped drop rule was not present after apply'
}

require_root

iptables_active=0
nftables_active=0
ufw_active=0
is_active iptables.service && iptables_active=1 || true
is_active nftables.service && nftables_active=1 || true
is_active ufw.service && ufw_active=1 || true

if [[ "$iptables_active" -eq 1 && "$nftables_active" -eq 1 ]]; then
    die 'iptables.service and nftables.service are both active; refusing to guess or mutate firewall state'
fi

if [[ "$ufw_active" -eq 1 && ("$iptables_active" -eq 1 || "$nftables_active" -eq 1) ]]; then
    die 'ufw.service and another firewall backend are both active; refusing to create a second packet-filtering path'
fi

if [[ "$iptables_active" -eq 1 ]]; then
    [[ -f "$IPTABLES_RULES" ]] || die "$IPTABLES_RULES is absent while iptables.service is active; refusing alternate persistence"
    bash "$SCRIPT_DIR/wifimic-server-iptables.sh"
    iptables-save | tee "$IPTABLES_RULES" >/dev/null
    systemctl restart iptables.service
    iptables -C INPUT -p udp -s "$PEER_IP" --dport "$UDP_PORT" -j ACCEPT
    iptables -C INPUT -p udp --dport "$UDP_PORT" -j DROP
    exit 0
fi

if [[ "$nftables_active" -eq 1 ]]; then
    apply_nft_rules
    exit 0
fi

# UFW is an active firewall manager on some Arch installations. It owns the
# iptables-nft ruleset here, so never enable nftables.service beside it.
if [[ "$ufw_active" -eq 1 ]]; then
    command -v ufw >/dev/null 2>&1 || die 'ufw.service is active but the ufw command is unavailable'
    current_rules="$(ufw status numbered 2>/dev/null || true)"
    if grep -Eq '(^|[[:space:]])6902/udp[[:space:]]+ALLOW IN[[:space:]]+Anywhere([[:space:]]|$)' <<<"$current_rules"; then
        die 'ufw already exposes UDP 6902 beyond the required peer; refusing to widen exposure'
    fi
    changed=0
    if ! grep -Eq '(^|[[:space:]])6902/udp[[:space:]]+ALLOW IN[[:space:]]+192\.168\.0\.200([[:space:]]|$)' <<<"$current_rules"; then
        ufw insert 1 allow from "$PEER_IP" to any port "$UDP_PORT" proto udp comment 'wifimic-server peer'
        changed=1
    fi
    if ! grep -Eq '(^|[[:space:]])6902/udp[[:space:]]+DENY IN[[:space:]]+Anywhere([[:space:]]|$)' <<<"$current_rules"; then
        ufw deny "$UDP_PORT"/udp comment 'wifimic-server default drop'
        changed=1
    fi
    if [[ "$changed" -eq 1 ]]; then
        ufw reload
    fi
    final_rules="$(ufw status numbered 2>/dev/null || true)"
    grep -Eq '(^|[[:space:]])6902/udp[[:space:]]+ALLOW IN[[:space:]]+192\.168\.0\.200([[:space:]]|$)' <<<"$final_rules" || die 'UFW peer allow rule was not present after reload'
    grep -Eq '(^|[[:space:]])6902/udp[[:space:]]+DENY IN[[:space:]]+Anywhere([[:space:]]|$)' <<<"$final_rules" || die 'UFW scoped drop rule was not present after reload'
    exit 0
fi

# No supported firewall service is active: follow the plan's nftables fallback.
command -v pacman >/dev/null 2>&1 || die 'neither firewall backend is active and pacman is unavailable'
command -v nft >/dev/null 2>&1 || pacman -S --noconfirm nftables
apply_nft_rules
systemctl enable --now nftables.service
