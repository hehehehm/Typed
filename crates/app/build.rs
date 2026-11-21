fn stamp_build_time() {
    let build_unix = std::env::var("TYPED_BUILD_UNIX").ok()
        .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs().to_string())
                .unwrap_or_else(|_| "0".into())
        });
    println!("cargo:rustc-env=TYPED_BUILD_UNIX={build_unix}");
    println!("cargo:rerun-if-env-changed=TYPED_BUILD_UNIX");
}

fn main() {
    stamp_build_time();
    let icon = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/typed.ico");
    println!("cargo:rerun-if-changed={}", icon.display());
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winres::WindowsResource::new();
        res.set_icon(icon.to_str().expect("icon path utf-8"));
        res.set("ProductName", "Typed");
        res.set("FileDescription", "Typed");
        res.set("InternalName", "Typed");
        res.set("OriginalFilename", "typed.exe");
        res.set("CompanyName", "Typed");
        res.set("LegalCopyright", "Typed");
        res.set("Comments", "Typed");
        if let Err(e) = res.compile() {
            println!("cargo:warning=Typed icon was not embedded ({e}). The window icon is still set at runtime.");
        }
    }
}
