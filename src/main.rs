mod cli;
mod client;
mod config;
mod function;
mod interactive;
mod mcp;
mod rag;
mod render;
mod resolver;
mod router;
mod serve;
#[macro_use]
mod utils;

#[macro_use]
extern crate log;

use crate::cli::Cli;
use crate::client::{
    call_chat_completions, call_chat_completions_streaming, list_models, ModelType,
};
use crate::config::{
    ensure_parent_exists, list_agents, load_env_file, macro_execute, Config, GlobalConfig, Input,
    Role, WorkingMode, CODE_ROLE, EXPLAIN_SHELL_ROLE, SHELL_ROLE, TEMP_SESSION_NAME,
};
use crate::interactive::InteractiveMode;
use crate::render::render_error;
use crate::router::{
    role_for_route, route_turn, select_route_model, TurnOperation, TurnPolicy, TurnRoute,
};
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

    let default_policy = match invoked_as_fiochat() {
        true => TurnPolicy::Chat,
        false => TurnPolicy::Auto,
    };
    let cli = Cli::parse();
    let text = cli.text()?;
    let working_mode = if cli.serve.is_some() {
        WorkingMode::Serve
    } else if text.is_none() && cli.file.is_empty() {
        WorkingMode::Interactive
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
    if let Err(err) = run(config, cli, text, default_policy).await {
        render_error(err);
        std::process::exit(1);
    }
    Ok(())
}

