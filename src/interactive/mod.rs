mod completer;
mod highlighter;
mod prompt;

use self::completer::InteractiveCompleter;
use self::highlighter::InteractiveHighlighter;
use self::prompt::InteractivePrompt;

use crate::client::{
    call_chat_completions, call_chat_completions_streaming, list_models, Model, ModelType,
};
use crate::config::{
    macro_execute, AgentVariables, AssertState, Config, GlobalConfig, Input, LastMessage,
    StateFlags,
};
use crate::resolver::Resolver;
use crate::router::route_turn;
use crate::render::render_error;
use crate::utils::{
    abortable_run_with_spinner, create_abort_signal, dimmed_text, set_text, temp_file, AbortSignal,
};

use anyhow::{bail, Context, Result};
use crossterm::cursor::SetCursorStyle;
use fancy_regex::Regex;
use reedline::CursorConfig;
use reedline::{
    default_emacs_keybindings, default_vi_insert_keybindings, default_vi_normal_keybindings,
    ColumnarMenu, EditCommand, EditMode, Emacs, KeyCode, KeyModifiers, Keybindings, Reedline,
    ReedlineEvent, ReedlineMenu, ValidationResult, Validator, Vi,
};
use reedline::{MenuBuilder, Signal};
use std::process;
use std::sync::LazyLock;

const MENU_NAME: &str = "completion_menu";
const SUSPEND_HOST_COMMAND: &str = "__fiochat_internal_suspend__";

static INTERACTIVE_COMMANDS: LazyLock<[InteractiveCommand; 39]> = LazyLock::new(|| {
    [
        InteractiveCommand::new(".help", "Show this help guide", AssertState::pass()),
        InteractiveCommand::new(".info", "Show system info", AssertState::pass()),
        InteractiveCommand::new(
            ".edit config",
            "Modify configuration file",
            AssertState::False(StateFlags::AGENT),
        ),
        InteractiveCommand::new(".model", "Manage fast/thinking model routing", AssertState::pass()),
        InteractiveCommand::new(
            ".prompt",
            "Set a temporary role using a prompt",
            AssertState::False(StateFlags::SESSION | StateFlags::AGENT),
        ),
        InteractiveCommand::new(
            ".role",
            "Create or switch to a role",
            AssertState::False(StateFlags::SESSION | StateFlags::AGENT),
        ),
        InteractiveCommand::new(
            ".info role",
            "Show role info",
            AssertState::True(StateFlags::ROLE),
        ),
        InteractiveCommand::new(
            ".edit role",
            "Modify current role",
            AssertState::TrueFalse(StateFlags::ROLE, StateFlags::SESSION),
        ),
        InteractiveCommand::new(
            ".save role",
            "Save current role to file",
            AssertState::TrueFalse(
                StateFlags::ROLE,
                StateFlags::SESSION_EMPTY | StateFlags::SESSION,
            ),
        ),
        InteractiveCommand::new(
            ".exit role",
            "Exit active role",
            AssertState::TrueFalse(StateFlags::ROLE, StateFlags::SESSION),
        ),
        InteractiveCommand::new(
            ".session",
            "Start or switch to a session",
            AssertState::False(StateFlags::SESSION_EMPTY | StateFlags::SESSION),
        ),
        InteractiveCommand::new(
            ".empty session",
            "Clear session messages",
            AssertState::True(StateFlags::SESSION),
        ),
        InteractiveCommand::new(
            ".compress session",
            "Compress session messages",
            AssertState::True(StateFlags::SESSION),
        ),
        InteractiveCommand::new(
            ".info session",
            "Show session info",
            AssertState::True(StateFlags::SESSION_EMPTY | StateFlags::SESSION),
        ),
        InteractiveCommand::new(
            ".edit session",
            "Modify current session",
            AssertState::True(StateFlags::SESSION_EMPTY | StateFlags::SESSION),
        ),
        InteractiveCommand::new(
            ".save session",
            "Save current session to file",
            AssertState::True(StateFlags::SESSION_EMPTY | StateFlags::SESSION),
        ),
        InteractiveCommand::new(
            ".exit session",
            "Exit active session",
            AssertState::True(StateFlags::SESSION_EMPTY | StateFlags::SESSION),
        ),
        InteractiveCommand::new(".agent", "Use an agent", AssertState::bare()),
        InteractiveCommand::new(
            ".starter",
            "Use a conversation starter",
            AssertState::True(StateFlags::AGENT),
        ),
        InteractiveCommand::new(
            ".edit agent-config",
            "Modify agent configuration file",
            AssertState::True(StateFlags::AGENT),
        ),
        InteractiveCommand::new(
            ".info agent",
            "Show agent info",
            AssertState::True(StateFlags::AGENT),
        ),
        InteractiveCommand::new(
            ".exit agent",
            "Leave agent",
            AssertState::True(StateFlags::AGENT),
        ),
        InteractiveCommand::new(
            ".rag",
            "Initialize or access RAG",
            AssertState::False(StateFlags::AGENT),
        ),
        InteractiveCommand::new(
            ".edit rag-docs",
            "Add or remove documents from an existing RAG",
            AssertState::TrueFalse(StateFlags::RAG, StateFlags::AGENT),
        ),
        InteractiveCommand::new(
            ".rebuild rag",
            "Rebuild RAG for document changes",
            AssertState::True(StateFlags::RAG),
        ),
        InteractiveCommand::new(
            ".sources rag",
            "Show citation sources used in last query",
            AssertState::True(StateFlags::RAG),
        ),
        InteractiveCommand::new(
            ".info rag",
            "Show RAG info",
            AssertState::True(StateFlags::RAG),
        ),
        InteractiveCommand::new(
            ".exit rag",
            "Leave RAG",
            AssertState::TrueFalse(StateFlags::RAG, StateFlags::AGENT),
        ),
        InteractiveCommand::new(".macro", "Execute a macro", AssertState::pass()),
        InteractiveCommand::new(
            ".file",
            "Include files, directories, URLs or commands",
            AssertState::pass(),
        ),
        InteractiveCommand::new(
            ".continue",
            "Continue previous response",
            AssertState::pass(),
        ),
        InteractiveCommand::new(
            ".regenerate",
            "Regenerate last response",
            AssertState::pass(),
        ),
        InteractiveCommand::new(".copy", "Copy last response", AssertState::pass()),
        InteractiveCommand::new(".mcp", "Manage MCP servers/tools", AssertState::pass()),
        InteractiveCommand::new(
            ".resolver",
            "Manage intent resolver (provider/workspace/action aliases)",
            AssertState::pass(),
        ),
        InteractiveCommand::new(".set", "Modify runtime settings", AssertState::pass()),
        InteractiveCommand::new(
            ".thinking",
            "Toggle/show reasoning visibility",
            AssertState::pass(),
        ),
        InteractiveCommand::new(
            ".delete",
            "Delete roles, sessions, RAGs, or agents",
            AssertState::pass(),
        ),
        InteractiveCommand::new(".exit", "Exit interactive mode", AssertState::pass()),
    ]
});
static COMMAND_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*(\.\S*)\s*").unwrap());
static MULTILINE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)^\s*:::\s*(.*)\s*:::\s*$").unwrap());

