use std::process::Command;

use crate::status::{ServiceProperty, ServiceQueries, StatusError};

const PINNED_SOURCE: &str = "alsa_input.pci-0000_00_1b.0.analog-stereo";
const FIREWALL_PORT: &str = "6902";

/// Reports failures from a doctor command that could not run a host query.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub(crate) enum DoctorQueryError {
    /// A host command could not be started.
    #[error("could not invoke {program}: {message}")]
    Invoke { program: String, message: String },
    /// A host command returned bytes that were not UTF-8.
    #[error("{program} output was not valid UTF-8: {message}")]
    InvalidOutput { program: String, message: String },
}

/// Provides the pinned PipeWire source enumeration query.
pub(crate) trait CaptureSourceQueries {
    /// Returns the output of `pactl list short sources`.
    fn list_sources(&self) -> Result<String, DoctorQueryError>;
}

/// Provides read-only firewall ruleset queries.
pub(crate) trait FirewallQueries {
    /// Returns the output of `nft list ruleset`.
    fn nft_ruleset(&self) -> Result<String, DoctorQueryError>;
    /// Returns the output of `iptables -L -n`.
    fn iptables_rules(&self) -> Result<String, DoctorQueryError>;
}

/// Executes the pinned PipeWire query using the host's pactl command.
#[derive(Debug, Default)]
pub(crate) struct NativeCaptureSourceQueries;

impl CaptureSourceQueries for NativeCaptureSourceQueries {
    fn list_sources(&self) -> Result<String, DoctorQueryError> {
        run_command("pactl", &["list", "short", "sources"])
    }
}

/// Executes read-only firewall queries using the host's nft and iptables commands.
#[derive(Debug, Default)]
pub(crate) struct NativeFirewallQueries;

impl FirewallQueries for NativeFirewallQueries {
    fn nft_ruleset(&self) -> Result<String, DoctorQueryError> {
        run_command("nft", &["list", "ruleset"])
    }

    fn iptables_rules(&self) -> Result<String, DoctorQueryError> {
        run_command("iptables", &["-L", "-n"])
    }
}

fn run_command(program: &str, arguments: &[&str]) -> Result<String, DoctorQueryError> {
    let output = Command::new(program)
        .args(arguments)
        .output()
        .map_err(|error| DoctorQueryError::Invoke {
            program: program.to_owned(),
            message: error.to_string(),
        })?;
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|error| DoctorQueryError::InvalidOutput {
            program: program.to_owned(),
            message: error.to_string(),
        })
}

/// Provides one user-service active-state query for doctor.
pub(crate) trait DoctorServiceQuery {
    /// Returns the trimmed active-state value.
    fn active_state(&self) -> Result<String, StatusError>;
}

impl<T> DoctorServiceQuery for T
where
    T: ServiceQueries,
{
    fn active_state(&self) -> Result<String, StatusError> {
        self.query(ServiceProperty::Active)
    }
}

/// Describes one PASS or FAIL item from a doctor report.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct DoctorItem {
    /// The stable check name.
    pub(crate) name: &'static str,
    /// Whether the check passed.
    pub(crate) passed: bool,
    /// Human-readable diagnostic detail.
    pub(crate) detail: String,
}

/// Contains all best-effort doctor checks and their aggregate result.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct DoctorReport {
    /// The embedded server version.
    pub(crate) version: &'static str,
    /// The individual checks in execution order.
    pub(crate) items: Vec<DoctorItem>,
}

impl DoctorReport {
    /// Returns whether every doctor check passed.
    #[must_use]
    pub(crate) fn all_passed(&self) -> bool {
        self.items.iter().all(|item| item.passed)
    }

    pub(crate) fn render(&self) -> String {
        let mut output = format!("版本：{}\n", self.version);
        for item in &self.items {
            let result = if item.passed { "PASS" } else { "FAIL" };
            output.push_str(&format!("{result} {}: {}\n", item.name, item.detail));
        }
        output
    }
}

/// Runs all doctor checks without stopping after the first host failure.
pub(crate) fn run_doctor<S, C, F>(
    service: &S,
    capture: &C,
    firewall: &F,
    version: &'static str,
) -> DoctorReport
where
    S: DoctorServiceQuery,
    C: CaptureSourceQueries,
    F: FirewallQueries,
{
    let service_item = match service.active_state() {
        Ok(state) => DoctorItem {
            name: "wifimic-server active",
            passed: state == "active",
            detail: state,
        },
        Err(error) => DoctorItem {
            name: "wifimic-server active",
            passed: false,
            detail: error.to_string(),
        },
    };
    let source_item = match capture.list_sources() {
        Ok(sources) => DoctorItem {
            name: "pinned PipeWire source",
            passed: sources.contains(PINNED_SOURCE),
            detail: if sources.contains(PINNED_SOURCE) {
                PINNED_SOURCE.to_owned()
            } else {
                format!("{PINNED_SOURCE} not found")
            },
        },
        Err(error) => DoctorItem {
            name: "pinned PipeWire source",
            passed: false,
            detail: error.to_string(),
        },
    };
    let firewall_item = firewall_item(firewall);
    DoctorReport {
        version,
        items: vec![service_item, source_item, firewall_item],
    }
}

