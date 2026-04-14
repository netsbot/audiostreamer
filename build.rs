fn main() {
    println!("cargo:rerun-if-env-changed=CEF_PATH");
    tauri_build::build();
    println!("cargo:rustc-link-lib=gpac");

    // Ensure runtime loader can find libcef.so on Linux installs where CEF lives in subdirs.
    if std::env::var("TARGET")
        .map(|target| target.contains("linux"))
        .unwrap_or(false)
    {
        let base = std::env::var("CEF_PATH").unwrap_or_else(|_| "/usr/lib".to_string());
        let candidates = [
            std::path::PathBuf::from(&base),
            std::path::PathBuf::from(&base).join("cef"),
            std::path::PathBuf::from(&base).join("Release"),
        ];

        if let Some(dir) = candidates.into_iter().find(|p| p.join("libcef.so").exists()) {
            println!("cargo:rustc-link-arg=-Wl,-rpath,{}", dir.display());
        }
    }
}
