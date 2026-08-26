#[cfg(target_os = "windows")]
fn main() {
    todo!("wired in todo 3 of v0-2-0-update-mechanism");
}

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("wifimic_client_updater is Windows-only");
    std::process::exit(1);
}