async fn run(
    config: GlobalConfig,
    cli: Cli,
    text: Option<String>,
    default_policy: TurnPolicy,
) -> Result<()> {
    let abort_signal = create_abort_signal();
    let requested_policy = cli.turn_policy(default_policy);
    let has_explicit_role = cli.prompt.is_some() || cli.role.is_some();

    // For non-interactive one-shot, resolve the effective policy up front.
    // The router will be called later for the actual routing decision.
    let effective_policy = if has_explicit_role || requested_policy != TurnPolicy::Auto {
        match requested_policy {
            TurnPolicy::Auto if has_explicit_role => TurnPolicy::Chat,
            other => other,
        }
    } else {
        // Will be resolved by route_turn later for one-shot
        TurnPolicy::Auto
    };

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

    // Apply route model based on effective policy (fast for chat, thinking for plan/execute).
    // Only takes effect when no role/session/agent explicitly overrides the model.
    // Falls back to the default model if the route setting is absent.
    let route_model_id = {
        let cfg = config.read();
        select_route_model(
            effective_policy,
            cfg.model_fast.as_deref(),
            cfg.model_thinking.as_deref(),
        )
    };
    if let Some(id) = route_model_id {
        config.write().set_model(&id)?;
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
        } else if matches!(effective_policy, TurnPolicy::Plan | TurnPolicy::Execute) {
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
    let is_interactive = config.read().working_mode.is_interactive();
    if cli.rebuild_rag {
        Config::rebuild_rag(&config, abort_signal.clone()).await?;
        if is_interactive {
            return Ok(());
        }
    }
    if let Some(name) = &cli.macro_name {
        macro_execute(&config, name, text.as_deref(), abort_signal.clone()).await?;
        return Ok(());
    }

    if !is_interactive {
        // One-shot path: route through the router when policy is Auto
        if effective_policy == TurnPolicy::Auto {
            if let Some(ref input_text) = text {
                let route = route_turn(&config, abort_signal.clone(), input_text).await?;

                if let Some(operation) = route.operation.clone() {
                    execute_route_operation(&config, &route, operation).await?;
                    return Ok(());
                }

                // Apply routed model
                if let Some(ref id) = route.model_id {
                    config.write().set_model(id)?;
                }

                // Apply routed policy
                let routed_policy = route.policy;
                if matches!(routed_policy, TurnPolicy::Plan | TurnPolicy::Execute) {
                    config.write().use_role(SHELL_ROLE)?;
                }
                let route_role = role_for_route(&config, &route);

                match routed_policy {
                    TurnPolicy::Plan | TurnPolicy::Execute => {
                        let input = create_input(
                            &config,
                            Some(route.text),
                            &cli.file,
                            abort_signal.clone(),
                            route_role,
                        )
                        .await?;
                        let auto_armed = scope_is_armed().unwrap_or(false);
                        let execute_without_confirm =
                            routed_policy == TurnPolicy::Execute || auto_armed;
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
                    TurnPolicy::Chat => {
                        config.write().apply_prelude()?;
                        let mut input = create_input(
                            &config,
                            Some(route.text),
                            &cli.file,
                            abort_signal.clone(),
                            route_role,
                        )
                        .await?;
                        input.use_embeddings(abort_signal.clone()).await?;
                        return start_directive(&config, input, cli.code, abort_signal).await;
                    }
                    TurnPolicy::Auto => unreachable!("router always resolves Auto"),
                }
            }
        }

        // Explicit policy (--plan, --execute, --chat) or no text
        if matches!(effective_policy, TurnPolicy::Plan | TurnPolicy::Execute) {
            let input = create_input(&config, text, &cli.file, abort_signal.clone(), None).await?;
            let auto_armed =
                requested_policy == TurnPolicy::Auto && scope_is_armed().unwrap_or(false);
            let execute_without_confirm = effective_policy == TurnPolicy::Execute || auto_armed;
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
        let mut input = create_input(&config, text, &cli.file, abort_signal.clone(), None).await?;
        input.use_embeddings(abort_signal.clone()).await?;
        return start_directive(&config, input, cli.code, abort_signal).await;
    }

    // Interactive path
    config.write().apply_prelude()?;
    if !*IS_STDOUT_TERMINAL {
        bail!("No TTY for interactive mode")
    }
    start_interactive(&config).await
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
    let mut interactive = InteractiveMode::init(config)?;
    interactive.run().await
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

        let command = color_text(eval_str.trim(), nu_ansi_term::Color::Rgb(255, 165, 0));
        let first_letter_color = nu_ansi_term::Color::Cyan;
        let esc_hint_color = nu_ansi_term::Color::Fixed(245);
        let prompt_text = [
            color_text("<Enter>", first_letter_color),
            format!("{}{}", color_text("e", first_letter_color), "dit"),
            format!("{}{}", color_text("d", first_letter_color), "escribe"),
            format!("{}{}", color_text("c", first_letter_color), "opy"),
            color_text("<Esc>", esc_hint_color),
        ]
        .join(&dimmed_text(" | "));
        loop {
            println!("{command}");
            let answer_char = read_single_key(
                &['e', 'd', 'c', '\u{1b}'],
                '\0',
                Some('\u{1b}'),
                &format!("{prompt_text}: "),
            )?;

            match answer_char {
                '\0' => {
                    debug!("{} {:?}", shell.cmd, &[&shell.arg, &eval_str]);
                    let code = run_command(&shell.cmd, &[&shell.arg, &eval_str], None)?;
                    if code == 0 && config.read().save_shell_history {
                        let _ = append_to_shell_history(&shell.name, &eval_str, code);
                    }
                    process::exit(code);
                }
                'e' => {
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
    println!("  preferred bin dir: {}", preferred_bin.to_string_lossy());
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
            println!(
                "  note: fio is '{}', fiochat is '{}'",
                fio.display(),
                fiochat.display()
            );
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
    role: Option<Role>,
) -> Result<Input> {
    let input = if file.is_empty() {
        Input::from_str(config, &text.unwrap_or_default(), role)
    } else {
        Input::from_files_with_spinner(
            config,
            &text.unwrap_or_default(),
            file.to_vec(),
            role,
            abort_signal,
        )
        .await?
    };
    if input.is_empty() {
        bail!("No input");
    }
    Ok(input)
}

async fn execute_route_operation(
    config: &GlobalConfig,
    route: &TurnRoute,
    operation: TurnOperation,
) -> Result<()> {
    match operation {
        TurnOperation::ConnectMcpServer(server_name) => {
            let server_name = ensure_connectable_server(config, &server_name).await?;
            match Config::mcp_connect_server(config, &server_name).await {
                Ok(()) => {}
                Err(err) if should_offer_linear_api_key_bootstrap(&server_name, &err) => {
                    Config::prompt_and_store_linear_api_key(config, &server_name).await?;
                    Config::mcp_connect_server(config, &server_name).await?;
                }
                Err(err) if server_uses_oauth(config, &server_name) => {
                    let start = Config::mcp_oauth_login_start(config, &server_name).await?;
                    println!("OAuth device login for '{}':", server_name);
                    println!(
                        "  verification_uri: {}",
                        start
                            .verification_uri_complete
                            .as_deref()
                            .unwrap_or(&start.verification_uri)
                    );
                    println!("  user_code: {}", start.user_code);
                    println!("Waiting for authorization...");
                    Config::mcp_oauth_login_complete(config, &server_name, &start).await?;
                    Config::mcp_connect_server(config, &server_name).await?;
                }
                Err(err) => return Err(err),
            }
            Config::refresh_functions(config).await?;
            if server_name.starts_with("linear-") {
                config
                    .write()
                    .set_current_linear_profile(Some(server_name.clone()));
                match Config::sync_linear_team_aliases(config, &server_name).await {
                    Ok(learned) if !learned.is_empty() => {
                        println!(
                            "Learned Linear team aliases for '{}': {}",
                            server_name,
                            learned.join(", ")
                        );
                    }
                    Ok(_) => {}
                    Err(err) => warn!(
                        "Failed to sync Linear team aliases for '{}': {}",
                        server_name, err
                    ),
                }
            }
            println!("✓ Connected to MCP server '{}'", server_name);
        }
        TurnOperation::DisconnectMcpServer(server_name) => {
            Config::mcp_disconnect_server(config, &server_name).await?;
            Config::refresh_functions(config).await?;
            if config.read().current_linear_profile() == Some(server_name.as_str()) {
                config.write().set_current_linear_profile(None);
            }
            println!("✓ Disconnected from MCP server '{}'", server_name);
        }
    }

    if let Some(intent) = route.intent.clone() {
        let cloned = config.read().resolver.clone();
        if let Some(mut r) = cloned {
            r.learn(&intent);
            if let Err(e) = r.save() {
                warn!("Resolver: failed to save after learning: {e}");
            } else {
                config.write().resolver = Some(r);
            }
        }
    }

    Ok(())
}

async fn ensure_connectable_server(config: &GlobalConfig, server_name: &str) -> Result<String> {
    if let Some(workspace_slug) = server_name.strip_prefix("linear-") {
        Config::ensure_linear_profile(config, workspace_slug).await
    } else {
        Ok(server_name.to_string())
    }
}

fn server_uses_oauth(config: &GlobalConfig, server_name: &str) -> bool {
    config
        .read()
        .mcp_servers
        .iter()
        .find(|server| server.name == server_name)
        .and_then(|server| server.auth.as_ref())
        .and_then(|auth| auth.oauth_config())
        .is_some()
}

fn should_offer_linear_api_key_bootstrap(server_name: &str, err: &anyhow::Error) -> bool {
    server_name.starts_with("linear-")
        && [
            "LINEAR_API_KEY",
            "LINEAR_CLIENT_ID",
            "LINEAR_CLIENT_SECRET",
            "FIOCHAT_MCP_TOKEN_STORE_KEY",
        ]
        .iter()
        .any(|needle| err.to_string().contains(needle))
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
    fn high_risk_detection_matches_force_push() {
        assert!(is_high_risk_command("git push --force-with-lease"));
        assert!(!is_high_risk_command("git push origin main"));
    }
}
