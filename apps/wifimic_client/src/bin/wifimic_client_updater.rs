#[cfg(target_os = "windows")]
use wifimic_client::{updater, updater_native::NativeUpdaterOperations};

fn validate_no_arguments(args: &[String]) -> Result<(), String> {
    if args.len() > 1 {
        Err("wifimic_client_updater does not accept command-line arguments".to_owned())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::validate_no_arguments;

    #[test]
    fn rejects_any_cli_argument_before_any_side_effect() {
        // Given
        let args = vec!["wifimic_client_updater".to_owned(), "--tag".to_owned()];

        // When
        let result = validate_no_arguments(&args);

        // Then
        assert!(result.is_err());
    }
}

#[cfg(target_os = "windows")]
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    if let Err(error) = validate_no_arguments(&args) {
        eprintln!("{error}");
        std::process::exit(2);
    }

    println!("檢查中...");
    let mut operations = NativeUpdaterOperations;
    let result = updater::run_update(&mut operations, env!("WIFIMIC_CLIENT_VERSION"));
    let succeeded = match result {
        Ok(updater::UpdaterOutcome::NoOp) => {
            println!("已是最新版本");
            true
        }
        Ok(updater::UpdaterOutcome::Installed { tag }) => {
            println!("發現新版本，更新中...");
            println!("已更新至 {tag}");
            true
        }
        Ok(updater::UpdaterOutcome::RolledBack) => {
            println!("更新失敗：更新未完成，已還原先前版本");
            false
        }
        Ok(updater::UpdaterOutcome::RollbackVerificationFailed) => {
            println!("更新失敗：更新失敗且無法確認還原狀態");
            false
        }
        Err(error) => {
            println!("更新失敗：{error}");
            false
        }
    };
    wait_for_keypress();
    if !succeeded {
        std::process::exit(1);
    }
}

#[cfg(target_os = "windows")]
fn wait_for_keypress() {
    // The required UAC manifest is evaluated before main. Declining UAC means
    // this process never reaches main, so Windows' cancellation dialog is the
    // only feedback; no application-level "declined" message is printed and
    // no file or scheduled-task state changes occur.
    println!("請按 Enter 鍵結束...");
    let mut input = String::new();
    let _ = std::io::stdin().read_line(&mut input);
}

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("wifimic_client_updater is Windows-only");
    std::process::exit(1);
}
