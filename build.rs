use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::process::Command;

fn apply_patches(manifest_dir: &PathBuf, target_dir: &str, patches: &[&str]) {
    let marker = manifest_dir
        .join("target")
        .join(format!(".{}-patched", target_dir));

    let mut hasher = Sha256::new();
    for patch_name in patches {
        let patch_path = manifest_dir.join("patches").join(patch_name);
        if patch_path.exists() {
            if let Ok(content) = std::fs::read(&patch_path) {
                hasher.update(content);
            }
        }
    }
    let current_hash = hasher
        .finalize()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();

    if marker.exists() {
        if let Ok(old_hash) = std::fs::read_to_string(&marker) {
            if old_hash.trim() == current_hash {
                return;
            }
        }
    }

    println!("cargo:warning=Patch mismatch or missing marker for {}. Re-patching...", target_dir);

    let dir = manifest_dir.join(target_dir);

    // Reset to clean state before applying new patches
    let _ = Command::new("git")
        .args(["checkout", "--", "."])
        .current_dir(&dir)
        .status();
    let _ = Command::new("git")
        .args(["clean", "-f"])
        .current_dir(&dir)
        .status();

    for patch_name in patches {
        let patch_path = manifest_dir.join("patches").join(patch_name);
        if !patch_path.exists() {
            continue;
        }

        let status = Command::new("git")
            .args(["apply", "--whitespace=fix"])
            .arg(patch_path.to_str().unwrap())
            .current_dir(&dir)
            .status()
            .unwrap_or_else(|e| panic!("git apply {} failed: {}", patch_name, e));

        if !status.success() {
            println!(
                "cargo:warning=Patch {} failed to apply",
                patch_name
            );
        }
    }

    std::fs::write(&marker, current_hash).ok();
}

fn build_wrapper() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let wrapper_dir = manifest_dir.join("wrapper");
    let build_dir = manifest_dir.join("target").join("wrapper-build");

    // Re-run if patches change
    println!("cargo:rerun-if-changed=patches/0001-unix-socket.patch");
    println!("cargo:rerun-if-changed=patches/wrapper_build_fix.patch");

    // Re-run if any wrapper source changes
    println!("cargo:rerun-if-changed=wrapper/CMakeLists.txt");
    println!("cargo:rerun-if-changed=wrapper/main.c");
    println!("cargo:rerun-if-changed=wrapper/main.cpp");
    println!("cargo:rerun-if-changed=wrapper/wrapper.c");
    println!("cargo:rerun-if-changed=wrapper/wrapper-rootless.c");
    println!("cargo:rerun-if-changed=wrapper/cmdline.c");
    println!("cargo:rerun-if-changed=wrapper/cmdline.h");
    println!("cargo:rerun-if-changed=wrapper/import.h");

    // Apply patches before build
    apply_patches(
        &manifest_dir,
        "wrapper",
        &["wrapper_build_fix.patch", "0001-unix-socket.patch"],
    );

    // Emit wrapper dir for runtime
    println!("cargo:rustc-env=WRAPPER_DIR={}", wrapper_dir.display());

    let ndk_dir = wrapper_dir.join("android-ndk-r23b");

    if !ndk_dir.exists() {
        println!("cargo:warning=NDK missing. Downloading...");
        let status = Command::new("curl")
            .args([
                "-fLO",
                "https://dl.google.com/android/repository/android-ndk-r23b-linux.zip",
            ])
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

    std::fs::create_dir_all(&build_dir).expect("failed to create wrapper build dir");

    // Clean stale cache only if path changed
    let cache_file = build_dir.join("CMakeCache.txt");
    if cache_file.exists() {
        if let Ok(cache_content) = std::fs::read_to_string(&cache_file) {
            let expected = wrapper_dir.to_str().unwrap_or("");
            if !cache_content.contains(expected) {
                let _ = std::fs::remove_file(&cache_file);
            }
        }
    }

    let cmake_status = Command::new("cmake")
        .arg("-DCMAKE_POLICY_VERSION_MINIMUM=3.5")
        .arg("-Wno-dev")
        .arg(wrapper_dir.to_str().unwrap())
        .current_dir(&build_dir)
        .status()
        .expect("cmake not found — install cmake");
    assert!(cmake_status.success(), "cmake failed in wrapper build");

    let make_status = Command::new("make")
        .current_dir(&build_dir)
        .status()
        .expect("make not found");
    assert!(make_status.success(), "make failed in wrapper build");
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

        if let Some(dir) = candidates
            .into_iter()
            .find(|p| p.join("libcef.so").exists())
        {
            println!("cargo:rustc-link-arg=-Wl,-rpath,{}", dir.display());
        }
    }
}
