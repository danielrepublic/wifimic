use thiserror::Error;
#[cfg(target_os = "windows")]
use wifimic_client::{
    updater,
    updater_native::{validate_no_arguments, NativeUpdaterOperations},
};

const INVALID_ARGUMENTS_EXIT_CODE: i32 = 2;
const UPDATE_FAILED_EXIT_CODE: i32 = 1;

#[derive(Debug, Error, PartialEq, Eq)]
enum UpdaterCliError {
    #[error("wifimic_client_updater does not accept command-line arguments")]
    UnexpectedArguments,
}

#[cfg(target_os = "windows")]
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    if validate_no_arguments(&args).is_err() {
        eprintln!("{}", UpdaterCliError::UnexpectedArguments);
        std::process::exit(INVALID_ARGUMENTS_EXIT_CODE);
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
        std::process::exit(UPDATE_FAILED_EXIT_CODE);
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
    std::process::exit(UPDATE_FAILED_EXIT_CODE);
}
