use anyhow::{bail, Result};
use clap::ValueEnum;
use serde::Serialize;
use serde_json::json;
use std::cell::Cell;
use std::collections::HashSet;
use std::ffi::OsString;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const MAX_INPUTS: usize = 64;
const MAX_METADATA_BYTES: u64 = 64 * 1024;
const MAX_GIT_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_FINDINGS: usize = 10_000;
const MAX_WORKTREES: usize = 10_000;
const MAX_SUBMODULES: usize = 1_000;
const GIT_TIMEOUT: Duration = Duration::from_secs(5);
const AUDIT_TIMEOUT: Duration = Duration::from_secs(60);

thread_local! {
    static AUDIT_DEADLINE: Cell<Option<Instant>> = const { Cell::new(None) };
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Clone, Debug, Serialize)]
pub struct Finding {
    pub code: &'static str,
    pub severity: Severity,
    pub input_index: usize,
    pub message: &'static str,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Summary {
    pub inputs: usize,
    pub repositories: usize,
    pub worktrees: usize,
    pub findings: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub schema_version: u8,
    pub kind: &'static str,
    pub complete: bool,
    pub summary: Summary,
    pub findings: Vec<Finding>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
    Sarif,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct AuditOptions {
    pub include_submodules: bool,
    pub check_tracked_status: bool,
}

pub fn audit(paths: &[PathBuf], options: AuditOptions) -> Result<Report> {
    if paths.is_empty() || paths.len() > MAX_INPUTS {
        bail!("invalid input count");
    }
    let mut report = Report {
        schema_version: 1,
        kind: "agent_worktree_audit",
        complete: true,
        summary: Summary {
            inputs: paths.len(),
            ..Summary::default()
        },
        findings: Vec::new(),
    };
    AUDIT_DEADLINE.set(Some(Instant::now() + AUDIT_TIMEOUT));
    let mut seen_repositories = HashSet::new();
    for (input_index, path) in paths.iter().enumerate() {
        if audit_deadline_reached() {
            incomplete(
                &mut report,
                input_index,
                "The total audit time limit was reached",
            );
            break;
        }
        audit_input(
            path,
            input_index,
            options,
            &mut seen_repositories,
            &mut report,
        );
    }
    AUDIT_DEADLINE.set(None);
    report.summary.findings = report.findings.len();
    Ok(report)
}

pub fn render(report: &Report, format: OutputFormat) -> Result<String> {
    match format {
        OutputFormat::Json => Ok(serde_json::to_string(report)?),
        OutputFormat::Text => {
            let mut output = format!(
                "agent_worktree_doctor: {} finding(s), complete={}\n",
                report.findings.len(),
                report.complete
            );
            for finding in &report.findings {
                output.push_str(&format!(
                    "{} {:?} input {}: {}\n",
                    finding.code, finding.severity, finding.input_index, finding.message
                ));
            }
            Ok(output)
        }
        OutputFormat::Sarif => Ok(serde_json::to_string(&json!({
            "version": "2.1.0",
            "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
            "runs": [{
                "tool": {"driver": {"name": "agent_worktree_doctor"}},
                "results": report.findings.iter().map(|finding| json!({
                    "ruleId": finding.code,
                    "level": if finding.severity == Severity::Error { "error" } else { "warning" },
                    "message": {"text": finding.message},
                    "properties": {"input_index": finding.input_index}
                })).collect::<Vec<_>>()
            }]
        }))?),
    }
}

fn audit_input(
    path: &Path,
    input_index: usize,
    options: AuditOptions,
    seen_repositories: &mut HashSet<PathBuf>,
    report: &mut Report,
) {
    if !path.is_dir() {
        add(
            report,
            "AWD001",
            Severity::Error,
            input_index,
            "Input path is missing or is not a directory",
        );
        return;
    }

    let dot_git = path.join(".git");
    if dot_git.is_file() {
        match read_gitfile(&dot_git) {
            Ok(target) if !target.is_dir() => {
                add(
                    report,
                    "AWD005",
                    Severity::Error,
                    input_index,
                    "The worktree gitdir target is missing or is not a directory",
                );
                return;
            }
            Err(_) => {
                add(
                    report,
                    "AWD004",
                    Severity::Error,
                    input_index,
                    "The worktree .git file is malformed or exceeds its limit",
                );
                return;
            }
            Ok(_) => {}
        }
    }

    let probe = match run_git(
        path,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--git-dir",
            "--git-common-dir",
            "--show-toplevel",
        ],
    ) {
        Ok(probe) if probe.success => probe,
        Ok(_) => {
            add(
                report,
                "AWD002",
                Severity::Error,
                input_index,
                "Input is not a Git worktree",
            );
            return;
        }
        Err(_) => {
            probe_failed(report, input_index, "A bounded Git probe failed");
            return;
        }
    };
    let lines: Vec<&[u8]> = probe
        .stdout
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .collect();
    if lines.len() != 3 {
        incomplete(
            report,
            input_index,
            "Git returned an invalid worktree response",
        );
        return;
    }
    let git_dir = path_from_git_bytes(lines[0]);
    let common_dir = path_from_git_bytes(lines[1]);
    let top_level = path_from_git_bytes(lines[2]);
    let repository_key = canonical(&common_dir).unwrap_or_else(|| common_dir.clone());
    let first_repository_input = seen_repositories.insert(repository_key);
    if first_repository_input {
        report.summary.repositories += 1;
    }

