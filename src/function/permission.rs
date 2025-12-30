use super::ToolCall;
use crate::config::{GlobalConfig, ToolPermissions};
use crate::utils::{color_text, dimmed_text, IS_STDOUT_TERMINAL};

use anyhow::Result;
use fancy_regex::Regex;
use inquire::Select;
use nu_ansi_term::Color;
use std::collections::HashSet;
use std::sync::LazyLock;

static WILDCARD_PATTERN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\*").unwrap());

#[derive(Debug, Clone, PartialEq)]
enum PermissionLevel {
    Always,
    Never,
    Ask,
}

impl PermissionLevel {
    fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "always" => PermissionLevel::Always,
            "never" => PermissionLevel::Never,
            "ask" => PermissionLevel::Ask,
            // Safer default for invalid values.
            _ => PermissionLevel::Ask,
        }
    }
}

#[derive(Debug)]
pub struct ToolPermission {
    config: GlobalConfig,
    session_allowed: HashSet<String>,
    role_tool_call_permission: Option<String>,
    role_tool_permissions: Option<ToolPermissions>,
}

impl ToolPermission {
    pub fn new_with_role(
        config: &GlobalConfig,
        role_tool_call_permission: Option<String>,
        role_tool_permissions: Option<ToolPermissions>,
    ) -> Self {
        let session_allowed = config
            .read()
            .session
            .as_ref()
            .map(|s| s.get_session_tool_permissions().clone())
            .unwrap_or_default();

        Self {
            config: config.clone(),
            session_allowed,
            role_tool_call_permission,
            role_tool_permissions,
        }
    }

    pub async fn check_permission(&mut self, tool_call: &ToolCall) -> Result<bool> {
        let tool_name = &tool_call.name;

        if self.session_allowed.contains(tool_name) {
            if self.config.read().verbose_tool_calls {
                self.print_tool_call_info(tool_call, "auto-allowed (session)");
            }
            return Ok(true);
        }

        // Snapshot config values (don't hold locks during prompts).
        let (verbose, global_perm, global_tool_perms, mcp_servers) = {
            let cfg = self.config.read();
            (
                cfg.verbose_tool_calls,
                cfg.tool_call_permission.clone(),
                cfg.tool_permissions.clone(),
                cfg.mcp_servers.clone(),
            )
        };

        // Trusted MCP server bypass.
        if tool_name.starts_with("mcp__") {
            if let Some(server_name) = crate::mcp::extract_server_name(tool_name) {
                if let Some(server_cfg) = mcp_servers.iter().find(|s| s.name == server_name) {
                    if server_cfg.trusted {
                        if verbose {
                            self.print_tool_call_info(tool_call, "auto-allowed (trusted server)");
                        }
                        return Ok(true);
                    }
                }
            }
        }

        let default_permission = if let Some(perm) = &self.role_tool_call_permission {
            PermissionLevel::from_str(perm)
        } else if let Some(perm) = &global_perm {
            PermissionLevel::from_str(perm)
        } else {
            // Backward compatible default.
            PermissionLevel::Always
        };

        let tool_perms = self.role_tool_permissions.as_ref().or(global_tool_perms.as_ref());
        if let Some(tool_perms) = tool_perms {
            if let Some(denied) = &tool_perms.denied {
                if self.matches_any_pattern(tool_name, denied) {
                    if verbose {
                        self.print_tool_call_info(tool_call, "denied");
                    }
                    return Ok(false);
                }
            }
            if let Some(allowed) = &tool_perms.allowed {
                if self.matches_any_pattern(tool_name, allowed) {
                    if verbose {
                        self.print_tool_call_info(tool_call, "auto-allowed (allowed list)");
                    }
                    return Ok(true);
                }
            }
            if let Some(ask) = &tool_perms.ask {
                if self.matches_any_pattern(tool_name, ask) {
                    return self.prompt_user(tool_call).await;
                }
            }
        }

        match default_permission {
            PermissionLevel::Always => {
                if verbose {
                    self.print_tool_call_info(tool_call, "auto-allowed (global)");
                }
                Ok(true)
            }
            PermissionLevel::Never => {
                if verbose {
                    self.print_tool_call_info(tool_call, "denied (global)");
                }
                Ok(false)
            }
            PermissionLevel::Ask => self.prompt_user(tool_call).await,
        }
    }

