fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-env-changed=WIFIMIC_SERVER_VERSION");
    println!("cargo:rerun-if-env-changed=GITHUB_REF_NAME");
    let package_version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let version = std::env::var("WIFIMIC_SERVER_VERSION")
        .or_else(|_| std::env::var("GITHUB_REF_NAME"))
        .unwrap_or_else(|_| format!("v{package_version}-dev"));
    println!("cargo:rustc-env=WIFIMIC_SERVER_VERSION={version}");
    Ok(())
}
