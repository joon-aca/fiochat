mod cli;
mod client;
mod config;
mod function;
mod mcp;
mod rag;
mod render;
mod repl;
mod serve;
#[macro_use]
mod utils;

#[macro_use]
extern crate log;

use crate::cli::Cli;
use crate::cli::PromptMode;
use crate::client::{
    call_chat_completions, call_chat_completions_streaming, list_models, ModelType,
};
use crate::config::{
    ensure_parent_exists, list_agents, load_env_file, macro_execute, Config, GlobalConfig, Input,
    WorkingMode, CODE_ROLE, EXPLAIN_SHELL_ROLE, SHELL_ROLE, TEMP_SESSION_NAME,
};
use crate::render::render_error;
use crate::repl::Repl;
use crate::utils::*;

use anyhow::{bail, Result};
use chrono::{Duration, Local, Utc};
use clap::Parser;
use inquire::Text;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use simplelog::{format_description, ConfigBuilder, LevelFilter, SimpleLogger, WriteLogger};
use std::collections::HashMap;
use std::fs::{read_to_string, write};
use std::path::{Path, PathBuf};
use std::{env, process, sync::Arc};

const ARM_TTL_MINUTES: i64 = 30;
const ARM_STATE_FILE: &str = "arm-state.yaml";
const INSTALL_BIN_DIR: &str = "/usr/local/bin";

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum UtilityCommand {
    Arm,
    Disarm,
    Doctor,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct ArmStateStore {
    #[serde(default)]
    entries: HashMap<String, i64>,
}

#[tokio::main]
async fn main() -> Result<()> {
    load_env_file()?;
    if let Some(command) = parse_utility_command() {
        handle_utility_command(command)?;
        return Ok(());
    }

    let default_prompt_mode = match invoked_as_fiochat() {
        true => PromptMode::Chat,
        false => PromptMode::Auto,
    };
    let cli = Cli::parse();
    let text = cli.text()?;
    let working_mode = if cli.serve.is_some() {
        WorkingMode::Serve
    } else if text.is_none() && cli.file.is_empty() {
        WorkingMode::Repl
    } else {
        WorkingMode::Cmd
    };
    let info_flag = cli.info
        || cli.sync_models
        || cli.list_models
        || cli.list_roles
        || cli.list_agents
        || cli.list_rags
        || cli.list_macros
        || cli.list_sessions;
    setup_logger(working_mode.is_serve())?;
    let config = Arc::new(RwLock::new(Config::init(working_mode, info_flag).await?));
    if let Err(err) = run(config, cli, text, default_prompt_mode).await {
        render_error(err);
        std::process::exit(1);
    }
    Ok(())
}

async fn run(
    config: GlobalConfig,
    cli: Cli,
    text: Option<String>,
    default_prompt_mode: PromptMode,
) -> Result<()> {
    let abort_signal = create_abort_signal();
    let requested_mode = cli.prompt_mode(default_prompt_mode);
    let has_explicit_role = cli.prompt.is_some() || cli.role.is_some();
    let effective_mode =
        resolve_effective_prompt_mode(requested_mode, text.as_deref(), has_explicit_role);

    if cli.sync_models {
        let url = config.read().sync_models_url();
        return Config::sync_models(&url, abort_signal.clone()).await;
    }

    if cli.list_models {
        for model in list_models(&config.read(), ModelType::Chat) {
            println!("{}", model.id());
        }
        return Ok(());
    }
    if cli.list_roles {
        let roles = Config::list_roles(true).join("\n");
        println!("{roles}");
        return Ok(());
    }
    if cli.list_agents {
        let agents = list_agents().join("\n");
        println!("{agents}");
        return Ok(());
    }
    if cli.list_rags {
        let rags = Config::list_rags().join("\n");
        println!("{rags}");
        return Ok(());
    }
    if cli.list_macros {
        let macros = Config::list_macros().join("\n");
        println!("{macros}");
        return Ok(());
    }

    if cli.dry_run {
        config.write().dry_run = true;
    }
    if cli.hide_thinking {
        config.write().hide_thinking = true;
    }

    if let Some(agent) = &cli.agent {
        let session = cli.session.as_ref().map(|v| match v {
            Some(v) => v.as_str(),
            None => TEMP_SESSION_NAME,
        });
        if !cli.agent_variable.is_empty() {
            config.write().agent_variables = Some(
                cli.agent_variable
                    .chunks(2)
                    .map(|v| (v[0].to_string(), v[1].to_string()))
                    .collect(),
            );
        }

        let ret = Config::use_agent(&config, agent, session, abort_signal.clone()).await;
        config.write().agent_variables = None;
        ret?;
    } else {
        if let Some(prompt) = &cli.prompt {
            config.write().use_prompt(prompt)?;
        } else if let Some(name) = &cli.role {
            config.write().use_role(name)?;
        } else if matches!(effective_mode, PromptMode::Plan | PromptMode::Execute) {
            config.write().use_role(SHELL_ROLE)?;
        } else if cli.code {
            config.write().use_role(CODE_ROLE)?;
        }
        if let Some(session) = &cli.session {
            config
                .write()
                .use_session(session.as_ref().map(|v| v.as_str()))?;
        }
        if let Some(rag) = &cli.rag {
            Config::use_rag(&config, Some(rag), abort_signal.clone()).await?;
        }
    }
    if cli.list_sessions {
        let sessions = config.read().list_sessions().join("\n");
        println!("{sessions}");
        return Ok(());
    }
    if let Some(model_id) = &cli.model {
        config.write().set_model(model_id)?;
    }
    if cli.no_stream {
        config.write().stream = false;
    }
    if cli.empty_session {
        config.write().empty_session()?;
    }
    if cli.save_session {
        config.write().set_save_session_this_time()?;
    }
    if cli.info {
        let info = config.read().info()?;
        println!("{info}");
        return Ok(());
    }
    if let Some(addr) = cli.serve {
        return serve::run(config, addr).await;
    }
    let is_repl = config.read().working_mode.is_repl();
    if cli.rebuild_rag {
        Config::rebuild_rag(&config, abort_signal.clone()).await?;
        if is_repl {
            return Ok(());
        }
    }
    if let Some(name) = &cli.macro_name {
        macro_execute(&config, name, text.as_deref(), abort_signal.clone()).await?;
        return Ok(());
    }
    if matches!(effective_mode, PromptMode::Plan | PromptMode::Execute) && !is_repl {
        let input = create_input(&config, text, &cli.file, abort_signal.clone()).await?;
        let auto_armed = requested_mode == PromptMode::Auto && scope_is_armed().unwrap_or(false);
        let execute_without_confirm = effective_mode == PromptMode::Execute || auto_armed;
        shell_execute(
            &config,
            &SHELL,
            input,
            abort_signal.clone(),
            execute_without_confirm,
        )
        .await?;
        return Ok(());
    }
    config.write().apply_prelude()?;
    match is_repl {
        false => {
            let mut input = create_input(&config, text, &cli.file, abort_signal.clone()).await?;
            input.use_embeddings(abort_signal.clone()).await?;
            start_directive(&config, input, cli.code, abort_signal).await
        }
        true => {
            if !*IS_STDOUT_TERMINAL {
                bail!("No TTY for REPL")
            }
            start_interactive(&config).await
        }
    }
}

#[async_recursion::async_recursion]
async fn start_directive(
    config: &GlobalConfig,
    input: Input,
    code_mode: bool,
    abort_signal: AbortSignal,
) -> Result<()> {
    let client = input.create_client()?;
    let extract_code = !*IS_STDOUT_TERMINAL && code_mode;
    config.write().before_chat_completion(&input)?;
    let (output, tool_results) = if !input.stream() || extract_code {
        call_chat_completions(
            &input,
            true,
            extract_code,
            client.as_ref(),
            abort_signal.clone(),
        )
        .await?
    } else {
        call_chat_completions_streaming(&input, client.as_ref(), abort_signal.clone()).await?
    };
    config
        .write()
        .after_chat_completion(&input, &output, &tool_results)?;

    if !tool_results.is_empty() {
        start_directive(
            config,
            input.merge_tool_results(output, tool_results),
            code_mode,
            abort_signal,
        )
        .await?;
    }

    config.write().exit_session()?;
    Ok(())
}

async fn start_interactive(config: &GlobalConfig) -> Result<()> {
    let mut repl: Repl = Repl::init(config)?;
    repl.run().await
}

#[async_recursion::async_recursion]
async fn shell_execute(
    config: &GlobalConfig,
    shell: &Shell,
    mut input: Input,
    abort_signal: AbortSignal,
    execute_without_confirm: bool,
) -> Result<()> {
    let client = input.create_client()?;
    config.write().before_chat_completion(&input)?;
    let (eval_str, _) =
        call_chat_completions(&input, false, true, client.as_ref(), abort_signal.clone()).await?;

    config
        .write()
        .after_chat_completion(&input, &eval_str, &[])?;
    if eval_str.is_empty() {
        bail!("No command generated");
    }
    if config.read().dry_run {
        config.read().print_markdown(&eval_str)?;
        return Ok(());
    }
    let high_risk = is_high_risk_command(&eval_str);
    let requires_confirmation = !execute_without_confirm || high_risk;
    if *IS_STDOUT_TERMINAL {
        if execute_without_confirm && !high_risk {
            debug!("{} {:?}", shell.cmd, &[&shell.arg, &eval_str]);
            let code = run_command(&shell.cmd, &[&shell.arg, &eval_str], None)?;
            if code == 0 && config.read().save_shell_history {
                let _ = append_to_shell_history(&shell.name, &eval_str, code);
            }
            process::exit(code);
        }

        if execute_without_confirm && requires_confirmation {
            println!(
                "{}",
                dimmed_text("High-risk command detected; explicit confirmation required.")
            );
        }

        let options = ["execute", "revise", "describe", "copy", "quit"];
        let command = color_text(eval_str.trim(), nu_ansi_term::Color::Rgb(255, 165, 0));
        let first_letter_color = nu_ansi_term::Color::Cyan;
        let prompt_text = options
            .iter()
            .map(|v| format!("{}{}", color_text(&v[0..1], first_letter_color), &v[1..]))
            .collect::<Vec<String>>()
            .join(&dimmed_text(" | "));
        loop {
            println!("{command}");
            let answer_char =
                read_single_key(&['e', 'r', 'd', 'c', 'q'], 'e', &format!("{prompt_text}: "))?;

            match answer_char {
                'e' => {
                    debug!("{} {:?}", shell.cmd, &[&shell.arg, &eval_str]);
                    let code = run_command(&shell.cmd, &[&shell.arg, &eval_str], None)?;
                    if code == 0 && config.read().save_shell_history {
                        let _ = append_to_shell_history(&shell.name, &eval_str, code);
                    }
                    process::exit(code);
                }
                'r' => {
                    let revision = Text::new("Enter your revision:").prompt()?;
                    let text = format!("{}\n{revision}", input.text());
                    input.set_text(text);
                    return shell_execute(
                        config,
                        shell,
                        input,
                        abort_signal.clone(),
                        execute_without_confirm,
                    )
                    .await;
                }
                'd' => {
                    let role = config.read().retrieve_role(EXPLAIN_SHELL_ROLE)?;
                    let input = Input::from_str(config, &eval_str, Some(role));
                    if input.stream() {
                        call_chat_completions_streaming(
                            &input,
                            client.as_ref(),
                            abort_signal.clone(),
                        )
                        .await?;
                    } else {
                        call_chat_completions(
                            &input,
                            true,
                            false,
                            client.as_ref(),
                            abort_signal.clone(),
                        )
                        .await?;
                    }
                    println!();
                    continue;
                }
                'c' => {
                    set_text(&eval_str)?;
                    println!("{}", dimmed_text("✓ Copied the command."));
                }
                _ => {}
            }
            break;
        }
    } else {
        println!("{eval_str}");
    }
    Ok(())
}

fn parse_utility_command() -> Option<UtilityCommand> {
    let args = env::args().collect::<Vec<_>>();
    if args.len() != 2 {
        return None;
    }
    match args[1].as_str() {
        "arm" => Some(UtilityCommand::Arm),
        "disarm" => Some(UtilityCommand::Disarm),
        "doctor" => Some(UtilityCommand::Doctor),
        _ => None,
    }
}

fn invoked_as_fiochat() -> bool {
    let argv0 = match env::args().next() {
        Some(v) => v,
        None => return false,
    };
    let path = Path::new(&argv0);
    let stem = path
        .file_stem()
        .and_then(|v| v.to_str())
        .unwrap_or_default();
    stem.eq_ignore_ascii_case("fiochat")
}

fn resolve_effective_prompt_mode(
    requested_mode: PromptMode,
    text: Option<&str>,
    has_explicit_role: bool,
) -> PromptMode {
    match requested_mode {
        PromptMode::Auto => {
            if has_explicit_role {
                PromptMode::Chat
            } else if looks_operational_prompt(text.unwrap_or_default()) {
                PromptMode::Plan
            } else {
                PromptMode::Chat
            }
        }
        _ => requested_mode,
    }
}

fn looks_operational_prompt(text: &str) -> bool {
    let text = text.trim().to_ascii_lowercase();
    if text.is_empty() {
        return false;
    }

    let explanatory_prefixes = [
        "what ",
        "why ",
        "how ",
        "how do ",
        "how to ",
        "explain ",
        "tell me ",
        "describe ",
        "can you explain ",
    ];
    if explanatory_prefixes
        .iter()
        .any(|prefix| text.starts_with(prefix))
    {
        return false;
    }

    let command_prefixes = [
        "git ",
        "docker ",
        "kubectl ",
        "terraform ",
        "ansible ",
        "helm ",
        "npm ",
        "pnpm ",
        "yarn ",
        "cargo ",
        "make ",
        "systemctl ",
        "brew ",
        "apt ",
        "yum ",
        "dnf ",
        "ssh ",
        "scp ",
        "rsync ",
    ];
    if command_prefixes
        .iter()
        .any(|prefix| text.starts_with(prefix))
    {
        return true;
    }

    let operational_keywords = [
        "commit",
        "push",
        "deploy",
        "release",
        "restart",
        "start",
        "stop",
        "install",
        "uninstall",
        "remove",
        "delete",
        "create",
        "run",
        "execute",
        "build",
        "test",
        "lint",
        "format",
        "rollback",
        "migrate",
        "kill",
        "tail",
        "grep",
        "checkout",
        "rebase",
        "merge",
        "cherry-pick",
    ];
    let first_word = text.split_whitespace().next().unwrap_or_default();
    if operational_keywords.contains(&first_word) {
        return true;
    }

    operational_keywords
        .iter()
        .any(|keyword| text.contains(keyword))
        && (text.contains("please")
            || text.contains("can you")
            || text.contains("could you")
            || text.contains(" and ")
            || text.contains(" then "))
}

fn is_high_risk_command(command: &str) -> bool {
    let command = command.to_ascii_lowercase();
    let high_risk_patterns = [
        "rm -rf /",
        "mkfs",
        "dd if=",
        "shutdown",
        "reboot",
        "halt",
        "poweroff",
        "git push --force",
        "git push -f",
        "git reset --hard",
        "git clean -fd",
        "terraform destroy",
        "kubectl delete",
        "drop database",
    ];
    high_risk_patterns
        .iter()
        .any(|pattern| command.contains(pattern))
}

fn current_scope_key() -> Result<String> {
    let cwd = env::current_dir()?;
    let root = detect_repo_root(&cwd);
    let canonical = root.canonicalize().unwrap_or(root);
    Ok(canonical.to_string_lossy().to_string())
}

fn detect_repo_root(start: &Path) -> PathBuf {
    let mut cursor = start.to_path_buf();
    loop {
        if cursor.join(".git").exists() {
            return cursor;
        }
        let parent = match cursor.parent() {
            Some(parent) => parent.to_path_buf(),
            None => return start.to_path_buf(),
        };
        if parent == cursor {
            return start.to_path_buf();
        }
        cursor = parent;
    }
}

fn arm_state_path() -> PathBuf {
    Config::local_path(ARM_STATE_FILE)
}

fn load_arm_state(path: &Path) -> ArmStateStore {
    read_to_string(path)
        .ok()
        .and_then(|v| serde_yaml::from_str::<ArmStateStore>(&v).ok())
        .unwrap_or_default()
}

fn save_arm_state(path: &Path, state: &ArmStateStore) -> Result<()> {
    ensure_parent_exists(path)?;
    let body = serde_yaml::to_string(state)?;
    write(path, body)?;
    Ok(())
}

fn scope_is_armed() -> Result<bool> {
    let path = arm_state_path();
    let scope = current_scope_key()?;
    let mut state = load_arm_state(&path);
    let now = Utc::now().timestamp();

    let armed = state
        .entries
        .get(&scope)
        .copied()
        .map(|expires_at| expires_at > now)
        .unwrap_or(false);

    if !armed && state.entries.remove(&scope).is_some() {
        let _ = save_arm_state(&path, &state);
    }

    Ok(armed)
}

fn handle_utility_command(command: UtilityCommand) -> Result<()> {
    if command == UtilityCommand::Doctor {
        return run_doctor();
    }

    let scope = current_scope_key()?;
    let path = arm_state_path();
    let mut state = load_arm_state(&path);

    match command {
        UtilityCommand::Arm => {
            let expires_at = Utc::now() + Duration::minutes(ARM_TTL_MINUTES);
            state.entries.insert(scope.clone(), expires_at.timestamp());
            save_arm_state(&path, &state)?;
            let local_expiry = expires_at.with_timezone(&Local);
            println!(
                "Armed execution for scope '{}' until {}.",
                scope,
                local_expiry.format("%Y-%m-%d %H:%M:%S %Z")
            );
        }
        UtilityCommand::Disarm => {
            state.entries.remove(&scope);
            save_arm_state(&path, &state)?;
            println!("Disarmed execution for scope '{}'.", scope);
        }
        UtilityCommand::Doctor => {}
    }

    Ok(())
}

fn run_doctor() -> Result<()> {
    let fio_path = which::which("fio").ok();
    let fiochat_path = which::which("fiochat").ok();
    let preferred_bin = PathBuf::from(INSTALL_BIN_DIR);

    println!("Fio doctor");
    println!(
        "  preferred bin dir: {}",
        preferred_bin.to_string_lossy()
    );
    println!(
        "  fio: {}",
        fio_path
            .as_ref()
            .map(|v| v.display().to_string())
            .unwrap_or_else(|| "not found".to_string())
    );
    println!(
        "  fiochat: {}",
        fiochat_path
            .as_ref()
            .map(|v| v.display().to_string())
            .unwrap_or_else(|| "not found".to_string())
    );

    match (&fio_path, &fiochat_path) {
        (Some(fio), Some(fiochat)) if same_path(fio, fiochat) => {
            println!("  status: OK (fio resolves to fiochat)");
        }
        (Some(fio), Some(fiochat)) => {
            println!("  status: COLLISION (fio points to another tool)");
            println!(
                "  note: alternate fio (flexible I/O tester) already exists on this machine. Installed as fiochat."
            );
            println!("  note: fio is '{}', fiochat is '{}'", fio.display(), fiochat.display());
            println!(
                "  fix: sudo ln -sf {} {} (this will override the existing fio).",
                fiochat.display(),
                preferred_bin.join("fio").display()
            );
        }
        (Some(fio), None) => {
            println!("  status: fio exists, fiochat missing");
            println!("  note: fio currently resolves to '{}'", fio.display());
        }
        (None, Some(fiochat)) => {
            println!("  status: fiochat available, fio alias missing");
            println!(
                "  fix: sudo ln -sf {} {} (this will override the existing fio).",
                fiochat.display(),
                preferred_bin.join("fio").display()
            );
        }
        (None, None) => {
            println!("  status: neither fio nor fiochat found in PATH");
        }
    }

    Ok(())
}

fn same_path(a: &Path, b: &Path) -> bool {
    let a = a.canonicalize().unwrap_or_else(|_| a.to_path_buf());
    let b = b.canonicalize().unwrap_or_else(|_| b.to_path_buf());
    a == b
}

async fn create_input(
    config: &GlobalConfig,
    text: Option<String>,
    file: &[String],
    abort_signal: AbortSignal,
) -> Result<Input> {
    let input = if file.is_empty() {
        Input::from_str(config, &text.unwrap_or_default(), None)
    } else {
        Input::from_files_with_spinner(
            config,
            &text.unwrap_or_default(),
            file.to_vec(),
            None,
            abort_signal,
        )
        .await?
    };
    if input.is_empty() {
        bail!("No input");
    }
    Ok(input)
}

fn setup_logger(is_serve: bool) -> Result<()> {
    let (log_level, log_path) = Config::log_config(is_serve)?;
    if log_level == LevelFilter::Off {
        return Ok(());
    }
    let crate_name = env!("CARGO_CRATE_NAME");
    let log_filter = match std::env::var(get_env_name("log_filter")) {
        Ok(v) => v,
        Err(_) => match is_serve {
            true => format!("{crate_name}::serve"),
            false => crate_name.into(),
        },
    };
    let config = ConfigBuilder::new()
        .add_filter_allow(log_filter)
        .set_time_format_custom(format_description!(
            "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z"
        ))
        .set_thread_level(LevelFilter::Off)
        .build();
    match log_path {
        None => {
            SimpleLogger::init(log_level, config)?;
        }
        Some(log_path) => {
            ensure_parent_exists(&log_path)?;
            let log_file = std::fs::File::create(log_path)?;
            WriteLogger::init(log_level, config, log_file)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operational_prompt_is_detected() {
        assert!(looks_operational_prompt("git commit and push the changes"));
        assert!(looks_operational_prompt("can you restart nginx and tail logs"));
    }

    #[test]
    fn explanatory_prompt_is_not_operational() {
        assert!(!looks_operational_prompt("how do I commit and push safely?"));
        assert!(!looks_operational_prompt("explain git rebase"));
    }

    #[test]
    fn auto_mode_routes_based_on_intent() {
        assert_eq!(
            resolve_effective_prompt_mode(PromptMode::Auto, Some("restart nginx"), false),
            PromptMode::Plan
        );
        assert_eq!(
            resolve_effective_prompt_mode(PromptMode::Auto, Some("why is nginx failing"), false),
            PromptMode::Chat
        );
    }

    #[test]
    fn high_risk_detection_matches_force_push() {
        assert!(is_high_risk_command("git push --force-with-lease"));
        assert!(!is_high_risk_command("git push origin main"));
    }
}
