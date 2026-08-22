#!/usr/bin/env bash
set -euo pipefail

readonly PEER_IP="192.168.0.200"
readonly UDP_PORT="6902"

add_rule_once() {
    local chain="$1"
    shift

    if ! iptables -C "$chain" "$@" 2>/dev/null; then
        iptables -A "$chain" "$@"
    fi
}

# The explicit drop is scoped to wifimic's UDP port. Other INPUT policy and
# rules remain owned by the active iptables service.
add_rule_once INPUT -p udp -s "$PEER_IP" --dport "$UDP_PORT" -j ACCEPT
add_rule_once INPUT -p udp --dport "$UDP_PORT" -j DROP
