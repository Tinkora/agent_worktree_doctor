use std::fs;
use std::process::Command;
use tempfile::tempdir;

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_agent_worktree_doctor"))
}

#[test]
fn missing_input_is_redacted_and_returns_finding() {
    let root = tempdir().unwrap();
    let missing = root.path().join("private-user").join("missing-repo");
    let output = binary()
        .args(["audit", missing.to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("AWD001"));
    assert!(!stdout.contains(root.path().to_str().unwrap()));
    assert!(!stdout.contains("private-user"));
}

#[test]
fn ordinary_directory_is_not_a_git_worktree() {
    let root = tempdir().unwrap();
    let output = binary()
        .args(["audit", root.path().to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8(output.stdout).unwrap().contains("AWD002"));
}

#[test]
fn healthy_main_and_linked_worktrees_have_no_findings() {
    let root = tempdir().unwrap();
    let main = root.path().join("main");
    let linked = root.path().join("linked");
    git(root.path(), &["init", main.to_str().unwrap()]);
    git(&main, &["config", "user.name", "Fixture"]);
    git(&main, &["config", "user.email", "fixture@example.invalid"]);
    fs::write(main.join("file.txt"), "fixture\n").unwrap();
    git(&main, &["add", "file.txt"]);
    git(&main, &["commit", "-m", "fixture"]);
    git(
        &main,
        &["worktree", "add", "--detach", linked.to_str().unwrap()],
    );

    let output = binary()
        .args(["audit", linked.to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"complete\":true"));
    assert!(!stdout.contains(root.path().to_str().unwrap()));
}

#[test]
fn broken_forward_gitdir_is_reported_without_exposing_target() {
    let root = tempdir().unwrap();
    fs::write(root.path().join(".git"), "gitdir: /secret/user/missing\n").unwrap();
    let output = binary()
        .args(["audit", root.path().to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("AWD005"));
    assert!(!stdout.contains("/secret/user/missing"));
}

#[test]
fn malformed_gitfile_is_reported() {
    let root = tempdir().unwrap();
    fs::write(root.path().join(".git"), "not a gitdir\nsecond line\n").unwrap();
    let output = binary()
        .args(["audit", root.path().to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8(output.stdout).unwrap().contains("AWD004"));
}

#[test]
fn mismatched_admin_backlink_is_reported() {
    let root = tempdir().unwrap();
    let main = root.path().join("main");
    let linked = root.path().join("linked");
    git(root.path(), &["init", main.to_str().unwrap()]);
    git(&main, &["config", "user.name", "Fixture"]);
    git(&main, &["config", "user.email", "fixture@example.invalid"]);
    fs::write(main.join("file.txt"), "fixture\n").unwrap();
    git(&main, &["add", "file.txt"]);
    git(&main, &["commit", "-m", "fixture"]);
    git(
        &main,
        &["worktree", "add", "--detach", linked.to_str().unwrap()],
    );
    let gitdir = git_stdout(&linked, &["rev-parse", "--absolute-git-dir"]);
    fs::write(
        std::path::Path::new(gitdir.trim()).join("gitdir"),
        "/private/wrong/.git\n",
    )
    .unwrap();

    let output = binary()
        .args(["audit", linked.to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("AWD008"));
    assert!(!stdout.contains("/private/wrong"));
}

#[test]
fn tracked_status_is_opt_in_and_redacted() {
    let root = tempdir().unwrap();
    git(root.path(), &["init"]);
    git(root.path(), &["config", "user.name", "Fixture"]);
    git(
        root.path(),
        &["config", "user.email", "fixture@example.invalid"],
    );
    fs::write(root.path().join("private-name.txt"), "first\n").unwrap();
    git(root.path(), &["add", "private-name.txt"]);
    git(root.path(), &["commit", "-m", "fixture"]);
    fs::write(root.path().join("private-name.txt"), "changed\n").unwrap();

    let output = binary()
        .args([
            "audit",
            root.path().to_str().unwrap(),
            "--check-tracked-status",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("AWD014"));
    assert!(!stdout.contains("private-name.txt"));
}

#[cfg(unix)]
#[test]
fn tracked_status_does_not_execute_repository_fsmonitor() {
    use std::os::unix::fs::PermissionsExt;

    let root = tempdir().unwrap();
    git(root.path(), &["init"]);
    let marker = root.path().join("fsmonitor-ran");
    let hook = root.path().join("fsmonitor.sh");
    fs::write(&hook, format!("#!/bin/sh\ntouch '{}'\n", marker.display())).unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    git(
        root.path(),
        &["config", "core.fsmonitor", hook.to_str().unwrap()],
    );

    let output = binary()
        .args([
            "audit",
            root.path().to_str().unwrap(),
            "--check-tracked-status",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(!marker.exists(), "the audit executed core.fsmonitor");
}

#[test]
fn oversized_gitmodules_makes_the_audit_incomplete() {
    let root = tempdir().unwrap();
    git(root.path(), &["init"]);
    fs::write(root.path().join(".gitmodules"), vec![b'x'; 65 * 1024]).unwrap();
    let output = binary()
        .args([
            "audit",
            root.path().to_str().unwrap(),
            "--include-submodules",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("AWD015"));
    assert!(stdout.contains("\"complete\":false"));
}

#[cfg(unix)]
#[test]
fn dangling_gitmodules_makes_the_audit_incomplete() {
    use std::os::unix::fs::symlink;

    let root = tempdir().unwrap();
    git(root.path(), &["init"]);
    symlink("missing-gitmodules", root.path().join(".gitmodules")).unwrap();
    let output = binary()
        .args([
            "audit",
            root.path().to_str().unwrap(),
            "--include-submodules",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8(output.stdout).unwrap().contains("AWD015"));
}

#[test]
fn repeated_inputs_share_one_repository_inventory() {
    let root = tempdir().unwrap();
    git(root.path(), &["init"]);
    let nested = root.path().join("nested");
    fs::create_dir(&nested).unwrap();
    let output = binary()
        .args([
            "audit",
            root.path().to_str().unwrap(),
            nested.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["summary"]["repositories"], 1);
    assert_eq!(report["summary"]["worktrees"], 1);
}

#[test]
fn missing_initialized_submodule_gitdir_is_reported() {
    let root = tempdir().unwrap();
    git(root.path(), &["init"]);
    fs::write(
        root.path().join(".gitmodules"),
        "[submodule \"private\"]\n\tpath = module\n\turl = ignored\n",
    )
    .unwrap();
    fs::create_dir(root.path().join("module")).unwrap();
    fs::write(
        root.path().join("module/.git"),
        "gitdir: ../missing-private-module\n",
    )
    .unwrap();

    let output = binary()
        .args([
            "audit",
            root.path().to_str().unwrap(),
            "--include-submodules",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("AWD012"));
    assert!(!stdout.contains("private"));
}

fn git(cwd: &std::path::Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(cwd)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(output.status.success(), "git {:?}: {:?}", args, output);
}

fn git_stdout(cwd: &std::path::Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(cwd)
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap()
}