    if git_dir != common_dir {
        audit_linked_metadata(&git_dir, &common_dir, &top_level, input_index, report);
    }
    if first_repository_input {
        audit_registry(path, input_index, report);
        audit_shared_core_worktree(path, &common_dir, input_index, report);
    }
    if options.include_submodules {
        audit_submodules(path, &top_level, input_index, report);
    }
    if options.check_tracked_status {
        audit_status(path, input_index, report);
    }
}

fn audit_linked_metadata(
    git_dir: &Path,
    common_dir: &Path,
    top_level: &Path,
    input_index: usize,
    report: &mut Report,
) {
    let commondir = git_dir.join("commondir");
    let resolved_common = read_pointer(&commondir, git_dir);
    if resolved_common.as_deref().and_then(canonical).as_deref() != canonical(common_dir).as_deref()
    {
        add(
            report,
            "AWD006",
            Severity::Error,
            input_index,
            "The linked worktree commondir is missing or inconsistent",
        );
    }
    let backward = git_dir.join("gitdir");
    let expected = top_level.join(".git");
    match read_pointer(&backward, git_dir) {
        None => add(
            report,
            "AWD007",
            Severity::Error,
            input_index,
            "The worktree administration backlink is missing or malformed",
        ),
        Some(target) if canonical(&target).as_deref() != canonical(&expected).as_deref() => add(
            report,
            "AWD008",
            Severity::Error,
            input_index,
            "The worktree forward and backward links disagree",
        ),
        Some(_) => {}
    }
}

fn audit_registry(path: &Path, input_index: usize, report: &mut Report) {
    match run_git(path, &["worktree", "list", "--porcelain", "-z"]) {
        Ok(probe) if probe.success => {
            let mut current_missing = false;
            for field in probe.stdout.split(|byte| *byte == 0) {
                if let Some(value) = field.strip_prefix(b"worktree ") {
                    if report.summary.worktrees >= MAX_WORKTREES {
                        incomplete(
                            report,
                            input_index,
                            "The registered worktree limit was reached",
                        );
                        return;
                    }
                    report.summary.worktrees += 1;
                    current_missing = !path_from_git_bytes(value).exists();
                } else if field.starts_with(b"prunable") {
                    add(
                        report,
                        "AWD009",
                        Severity::Warning,
                        input_index,
                        "Git marks a registered worktree as prunable",
                    );
                } else if field.starts_with(b"locked") && current_missing {
                    add(
                        report,
                        "AWD010",
                        Severity::Warning,
                        input_index,
                        "A locked worktree is currently unavailable",
                    );
                }
            }
        }
        _ => probe_failed(
            report,
            input_index,
            "Git could not enumerate registered worktrees",
        ),
    }
}

fn audit_shared_core_worktree(
    path: &Path,
    common_dir: &Path,
    input_index: usize,
    report: &mut Report,
) {
    let extension = run_git(
        path,
        &[
            "config",
            "--type=bool",
            "--get",
            "extensions.worktreeConfig",
        ],
    );
    let enabled =
        matches!(extension, Ok(ref probe) if probe.success && probe.stdout.starts_with(b"true"));
    if !enabled {
        return;
    }
    let config = common_dir.join("config");
    let shared = run_git_os(
        path,
        &[
            OsString::from("config"),
            OsString::from("--file"),
            config.into_os_string(),
            OsString::from("--get"),
            OsString::from("core.worktree"),
        ],
    );
    if matches!(shared, Ok(ref probe) if probe.success) {
        add(
            report,
            "AWD011",
            Severity::Error,
            input_index,
            "Shared core.worktree conflicts with worktree-specific configuration",
        );
    }
}

