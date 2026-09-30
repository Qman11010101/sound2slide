use std::{env, path::PathBuf};

fn main() {
    println!("cargo::rerun-if-changed=vendor/marmkmt/CMakeLists.txt");
    println!("cargo::rerun-if-changed=vendor/marmkmt/include/marmkmt.h");
    println!("cargo::rerun-if-changed=vendor/marmkmt/src/marmkmt.cpp");
    println!(
        "cargo::rerun-if-changed=vendor/marmkmt/third_party/MargretePluginSDK/include/MargretePlugin.h"
    );

    require_target("CARGO_CFG_TARGET_OS", "windows");
    require_target("CARGO_CFG_TARGET_ARCH", "x86_64");
    require_target("CARGO_CFG_TARGET_ENV", "msvc");

    if !target_has_crt_static() {
        panic!(
            "marmkmt-sys requires the static MSVC CRT; build with \
             RUSTFLAGS=-Ctarget-feature=+crt-static"
        );
    }

    let dst = cmake::Config::new("vendor/marmkmt")
        .define("MARMKMT_BUILD_EXAMPLES", "OFF")
        .define("BUILD_TESTING", "OFF")
        .define("CMAKE_MSVC_RUNTIME_LIBRARY", "MultiThreaded")
        .build();

    let lib_dir = find_library_dir(&dst);
    println!("cargo::rustc-link-search=native={}", lib_dir.display());
    println!("cargo::rustc-link-lib=static=marmkmt");
    println!("cargo::rustc-link-lib=user32");
}

fn require_target(name: &str, expected: &str) {
    let actual = env::var(name).unwrap_or_default();
    assert_eq!(
        actual, expected,
        "marmkmt-sys supports only x86_64-pc-windows-msvc ({}={actual:?})",
        name
    );
}

fn target_has_crt_static() -> bool {
    env::var("CARGO_CFG_TARGET_FEATURE")
        .unwrap_or_default()
        .split(',')
        .any(|feature| feature == "crt-static")
}

fn find_library_dir(dst: &std::path::Path) -> PathBuf {
    [dst.join("lib"), dst.join("build").join("Release")]
        .into_iter()
        .find(|path| path.join("marmkmt.lib").is_file())
        .unwrap_or_else(|| panic!("CMake did not produce marmkmt.lib under {}", dst.display()))
}
