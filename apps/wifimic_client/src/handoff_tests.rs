use std::cell::Cell;
use std::path::Path;

use super::{
    run_upgrade, write_handoff_script_in, HandoffError, HandoffOperations, UpgradeOutcome,
    HANDOFF_SCRIPT_TEMPLATE,
};
use wifimic_update::{UpdateError, UpdateTarget};

struct FakeHandoff {
    write_calls: Cell<u8>,
    launch_calls: Cell<u8>,
}

impl HandoffOperations for FakeHandoff {
    fn discover_latest_tag(&self) -> Result<String, UpdateError> {
        Ok("v1.2.3".to_owned())
    }

    fn write_script(&self, _tag: &str) -> Result<std::path::PathBuf, HandoffError> {
        self.write_calls.set(self.write_calls.get() + 1);
        Ok(std::path::PathBuf::from("handoff.ps1"))
    }

    fn launch_script(&self, _script: &Path, _tag: &str) -> Result<(), HandoffError> {
        self.launch_calls.set(self.launch_calls.get() + 1);
        Ok(())
    }
}

#[test]
fn write_handoff_script_rejects_malformed_tag_before_creating_a_file() {
    // Given
    let malformed_tag = "v1.2.3; Remove-Item C:\\*";

    // When
    let result = write_handoff_script_in(malformed_tag, Path::new(r"Z:\\missing-directory"));

    // Then
    assert!(matches!(
        result,
        Err(HandoffError::InvalidTag { tag }) if tag == malformed_tag
    ));
}

#[test]
fn handoff_template_waits_for_the_parent_with_a_bounded_poll() {
    // Given
    let template = HANDOFF_SCRIPT_TEMPLATE;

    // When
    let waits_for_parent =
        template.contains("Get-Process -Id $ParentProcessId -ErrorAction SilentlyContinue");

    // Then
    assert!(waits_for_parent);
    assert!(template.contains("[TimeSpan]::FromSeconds(120)"));
    assert!(template.contains("Start-Sleep -Milliseconds 250"));
    assert!(template.contains("$exitCode = 10"));
}

#[test]
fn handoff_template_copies_and_invokes_a_temporary_runner_for_the_release_tag() {
    // Given
    let template = HANDOFF_SCRIPT_TEMPLATE;

    // When
    let runner_copy = template
        .contains("Copy-Item -LiteralPath $ClientExecutable -Destination $runnerPath -Force");

    // Then
    assert!(runner_copy);
    assert!(template.contains("wifimic-client-upgrade-runner-"));
    assert!(template.contains("& $runnerPath --internal-apply-upgrade $ReleaseTag"));
    assert!(template.contains("$LASTEXITCODE"));
    assert!(template.contains("$exitCode = 30"));
}

#[test]
fn handoff_template_leaves_task_startup_to_the_update_transaction_and_deletes_itself_last() {
    // Given
    let template = HANDOFF_SCRIPT_TEMPLATE;

    // When
    let self_delete =
        template.find("Remove-Item -LiteralPath $PSCommandPath -Force -ErrorAction Stop");
    let runner_delete =
        template.find("Remove-Item -LiteralPath $runnerPath -Force -ErrorAction Stop");

    // Then
    assert!(!template.contains("Start-ScheduledTask"));
    assert!(runner_delete.is_some());
    assert!(self_delete.is_some());
    assert!(runner_delete < self_delete);
}

#[cfg(target_os = "windows")]
#[test]
fn generated_handoff_waits_for_parent_and_reports_runner_failure(
) -> Result<(), Box<dyn std::error::Error>> {
    // Given
    let directory = std::env::temp_dir().join(format!(
        "wifimic-handoff-test-{}-{}",
        std::process::id(),
        super::timestamp()
    ));
    std::fs::create_dir(&directory)?;
    let script = write_handoff_script_in("v1.2.3", &directory)?;
    let system_root = std::env::var_os("SystemRoot").ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "SystemRoot is unavailable")
    })?;
    let client_executable = std::path::PathBuf::from(system_root)
        .join("System32")
        .join("WindowsPowerShell")
        .join("v1.0")
        .join("powershell.exe");
    let parent = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-Command", "Start-Sleep -Seconds 1"])
        .spawn()?;
    let parent_process_id = parent.id().to_string();
    let started = std::time::Instant::now();

    // When
    let status = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(&script)
        .args([
            "-ParentProcessId",
            &parent_process_id,
            "-ReleaseTag",
            "v1.2.3",
            "-ClientExecutable",
        ])
        .arg(client_executable)
        .status()?;

    // Then
    assert!(started.elapsed() >= std::time::Duration::from_millis(500));
    assert_eq!(status.code(), Some(30));
    assert!(!script.exists());
    std::fs::remove_dir_all(&directory)?;
    Ok(())
}

#[test]
fn upgrade_no_op_never_writes_or_launches_a_handoff_script() {
    // Given
    let handoff = FakeHandoff {
        write_calls: Cell::new(0),
        launch_calls: Cell::new(0),
    };

    // When
    let result = run_upgrade(&handoff, &UpdateTarget::Latest, "v1.2.3")
        .expect("equal versions should not need elevation");

    // Then
    assert!(matches!(result, UpgradeOutcome::NoOp { .. }));
    assert_eq!(handoff.write_calls.get(), 0);
    assert_eq!(handoff.launch_calls.get(), 0);
}
