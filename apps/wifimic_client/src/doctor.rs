use wifimic_client::task_query::TaskQuery;

#[path = "doctor_native.rs"]
mod native;
pub(crate) use native::{NativeFirewallQuery, NativeInstallQuery, NativeRenderEndpointQuery};

const CLIENT_INSTALL_PATH: &str = r"C:\Program Files\wifimic-client\wifimic_client.exe";
const EXPECTED_FIREWALL_DISPLAY_NAME: &str = "wifimic-client";
const EXPECTED_FIREWALL_PROTOCOL: &str = "UDP";
const EXPECTED_FIREWALL_LOCAL_PORT: &str = "6902";
// Windows reports a single-host `RemoteAddress` filter without a `/32`
// suffix from `Get-NetFirewallAddressFilter`, even when the rule was
// created with an explicit `/32` (confirmed against a live rule created by
// `install-wifimic-client.ps1`). Matching the bare host address here avoids
// a permanent false-negative doctor check against an otherwise correctly
// scoped rule.
const EXPECTED_FIREWALL_REMOTE_ADDRESS: &str = "192.168.0.210";
const EXPECTED_FIREWALL_DIRECTION: &str = "Inbound";
const EXPECTED_FIREWALL_PROFILE: &str = "Any";
const EXPECTED_FIREWALL_ACTION: &str = "Allow";
const EXPECTED_FIREWALL_ENABLED: &str = "True";

/// Reports failures from a doctor check that could not run a host query.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub(crate) enum DoctorQueryError {
    /// A host command could not be started or exited unsuccessfully.
    #[error("could not invoke {operation}: {message}")]
    Invoke {
        operation: &'static str,
        message: String,
    },
    /// A host command returned output that did not match the expected shape.
    #[error("{operation} returned malformed output: {message}")]
    Malformed {
        operation: &'static str,
        message: String,
    },
}

/// Provides the installed-executable existence check.
pub(crate) trait InstallQuery {
    /// Returns whether the canonical client executable exists.
    fn executable_exists(&self) -> bool;
}

/// Provides the render endpoint enumeration query.
pub(crate) trait RenderEndpointQuery {
    /// Returns every enumerable render endpoint's friendly name.
    fn enumerate(&self) -> Result<Vec<String>, DoctorQueryError>;
}

/// Reports every field of the installer-owned firewall rule signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FirewallSignature {
    pub(crate) display_name: String,
    pub(crate) protocol: String,
    pub(crate) local_port: String,
    pub(crate) remote_address: String,
    pub(crate) direction: String,
    pub(crate) profile: String,
    pub(crate) action: String,
    pub(crate) enabled: String,
}

/// Provides the canonical firewall rule's full signature query.
pub(crate) trait FirewallQuery {
    /// Returns the firewall rule's full signature.
    fn signature(&self) -> Result<FirewallSignature, DoctorQueryError>;
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
    /// The embedded client version.
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

/// Groups the injectable read-only queries [`run_doctor`] needs.
pub(crate) struct DoctorQueries<'a, T, Q, R, F> {
    pub(crate) task: &'a T,
    pub(crate) install: &'a Q,
    pub(crate) endpoint: &'a R,
    pub(crate) firewall: &'a F,
}

/// Runs all doctor checks without stopping after the first host failure.
///
/// Every check is read-only: no scheduled-task, firewall, or file mutation
/// occurs, and no elevated session is required.
pub(crate) fn run_doctor<T, Q, R, F>(
    queries: DoctorQueries<'_, T, Q, R, F>,
    version: &'static str,
) -> DoctorReport
where
    T: TaskQuery,
    Q: InstallQuery,
    R: RenderEndpointQuery,
    F: FirewallQuery,
{
    DoctorReport {
        version,
        items: vec![
            install_item(queries.install),
            task_item(queries.task),
            endpoint_item(queries.endpoint),
            firewall_item(queries.firewall),
        ],
    }
}

fn install_item<Q: InstallQuery>(install: &Q) -> DoctorItem {
    let exists = install.executable_exists();
    DoctorItem {
        name: "wifimic_client.exe installed",
        passed: exists,
        detail: if exists {
            CLIENT_INSTALL_PATH.to_owned()
        } else {
            format!("{CLIENT_INSTALL_PATH} not found")
        },
    }
}

fn task_item<T: TaskQuery>(task: &T) -> DoctorItem {
    match task.state() {
        Ok(state) => DoctorItem {
            name: "scheduled task enabled",
            passed: state.enabled,
            detail: format!(
                "enabled={} running={} ready={}",
                state.enabled, state.running, state.ready
            ),
        },
        Err(error) => DoctorItem {
            name: "scheduled task enabled",
            passed: false,
            detail: error.to_string(),
        },
    }
}

fn endpoint_item<R: RenderEndpointQuery>(endpoint: &R) -> DoctorItem {
    match endpoint.enumerate() {
        Ok(endpoints) => {
            let found = endpoints
                .iter()
                .any(|name| name == wifimic_client::render::DEFAULT_RENDER_ENDPOINT);
            DoctorItem {
                name: "render endpoint enumerable",
                passed: found,
                detail: if found {
                    wifimic_client::render::DEFAULT_RENDER_ENDPOINT.to_owned()
                } else {
                    format!(
                        "{} not found among {} enumerated endpoints",
                        wifimic_client::render::DEFAULT_RENDER_ENDPOINT,
                        endpoints.len()
                    )
                },
            }
        }
        Err(error) => DoctorItem {
            name: "render endpoint enumerable",
            passed: false,
            detail: error.to_string(),
        },
    }
}

fn firewall_item<F: FirewallQuery>(firewall: &F) -> DoctorItem {
    match firewall.signature() {
        Ok(signature) => {
            let passed = signature.display_name == EXPECTED_FIREWALL_DISPLAY_NAME
                && signature
                    .protocol
                    .eq_ignore_ascii_case(EXPECTED_FIREWALL_PROTOCOL)
                && signature.local_port == EXPECTED_FIREWALL_LOCAL_PORT
                && signature.remote_address == EXPECTED_FIREWALL_REMOTE_ADDRESS
                && signature
                    .direction
                    .eq_ignore_ascii_case(EXPECTED_FIREWALL_DIRECTION)
                && signature
                    .profile
                    .eq_ignore_ascii_case(EXPECTED_FIREWALL_PROFILE)
                && signature
                    .action
                    .eq_ignore_ascii_case(EXPECTED_FIREWALL_ACTION)
                && signature
                    .enabled
                    .eq_ignore_ascii_case(EXPECTED_FIREWALL_ENABLED);
            DoctorItem {
                name: "UDP 6902 firewall rule",
                passed,
                detail: if passed {
                    format!(
                        "UDP {EXPECTED_FIREWALL_LOCAL_PORT} rule found for {EXPECTED_FIREWALL_REMOTE_ADDRESS}"
                    )
                } else {
                    format!(
                        "firewall signature mismatch: display_name={} protocol={} local_port={} remote_address={} direction={} profile={} action={} enabled={}",
                        signature.display_name,
                        signature.protocol,
                        signature.local_port,
                        signature.remote_address,
                        signature.direction,
                        signature.profile,
                        signature.action,
                        signature.enabled
                    )
                },
            }
        }
        Err(error) => DoctorItem {
            name: "UDP 6902 firewall rule",
            passed: false,
            detail: error.to_string(),
        },
    }
}

#[cfg(test)]
#[path = "doctor_tests.rs"]
mod tests;
