use crate::client::call_chat_completions;
use crate::config::{GlobalConfig, Input, Role};
use crate::resolver::{extract_json_object, ResolvedIntent, ResolutionOutcome, Resolver};
use crate::utils::{dimmed_text, AbortSignal};

use anyhow::Result;

/// Execution policy for a single turn.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum TurnPolicy {
    Auto,
    Chat,
    Plan,
    Execute,
}

/// The result of routing a single user turn.
pub struct TurnRoute {
    /// Enriched text (with preamble if resolved).
    pub text: String,
    /// Model override (None = use current).
    pub model_id: Option<String>,
    /// How to fulfill this turn.
    pub policy: TurnPolicy,
    /// For post-turn learning.
    pub intent: Option<ResolvedIntent>,
}

/// Central per-turn routing. Both one-shot and interactive surfaces call this.
///
/// Logic:
/// 1. Run resolver (if available and non-empty)
///    - Resolved → thinking model, Chat policy, enriched text
///    - NeedsAi → AI fallback → if resolved: same; else fall through
///    - PassThrough → fall through
/// 2. Fall back to `looks_operational_prompt()` heuristic
///    - Operational → thinking model, Plan policy, original text
///    - Not operational → fast model, Chat policy, original text
/// 3. Return TurnRoute
pub async fn route_turn(
    config: &GlobalConfig,
    abort_signal: AbortSignal,
    text: &str,
) -> Result<TurnRoute> {
    // Step 1: resolver
    let resolver = config.read().resolver.clone();
    if let Some(ref resolver) = resolver {
        if !resolver.is_empty() {
            match resolver.resolve(text) {
                ResolutionOutcome::Resolved(intent) => {
                    let preamble = intent.to_preamble();
                    println!("{}", dimmed_text(&preamble));
                    let model_id = config.read().model_thinking.clone();
                    return Ok(TurnRoute {
                        text: format!("{preamble}\n{text}"),
                        model_id,
                        policy: TurnPolicy::Chat,
                        intent: Some(intent),
                    });
                }
                ResolutionOutcome::NeedsAi => {
                    match resolver_ai_fallback(resolver, config, abort_signal, text).await {
                        Ok(Some(intent)) => {
                            let preamble = intent.to_preamble();
                            println!("{}", dimmed_text(&preamble));
                            let model_id = config.read().model_thinking.clone();
                            return Ok(TurnRoute {
                                text: format!("{preamble}\n{text}"),
                                model_id,
                                policy: TurnPolicy::Chat,
                                intent: Some(intent),
                            });
                        }
                        Ok(None) => {
                            println!(
                                "Ambiguous intent — not sure which provider/workspace/action you mean.\n\
                                 Use `/resolver learn` to teach me, or be more explicit."
                            );
                            // Fall through to heuristic
                        }
                        Err(e) => {
                            warn!("Resolver AI fallback failed: {e}");
                            // Fall through to heuristic
                        }
                    }
                }
                ResolutionOutcome::PassThrough => {
                    // Fall through to heuristic
                }
            }
        }
    }

    // Step 2: heuristic fallback
    if looks_operational_prompt(text) {
        let model_id = config.read().model_thinking.clone();
        Ok(TurnRoute {
            text: text.to_string(),
            model_id,
            policy: TurnPolicy::Plan,
            intent: None,
        })
    } else {
        let model_id = config.read().model_fast.clone();
        Ok(TurnRoute {
            text: text.to_string(),
            model_id,
            policy: TurnPolicy::Chat,
            intent: None,
        })
    }
}

/// Select a route model based on policy and config slots.
pub fn select_route_model(
    policy: TurnPolicy,
    model_fast: Option<&str>,
    model_thinking: Option<&str>,
) -> Option<String> {
    match policy {
        TurnPolicy::Chat => model_fast.map(String::from),
        TurnPolicy::Plan | TurnPolicy::Execute => model_thinking.map(String::from),
        TurnPolicy::Auto => None,
    }
}

