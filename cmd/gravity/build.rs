use std::path::Path;
use std::process::Command;

/// Runs git with `args` and returns its trimmed output, or `None` when git is
/// missing, fails, or prints nothing.
fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let out = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (!out.is_empty()).then_some(out)
}

/// The absolute path of `name` inside the git directory. Git resolves it for
/// a worktree, whose `.git` is a file, and finds a branch's ref in the common
/// directory a worktree shares.
fn git_path(name: &str) -> Option<String> {
    git(&["rev-parse", "--path-format=absolute", "--git-path", name])
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    // Cargo treats a rerun-if-changed path that does not exist as changed, and
    // reruns this script and recompiles the crate on every build, so only
    // existing files are named. HEAD changes when a branch is checked out;
    // HEAD's reflog on every commit, checkout and reset; the branch's ref file
    // or packed-refs when the branch moves.
    let mut watched = vec!["HEAD".to_string(), "logs/HEAD".to_string()];
    if let Some(branch) = git(&["symbolic-ref", "-q", "HEAD"]) {
        watched.push(branch);
    }
    watched.push("packed-refs".to_string());
    for name in &watched {
        if let Some(path) = git_path(name).filter(|path| Path::new(path).is_file()) {
            println!("cargo:rerun-if-changed={path}");
        }
    }

    let git_hash = git(&["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=GIT_HASH={git_hash}");
}
