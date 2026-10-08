use std::{path::Path, process::Command};

fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    for reference in [
        Some("HEAD".to_owned()),
        git(&["symbolic-ref", "-q", "HEAD"]),
        Some("packed-refs".to_owned()),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(path) = git(&["rev-parse", "--git-path", &reference]) {
            let path = Path::new(&path);
            let watched = if path.exists() {
                path
            } else {
                path.parent().unwrap_or(path)
            };
            println!("cargo::rerun-if-changed={}", watched.display());
        }
    }
    let sha = git(&["rev-parse", "HEAD"]).unwrap_or_default();
    println!("cargo::rustc-env=SOUND2SLIDE_BUILD_SHA={sha}");
}
