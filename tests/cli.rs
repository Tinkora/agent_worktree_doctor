use std::fs;
use std::process::Command;
use tempfile::tempdir;

fn binary() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_agent_worktree_doctor"));
    let absolute_path = std::env::join_paths(
        std::env::split_paths(&std::env::var_os("PATH").unwrap())
            .filter(|entry| entry.is_absolute()),
    )
    .unwrap();
    command.env("PATH", absolute_path);
    command
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
fn healthy_separate_git_directory_has_no_findings() {
    let root = tempdir().unwrap();
    let worktree = root.path().join("worktree");
    let admin = root.path().join("admin");
    git(
        root.path(),
        &[
            "init",
            "--separate-git-dir",
            admin.to_str().unwrap(),
            worktree.to_str().unwrap(),
        ],
    );

    let output = binary()
        .args(["audit", worktree.to_str().unwrap(), "--format", "json"])
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
fn inherited_git_dir_and_work_tree_cannot_redirect_a_probe() {
    let root = tempdir().unwrap();
    let actual = root.path().join("actual");
    let ordinary = root.path().join("ordinary");
    git(root.path(), &["init", actual.to_str().unwrap()]);
    fs::create_dir(&ordinary).unwrap();
    let output = binary()
        .args(["audit", ordinary.to_str().unwrap(), "--format", "json"])
        .env("GIT_DIR", actual.join(".git"))
        .env("GIT_WORK_TREE", &ordinary)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8(output.stdout).unwrap().contains("AWD002"));
}

#[test]
fn inherited_git_trace_cannot_create_a_trace_file() {
    let root = tempdir().unwrap();
    git(root.path(), &["init"]);
    let marker = root.path().join("private-trace");
    let output = binary()
        .args(["audit", root.path().to_str().unwrap(), "--format", "json"])
        .env("GIT_TRACE", &marker)
        .env("GIT_TRACE2", &marker)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(!marker.exists(), "the audit inherited Git tracing controls");
}

#[test]
fn core_worktree_cannot_redirect_the_input_outside_the_reported_root() {
    let root = tempdir().unwrap();
    let repository = root.path().join("repository");
    let external = root.path().join("private-external");
    git(root.path(), &["init", repository.to_str().unwrap()]);
    fs::create_dir(&external).unwrap();
    git(
        &repository,
        &["config", "core.worktree", external.to_str().unwrap()],
    );
    let output = binary()
        .args(["audit", repository.to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("AWD011"));
    assert!(!stdout.contains("private-external"));
}

#[test]
fn every_explicit_core_worktree_stops_before_derived_metadata_reads() {
    for scenario in [
        "temporary-root",
        "filesystem-root",
        "input-parent",
        "nested-input",
    ] {
        let root = tempdir().unwrap();
        let repository = root.path().join("repository");
        let nested = repository.join("nested");
        fs::create_dir_all(&nested).unwrap();
        git(root.path(), &["init", repository.to_str().unwrap()]);
        let (input, target) = match scenario {
            "temporary-root" => (repository.as_path(), root.path().to_path_buf()),
            "filesystem-root" => (
                repository.as_path(),
                root.path().ancestors().last().unwrap().to_path_buf(),
            ),
            "input-parent" => (nested.as_path(), repository.clone()),
            "nested-input" => (nested.as_path(), nested.clone()),
            _ => unreachable!(),
        };
        git(
            &repository,
            &["config", "core.worktree", target.to_str().unwrap()],
        );
        if target.starts_with(root.path()) {
            fs::write(
                target.join(".gitmodules"),
                "[submodule \"outside-marker\"]\npath = ../private-marker\n",
            )
            .unwrap();
        }

        let output = binary()
            .args([
                "audit",
                input.to_str().unwrap(),
                "--include-submodules",
                "--format",
                "json",
            ])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(2), "{scenario}: {output:?}");
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains("AWD011"), "{scenario}: {stdout}");
        assert!(!stdout.contains("AWD015"), "{scenario}: {stdout}");
        assert!(!stdout.contains("outside-marker"), "{scenario}: {stdout}");
        assert!(!stdout.contains("private-marker"), "{scenario}: {stdout}");
    }
}

