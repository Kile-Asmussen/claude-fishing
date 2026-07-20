//! # Claude Fishing: hooks suite for Claude Code CLI
//!
//! Intended to enable Claude to act with greater autonomy but remaining safe
//! such as preventing reading secrets files, not reading files outside the project
//! directory, limiting web fetch to certain domains, and so on.

use clap::{Parser, Subcommand};
use claude_fishing::hook_input::{ConfigChangeInput, HookCheck, PreToolUseInput};
use claude_fishing::hooks;
use claude_fishing::hooks::env::HookEnv;
use rootcause::{prelude::ResultExt, report};
use std::{
    io::{Read, stdin},
    path::PathBuf,
};
use strum_macros::{EnumIs, IntoStaticStr};

/// Claude Code hook enforcement suite
#[derive(Parser)]
struct Cli {
    /// Use the current working directory instead of $CLAUDE_PROJECT_DIR
    #[arg(long, global = true)]
    cwd: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, IntoStaticStr, EnumIs)]
enum Command {
    /// run as a PreToolUse hook, enforcing bash/paths/webfetch/write allowlists
    PreToolUse,
    /// ConfigChange hook that validates $CLAUDE_PROJECT_DIR/.claude/settings.json
    ConfigChange {
        /// Only check that settings.json is valid JSON, skip schema validation
        #[arg(long, conflicts_with = "schema")]
        json_only: bool,
        /// Schema URL or file:// path to validate against (default: the official schema from json.schemastore.org)
        #[arg(long, conflicts_with = "json_only")]
        schema: Option<String>,
    },
    /// SessionStart hook, clearing the log for the current session (preserves old log with ~ postfix)
    RotateLog,
    /// Create missing config files under .claude/ with example contents
    Init {
        /// Command used to invoke the hooks binary in injected hook entries
        #[arg(long, default_value = "fishing")]
        inject: String,
        /// Command used to invoke the MCP binary in the injected mcpServers entry
        #[arg(long, default_value = "fishing-grep-glob-mcp")]
        mcp: String,
    },
    /// CwdChanged hook that blocks all cwd changes
    CwdChanged,
}

fn project_dir(cwd: bool) -> Result<PathBuf, rootcause::Report> {
    if cwd {
        Ok(std::env::current_dir().context(format!("could not read current working directory"))?)
    } else {
        Ok(std::env::var_os("CLAUDE_PROJECT_DIR")
            .map(PathBuf::from)
            .ok_or_else(|| report!("CLAUDE_PROJECT_DIR is not set and --cwd was not specified"))?)
    }
}

fn run() -> Result<(), rootcause::Report> {
    let cli = Cli::parse();

    let project_dir = project_dir(cli.cwd || cli.command.is_init())?;

    let claude = project_dir.join(".claude");

    let log = Some(claude.join("fishing.log"));

    let mut env = HookEnv::from_claude_dir(&claude, log.clone());

    env.log(&format!(
        "[{}] {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        Into::<&'static str>::into(&cli.command)
    ));

    match cli.command {
        Command::CwdChanged => {
            env.config_block("Changing the current working directory is not allowed");
        }

        Command::RotateLog => {
            if let Some(ref log) = log {
                hooks::rotate_log::rotate(log);
            }
        }

        Command::Init { inject, mcp } => {
            let r = hooks::init::init(&project_dir, &mut env, &inject, &mcp);
            env.report(r, "Initialization failed")?;
        }

        Command::ConfigChange { json_only, schema } => {
            let mode = if json_only {
                hooks::settings::Mode::JsonOnly
            } else if let Some(s) = schema {
                hooks::settings::Mode::Schema(s)
            } else {
                hooks::settings::Mode::Default
            };
            let mut raw = String::new();
            env.report(stdin().read_to_string(&mut raw), "unable to read stdin")?;
            env.log(format!("stdin:\n{raw}"));

            let input: ConfigChangeInput = env.report(
                serde_json::from_str(&raw),
                "unable to parse JSON from stdin",
            )?;

            input.check(&project_dir, &mut env, mode);
        }

        Command::PreToolUse => {
            let mut raw = String::new();
            env.report(stdin().read_to_string(&mut raw), "unable to read stdin")?;

            env.log(format!("stdin:\n{raw}"));
            let input: PreToolUseInput = env.report(
                serde_json::from_str(&raw),
                "unable to parse JSON from stdin",
            )?;
            input.check(&project_dir, &mut env, ());
        }
    }
    env.flush();

    Ok(())
}

fn main() {
    match std::panic::catch_unwind(run) {
        Err(_) => {
            eprintln!("panic");
            std::process::exit(2);
        }
        Ok(Err(e)) => {
            eprintln!("{}", e);
            std::process::exit(2);
        }
        _ => {}
    }
}
