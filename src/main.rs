use agent_worktree_doctor::{audit, render, AuditOptions, OutputFormat};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Audit {
        #[arg(required = true, num_args = 1..)]
        paths: Vec<PathBuf>,
        #[arg(long)]
        include_submodules: bool,
        #[arg(long)]
        check_tracked_status: bool,
        #[arg(long, value_enum, default_value = "text")]
        format: OutputFormat,
    },
}

fn main() {
    let result = match Cli::parse().command {
        Command::Audit {
            paths,
            include_submodules,
            check_tracked_status,
            format,
        } => audit(
            &paths,
            AuditOptions {
                include_submodules,
                check_tracked_status,
            },
        )
        .and_then(|report| render(&report, format).map(|output| (report, output))),
    };
    match result {
        Ok((report, output)) => {
            println!("{output}");
            let code = if !report.complete {
                2
            } else if report.findings.is_empty() {
                0
            } else {
                1
            };
            std::process::exit(code);
        }
        Err(_) => {
            eprintln!("agent_worktree_doctor: audit could not be completed");
            std::process::exit(2);
        }
    }
}
