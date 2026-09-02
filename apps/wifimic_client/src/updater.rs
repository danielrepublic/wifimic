/// Maximum time allowed for the updated client to become healthy.
pub const HEALTH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(45);

pub(crate) const CLIENT_EXECUTABLE_NAME: &str = "wifimic_client.exe";
const LOGON_STARTUP_DELAY: &str = "PT30S";

/// Captures the task definition and lifecycle state before an update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSnapshot {
    xml: String,
    enabled: bool,
    running: bool,
}

impl TaskSnapshot {
    /// Creates a task snapshot for an adapter or test double.
    #[must_use]
    pub fn new(xml: String, enabled: bool, running: bool) -> Self {
        Self {
            xml,
            enabled,
            running,
        }
    }

    /// Returns the captured task XML.
    #[must_use]
    pub fn xml(&self) -> &str {
        &self.xml
    }

    /// Returns whether the task was enabled before the update.
    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    /// Returns whether the task was running before the update.
    #[must_use]
    pub const fn running(&self) -> bool {
        self.running
    }

    /// Returns this task definition with the required post-logon startup delay.
    #[must_use]
    pub(crate) fn with_logon_startup_delay(&self) -> Self {
        let Some(trigger_start) = self.xml.find("<LogonTrigger") else {
            return self.clone();
        };
        let Some(open_end) = self.xml[trigger_start..].find('>') else {
            return self.clone();
        };
        let open_end = trigger_start + open_end;
        let opening_tag = &self.xml[trigger_start..=open_end];
        let xml = if opening_tag.trim_end().ends_with("/>") {
            format!(
                "{}<LogonTrigger><Delay>{LOGON_STARTUP_DELAY}</Delay></LogonTrigger>{}",
                &self.xml[..trigger_start],
                &self.xml[open_end + 1..]
            )
        } else {
            let Some(close_offset) = self.xml[open_end + 1..].find("</LogonTrigger>") else {
                return self.clone();
            };
            let close_start = open_end + 1 + close_offset;
            if self.xml[open_end + 1..close_start].contains("<Delay>") {
                self.xml.clone()
            } else {
                format!(
                    "{}<Delay>{LOGON_STARTUP_DELAY}</Delay>{}",
                    &self.xml[..close_start],
                    &self.xml[close_start..]
                )
            }
        };
        Self::new(xml, self.enabled, self.running)
    }
}

pub(crate) fn task_xml_bytes(xml: &str) -> Vec<u8> {
    let mut bytes = vec![0xFF, 0xFE];
    for code_unit in xml.encode_utf16() {
        bytes.extend_from_slice(&code_unit.to_le_bytes());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::{task_xml_bytes, TaskSnapshot};

    #[test]
    fn task_snapshot_serializes_declared_utf16_xml() {
        // Given
        let xml = "<?xml version=\"1.0\" encoding=\"UTF-16\"?>\r\n<Task/>";

        // When
        let bytes = task_xml_bytes(xml);

        // Then
        assert_eq!(&bytes[..2], &[0xFF, 0xFE]);
        let code_units = bytes[2..]
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect::<Vec<_>>();
        assert_eq!(
            String::from_utf16(&code_units).expect("UTF-16 bytes decode"),
            xml
        );
    }

    #[test]
    fn task_snapshot_migrates_a_self_closing_logon_trigger_to_the_startup_delay() {
        // Given
        let snapshot = TaskSnapshot::new(
            "<Task><Triggers><LogonTrigger /></Triggers></Task>".to_owned(),
            true,
            false,
        );

        // When
        let migrated = snapshot.with_logon_startup_delay();

        // Then
        assert!(migrated
            .xml()
            .contains("<LogonTrigger><Delay>PT30S</Delay></LogonTrigger>"));
    }
}