pub struct InteractiveMode {
    config: GlobalConfig,
    editor: Reedline,
    prompt: InteractivePrompt,
    abort_signal: AbortSignal,
}

impl InteractiveMode {
    pub fn init(config: &GlobalConfig) -> Result<Self> {
        let editor = Self::create_editor(config)?;

        let prompt = InteractivePrompt::new(config);
        let abort_signal = create_abort_signal();

        Ok(Self {
            config: config.clone(),
            editor,
            prompt,
            abort_signal,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        if AssertState::False(StateFlags::AGENT | StateFlags::RAG)
            .assert(self.config.read().state())
        {
            print!("{}\n", fio_greeting())
        }

        loop {
            if self.abort_signal.aborted_ctrld() {
                break;
            }
            let sig = self.editor.read_line(&self.prompt);
            match sig {
                Ok(Signal::Success(line)) => {
                    if line == SUSPEND_HOST_COMMAND {
                        if let Err(err) = suspend_current_process() {
                            render_error(err);
                            println!();
                        }
                        continue;
                    }
                    self.abort_signal.reset();
                    match run_interactive_command(&self.config, self.abort_signal.clone(), &line).await {
                        Ok(exit) => {
                            if exit {
                                break;
                            }
                        }
                        Err(err) => {
                            render_error(err);
                            println!()
                        }
                    }
                }
                Ok(Signal::CtrlC) => {
                    self.abort_signal.set_ctrlc();
                    println!("(To exit, press Ctrl+D or enter \"/exit\")\n");
                }
                Ok(Signal::CtrlD) => {
                    self.abort_signal.set_ctrld();
                    break;
                }
                _ => {}
            }
        }
        self.config.write().exit_session()?;
        Ok(())
    }

    fn create_editor(config: &GlobalConfig) -> Result<Reedline> {
        let completer = InteractiveCompleter::new(config);
        let highlighter = InteractiveHighlighter::new(config);
        let menu = Self::create_menu();
        let edit_mode = Self::create_edit_mode(config);
        let cursor_config = CursorConfig {
            vi_insert: Some(SetCursorStyle::BlinkingBar),
            vi_normal: Some(SetCursorStyle::SteadyBlock),
            emacs: None,
        };
        let mut editor = Reedline::create()
            .with_completer(Box::new(completer))
            .with_highlighter(Box::new(highlighter))
            .with_menu(menu)
            .with_edit_mode(edit_mode)
            .with_cursor_config(cursor_config)
            .with_quick_completions(true)
            .with_partial_completions(true)
            .use_bracketed_paste(true)
            .with_validator(Box::new(ReplValidator))
            .with_ansi_colors(true);

        if let Ok(cmd) = config.read().editor() {
            let temp_file = temp_file("-repl-", ".md");
            let command = process::Command::new(cmd);
            editor = editor.with_buffer_editor(command, temp_file);
        }

        Ok(editor)
    }

    fn extra_keybindings(keybindings: &mut Keybindings) {
        keybindings.add_binding(
            KeyModifiers::NONE,
            KeyCode::Tab,
            ReedlineEvent::UntilFound(vec![
                ReedlineEvent::Menu(MENU_NAME.to_string()),
                ReedlineEvent::MenuNext,
            ]),
        );
        keybindings.add_binding(
            KeyModifiers::SHIFT,
            KeyCode::BackTab,
            ReedlineEvent::MenuPrevious,
        );
        keybindings.add_binding(
            KeyModifiers::CONTROL,
            KeyCode::Enter,
            ReedlineEvent::Edit(vec![EditCommand::InsertNewline]),
        );
        keybindings.add_binding(
            KeyModifiers::CONTROL,
            KeyCode::Char('j'),
            ReedlineEvent::Edit(vec![EditCommand::InsertNewline]),
        );
        keybindings.add_binding(
            KeyModifiers::CONTROL,
            KeyCode::Char('z'),
            ReedlineEvent::ExecuteHostCommand(SUSPEND_HOST_COMMAND.to_string()),
        );
    }

    fn create_edit_mode(config: &GlobalConfig) -> Box<dyn EditMode> {
        let edit_mode: Box<dyn EditMode> = if config.read().keybindings == "vi" {
            let mut insert_keybindings = default_vi_insert_keybindings();
            Self::extra_keybindings(&mut insert_keybindings);
            Box::new(Vi::new(insert_keybindings, default_vi_normal_keybindings()))
        } else {
            let mut keybindings = default_emacs_keybindings();
            Self::extra_keybindings(&mut keybindings);
            Box::new(Emacs::new(keybindings))
        };
        edit_mode
    }

    fn create_menu() -> ReedlineMenu {
        let completion_menu = ColumnarMenu::default().with_name(MENU_NAME);
        ReedlineMenu::EngineCompleter(Box::new(completion_menu))
    }
}

#[derive(Debug, Clone)]
pub struct InteractiveCommand {
    name: &'static str,
    description: &'static str,
    state: AssertState,
}

impl InteractiveCommand {
    fn new(name: &'static str, desc: &'static str, state: AssertState) -> Self {
        Self {
            name,
            description: desc,
            state,
        }
    }

    fn is_valid(&self, flags: StateFlags) -> bool {
        self.state.assert(flags)
    }
}

/// A default validator which checks for mismatched quotes and brackets
struct ReplValidator;

impl Validator for ReplValidator {
    fn validate(&self, line: &str) -> ValidationResult {
        let line = line.trim();
        if line.starts_with(r#":::"#) && !line[3..].ends_with(r#":::"#) {
            ValidationResult::Incomplete
        } else {
            ValidationResult::Complete
        }
    }
}

pub async fn run_interactive_command(
    config: &GlobalConfig,
    abort_signal: AbortSignal,
    mut line: &str,
) -> Result<bool> {
    if let Ok(Some(captures)) = MULTILINE_RE.captures(line) {
        if let Some(text_match) = captures.get(1) {
            line = text_match.as_str();
        }
    }
    let normalized_line = normalize_command_prefix(line);
    if let Some(value) = normalized_line.as_deref() {
        line = value;
    }
    if let Some(command) = parse_plain_command(line) {
        match command {
            PlainCommand::Exit => return Ok(true),
            PlainCommand::Help => {
                dump_repl_help();
                println!();
                return Ok(false);
            }
        }
    }
    match parse_command(line) {
        Some((cmd, args)) => match cmd {
            ".help" => {
                dump_repl_help();
            }
            ".info" => match args {
                Some("role") => {
                    let info = config.read().role_info()?;
                    print!("{info}");
                }
                Some("session") => {
                    let info = config.read().session_info()?;
                    print!("{info}");
                }
                Some("rag") => {
                    let info = config.read().rag_info()?;
                    print!("{info}");
                }
                Some("agent") => {
                    let info = config.read().agent_info()?;
                    print!("{info}");
                }
                Some(_) => unknown_command()?,
                None => {
                    let output = config.read().sysinfo()?;
                    print!("{output}");
                }
            },
            ".model" | ".models" => match args {
                Some(value) => {
                    let value = value.trim();
                    if value.eq_ignore_ascii_case("list") || value.is_empty() {
                        print_model_overview(config);
                    } else if value.starts_with("thinking ") || value.starts_with("thinking\t") {
                        let id_part = value["thinking".len()..].trim();
                        let model_id = match id_part.parse::<usize>() {
                            Ok(index) => model_id_from_index(config, index)?,
                            Err(_) => id_part.to_string(),
                        };
                        Model::retrieve_model(&config.read(), &model_id, ModelType::Chat)?;
                        config.write().model_thinking = Some(model_id.clone());
                        println!("✓ Thinking model set to {model_id}");
                    } else {
                        let model_id = match value.parse::<usize>() {
                            Ok(index) => model_id_from_index(config, index)?,
                            Err(_) => value.to_string(),
                        };
                        Model::retrieve_model(&config.read(), &model_id, ModelType::Chat)?;
                        config.write().model_fast = Some(model_id.clone());
                        println!("✓ Fast model set to {model_id}");
                    }
                }
                None => print_model_overview(config),
            },
            ".prompt" => match args {
                Some(text) => {
                    config.write().use_prompt(text)?;
                }
                None => println!("Usage: /prompt <text>..."),
            },
            ".role" => match args {
                Some(args) => match args.split_once(['\n', ' ']) {
                    Some((name, text)) => {
                        let role = config.read().retrieve_role(name.trim())?;
                        let input = Input::from_str(config, text, Some(role));
                        ask(config, abort_signal.clone(), input, false).await?;
                    }
                    None => {
                        let name = args;
                        if !Config::has_role(name) {
                            config.write().new_role(name)?;
                        }
                        config.write().use_role(name)?;
                    }
                },
                None => println!(
                    r#"Usage:
    /role <name>                    # If the role exists, switch to it; otherwise, create a new role
    /role <name> [text]...          # Temporarily switch to the role, send the text, and switch back"#
                ),
            },
            ".session" => {
                config.write().use_session(args)?;
                Config::maybe_autoname_session(config.clone());
            }
            ".rag" => {
                Config::use_rag(config, args, abort_signal.clone()).await?;
            }
            ".agent" => match split_first_arg(args) {
                Some((agent_name, args)) => {
                    let (new_args, _) = split_args_text(args.unwrap_or_default(), cfg!(windows));
                    let (session_name, variable_pairs) = match new_args.first() {
                        Some(name) if name.contains('=') => (None, new_args.as_slice()),
                        Some(name) => (Some(name.as_str()), &new_args[1..]),
                        None => (None, &[] as &[String]),
                    };
                    let variables: AgentVariables = variable_pairs
                        .iter()
                        .filter_map(|v| v.split_once('='))
                        .map(|(key, value)| (key.to_string(), value.to_string()))
                        .collect();
                    if variables.len() != variable_pairs.len() {
                        bail!("Some variable values are not key=value pairs");
                    }
                    if !variables.is_empty() {
                        config.write().agent_variables = Some(variables);
                    }
                    let ret =
                        Config::use_agent(config, agent_name, session_name, abort_signal.clone())
                            .await;
                    config.write().agent_variables = None;
                    ret?;
                }
                None => {
                    println!(r#"Usage: /agent <agent-name> [session-name] [key=value]..."#)
                }
            },
            ".starter" => match args {
                Some(id) => {
                    let mut text = None;
                    if let Some(agent) = config.read().agent.as_ref() {
                        for (i, value) in agent.conversation_staters().iter().enumerate() {
                            if (i + 1).to_string() == id {
                                text = Some(value.clone());
                            }
                        }
                    }
                    match text {
                        Some(text) => {
                            println!("{}", dimmed_text(&format!(">> {text}")));
                            let input = Input::from_str(config, &text, None);
                            ask(config, abort_signal.clone(), input, true).await?;
                        }
                        None => {
                            bail!("Invalid starter value");
                        }
                    }
                }
                None => {
                    let banner = config.read().agent_banner()?;
                    config.read().print_markdown(&banner)?;
                }
            },
            ".save" => match split_first_arg(args) {
                Some(("role", name)) => {
                    config.write().save_role(name)?;
                }
                Some(("session", name)) => {
                    config.write().save_session(name)?;
                }
                _ => {
                    println!(r#"Usage: /save <role|session> [name]"#)
                }
            },
            ".edit" => {
                if config.read().macro_flag {
                    bail!("Cannot perform this operation because you are in a macro")
                }
                match args {
                    Some("config") => {
                        config.read().edit_config()?;
                    }
                    Some("role") => {
                        config.write().edit_role()?;
                    }
                    Some("session") => {
                        config.write().edit_session()?;
                    }
                    Some("rag-docs") => {
                        Config::edit_rag_docs(config, abort_signal.clone()).await?;
                    }
                    Some("agent-config") => {
                        config.write().edit_agent_config()?;
                    }
                    _ => {
                        println!(r#"Usage: /edit <config|role|session|rag-docs|agent-config>"#)
                    }
                }
            }
            ".compress" => match args {
                Some("session") => {
                    abortable_run_with_spinner(
                        Config::compress_session(config),
                        "Compressing",
                        abort_signal.clone(),
                    )
                    .await?;
                    println!("✓ Successfully compressed the session.");
                }
                _ => {
                    println!(r#"Usage: /compress session"#)
                }
            },
            ".empty" => match args {
                Some("session") => {
                    config.write().empty_session()?;
                }
                _ => {
                    println!(r#"Usage: /empty session"#)
                }
            },
            ".rebuild" => match args {
                Some("rag") => {
                    Config::rebuild_rag(config, abort_signal.clone()).await?;
                }
                _ => {
                    println!(r#"Usage: /rebuild rag"#)
                }
            },
            ".sources" => match args {
                Some("rag") => {
                    let output = Config::rag_sources(config)?;
                    println!("{output}");
                }
                _ => {
                    println!(r#"Usage: /sources rag"#)
                }
            },
            ".macro" => match split_first_arg(args) {
                Some((name, extra)) => {
                    if !Config::has_macro(name) && extra.is_none() {
                        config.write().new_macro(name)?;
                    } else {
                        macro_execute(config, name, extra, abort_signal.clone()).await?;
                    }
                }
                None => println!("Usage: /macro <name> <text>..."),
            },
            ".file" => match args {
                Some(args) => {
                    let (files, text) = split_args_text(args, cfg!(windows));
                    let input = Input::from_files_with_spinner(
                        config,
                        text,
                        files,
                        None,
                        abort_signal.clone(),
                    )
                    .await?;
                    ask(config, abort_signal.clone(), input, true).await?;
                }
                None => println!(
                    r#"Usage: /file <file|dir|url|cmd|loader:resource|%%>... [-- <text>...]

/file /tmp/file.txt
/file src/ Cargo.toml -- analyze
/file https://example.com/file.txt -- summarize
/file https://example.com/image.png -- recognize text
/file `git diff` -- Generate git commit message
/file jina:https://example.com
/file %% -- translate last reply to english"#
                ),
            },
            ".continue" => {
                let LastMessage {
                    mut input, output, ..
                } = match config
                    .read()
                    .last_message
                    .as_ref()
                    .filter(|v| v.continuous && !v.output.is_empty())
                    .cloned()
                {
                    Some(v) => v,
                    None => bail!("Unable to continue the response"),
                };
                input.set_continue_output(&output);
                ask(config, abort_signal.clone(), input, true).await?;
            }
            ".regenerate" => {
                let LastMessage { mut input, .. } = match config
                    .read()
                    .last_message
                    .as_ref()
                    .filter(|v| v.continuous)
                    .cloned()
                {
                    Some(v) => v,
                    None => bail!("Unable to regenerate the response"),
                };
                input.set_regenerate();
                ask(config, abort_signal.clone(), input, true).await?;
            }
            ".set" => match args {
                Some(args) => {
                    Config::update(config, args)?;
                }
                _ => {
                    println!("Usage: /set <key> <value>...")
                }
            },
            ".thinking" => match args {
                None => {
                    if config.read().hide_thinking {
                        println!("Thinking blocks are currently hidden.");
                    } else {
                        println!("Thinking blocks are currently visible.");
                    }
                }
                Some(value) => match parse_thinking_toggle(value) {
                    Some(hide_thinking) => {
                        config.write().hide_thinking = hide_thinking;
                        if hide_thinking {
                            println!("✓ Thinking blocks will be hidden.");
                        } else {
                            println!("✓ Thinking blocks will be shown.");
                        }
                    }
                    None => {
                        println!("Usage: /thinking [on|off|show|hide]");
                    }
                },
            },
            ".delete" => match args {
                Some(args) => {
                    Config::delete(config, args)?;
                }
                _ => {
                    println!("Usage: /delete <role|session|rag|macro|agent-data>")
                }
            },
            ".copy" => {
                let output = match config
                    .read()
                    .last_message
                    .as_ref()
                    .filter(|v| !v.output.is_empty())
                    .map(|v| v.output.clone())
                {
                    Some(v) => v,
                    None => bail!("No chat response to copy"),
                };
                set_text(&output).context("Failed to copy the last chat response")?;
            }
            ".mcp" => match split_first_arg(args) {
                Some(("list", None)) => {
                    let servers = Config::mcp_list_servers(config).await;
                    if servers.is_empty() {
                        println!("No MCP servers configured");
                    } else {
                        println!("MCP Servers:");
                        for (name, connected, description) in servers {
                            let status = if connected {
                                "connected"
                            } else {
                                "disconnected"
                            };
                            let desc = description.map(|d| format!(" - {}", d)).unwrap_or_default();
                            println!("  {} [{}]{}", name, status, desc);
                        }
                    }
                }
                Some(("connect", Some(server_name))) => {
                    Config::mcp_connect_server(config, server_name).await?;
                    Config::refresh_functions(config).await?;
                    println!("✓ Connected to MCP server '{}'", server_name);
                }
                Some(("disconnect", Some(server_name))) => {
                    Config::mcp_disconnect_server(config, server_name).await?;
                    Config::refresh_functions(config).await?;
                    println!("✓ Disconnected from MCP server '{}'", server_name);
                }
                Some(("tools", server_name)) => {
                    let mcp_manager = config.read().mcp_manager.clone();
                    if let Some(manager) = mcp_manager {
                        let tools = if let Some(name) = server_name {
                            manager.get_server_tools(name).await?
                        } else {
                            manager.get_all_tools().await
                        };
                        if tools.is_empty() {
                            println!("No tools available");
                        } else {
                            println!("Available MCP Tools:");
                            for tool in tools {
                                println!("  {} - {}", tool.name, tool.description);
                            }
                        }
                    } else {
                        println!("MCP is not configured");
                    }
                }
                Some(("auth", Some(auth_args))) => match split_first_arg(Some(auth_args)) {
                    Some(("status", Some(server_name))) => {
                        let status = Config::mcp_oauth_status(config, server_name).await?;
                        println!(
                            "OAuth status for '{}': {}",
                            server_name,
                            status.kind.as_str()
                        );
                        if let Some(expires_at) = status.expires_at_unix {
                            use chrono::TimeZone;
                            let expires_local = chrono::Local
                                .timestamp_opt(expires_at, 0)
                                .single()
                                .map(|ts| ts.format("%Y-%m-%d %H:%M:%S %Z").to_string())
                                .unwrap_or_else(|| format!("unix:{expires_at}"));
                            println!("  expires_at: {}", expires_local);
                        }
                        if let Some(detail) = status.detail {
                            println!("  detail: {}", detail);
                        }
                    }
                    Some(("login", Some(server_name))) => {
                        let start = Config::mcp_oauth_login_start(config, server_name).await?;
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
                        Config::mcp_oauth_login_complete(config, server_name, &start).await?;
                        println!("✓ OAuth login complete for '{}'", server_name);

                        let should_auto_connect = {
                            config
                                .read()
                                .mcp_servers
                                .iter()
                                .find(|v| v.name == server_name)
                                .map(|v| v.enabled)
                                .unwrap_or(false)
                        };
                        if should_auto_connect {
                            match Config::mcp_connect_server(config, server_name).await {
                                Ok(_) => {
                                    Config::refresh_functions(config).await?;
                                    println!("✓ Connected to MCP server '{}'", server_name);
                                }
                                Err(err) => {
                                    println!(
                                        "OAuth login succeeded, but connect failed for '{}': {}",
                                        server_name, err
                                    );
                                }
                            }
                        }
                    }
                    Some(("logout", Some(server_name))) => {
                        let deleted = Config::mcp_oauth_logout(config, server_name).await?;
                        Config::refresh_functions(config).await?;
                        if deleted {
                            println!("✓ OAuth token removed for '{}'", server_name);
                        } else {
                            println!("No stored OAuth token found for '{}'", server_name);
                        }
                    }
                    _ => {
                        println!(
                            r#"Usage: /mcp auth <command> <server>

Commands:
  /mcp auth status <server>  - Show OAuth token status for a server
  /mcp auth login <server>   - Start device-code OAuth login flow
  /mcp auth logout <server>  - Delete stored OAuth token for a server"#
                        );
                    }
                },
                _ => {
                    println!(
                        r#"Usage: /mcp <command>

Commands:
  /mcp list                  - List configured MCP servers
  /mcp connect <server>      - Connect to an MCP server
  /mcp disconnect <server>   - Disconnect from an MCP server
  /mcp tools [server]        - List available tools (all or per server)
  /mcp auth <...>            - Manage OAuth login/status/logout for MCP servers"#
                    );
                }
            },
            ".resolver" => match split_first_arg(args) {
                Some(("list", _)) => {
                    let resolver = config.read().resolver.clone();
                    match resolver {
                        None => println!("Resolver not initialized"),
                        Some(r) if r.is_empty() => println!(
                            "Resolver store is empty. Use `/resolver learn` to add entries.\nStore path: {}",
                            r.path().display()
                        ),
                        Some(r) => {
                            println!("Resolver store: {}", r.path().display());
                            if !r.store.providers.is_empty() {
                                println!("\nProviders:");
                                let mut providers: Vec<_> =
                                    r.store.providers.iter().collect();
                                providers.sort_by_key(|(k, _)| k.as_str());
                                for (key, entry) in providers {
                                    println!(
                                        "  {} (aliases: {})",
                                        key,
                                        entry.alias.aliases.join(", ")
                                    );
                                    let mut workspaces: Vec<_> =
                                        entry.workspaces.iter().collect();
                                    workspaces.sort_by_key(|(k, _)| k.as_str());
                                    for (ws_key, ws_entry) in workspaces {
                                        println!(
                                            "    workspace: {} \"{}\" (aliases: {})",
                                            ws_key,
                                            ws_entry.name,
                                            ws_entry.alias.aliases.join(", ")
                                        );
                                    }
                                }
                            }
                            if !r.store.actions.is_empty() {
                                println!("\nActions:");
                                let mut actions: Vec<_> = r.store.actions.iter().collect();
                                actions.sort_by_key(|(k, _)| k.as_str());
                                for (key, entry) in actions {
                                    println!(
                                        "  {} (aliases: {})",
                                        key,
                                        entry.aliases.join(", ")
                                    );
                                }
                            }
                        }
                    }
                }
                Some(("learn", Some(rest))) => match split_first_arg(Some(rest)) {
                    Some(("provider", Some(rest))) => {
                        let mut parts = rest.splitn(2, ' ');
                        let name = parts.next().unwrap_or("").trim();
                        let alias = parts.next().map(str::trim).filter(|s| !s.is_empty());
                        if name.is_empty() {
                            bail!("Usage: /resolver learn provider <name> [alias]");
                        }
                        update_resolver(config, |r| r.add_provider(name, alias))?;
                        println!("✓ Provider '{name}' added/updated");
                    }
                    Some(("workspace", Some(rest))) => {
                        let mut parts = rest.splitn(3, ' ');
                        let provider = parts.next().unwrap_or("").trim();
                        let name = parts.next().unwrap_or("").trim();
                        let alias = parts.next().map(str::trim).filter(|s| !s.is_empty());
                        if provider.is_empty() || name.is_empty() {
                            bail!("Usage: /resolver learn workspace <provider> <name> [alias]");
                        }
                        update_resolver(config, |r| r.add_workspace(provider, name, alias))?;
                        println!("✓ Workspace '{provider}/{name}' added/updated");
                    }
                    Some(("action", Some(rest))) => {
                        let mut parts = rest.splitn(2, ' ');
                        let name = parts.next().unwrap_or("").trim();
                        let alias = parts.next().map(str::trim).unwrap_or("").trim();
                        if name.is_empty() || alias.is_empty() {
                            bail!("Usage: /resolver learn action <name> <alias>");
                        }
                        update_resolver(config, |r| r.add_action(name, alias))?;
                        println!("✓ Action '{name}' alias '{alias}' added");
                    }
                    _ => println!(
                        "Usage: /resolver learn <type> <args>

Types:
  provider <name> [alias]               - Add or update a provider
  workspace <provider> <name> [alias]   - Add or update a workspace
  action <name> <alias>                 - Add an alias to an action"
                    ),
                },
                Some(("forget", Some(rest))) => match split_first_arg(Some(rest)) {
                    Some(("provider", Some(name))) => {
                        let name = name.trim();
                        update_resolver(config, |r| {
                            if r.remove_provider(name) {
                                println!("✓ Provider '{name}' removed");
                            } else {
                                println!("Provider '{name}' not found");
                            }
                            Ok(())
                        })?;
                    }
                    Some(("workspace", Some(rest))) => {
                        let mut parts = rest.splitn(2, ' ');
                        let provider = parts.next().unwrap_or("").trim();
                        let name = parts.next().unwrap_or("").trim();
                        if provider.is_empty() || name.is_empty() {
                            bail!("Usage: /resolver forget workspace <provider> <name>");
                        }
                        update_resolver(config, |r| {
                            if r.remove_workspace(provider, name)? {
                                println!("✓ Workspace '{provider}/{name}' removed");
                            } else {
                                println!("Workspace '{provider}/{name}' not found");
                            }
                            Ok(())
                        })?;
                    }
                    Some(("action", Some(name))) => {
                        let name = name.trim();
                        update_resolver(config, |r| {
                            if r.remove_action(name) {
                                println!("✓ Action '{name}' removed");
                            } else {
                                println!("Action '{name}' not found");
                            }
                            Ok(())
                        })?;
                    }
                    _ => println!(
                        "Usage: /resolver forget <type> <args>

Types:
  provider <name>               - Remove a provider and all its workspaces
  workspace <provider> <name>   - Remove a workspace
  action <name>                 - Remove an action"
                    ),
                },
                _ => println!(
                    "Usage: /resolver <command>

Commands:
  list                                - List all resolver entries
  learn provider <name> [alias]       - Add or update a provider alias
  learn workspace <p> <name> [alias]  - Add or update a workspace alias
  learn action <name> <alias>         - Add an action alias
  forget provider <name>              - Remove a provider
  forget workspace <provider> <name>  - Remove a workspace
  forget action <name>                - Remove an action"
                ),
            },
            ".exit" => match args {
                Some("role") => {
                    config.write().exit_role()?;
                }
                Some("session") => {
                    if config.read().agent.is_some() {
                        config.write().exit_agent_session()?;
                    } else {
                        config.write().exit_session()?;
                    }
                }
                Some("rag") => {
                    config.write().exit_rag()?;
                }
                Some("agent") => {
                    config.write().exit_agent()?;
                }
                Some(_) => unknown_command()?,
                None => {
                    return Ok(true);
                }
            },
            ".clear" => match args {
                Some("messages") => {
                    bail!("Use '/empty session' instead");
                }
                _ => unknown_command()?,
            },
            _ => unknown_command()?,
        },
        None => {
            let route = route_turn(config, abort_signal.clone(), line).await?;

            // Temporarily switch model for this turn
            let prev_model = config.read().current_model().id();
            if let Some(ref id) = route.model_id {
                config.write().set_model(id)?;
            }

            let input = Input::from_str(config, &route.text, None);
            ask(config, abort_signal.clone(), input, true).await?;

            // Restore model
            if route.model_id.is_some() {
                let _ = config.write().set_model(&prev_model);
            }

            // Learn from successful resolution (not called if ask() errored).
            if let Some(intent) = route.intent {
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
        }
    }

    if !config.read().macro_flag {
        println!();
    }

    Ok(false)
}

#[async_recursion::async_recursion]
async fn ask(
    config: &GlobalConfig,
    abort_signal: AbortSignal,
    mut input: Input,
    with_embeddings: bool,
) -> Result<()> {
    if input.is_empty() {
        return Ok(());
    }
    if with_embeddings {
        input.use_embeddings(abort_signal.clone()).await?;
    }
    while config.read().is_compressing_session() {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    let client = input.create_client()?;
    config.write().before_chat_completion(&input)?;
    let (output, tool_results) = if input.stream() {
        call_chat_completions_streaming(&input, client.as_ref(), abort_signal.clone()).await?
    } else {
        call_chat_completions(&input, true, false, client.as_ref(), abort_signal.clone()).await?
    };
    config
        .write()
        .after_chat_completion(&input, &output, &tool_results)?;
    if !tool_results.is_empty() {
        ask(
            config,
            abort_signal,
            input.merge_tool_results(output, tool_results),
            false,
        )
        .await
    } else {
        Config::maybe_autoname_session(config.clone());
        Config::maybe_compress_session(config.clone());
        Ok(())
    }
}

fn unknown_command() -> Result<()> {
    bail!(r#"Unknown command. Type "/help" for additional help."#);
}

fn print_model_overview(config: &GlobalConfig) {
    let config = config.read();
    let base_model = config.current_model().id();
    let fast = config
        .model_fast
        .as_deref()
        .unwrap_or("(not set)");
    let thinking = config
        .model_thinking
        .as_deref()
        .unwrap_or("(not set)");

    println!("Fast model (chat):     {fast}");
    println!("Thinking model (ops):  {thinking}");
    println!("Base model (fallback): {base_model}");

    let models = list_models(&config, ModelType::Chat);
    if models.is_empty() {
        println!("\nAvailable models: (none)");
        return;
    }

    println!("\nAvailable models:");
    for (i, model) in models.iter().enumerate() {
        let model_id = model.id();
        let mut labels = Vec::new();
        if Some(model_id.as_str()) == config.model_fast.as_deref() {
            labels.push("fast");
        }
        if Some(model_id.as_str()) == config.model_thinking.as_deref() {
            labels.push("thinking");
        }
        if model_id == base_model
            && config.model_fast.as_deref() != Some(model_id.as_str())
            && config.model_thinking.as_deref() != Some(model_id.as_str())
        {
            labels.push("base");
        }
        let label = if labels.is_empty() {
            String::new()
        } else {
            format!(" ({})", labels.join(", "))
        };
        let data = model.data();
        let has_descriptive_metadata = data.max_input_tokens.is_some()
            || data.max_output_tokens.is_some()
            || data.input_price.is_some()
            || data.output_price.is_some()
            || data.supports_vision
            || data.supports_function_calling;
        if has_descriptive_metadata {
            let description = model.description();
            println!(
                "  {:>2}. {}{} - {}",
                i + 1,
                model_id,
                label,
                description
            );
        } else {
            println!("  {:>2}. {}{}", i + 1, model_id, label);
        }
    }
    println!("\nUse /model <number|name> to set fast model.");
    println!("Use /model thinking <number|name> to set thinking model.");
}

fn model_id_from_index(config: &GlobalConfig, index: usize) -> Result<String> {
    let config = config.read();
    let models = list_models(&config, ModelType::Chat);
    if index == 0 || index > models.len() {
        bail!(
            "Invalid model index '{}'. Run '/model list' to view available models.",
            index
        );
    }
    Ok(models[index - 1].id())
}

fn dump_repl_help() {
    let head = INTERACTIVE_COMMANDS
        .iter()
        .map(|cmd| format!("{:<24} {}", display_command_name(cmd.name), cmd.description))
        .collect::<Vec<String>>()
        .join("\n");
    println!(
        r###"{head}

Type ::: to start multi-line editing, type ::: to finish it.
Press Ctrl+O to open an editor for editing the input buffer.
Press Ctrl+C to cancel the response, Ctrl+D to exit the REPL.
On Unix, press Ctrl+Z to suspend (run "fg" to resume).
Slash commands are shown by default; dot-prefixed aliases are also supported."###,
    );
}

fn suspend_current_process() -> Result<()> {
    #[cfg(unix)]
    {
        let pid = process::id().to_string();
        let status = process::Command::new("kill")
            .args(["-TSTP", &pid])
            .status()
            .context("Failed to execute kill command for process suspension")?;
        if !status.success() {
            bail!("Failed to suspend process");
        }
    }
    #[cfg(not(unix))]
    {
        bail!("Ctrl+Z suspend is only supported on Unix-like systems")
    }
    Ok(())
}

fn display_command_name(name: &str) -> String {
    name.strip_prefix('.')
        .map(|value| format!("/{value}"))
        .unwrap_or_else(|| name.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlainCommand {
    Exit,
    Help,
}

fn parse_plain_command(line: &str) -> Option<PlainCommand> {
    let line = line.trim().to_ascii_lowercase();
    match line.as_str() {
        "exit" | "quit" | ":q" => Some(PlainCommand::Exit),
        "help" => Some(PlainCommand::Help),
        _ => None,
    }
}

/// Clone resolver from config, apply `f`, save to disk, write back.
fn update_resolver<F>(config: &GlobalConfig, f: F) -> Result<()>
where
    F: FnOnce(&mut Resolver) -> Result<()>,
{
    let mut resolver = config
        .read()
        .resolver
        .clone()
        .ok_or_else(|| anyhow::anyhow!("Resolver not initialized"))?;
    f(&mut resolver)?;
    resolver.save()?;
    config.write().resolver = Some(resolver);
    Ok(())
}


fn parse_command(line: &str) -> Option<(&str, Option<&str>)> {
    match COMMAND_RE.captures(line) {
        Ok(Some(captures)) => {
            let cmd = captures.get(1)?.as_str();
            let args = line[captures[0].len()..].trim();
            let args = if args.is_empty() { None } else { Some(args) };
            Some((cmd, args))
        }
        _ => None,
    }
}

fn normalize_command_prefix(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('/') {
        return None;
    }
    let leading_ws_len = line.len().saturating_sub(trimmed.len());
    let mut out = String::with_capacity(line.len());
    out.push_str(&line[..leading_ws_len]);
    out.push('.');
    out.push_str(&trimmed[1..]);
    Some(out)
}

fn parse_thinking_toggle(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "off" | "hide" | "hidden" | "true" => Some(true),
        "on" | "show" | "visible" | "false" => Some(false),
        _ => None,
    }
}

fn split_first_arg(args: Option<&str>) -> Option<(&str, Option<&str>)> {
    args.map(|v| match v.split_once(' ') {
        Some((subcmd, args)) => (subcmd, Some(args.trim())),
        None => (v, None),
    })
}

pub fn split_args_text(line: &str, is_win: bool) -> (Vec<String>, &str) {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut unbalance: Option<char> = None;
    let mut prev_char: Option<char> = None;
    let mut text_starts_at = None;
    let unquote_word = |word: &str| {
        if ((word.starts_with('"') && word.ends_with('"'))
            || (word.starts_with('\'') && word.ends_with('\'')))
            && word.len() >= 2
        {
            word[1..word.len() - 1].to_string()
        } else {
            word.to_string()
        }
    };
    let chars: Vec<char> = line.chars().collect();

    for (i, char) in chars.iter().cloned().enumerate() {
        match unbalance {
            Some(ub_char) if ub_char == char => {
                word.push(char);
                unbalance = None;
            }
            Some(_) => {
                word.push(char);
            }
            None => match char {
                ' ' | '\t' | '\r' | '\n' => {
                    if char == '\r' && chars.get(i + 1) == Some(&'\n') {
                        continue;
                    }
                    if let Some('\\') = prev_char.filter(|_| !is_win) {
                        word.push(char);
                    } else if !word.is_empty() {
                        if word == "--" {
                            word.clear();
                            text_starts_at = Some(i + 1);
                            break;
                        }
                        words.push(unquote_word(&word));
                        word.clear();
                    }
                }
                '\'' | '"' | '`' => {
                    word.push(char);
                    unbalance = Some(char);
                }
                '\\' => {
                    if is_win || prev_char.map(|c| c == '\\').unwrap_or_default() {
                        word.push(char);
                    }
                }
                _ => {
                    word.push(char);
                }
            },
        }
        prev_char = Some(char);
    }

    if !word.is_empty() && word != "--" {
        words.push(unquote_word(&word));
    }
    let text = match text_starts_at {
        Some(start) => &line[start..],
        None => "",
    };

    (words, text)
}

fn fio_greeting() -> &'static str {
    const GREETINGS: &[&str] = &[
        "Hey, Fio here! What are we building today?",
        "Fio, reporting for duty. What's the plan?",
        "Hi! Fio here — let's make something cool.",
        "Hey! What's on the workbench today?",
        "Fio here. Got something interesting for me?",
        "Hi there! Ready when you are.",
        "Hey! Let's figure this out together.",
        "Fio here — sleeves rolled, let's go.",
        "Hi! What kind of trouble are we getting into today?",
        "Hey, Fio here. Show me what we're working with.",
        "Ready to go! What do you need?",
        "Hi! I've got a good feeling about today.",
        "Fio here. What are we fixing, building, or breaking?",
        "Hey! Grab a wrench, let's get started.",
        "Hi! Got a puzzle for me?",
        "Fio, checking in. What's the mission?",
        "Hey there! What are we tinkering with?",
        "Hi! Let's see what we can do.",
        "Fio here — what's the adventure today?",
        "Hey! Something tells me this is going to be fun.",
        "Hi! I just got here but I'm already curious.",
        "Fio here. Let's build something we're proud of.",
        "Hey! What's cooking?",
        "Hi there! Point me at the problem.",
        "Fio here — got my toolkit, what do you need?",
        "Hey! Let's find that seam and crack it open.",
        "Hi! Another day, another interesting challenge.",
        "Fio, ready to roll. What have we got?",
        "Hey! I was hoping you'd have something good for me.",
        "Hi! Let's get our hands dirty.",
        "Fio here. No problem too weird, no bug too sneaky.",
        "Hey there! What are we making happen?",
        "Hi! I brought coffee and curiosity. Let's go.",
        "Fio here — every problem has a seam. Let's find it.",
        "Hey! What's the story today?",
        "Hi! Ready to dive in whenever you are.",
        "Fio, at your service. What needs doing?",
        "Hey! I like the look of this one already.",
        "Hi there! Let's see what we're dealing with.",
        "Fio here. Tell me everything.",
    ];
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as usize;
    GREETINGS[nanos % GREETINGS.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_command_line() {
        assert_eq!(parse_command(" ."), Some((".", None)));
        assert_eq!(parse_command(" .role"), Some((".role", None)));
        assert_eq!(parse_command(" .role  "), Some((".role", None)));
        assert_eq!(
            parse_command(" .set dry_run true"),
            Some((".set", Some("dry_run true")))
        );
        assert_eq!(
            parse_command(" .set dry_run true  "),
            Some((".set", Some("dry_run true")))
        );
        assert_eq!(
            parse_command(".prompt \nabc\n"),
            Some((".prompt", Some("abc")))
        );
    }

    #[test]
    fn test_normalize_command_prefix() {
        assert_eq!(normalize_command_prefix("/help"), Some(".help".to_string()));
        assert_eq!(
            normalize_command_prefix("  /set stream false"),
            Some("  .set stream false".to_string())
        );
        assert_eq!(normalize_command_prefix(".help"), None);
        assert_eq!(normalize_command_prefix("hello /help"), None);
    }

    #[test]
    fn test_parse_plain_command() {
        assert_eq!(parse_plain_command("exit"), Some(PlainCommand::Exit));
        assert_eq!(parse_plain_command(" quit "), Some(PlainCommand::Exit));
        assert_eq!(parse_plain_command(":q"), Some(PlainCommand::Exit));
        assert_eq!(parse_plain_command("help"), Some(PlainCommand::Help));
        assert_eq!(parse_plain_command(".help"), None);
        assert_eq!(parse_plain_command("exiting"), None);
    }

    #[test]
    fn test_parse_thinking_toggle() {
        assert_eq!(parse_thinking_toggle("off"), Some(true));
        assert_eq!(parse_thinking_toggle("hide"), Some(true));
        assert_eq!(parse_thinking_toggle("true"), Some(true));
        assert_eq!(parse_thinking_toggle("on"), Some(false));
        assert_eq!(parse_thinking_toggle("show"), Some(false));
        assert_eq!(parse_thinking_toggle("false"), Some(false));
        assert_eq!(parse_thinking_toggle("nope"), None);
    }

    #[test]
    fn test_model_list_alias() {
        assert!(normalize_command_prefix("/model list").is_some());
        assert!(normalize_command_prefix("/models list").is_some());
        assert_eq!(parse_command(".model list"), Some((".model", Some("list"))));
        assert_eq!(
            parse_command(".models list"),
            Some((".models", Some("list")))
        );
    }

    #[test]
    fn test_mcp_auth_command_parsing() {
        assert_eq!(
            parse_command(".mcp auth status linear"),
            Some((".mcp", Some("auth status linear")))
        );
        assert_eq!(
            parse_command(".mcp auth login linear"),
            Some((".mcp", Some("auth login linear")))
        );
        assert_eq!(
            parse_command(".mcp auth logout linear"),
            Some((".mcp", Some("auth logout linear")))
        );
    }

    #[test]
    fn test_split_args_text() {
        assert_eq!(split_args_text("", false), (vec![], ""));
        assert_eq!(
            split_args_text("file.txt", false),
            (vec!["file.txt".into()], "")
        );
        assert_eq!(
            split_args_text("file.txt --", false),
            (vec!["file.txt".into()], "")
        );
        assert_eq!(
            split_args_text("file.txt -- hello", false),
            (vec!["file.txt".into()], "hello")
        );
        assert_eq!(
            split_args_text("file.txt -- \thello", false),
            (vec!["file.txt".into()], "\thello")
        );
        assert_eq!(
            split_args_text("file.txt --\nhello", false),
            (vec!["file.txt".into()], "hello")
        );
        assert_eq!(
            split_args_text("file.txt --\r\nhello", false),
            (vec!["file.txt".into()], "hello")
        );
        assert_eq!(
            split_args_text("file.txt --\rhello", false),
            (vec!["file.txt".into()], "hello")
        );
        assert_eq!(
            split_args_text(r#"file1.txt 'file2.txt' "file3.txt""#, false),
            (
                vec!["file1.txt".into(), "file2.txt".into(), "file3.txt".into()],
                ""
            )
        );
        assert_eq!(
            split_args_text(r#"./file1.txt 'file1 - Copy.txt' file\ 2.txt"#, false),
            (
                vec![
                    "./file1.txt".into(),
                    "file1 - Copy.txt".into(),
                    "file 2.txt".into()
                ],
                ""
            )
        );
        assert_eq!(
            split_args_text(r#".\file.txt C:\dir\file.txt"#, true),
            (vec![".\\file.txt".into(), "C:\\dir\\file.txt".into()], "")
        );
    }
}
