fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var_os("CARGO_CFG_TARGET_OS").is_some_and(|target| target == "windows") {
        embed_resource::compile("assets/tray-icon.rc", embed_resource::NONE).manifest_optional()?;
    }
    println!("cargo:rerun-if-env-changed=WIFIMIC_CLIENT_VERSION");
    println!("cargo:rerun-if-env-changed=GITHUB_REF_NAME");
    let package_version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let version = std::env::var("WIFIMIC_CLIENT_VERSION")
        .or_else(|_| std::env::var("GITHUB_REF_NAME"))
        .unwrap_or_else(|_| format!("v{package_version}-dev"));
    println!("cargo:rustc-env=WIFIMIC_CLIENT_VERSION={version}");
    Ok(())
}
