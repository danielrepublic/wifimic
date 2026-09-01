use crate::{cli, doctor, handoff, logging, status};

/// Runs the command selected by [`cli::parse_command`].
///
/// [`cli::Command::RunAudio`] preserves today's default behavior exactly:
/// diagnostics logging is initialized, then the audio client loop runs. The
/// other commands are one-shot, non-mutating queries — they never
/// initialize diagnostics logging (see [`requires_diagnostics`]) and, on
/// Windows, attach to an inherited console so their output is visible, since
/// this binary is a GUI-subsystem process with no console by default.
/// [`cli::Command::InternalApplyUpgrade`] is the sole exception: its stdout
/// is owned by the elevated handoff script's own capture, so it does not
/// attach a console.
pub(crate) fn dispatch(command: cli::Command) -> Result<(), Box<dyn std::error::Error>> {
    if requires_diagnostics(&command) {
        let (_diagnostics, _startup_rotation) = logging::initialize_diagnostics()?;
    }
    match command {
        cli::Command::RunAudio => {
            #[cfg(target_os = "windows")]
            super::run_windows_client()?;
            Ok(())
        }
        cli::Command::Update => {
            attach_console();
            let result = wifimic_update::check::run_check_update(
                env!("WIFIMIC_CLIENT_VERSION"),
                wifimic_update::discover_latest_tag,
            );
            println!(
                "{}",
                wifimic_update::check::render_check_update(&result, "wifimic_client")
            );
            if wifimic_update::check::check_update_exit_code(&result) == 0 {
                Ok(())
            } else {
                Err("update check failed".into())
            }
        }
        cli::Command::Status => {
            attach_console();
            let result = status::run_status(
                &wifimic_client::task_query::NativeTaskQuery,
                env!("WIFIMIC_CLIENT_VERSION"),
            );
            println!("{}", status::render_status(&result));
            if status::status_exit_code(&result) == 0 {
                Ok(())
            } else {
                Err("status query failed".into())
            }
        }
        cli::Command::Doctor => {
            attach_console();
            let report = doctor::run_doctor(
                doctor::DoctorQueries {
                    task: &wifimic_client::task_query::NativeTaskQuery,
                    install: &doctor::NativeInstallQuery,
                    endpoint: &doctor::NativeRenderEndpointQuery,
                    firewall: &doctor::NativeFirewallQuery,
                },
                env!("WIFIMIC_CLIENT_VERSION"),
            );
            print!("{}", report.render());
            if report.all_passed() {
                Ok(())
            } else {
                Err("one or more doctor checks failed".into())
            }
        }
        cli::Command::Upgrade { target } => {
            attach_console();
            match handoff::run_upgrade(
                &handoff::NativeHandoffOperations,
                &target,
                env!("WIFIMIC_CLIENT_VERSION"),
            )? {
                handoff::UpgradeOutcome::NoOp { current, latest } => {
                    println!("目前版本 {current} 已是最新版本 ({latest})");
                }
                handoff::UpgradeOutcome::HandoffLaunched { tag } => {
                    println!("正在以系統管理員權限準備更新至 {tag}");
                }
            }
            Ok(())
        }
        cli::Command::InternalApplyUpgrade { tag } => apply_internal_upgrade(&tag),
    }
}

#[cfg(target_os = "windows")]
fn apply_internal_upgrade(tag: &str) -> Result<(), Box<dyn std::error::Error>> {
    use wifimic_update::{run_update_transaction, TransactionOutcome, UpdateTarget};

    let outcome = run_update_transaction(
        &mut wifimic_client::updater_native::WindowsUpgradeAdapter::default(),
        UpdateTarget::Tag(tag.to_owned()),
        env!("WIFIMIC_CLIENT_VERSION"),
        wifimic_client::updater::HEALTH_TIMEOUT,
    )?;
    match outcome {
        TransactionOutcome::NoOp { .. } | TransactionOutcome::Installed { .. } => Ok(()),
        TransactionOutcome::RolledBack { cause } => Err(cause),
        TransactionOutcome::RollbackVerificationFailed { cause } => Err(cause),
    }
}

#[cfg(not(target_os = "windows"))]
fn apply_internal_upgrade(_tag: &str) -> Result<(), Box<dyn std::error::Error>> {
    Err("--internal-apply-upgrade is Windows-only".into())
}

/// Returns whether `command` must initialize diagnostics logging before it
/// runs. Only [`cli::Command::RunAudio`] does; `Update`/`Status`/`Doctor`
/// are non-mutating queries that must never create or rotate the
/// diagnostics log directory.
pub(crate) fn requires_diagnostics(command: &cli::Command) -> bool {
    matches!(command, cli::Command::RunAudio)
}

/// Attaches this GUI-subsystem process to an inherited console, if any, so
/// one-shot command output is visible in an interactive PowerShell window.
#[cfg(target_os = "windows")]
fn attach_console() {
    // SAFETY: best-effort attach; the Result is intentionally ignored
    // because it fails harmlessly (e.g. ERROR_INVALID_HANDLE) when launched
    // with no parent console, which is not an error for this process.
    let _ = unsafe {
        windows::Win32::System::Console::AttachConsole(
            windows::Win32::System::Console::ATTACH_PARENT_PROCESS,
        )
    };
}

#[cfg(not(target_os = "windows"))]
fn attach_console() {}

#[cfg(test)]
#[path = "dispatch_tests.rs"]
mod tests;