fn audit_submodules(path: &Path, top_level: &Path, input_index: usize, report: &mut Report) {
    let modules = top_level.join(".gitmodules");
    match fs::symlink_metadata(&modules) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
        Ok(metadata)
            if metadata.file_type().is_file()
                && !metadata.file_type().is_symlink()
                && metadata.len() <= MAX_METADATA_BYTES => {}
        _ => {
            incomplete(
                report,
                input_index,
                "Submodule metadata is inaccessible or exceeds its limit",
            );
            return;
        }
    }
    let probe = run_git_os(
        path,
        &[
            OsString::from("config"),
            OsString::from("--file"),
            modules.into_os_string(),
            OsString::from("--null"),
            OsString::from("--get-regexp"),
            OsString::from("^submodule\\..*\\.path$"),
        ],
    );
    let Ok(probe) = probe else {
        probe_failed(
            report,
            input_index,
            "Submodule metadata could not be audited",
        );
        return;
    };
    if !probe.success {
        return;
    }
    let lines: Vec<&[u8]> = probe
        .stdout
        .split(|byte| *byte == 0)
        .filter(|line| !line.is_empty())
        .collect();
    if lines.len() > MAX_SUBMODULES {
        incomplete(
            report,
            input_index,
            "The registered submodule limit was reached",
        );
        return;
    }
    for line in lines {
        let Some(separator) = line.iter().position(|byte| *byte == b'\n') else {
            continue;
        };
        let value = &line[separator + 1..];
        let relative = path_from_git_bytes(value);
        if relative.is_absolute()
            || relative
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            continue;
        }
        let gitfile = top_level.join(relative).join(".git");
        if gitfile.is_file() {
            let invalid = match read_gitfile(&gitfile) {
                Ok(target) => !target.is_dir(),
                Err(_) => true,
            };
            if invalid {
                add(
                    report,
                    "AWD012",
                    Severity::Error,
                    input_index,
                    "An initialized submodule has a missing or invalid gitdir target",
                );
            } else {
                match run_git(
                    gitfile.parent().unwrap_or(top_level),
                    &["rev-parse", "--path-format=absolute", "--show-toplevel"],
                ) {
                    Ok(scope) if scope.success => {
                        let observed = path_from_git_bytes(trim_line(&scope.stdout));
                        let expected = gitfile.parent().unwrap_or(top_level);
                        if canonical(&observed).as_deref() != canonical(expected).as_deref() {
                            add(
                                report,
                                "AWD013",
                                Severity::Warning,
                                input_index,
                                "A submodule work directory resolves to an unexpected Git scope",
                            );
                        }
                    }
                    Ok(_) => add(
                        report,
                        "AWD013",
                        Severity::Warning,
                        input_index,
                        "A submodule work directory resolves to an unexpected Git scope",
                    ),
                    Err(_) => probe_failed(
                        report,
                        input_index,
                        "A submodule scope probe could not be completed",
                    ),
                }
            }
        }
    }
}

fn audit_status(path: &Path, input_index: usize, report: &mut Report) {
    match run_git(
        path,
        &[
            "-c",
            "core.fsmonitor=false",
            "status",
            "--porcelain=v2",
            "-z",
            "--untracked-files=no",
        ],
    ) {
        Ok(probe) if probe.success && !probe.stdout.is_empty() => add(
            report,
            "AWD014",
            Severity::Warning,
            input_index,
            "Tracked or index changes are present",
        ),
        Ok(probe) if probe.success => {}
        _ => probe_failed(report, input_index, "Tracked status could not be audited"),
    }
}

fn read_gitfile(path: &Path) -> Result<PathBuf> {
    let target = read_limited(path)?;
    if target.contains(&0) || target.iter().filter(|byte| **byte == b'\n').count() > 1 {
        bail!("invalid gitfile");
    }
    let line = trim_line(&target);
    let value = line
        .strip_prefix(b"gitdir: ")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("invalid gitfile"))?;
    let value = path_from_git_bytes(value);
    Ok(if value.is_absolute() {
        value
    } else {
        path.parent().unwrap_or(Path::new(".")).join(value)
    })
}

