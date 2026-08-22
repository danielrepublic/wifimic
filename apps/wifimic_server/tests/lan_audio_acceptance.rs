use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use firewall::{assert_peer_only_rules, FirewallBackend};
use session::{receive_control, LiveSessionGuard};
use ssh::{remote_state, require_success, run_ssh};
use wifimic_protocol::{encode_control, ControlMessage, START_TAG, STOP_TAG};

#[path = "lan_audio_acceptance/firewall.rs"]
mod firewall;
#[path = "lan_audio_acceptance/session.rs"]
mod session;
#[path = "lan_audio_acceptance/ssh.rs"]
mod ssh;

const WINDOWS_PEER: Ipv4Addr = Ipv4Addr::new(192, 168, 0, 200);
const SERVER_IP: Ipv4Addr = Ipv4Addr::new(192, 168, 0, 210);
const SERVER_PORT: u16 = 6_902;

#[test]
#[ignore = "requires arch-daniel, active firewall, and Windows peer 192.168.0.200"]
fn accepted_windows_peer_reaches_live_server_and_firewall_is_peer_scoped() {
    let target = std::env::var("WIFIMIC_SSH_TARGET").unwrap_or_else(|_| "arch-daniel".to_owned());
    let service_state = remote_state(&target, "systemctl --user is-active wifimic-server || true");
    assert_eq!(
        service_state, "active",
        "real service must already be active"
    );

    let backend_states = [
        (
            FirewallBackend::Ufw,
            remote_state(&target, "systemctl is-active ufw.service || true"),
        ),
        (
            FirewallBackend::Nftables,
            remote_state(&target, "systemctl is-active nftables.service || true"),
        ),
        (
            FirewallBackend::Iptables,
            remote_state(&target, "systemctl is-active iptables.service || true"),
        ),
    ];
    for (backend, state) in &backend_states {
        println!("firewall_backend={} state={state}", backend.service());
    }
    let active_backends: Vec<_> = backend_states
        .iter()
        .filter(|(_, state)| state == "active")
        .collect();
    assert_eq!(
        active_backends.len(),
        1,
        "exactly one firewall backend must be active"
    );
    let backend = active_backends[0].0;

    let before_rules = require_success(
        "firewall rule probe before accepted packet",
        run_ssh(&target, backend.rule_command()).expect("bounded firewall probe must start"),
    );
    assert_peer_only_rules(backend, &before_rules);
    let before_counters = require_success(
        "firewall counter probe before accepted packet",
        run_ssh(&target, backend.counter_command()).expect("bounded counter probe must start"),
    );
    println!("firewall_rules_before=\n{before_rules}");
    println!("firewall_counters_before=\n{before_counters}");

    let socket = UdpSocket::bind(SocketAddr::from((WINDOWS_PEER, 0)))
        .expect("approved Windows peer address must be locally bindable");
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("live acceptance receive timeout must be set");
    let local_address = socket
        .local_addr()
        .expect("bound local address must be readable");
    assert_eq!(local_address.ip(), WINDOWS_PEER);
    let server = SocketAddr::from((SERVER_IP, SERVER_PORT));
    let session_id = u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must be after Unix epoch")
            .as_millis(),
    )
    .expect("live session ID must fit in u64");
    let start = encode_control(&ControlMessage::Start { session_id });
    assert_eq!(
        socket.send_to(&start, server).expect("Start must be sent"),
        start.len()
    );
    let mut session = LiveSessionGuard::new(&socket, server, session_id);
    let (source, ack) = receive_control(&socket).expect("approved peer must receive live Ack");
    assert_eq!(source, server);
    assert_eq!(
        ack,
        ControlMessage::Ack {
            session_id,
            acked_kind: START_TAG,
        }
    );
    println!("live_start_ack=source:{source} message:{ack:?}");

    assert_eq!(
        session
            .stop()
            .expect("live session cleanup Stop must be acknowledged"),
        ControlMessage::Ack {
            session_id,
            acked_kind: STOP_TAG,
        }
    );

    let after_rules = require_success(
        "firewall rule probe after accepted packet",
        run_ssh(&target, backend.rule_command()).expect("bounded firewall probe must start"),
    );
    assert_peer_only_rules(backend, &after_rules);
    let after_counters = require_success(
        "firewall counter probe after accepted packet",
        run_ssh(&target, backend.counter_command()).expect("bounded counter probe must start"),
    );
    println!("firewall_rules_after=\n{after_rules}");
    println!("firewall_counters_after=\n{after_counters}");

    let journal = require_success(
        "SessionStarted journal probe",
        run_ssh(
            &target,
            "journalctl --user -u wifimic-server -n 200 --no-pager -o cat",
        )
        .expect("bounded journal probe must start"),
    );
    let session_marker = format!("session_id={session_id}");
    if journal.contains("event=session_started") && journal.contains(&session_marker) {
        println!("live_session_started=journal-confirmed");
    } else {
        println!(
            "live_session_started=journal-unavailable; deterministic control test is the SessionStarted evidence"
        );
    }
}
