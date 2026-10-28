//! Stamps the git commit into the sim for `SIM_BUILD` (design, Snapshot
//! contents: crate version + git hash), and reruns when HEAD moves so the
//! stamp never goes stale.

use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn main() {
    let hash = git(&["rev-parse", "--short=12", "HEAD"]).unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=SIM_GIT_HASH={hash}");
    println!("cargo:rerun-if-changed=build.rs");
    // HEAD itself (a checkout) and the branch it points at (a commit).
    let branch = git(&["rev-parse", "--symbolic-full-name", "HEAD"]);
    let watched = [
        git(&["rev-parse", "--git-path", "HEAD"]),
        branch.and_then(|name| git(&["rev-parse", "--git-path", &name])),
    ];
    for path in watched.into_iter().flatten() {
        println!("cargo:rerun-if-changed={path}");
    }
}
