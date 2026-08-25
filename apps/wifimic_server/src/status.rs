use std::process::Command;

/// Selects the systemd property queried by the status and doctor commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ServiceProperty {
    /// Queries whether the user service is active.
    Active,
    /// Queries whether the user service is enabled.
    Enabled,
}

/// Reports failures while invoking or decoding `systemctl` output.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub(crate) enum StatusError {
    /// The systemctl process could not be started.
    #[error("could not invoke systemctl: {message}")]
    Invoke { message: String },
    /// The command output was not valid UTF-8.
    #[error("systemctl output was not valid UTF-8: {message}")]
    InvalidOutput { message: String },
}

/// Provides read-only user-service state queries.
pub(crate) trait ServiceQueries {
    /// Runs one systemd property query and returns trimmed standard output.
    fn query(&self, property: ServiceProperty) -> Result<String, StatusError>;
}

/// Executes status queries against the current user's systemd session.
#[derive(Debug, Default)]
pub(crate) struct NativeServiceQueries;

impl ServiceQueries for NativeServiceQueries {
    fn query(&self, property: ServiceProperty) -> Result<String, StatusError> {
        let property = match property {
            ServiceProperty::Active => "is-active",
            ServiceProperty::Enabled => "is-enabled",
        };
        let output = Command::new("systemctl")
            .args(["--user", property, "wifimic-server"])
            .output()
            .map_err(|error| StatusError::Invoke {
                message: error.to_string(),
            })?;
        String::from_utf8(output.stdout)
            .map(|value| value.trim().to_owned())
            .map_err(|error| StatusError::InvalidOutput {
                message: error.to_string(),
            })
    }
}

/// Contains the version and service state displayed by `status`.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct StatusReport {
    /// The embedded server version.
    pub(crate) version: &'static str,
    /// The active-state query result.
    pub(crate) active: String,
    /// The enabled-state query result.
    pub(crate) enabled: String,
}

/// Queries and formats the user service status.
///
/// # Errors
/// Returns [`StatusError`] when either systemctl query cannot be invoked or decoded.
pub(crate) fn run_status<Q: ServiceQueries>(
    queries: &Q,
    version: &'static str,
) -> Result<StatusReport, StatusError> {
    Ok(StatusReport {
        version,
        active: queries.query(ServiceProperty::Active)?,
        enabled: queries.query(ServiceProperty::Enabled)?,
    })
}

impl StatusReport {
    pub(crate) fn render(&self) -> String {
        format!(
            "版本：{}；wifimic-server active={} enabled={}",
            self.version, self.active, self.enabled
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{run_status, ServiceProperty, ServiceQueries, StatusError};

    #[derive(Debug)]
    struct FakeQueries;

    impl ServiceQueries for FakeQueries {
        fn query(&self, property: ServiceProperty) -> Result<String, StatusError> {
            Ok(match property {
                ServiceProperty::Active => "inactive".to_owned(),
                ServiceProperty::Enabled => "disabled".to_owned(),
            })
        }
    }

    #[test]
    fn reports_inactive_and_disabled_as_successful_queries() {
        // Given
        let queries = FakeQueries;

        // When
        let report = run_status(&queries, "v0.1.12").expect("fake queries succeed");

        // Then
        assert_eq!(report.active, "inactive");
        assert_eq!(report.enabled, "disabled");
        assert_eq!(report.version, "v0.1.12");
    }
}