fn read_pointer(path: &Path, base: &Path) -> Option<PathBuf> {
    let bytes = read_limited(path).ok()?;
    if bytes.contains(&0) || bytes.iter().filter(|byte| **byte == b'\n').count() > 1 {
        return None;
    }
    let value = trim_line(&bytes);
    if value.is_empty() {
        return None;
    }
    let value = path_from_git_bytes(value);
    Some(if value.is_absolute() {
        value
    } else {
        base.join(value)
    })
}

fn read_limited(path: &Path) -> Result<Vec<u8>> {
    if fs::symlink_metadata(path)?.file_type().is_symlink()
        || fs::metadata(path)?.len() > MAX_METADATA_BYTES
    {
        bail!("invalid metadata file");
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(MAX_METADATA_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_METADATA_BYTES {
        bail!("metadata limit exceeded");
    }
    Ok(bytes)
}

fn canonical(path: &Path) -> Option<PathBuf> {
    fs::canonicalize(path).ok()
}

fn trim_line(bytes: &[u8]) -> &[u8] {
    let bytes = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    bytes.strip_suffix(b"\r").unwrap_or(bytes)
}

#[cfg(unix)]
fn path_from_git_bytes(bytes: &[u8]) -> PathBuf {
    use std::os::unix::ffi::OsStringExt;
    std::ffi::OsString::from_vec(bytes.to_vec()).into()
}

#[cfg(not(unix))]
fn path_from_git_bytes(bytes: &[u8]) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
}

struct GitProbe {
    success: bool,
    stdout: Vec<u8>,
}

fn run_git(cwd: &Path, args: &[&str]) -> Result<GitProbe> {
    let args: Vec<OsString> = args.iter().map(OsString::from).collect();
    run_git_os(cwd, &args)
}

fn run_git_os(cwd: &Path, args: &[OsString]) -> Result<GitProbe> {
    let mut child = Command::new("git")
        .arg("--no-pager")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let out_reader = thread::spawn(move || read_process_output(stdout));
    let err_reader = thread::spawn(move || read_process_output(stderr));
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() >= GIT_TIMEOUT || audit_deadline_reached() {
            child.kill()?;
            let _ = child.wait();
            bail!("Git probe timed out");
        }
        thread::sleep(Duration::from_millis(10));
    };
    let stdout = out_reader
        .join()
        .map_err(|_| anyhow::anyhow!("stdout reader failed"))??;
    let _stderr = err_reader
        .join()
        .map_err(|_| anyhow::anyhow!("stderr reader failed"))??;
    Ok(GitProbe {
        success: status.success(),
        stdout,
    })
}

fn audit_deadline_reached() -> bool {
    AUDIT_DEADLINE
        .get()
        .is_some_and(|deadline| Instant::now() >= deadline)
}

fn read_process_output(reader: impl Read) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    reader
        .take((MAX_GIT_OUTPUT_BYTES + 1) as u64)
        .read_to_end(&mut output)?;
    if output.len() > MAX_GIT_OUTPUT_BYTES {
        bail!("Git output limit exceeded");
    }
    Ok(output)
}

fn add(
    report: &mut Report,
    code: &'static str,
    severity: Severity,
    input_index: usize,
    message: &'static str,
) {
    if report.findings.len() >= MAX_FINDINGS {
        report.complete = false;
        return;
    }
    report.findings.push(Finding {
        code,
        severity,
        input_index,
        message,
    });
}

fn incomplete(report: &mut Report, input_index: usize, message: &'static str) {
    report.complete = false;
    add(report, "AWD015", Severity::Error, input_index, message);
}

fn probe_failed(report: &mut Report, input_index: usize, message: &'static str) {
    add(
        report,
        "AWD003",
        Severity::Error,
        input_index,
        "A bounded read-only Git probe failed",
    );
    incomplete(report, input_index, message);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_gitfiles_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(".git");
        fs::write(&path, "gitdir: first\nsecond\n").unwrap();
        assert!(read_gitfile(&path).is_err());
    }

    #[test]
    fn report_formats_never_need_source_paths() {
        let report = Report {
            schema_version: 1,
            kind: "agent_worktree_audit",
            complete: true,
            summary: Summary::default(),
            findings: vec![Finding {
                code: "AWD001",
                severity: Severity::Error,
                input_index: 0,
                message: "fixed",
            }],
        };
        for format in [OutputFormat::Text, OutputFormat::Json, OutputFormat::Sarif] {
            let output = render(&report, format).unwrap();
            assert!(!output.contains("/private/"));
        }
    }
}
