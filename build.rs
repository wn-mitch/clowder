use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/index");
    println!("cargo:rerun-if-changed=build.rs");

    emit("GIT_HASH", git(&["rev-parse", "HEAD"]));
    emit("GIT_HASH_SHORT", git(&["rev-parse", "--short", "HEAD"]));
    emit(
        "GIT_COMMIT_TIME",
        git(&["show", "-s", "--format=%cI", "HEAD"]),
    );

    let dirty = match Command::new("git").args(["status", "--porcelain"]).output() {
        Ok(out) if out.status.success() => !out.stdout.is_empty(),
        _ => false,
    };
    emit("GIT_DIRTY", Some(dirty.to_string()));
}

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn emit(key: &str, value: Option<String>) {
    let v = value.unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env={key}={v}");
}