    async fn prompt_user(&mut self, tool_call: &ToolCall) -> Result<bool> {
        if !*IS_STDOUT_TERMINAL {
            // No interactive prompt available; fail closed.
            return Ok(false);
        }

        let tool_name = tool_call.name.clone();
        let args_display = if tool_call.arguments.is_object() {
            serde_json::to_string_pretty(&tool_call.arguments).unwrap_or_else(|_| "{}".to_string())
        } else {
            tool_call.arguments.to_string()
        };
        let args_display = if args_display.len() > 400 {
            format!("{}... (truncated)", &args_display[..400])
        } else {
            args_display
        };

        println!();
        println!(
            "Can I run {} with the following arguments?\n{}",
            color_text(&tool_name, Color::Cyan),
            dimmed_text(&args_display)
        );

        let choice = tokio::task::spawn_blocking(move || {
            let options = vec!["Yes (this time only)", "Yes (for this session)", "No"];
            Select::new("Allow this tool call?", options)
                .with_help_message("Choose how to respond to this tool call")
                .prompt()
                .map(|v| v.to_string())
                .ok()
        })
        .await
        .unwrap_or(None);

        match choice.as_deref() {
            Some("Yes (this time only)") => Ok(true),
            Some("Yes (for this session)") => {
                self.session_allowed.insert(tool_name.clone());
                if let Some(session) = self.config.write().session.as_mut() {
                    session.add_session_tool_permission(tool_name);
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn matches_any_pattern(&self, tool_name: &str, patterns: &[String]) -> bool {
        patterns
            .iter()
            .any(|pattern| self.matches_pattern(tool_name, pattern))
    }

    fn matches_pattern(&self, tool_name: &str, pattern: &str) -> bool {
        if pattern == tool_name {
            return true;
        }
        if pattern.contains('*') {
            let regex_pattern = WILDCARD_PATTERN.replace_all(pattern, ".*");
            let regex_pattern = format!("^{}$", regex_pattern);
            if let Ok(re) = Regex::new(&regex_pattern) {
                if let Ok(is_match) = re.is_match(tool_name) {
                    return is_match;
                }
            }
        }
        false
    }

    fn print_tool_call_info(&self, tool_call: &ToolCall, status: &str) {
        let prompt = format!("Call {} {} [{}]", tool_call.name, tool_call.arguments, status);
        println!("{}", dimmed_text(&prompt));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::mcp::McpServerConfig;
    use parking_lot::RwLock;
    use serde_json::json;
    use std::sync::Arc;

    fn create_config() -> GlobalConfig {
        Arc::new(RwLock::new(Config::default()))
    }

    #[test]
    fn test_matches_pattern() {
        let config = create_config();
        let perm = ToolPermission::new_with_role(&config, None, None);
        
        assert!(perm.matches_pattern("fs_read", "fs_read"));
        assert!(perm.matches_pattern("fs_read", "fs_*"));
        assert!(perm.matches_pattern("mcp__server__tool", "mcp__*"));
        assert!(!perm.matches_pattern("fs_read", "net_*"));
    }

    #[tokio::test]
    async fn test_check_permission_trusted_mcp() {
        let config = create_config();
        {
            let mut cfg = config.write();
            cfg.mcp_servers.push(McpServerConfig {
                name: "trusted_server".to_string(),
                command: "echo".to_string(),
                args: vec![],
                env: Default::default(),
                enabled: true,
                trusted: true,
                description: None,
            });
            cfg.mcp_servers.push(McpServerConfig {
                name: "untrusted_server".to_string(),
                command: "echo".to_string(),
                args: vec![],
                env: Default::default(),
                enabled: true,
                trusted: false,
                description: None,
            });
            cfg.tool_call_permission = Some("never".to_string()); // Default deny
        }

        let mut perm = ToolPermission::new_with_role(&config, None, None);

        let trusted_call = ToolCall {
            name: "mcp__trusted_server__tool".to_string(),
            arguments: json!({}),
            id: None,
        };
        
        let untrusted_call = ToolCall {
            name: "mcp__untrusted_server__tool".to_string(),
            arguments: json!({}),
            id: None,
        };

        assert!(perm.check_permission(&trusted_call).await.unwrap());
        assert!(!perm.check_permission(&untrusted_call).await.unwrap());
    }
}


