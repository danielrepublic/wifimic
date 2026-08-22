#[derive(Clone, Copy, Debug)]
pub(super) enum FirewallBackend {
    Ufw,
    Nftables,
    Iptables,
}

impl FirewallBackend {
    pub(super) const fn service(self) -> &'static str {
        match self {
            Self::Ufw => "ufw.service",
            Self::Nftables => "nftables.service",
            Self::Iptables => "iptables.service",
        }
    }

    pub(super) const fn rule_command(self) -> &'static str {
        match self {
            Self::Ufw => "sudo -n ufw status numbered",
            Self::Nftables => "sudo -n nft list chain inet wifimic_server input",
            Self::Iptables => "sudo -n iptables -S INPUT",
        }
    }

    pub(super) const fn counter_command(self) -> &'static str {
        match self {
            Self::Ufw => "sudo -n nft -a list chain ip filter ufw-user-input",
            Self::Nftables => "sudo -n nft -a list chain inet wifimic_server input",
            Self::Iptables => "sudo -n iptables -L INPUT -v -n -x --line-numbers",
        }
    }
}

fn normalize(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(super) fn assert_peer_only_rules(backend: FirewallBackend, rules: &str) {
    match backend {
        FirewallBackend::Ufw => {
            let rules = normalize(rules);
            assert!(rules.contains("6902/udp ALLOW IN 192.168.0.200"));
            assert!(rules.contains("6902/udp DENY IN Anywhere"));
            assert!(!rules.contains("6902/udp ALLOW IN Anywhere"));
        }
        FirewallBackend::Nftables => {
            let has_peer_accept = rules.lines().any(|line| {
                line.contains("ip saddr 192.168.0.200")
                    && line.contains("udp dport 6902")
                    && line.contains("accept")
            });
            let has_scoped_drop = rules
                .lines()
                .any(|line| line.contains("udp dport 6902") && line.contains("drop"));
            let has_broad_accept = rules.lines().any(|line| {
                !line.contains("ip saddr 192.168.0.200")
                    && line.contains("udp dport 6902")
                    && line.contains("accept")
            });
            assert!(has_peer_accept, "nftables peer allow rule missing: {rules}");
            assert!(
                has_scoped_drop,
                "nftables scoped drop rule missing: {rules}"
            );
            assert!(
                !has_broad_accept,
                "nftables broad UDP 6902 allow found: {rules}"
            );
        }
        FirewallBackend::Iptables => {
            let rules = normalize(rules);
            assert!(rules.contains("-A INPUT -p udp -s 192.168.0.200 --dport 6902 -j ACCEPT"));
            assert!(rules.contains("-A INPUT -p udp --dport 6902 -j DROP"));
            assert!(!rules.contains("-A INPUT -p udp --dport 6902 -j ACCEPT"));
        }
    }
}