#[cfg(unix)]
#[test]
fn relative_path_entries_cannot_select_an_untrusted_git_executable() {
    use std::os::unix::fs::PermissionsExt;

    let root = tempdir().unwrap();
    let bin = root.path().join("relative-bin");
    fs::create_dir(&bin).unwrap();
    let marker = root.path().join("fake-git-ran");
    let fake_git = bin.join("git");
    fs::write(
        &fake_git,
        format!("#!/bin/sh\ntouch '{}'\nexit 0\n", marker.display()),
    )
    .unwrap();
    fs::set_permissions(&fake_git, fs::Permissions::from_mode(0o755)).unwrap();
    let system_path = std::env::var_os("PATH").unwrap();
    let mut entries = vec![std::path::PathBuf::from("relative-bin")];
    entries.extend(std::env::split_paths(&system_path));
    let untrusted_path = std::env::join_paths(entries).unwrap();

    let output = binary()
        .current_dir(root.path())
        .args(["audit", ".", "--format", "json"])
        .env("PATH", untrusted_path)
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert!(!marker.exists(), "a relative PATH entry selected fake git");
}

#[cfg(all(unix, not(target_os = "macos")))]
#[test]
fn non_utf8_worktree_path_is_supported_without_disclosure() {
    use std::os::unix::ffi::OsStringExt;

    let root = tempdir().unwrap();
    let name = std::ffi::OsString::from_vec(b"worktree-\xff".to_vec());
    let repository = root.path().join(name);
    fs::create_dir(&repository).unwrap();

    let output = binary()
        .arg("audit")
        .arg(&repository)
        .args(["--format", "json"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.windows(6).any(|bytes| bytes == b"AWD002"));
    assert!(!output
        .stdout
        .windows(10)
        .any(|bytes| bytes == b"worktree-\xff"));
}

#[test]
fn healthy_worktrees_support_spaces_and_unicode() {
    let root = tempdir().unwrap();
    let main = root.path().join("main space 中文🧰");
    let linked = root.path().join("linked space 中文🧰");
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
    assert!(output.status.success(), "{output:?}");
    assert!(!String::from_utf8(output.stdout)
        .unwrap()
        .contains("main space"));
}

#[cfg(windows)]
#[test]
fn windows_case_variant_and_long_path_are_supported_when_available() {
    let root = tempdir().unwrap();
    let repository = root
        .path()
        .join("CaseMain")
        .join("long_segment_0123456789".repeat(5));
    fs::create_dir_all(repository.parent().unwrap()).unwrap();
    git(root.path(), &["init", repository.to_str().unwrap()]);
    let case_variant =
        std::path::PathBuf::from(repository.to_string_lossy().replace("CaseMain", "casemain"));
    let output = binary()
        .args(["audit", case_variant.to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
}

#[cfg(unix)]
#[test]
fn fifo_admin_backlink_is_rejected_without_blocking() {
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
    let backlink = std::path::Path::new(gitdir.trim()).join("gitdir");
    fs::remove_file(&backlink).unwrap();
    let status = Command::new("mkfifo").arg(&backlink).status().unwrap();
    assert!(status.success());
    let started = std::time::Instant::now();
    let output = binary()
        .args(["audit", linked.to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap();
    assert!(started.elapsed() < std::time::Duration::from_secs(2));
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8(output.stdout).unwrap().contains("AWD007"));
}

#[test]
fn too_many_inputs_fail_without_echoing_any_path() {
    let root = tempdir().unwrap();
    let paths: Vec<_> = (0..65)
        .map(|index| root.path().join(format!("private-input-{index}")))
        .collect();
    let mut command = binary();
    command.arg("audit");
    command.args(&paths);
    let output = command.output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!combined.contains(root.path().to_str().unwrap()));
    assert!(!combined.contains("private-input"));
}

#[test]
fn invalid_submodule_path_makes_the_audit_incomplete_without_leaking_it() {
    let root = tempdir().unwrap();
    git(root.path(), &["init"]);
    fs::write(
        root.path().join(".gitmodules"),
        "[submodule \"private\"]\n\tpath = ../private-secret\n\turl = ignored\n",
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
    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("AWD015"));
    assert!(!stdout.contains("private-secret"));
}

#[test]
fn sarif_carries_machine_readable_audit_state() {
    let root = tempdir().unwrap();
    let output = binary()
        .args(["audit", root.path().to_str().unwrap(), "--format", "sarif"])
        .output()
        .unwrap();
    let sarif: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let properties = &sarif["runs"][0]["properties"];
    assert_eq!(properties["schema_version"], 1);
    assert_eq!(properties["kind"], "agent_worktree_audit");
    assert_eq!(properties["complete"], true);
    assert!(properties["summary"].is_object());
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
