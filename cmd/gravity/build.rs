use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    // Tell Cargo to re-run this build script if .git/HEAD changes
    // This is safe even if .git/HEAD doesn't exist (e.g., source tarball builds)
    println!("cargo:rerun-if-changed=.git/HEAD");

    // If .git/HEAD exists and points to a branch ref, also watch that ref file
    if let Ok(head_content) = fs::read_to_string(".git/HEAD") {
        // .git/HEAD contains something like "ref: refs/heads/main"
        if let Some(ref_path) = head_content.strip_prefix("ref: ") {
            let ref_path = ref_path.trim();
            let full_ref_path = format!(".git/{}", ref_path);
            if Path::new(&full_ref_path).exists() {
                println!("cargo:rerun-if-changed={}", full_ref_path);
            }
        }
    }

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
