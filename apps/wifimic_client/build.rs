fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var_os("CARGO_CFG_TARGET_OS").is_some_and(|target| target == "windows") {
        embed_resource::compile("assets/tray-icon.rc", embed_resource::NONE).manifest_optional()?;
    }
    Ok(())
}
