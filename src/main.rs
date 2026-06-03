mod defaults;
mod hook_input;
mod hooks;

use clap::{Parser, Subcommand};
use hook_input::{HookCheck, HookInput};
use hooks::env::HookEnv;
use std::path::PathBuf;

/// Claude Code hook enforcement suite
#[derive(Parser)]
struct Cli {
    /// Log file path (default: $CLAUDE_PROJECT_DIR/.claude/log); used by tool-use for appending decisions, by rotate-log as the rotation target
    #[arg(long, global = true)]
    log: Option<PathBuf>,

    /// Use the current working directory instead of $CLAUDE_PROJECT_DIR
    #[arg(long, short = 'C', global = true)]
    cwd: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// PreToolUse hook: reads stdin JSON, enforces bash/paths/webfetch/write allowlists
    ToolUse,
    /// ConfigChange hook: validates $CLAUDE_PROJECT_DIR/.claude/settings.json against its JSON Schema
    Settings {
        /// Only check that settings.json is valid JSON, skip schema validation
        #[arg(long)]
        json_only: bool,
        /// Schema URL or file:// path to validate against (default: json.schemastore.org)
        #[arg(long)]
        schema: Option<String>,
    },
    /// SessionStart hook: renames the log file by appending ~, clearing it for the new session
    RotateLog,
    /// Create missing config files under .claude/ with safe default contents
    Init {
        /// Also inject hooks into .claude/settings.json using this command prefix
        #[arg(long)]
        inject: Option<String>,
    },
}

fn project_dir(cwd: bool) -> PathBuf {
    if cwd {
        std::env::current_dir().expect("could not read current working directory")
    } else {
        std::env::var_os("CLAUDE_PROJECT_DIR")
            .map(PathBuf::from)
            .expect("CLAUDE_PROJECT_DIR is not set and --cwd was not specified")
    }
}

fn run() {
    let cli = Cli::parse();

    let project_dir = project_dir(cli.cwd);
    let claude = project_dir.join(".claude");

    let log: Option<PathBuf> = cli.log.or_else(|| Some(claude.join("log")));

    let mut env = HookEnv {
        bash:     hooks::env::HookConfig::File(claude.join("bash")),
        paths:    hooks::env::HookConfig::File(claude.join("paths")),
        webfetch: hooks::env::HookConfig::File(claude.join("webfetch")),
        settings: hooks::env::HookConfig::File(claude.join("settings.json")),
        log_path: log.clone(),
        log_buf:  String::new(),
        response: None,
    };

    match cli.command {
        Command::RotateLog => {
            if let Some(ref log) = log {
                hooks::rotate_log::rotate(log);
            }
        }

        Command::Init { inject } => {
            if let Err(e) = hooks::init::init(&project_dir, &mut env, inject.as_deref()) {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }

        Command::Settings { json_only, schema } => {
            let mode = if json_only {
                hooks::settings::Mode::JsonOnly
            } else if let Some(s) = schema {
                hooks::settings::Mode::Schema(s)
            } else {
                hooks::settings::Mode::Default
            };
            hooks::settings::check(&project_dir, &mut env, mode);
            env.flush();
        }

        Command::ToolUse => {
            let mut raw = String::new();
            if let Err(e) = std::io::Read::read_to_string(&mut std::io::stdin(), &mut raw) {
                eprintln!("failed to read stdin: {e}");
                std::process::exit(2);
            }
            env.log(format!("stdin: {raw}"));
            let input: HookInput = match serde_json::from_str(&raw) {
                Ok(v) => v,
                Err(e) => {
                    env.log(format!("stdin parse error: {e}"));
                    env.flush();
                    eprintln!("failed to parse stdin as JSON: {e}");
                    std::process::exit(2);
                }
            };
            match input.tool_input {
                Some(tool_input) => tool_input.check(&project_dir, &mut env),
                None => env.allow("unrecognised tool, allowing by default"),
            }
            env.flush();
        }
    }
}

fn main() {
    if std::panic::catch_unwind(run).is_err() {
        std::process::exit(2);
    }
}