fn firewall_item<F: FirewallQueries>(firewall: &F) -> DoctorItem {
    let nft = firewall.nft_ruleset();
    let iptables = firewall.iptables_rules();
    let nft_match = nft
        .as_ref()
        .is_ok_and(|rules| rules.contains(FIREWALL_PORT));
    let iptables_match = iptables
        .as_ref()
        .is_ok_and(|rules| rules.contains(FIREWALL_PORT));
    let passed = nft_match || iptables_match;
    let backend = match (nft_match, iptables_match) {
        (true, true) => "nftables and iptables",
        (true, false) => "nftables",
        (false, true) => "iptables",
        (false, false) => "none",
    };
    let detail = if passed {
        format!("UDP {FIREWALL_PORT} rule found via {backend}")
    } else {
        format!(
            "UDP {FIREWALL_PORT} rule not found via {backend}; nft={}; iptables={}",
            query_detail(nft),
            query_detail(iptables)
        )
    };
    DoctorItem {
        name: "UDP 6902 firewall rule",
        passed,
        detail,
    }
}

fn query_detail(result: Result<String, DoctorQueryError>) -> String {
    match result {
        Ok(output) if output.is_empty() => "empty output".to_owned(),
        Ok(_) => "rule absent".to_owned(),
        Err(error) => error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{run_doctor, CaptureSourceQueries, DoctorQueryError, FirewallQueries};
    use crate::status::{ServiceProperty, ServiceQueries, StatusError};

    #[derive(Debug)]
    struct FakeService;

    impl ServiceQueries for FakeService {
        fn query(&self, property: ServiceProperty) -> Result<String, StatusError> {
            Ok(match property {
                ServiceProperty::Active => "active".to_owned(),
                ServiceProperty::Enabled => "enabled".to_owned(),
            })
        }
    }

    #[derive(Debug)]
    struct FakeCapture;

    impl CaptureSourceQueries for FakeCapture {
        fn list_sources(&self) -> Result<String, DoctorQueryError> {
            Ok("42 alsa_input.pci-0000_00_1b.0.analog-stereo PipeWire".to_owned())
        }
    }

    #[derive(Debug)]
    struct FakeFirewall;

    impl FirewallQueries for FakeFirewall {
        fn nft_ruleset(&self) -> Result<String, DoctorQueryError> {
            Ok("udp dport 6902 accept".to_owned())
        }

        fn iptables_rules(&self) -> Result<String, DoctorQueryError> {
            Err(DoctorQueryError::Invoke {
                program: "iptables".to_owned(),
                message: "not installed".to_owned(),
            })
        }
    }

    #[test]
    fn reports_pass_when_service_source_and_one_firewall_backend_are_ready() {
        // Given
        let service = FakeService;
        let capture = FakeCapture;
        let firewall = FakeFirewall;

        // When
        let report = run_doctor(&service, &capture, &firewall, "v0.1.12");

        // Then
        assert!(report.all_passed());
        assert_eq!(report.items.len(), 3);
        assert!(report.items[2].detail.contains("nftables"));
    }

    #[test]
    fn reports_fail_when_neither_firewall_backend_has_the_port_rule() {
        // Given
        let service = FakeService;
        let capture = FakeCapture;
        let firewall = MissingFirewall;

        // When
        let report = run_doctor(&service, &capture, &firewall, "v0.1.12");

        // Then
        assert!(!report.all_passed());
        assert!(!report.items[2].passed);
    }

    #[derive(Debug)]
    struct MissingFirewall;

    impl FirewallQueries for MissingFirewall {
        fn nft_ruleset(&self) -> Result<String, DoctorQueryError> {
            Err(DoctorQueryError::Invoke {
                program: "nft".to_owned(),
                message: "not installed".to_owned(),
            })
        }

        fn iptables_rules(&self) -> Result<String, DoctorQueryError> {
            Err(DoctorQueryError::Invoke {
                program: "iptables".to_owned(),
                message: "not installed".to_owned(),
            })
        }
    }
}
