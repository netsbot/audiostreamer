use std::path::PathBuf;
use std::process::Command;

fn build_wrapper() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let wrapper_dir = manifest_dir.join("wrapper");
    let build_dir = wrapper_dir.join("build");

    // Re-run if any wrapper source changes
    println!("cargo:rerun-if-changed=wrapper/CMakeLists.txt");
    println!("cargo:rerun-if-changed=wrapper/main.c");
    println!("cargo:rerun-if-changed=wrapper/main.cpp");
    println!("cargo:rerun-if-changed=wrapper/wrapper.c");
    println!("cargo:rerun-if-changed=wrapper/wrapper-rootless.c");
    println!("cargo:rerun-if-changed=wrapper/cmdline.c");
    println!("cargo:rerun-if-changed=wrapper/cmdline.h");
    println!("cargo:rerun-if-changed=wrapper/import.h");

    let ndk_dir = wrapper_dir.join("android-ndk-r23b");

    if !ndk_dir.exists() {
        println!("cargo:warning=NDK missing. Downloading...");
        let status = Command::new("curl")
            .args(["-fLO", "https://dl.google.com/android/repository/android-ndk-r23b-linux.zip"])
            .current_dir(&wrapper_dir)
            .status()
            .expect("curl failed");
        assert!(status.success(), "failed to download NDK");

        let status = Command::new("unzip")
            .arg("android-ndk-r23b-linux.zip")
            .current_dir(&wrapper_dir)
            .status()
            .expect("unzip failed");
        assert!(status.success(), "failed to unzip NDK");

        let _ = std::fs::remove_file(wrapper_dir.join("android-ndk-r23b-linux.zip"));
    }

    std::fs::create_dir_all(&build_dir).expect("failed to create wrapper/build");

    // Clean stale cache if path changed
    let cache_file = build_dir.join("CMakeCache.txt");
    if cache_file.exists() {
        let _ = std::fs::remove_file(cache_file);
    }

    let cmake_status = Command::new("cmake")
        .arg("..")
        .current_dir(&build_dir)
        .status()
        .expect("cmake not found — install cmake");
    assert!(cmake_status.success(), "cmake .. failed in wrapper/build");

    let make_status = Command::new("make")
        .current_dir(&build_dir)
        .status()
        .expect("make not found");
    assert!(make_status.success(), "make failed in wrapper/build");
}

fn main() {
    build_wrapper();

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
