use super::native::parse_firewall_signature;
use super::{
    run_doctor, DoctorQueries, DoctorQueryError, FirewallQuery, FirewallSignature, InstallQuery,
    RenderEndpointQuery,
};
use wifimic_client::task_query::{TaskQuery, TaskQueryError, TaskState};

#[derive(Debug, Clone)]
struct FakeTaskQuery(Result<TaskState, TaskQueryError>);

impl TaskQuery for FakeTaskQuery {
    fn state(&self) -> Result<TaskState, TaskQueryError> {
        self.0.clone()
    }
}

#[derive(Debug, Clone, Copy)]
struct FakeInstallQuery(bool);

impl InstallQuery for FakeInstallQuery {
    fn executable_exists(&self) -> bool {
        self.0
    }
}

#[derive(Debug, Clone)]
struct FakeEndpointQuery(Result<Vec<String>, DoctorQueryError>);

impl RenderEndpointQuery for FakeEndpointQuery {
    fn enumerate(&self) -> Result<Vec<String>, DoctorQueryError> {
        self.0.clone()
    }
}

#[derive(Debug, Clone)]
struct FakeFirewallQuery(Result<FirewallSignature, DoctorQueryError>);

impl FirewallQuery for FakeFirewallQuery {
    fn signature(&self) -> Result<FirewallSignature, DoctorQueryError> {
        self.0.clone()
    }
}

fn passing_signature() -> FirewallSignature {
    FirewallSignature {
        display_name: "wifimic-client".to_owned(),
        protocol: "UDP".to_owned(),
        local_port: "6902".to_owned(),
        remote_address: "192.168.0.210".to_owned(),
        direction: "Inbound".to_owned(),
        profile: "Any".to_owned(),
        action: "Allow".to_owned(),
        enabled: "True".to_owned(),
    }
}

fn passing_task_state() -> TaskState {
    TaskState {
        enabled: true,
        running: false,
        ready: true,
    }
}

fn passing_endpoints() -> Vec<String> {
    vec!["CABLE Input (VB-Audio Virtual Cable)".to_owned()]
}

#[test]
fn parses_the_eight_expected_firewall_fields_in_order() {
    // Given
    let output =
        "wifimic-client\r\nUDP\r\n6902\r\n192.168.0.210\r\nInbound\r\nAny\r\nAllow\r\nTrue\r\n";

    // When
    let signature = parse_firewall_signature(output).expect("firewall signature parses");

    // Then
    assert_eq!(signature, passing_signature());
}

#[test]
fn rejects_firewall_output_missing_a_field() {
    // Given
    let output = "wifimic-client\r\nUDP\r\n";

    // When
    let result = parse_firewall_signature(output);

    // Then
    assert!(matches!(result, Err(DoctorQueryError::Malformed { .. })));
}

#[test]
fn reports_all_pass_when_every_check_succeeds() {
    // Given
    let task = FakeTaskQuery(Ok(passing_task_state()));
    let install = FakeInstallQuery(true);
    let endpoint = FakeEndpointQuery(Ok(passing_endpoints()));
    let firewall = FakeFirewallQuery(Ok(passing_signature()));

    // When
    let report = run_doctor(
        DoctorQueries {
            task: &task,
            install: &install,
            endpoint: &endpoint,
            firewall: &firewall,
        },
        "v0.1.12",
    );

    // Then
    assert!(report.all_passed());
    assert_eq!(report.items.len(), 4);
    assert_eq!(report.version, "v0.1.12");
}

#[test]
fn reports_failure_when_the_render_endpoint_is_not_enumerable() {
    // Given
    let task = FakeTaskQuery(Ok(passing_task_state()));
    let install = FakeInstallQuery(true);
    let endpoint = FakeEndpointQuery(Ok(vec!["Speakers".to_owned()]));
    let firewall = FakeFirewallQuery(Ok(passing_signature()));

    // When
    let report = run_doctor(
        DoctorQueries {
            task: &task,
            install: &install,
            endpoint: &endpoint,
            firewall: &firewall,
        },
        "v0.1.12",
    );

    // Then
    assert!(!report.all_passed());
    let item = report
        .items
        .iter()
        .find(|item| item.name == "render endpoint enumerable")
        .expect("endpoint item present");
    assert!(!item.passed);
}

#[test]
fn reports_failure_when_the_firewall_signature_is_missing_the_enabled_field() {
    // Given
    let task = FakeTaskQuery(Ok(passing_task_state()));
    let install = FakeInstallQuery(true);
    let endpoint = FakeEndpointQuery(Ok(passing_endpoints()));
    let mut signature = passing_signature();
    signature.enabled = "False".to_owned();
    let firewall = FakeFirewallQuery(Ok(signature));

    // When
    let report = run_doctor(
        DoctorQueries {
            task: &task,
            install: &install,
            endpoint: &endpoint,
            firewall: &firewall,
        },
        "v0.1.12",
    );

    // Then
    assert!(!report.all_passed());
    let item = report
        .items
        .iter()
        .find(|item| item.name == "UDP 6902 firewall rule")
        .expect("firewall item present");
    assert!(!item.passed);
}

#[test]
fn reports_failure_when_the_scheduled_task_is_disabled() {
    // Given
    let task = FakeTaskQuery(Ok(TaskState {
        enabled: false,
        running: false,
        ready: false,
    }));
    let install = FakeInstallQuery(true);
    let endpoint = FakeEndpointQuery(Ok(passing_endpoints()));
    let firewall = FakeFirewallQuery(Ok(passing_signature()));

    // When
    let report = run_doctor(
        DoctorQueries {
            task: &task,
            install: &install,
            endpoint: &endpoint,
            firewall: &firewall,
        },
        "v0.1.12",
    );

    // Then
    assert!(!report.all_passed());
    let item = report
        .items
        .iter()
        .find(|item| item.name == "scheduled task enabled")
        .expect("task item present");
    assert!(!item.passed);
}

#[test]
fn reports_failure_when_the_executable_is_not_installed() {
    // Given
    let task = FakeTaskQuery(Ok(passing_task_state()));
    let install = FakeInstallQuery(false);
    let endpoint = FakeEndpointQuery(Ok(passing_endpoints()));
    let firewall = FakeFirewallQuery(Ok(passing_signature()));

    // When
    let report = run_doctor(
        DoctorQueries {
            task: &task,
            install: &install,
            endpoint: &endpoint,
            firewall: &firewall,
        },
        "v0.1.12",
    );

    // Then
    assert!(!report.all_passed());
    let item = report
        .items
        .iter()
        .find(|item| item.name == "wifimic_client.exe installed")
        .expect("install item present");
    assert!(!item.passed);
}
