//! Formats generated Go with `gofmt`.
//!
//! genco emits valid Go but not gofmt's layout: struct fields and constants
//! are not aligned, `interface {}` keeps its space and binary expressions are
//! spaced uniformly. A consumer that tracks the bindings and gates them with
//! `gofmt -l` would otherwise have to post-process every regeneration, so
//! `--gofmt` hands the output to the Go toolchain's own formatter. gofmt only
//! rewrites whitespace in what gravity emits; the tokens are unchanged.

use std::{
    ffi::OsString,
    io::Write,
    process::{Command, Stdio},
};

/// The formatter to run: `$GOFMT` when set, else `gofmt` from `PATH`.
pub fn program() -> OsString {
    std::env::var_os("GOFMT").unwrap_or_else(|| OsString::from("gofmt"))
}

/// Runs `program` with `source` on stdin and answers its stdout.
///
/// An error names the program and carries its stderr, so a syntax error in
/// generated code reads as gofmt's own diagnostic.
pub fn format(source: &str, program: &OsString) -> Result<String, String> {
    let name = program.to_string_lossy();
    let mut child = Command::new(program)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("could not run {name}: {err}"))?;
    child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(source.as_bytes())
        .map_err(|err| format!("could not write to {name}: {err}"))?;
    let output = child
        .wait_with_output()
        .map_err(|err| format!("{name} did not finish: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "{name} failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout).map_err(|err| format!("{name} wrote invalid UTF-8: {err}"))
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::format;

    #[test]
    fn test_format_answers_the_programs_stdout() {
        // `cat` stands in for gofmt: the identity formatter.
        let out = format("package x\n", &OsString::from("cat")).expect("cat should run");
        assert_eq!(out, "package x\n");
    }

    #[test]
    fn test_format_reports_a_failing_formatter() {
        let err = format("package x\n", &OsString::from("false")).unwrap_err();
        assert!(err.starts_with("false failed"), "got: {err}");
    }

    #[test]
    fn test_format_reports_a_missing_formatter() {
        let err = format("", &OsString::from("gravity-no-such-gofmt")).unwrap_err();
        assert!(
            err.starts_with("could not run gravity-no-such-gofmt"),
            "got: {err}"
        );
    }
}