pub fn looks_operational_prompt(text: &str) -> bool {
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

/// Call the configured LLM to resolve ambiguous intent into structured JSON.
/// Returns `None` when the AI confidence is below threshold.
async fn resolver_ai_fallback(
    resolver: &Resolver,
    config: &GlobalConfig,
    abort_signal: AbortSignal,
    text: &str,
) -> Result<Option<ResolvedIntent>> {
    let mut options = String::new();
    for (prov_key, prov_entry) in &resolver.store.providers {
        let aliases = prov_entry.alias.aliases.join(", ");
        options.push_str(&format!("- provider: {prov_key} (aliases: {aliases})\n"));
        for (ws_key, ws_entry) in &prov_entry.workspaces {
            let ws_aliases = ws_entry.alias.aliases.join(", ");
            options.push_str(&format!(
                "  - workspace: {ws_key} / {} (aliases: {ws_aliases})\n",
                ws_entry.name
            ));
        }
    }
    for (action_key, action_entry) in &resolver.store.actions {
        let aliases = action_entry.aliases.join(", ");
        options.push_str(&format!("- action: {action_key} (aliases: {aliases})\n"));
    }

    let prompt = format!(
        r#"You are an intent resolver. Extract provider/workspace/action from the user request.

Available entries:
{options}
User request: "{text}"

Respond with ONLY valid JSON on a single line. No markdown, no explanation.
Fields: provider (string|null), workspace (string|null), action (string|null), confidence (0.0-1.0), reason (string).
Example: {{"provider":"linear","workspace":"SAM","action":"create_tickets","confidence":0.95,"reason":"matched all fields"}}"#
    );

    // Bare role: uses global model, no session, no tools.
    let model = config.read().model.clone();
    let mut ai_role = Role::new("__resolver__", "");
    ai_role.batch_set(&model, None, None, Some("none".to_string()));
    let input = Input::from_str(config, &prompt, Some(ai_role));
    let client = input.create_client()?;
    let (output, _) =
        call_chat_completions(&input, false, false, client.as_ref(), abort_signal).await?;

    let json_str = match extract_json_object(&output) {
        Some(s) => s,
        None => return Ok(None),
    };

    #[derive(serde::Deserialize)]
    struct AiOut {
        provider: Option<String>,
        workspace: Option<String>,
        action: Option<String>,
        confidence: f32,
        reason: String,
    }
    let out: AiOut = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };

    const AI_THRESHOLD: f32 = 0.70;
    if out.confidence < AI_THRESHOLD {
        return Ok(None);
    }
    let Some(provider) = out.provider else {
        return Ok(None);
    };
    let intent = ResolvedIntent {
        provider,
        workspace: out.workspace,
        action: out.action,
        confidence: out.confidence,
        reason: format!("AI: {}", out.reason),
    };

    let Some(intent) = resolver.validate_ai_intent(intent) else {
        return Ok(None);
    };
    if intent.confidence < AI_THRESHOLD {
        return Ok(None);
    }
    Ok(Some(intent))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operational_prompt_is_detected() {
        assert!(looks_operational_prompt("git commit and push the changes"));
        assert!(looks_operational_prompt(
            "can you restart nginx and tail logs"
        ));
    }

    #[test]
    fn explanatory_prompt_is_not_operational() {
        assert!(!looks_operational_prompt(
            "how do I commit and push safely?"
        ));
        assert!(!looks_operational_prompt("explain git rebase"));
    }

    #[test]
    fn route_model_selects_fast_for_chat() {
        assert_eq!(
            select_route_model(TurnPolicy::Chat, Some("openai:gpt-4o-mini"), Some("openai:o1")),
            Some("openai:gpt-4o-mini".into())
        );
    }

    #[test]
    fn route_model_selects_thinking_for_plan_and_execute() {
        assert_eq!(
            select_route_model(TurnPolicy::Plan, Some("openai:gpt-4o-mini"), Some("openai:o1")),
            Some("openai:o1".into())
        );
        assert_eq!(
            select_route_model(TurnPolicy::Execute, Some("openai:gpt-4o-mini"), Some("openai:o1")),
            Some("openai:o1".into())
        );
    }

    #[test]
    fn route_model_falls_back_when_unset() {
        assert_eq!(select_route_model(TurnPolicy::Chat, None, None), None);
        assert_eq!(select_route_model(TurnPolicy::Plan, None, None), None);
        assert_eq!(select_route_model(TurnPolicy::Auto, Some("openai:gpt-4o-mini"), Some("openai:o1")), None);
    }
}
