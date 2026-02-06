use std::process::Command;

fn main() {
    // Get the git hash at build time
    let git_hash = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Make the git hash available as an environment variable at compile time
    println!("cargo:rustc-env=GIT_HASH={}", git_hash);
}
