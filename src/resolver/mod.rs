mod store;
pub mod types;

pub use types::{AliasEntry, ProviderEntry, ResolvedIntent, ResolverStore, ResolutionOutcome,
    WorkspaceEntry};

use anyhow::{anyhow, bail, Result};
use std::path::{Path, PathBuf};
use types::word_boundary_match;

/// Confidence above which the deterministic pass reports a confident match.
const CONFIDENT: f32 = 0.80;
/// Confidence above which we fall back to the AI rather than passing through.
const NEEDS_AI: f32 = 0.45;

/// Contribution of each component to overall confidence (must sum to 1.0).
const W_PROVIDER: f32 = 0.40;
const W_WORKSPACE: f32 = 0.35;
const W_ACTION: f32 = 0.25;

/// The resolver: loads from `resolver.json`, does deterministic matching,
/// and supports learning from confirmed resolutions.
#[derive(Debug, Clone)]
pub struct Resolver {
    pub store: ResolverStore,
    path: PathBuf,
}

impl Resolver {
    pub fn load(config_dir: &Path) -> Result<Self> {
        let path = store::resolver_path(config_dir);
        let store = store::load(&path)?;
        Ok(Self { store, path })
    }

    pub fn save(&self) -> Result<()> {
        store::save(&self.path, &self.store)
    }

    pub fn is_empty(&self) -> bool {
        self.store.providers.is_empty()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    // -------------------------------------------------------------------------
    // Resolution
    // -------------------------------------------------------------------------

    /// Deterministic resolution pass. Returns the outcome without any AI call.
    pub fn resolve(&self, text: &str) -> ResolutionOutcome {
        if self.is_empty() {
            return ResolutionOutcome::PassThrough;
        }
        let lower = text.to_lowercase();

        // --- Provider (deterministic: prefer the provider whose longest matching alias wins) ---
        let provider_match = self
            .store
            .providers
            .iter()
            .filter_map(|(key, entry)| {
                // Check canonical key first, then aliases — all with word boundaries.
                let best_len = std::iter::once(key.as_str())
                    .chain(entry.alias.aliases.iter().map(String::as_str))
                    .filter(|a| word_boundary_match(&lower, a))
                    .map(|a| a.len())
                    .max()?;
                Some((key.clone(), best_len))
            })
            .max_by_key(|(_, len)| *len)
            .map(|(key, _)| key);

        let provider_score = if provider_match.is_some() {
            W_PROVIDER
        } else {
            0.0
        };

        // --- Workspace (only when a provider matched) ---
        // Requires a preposition ("in X", "for X", "at X") with word-boundary matching.
        let workspace_match = provider_match.as_ref().and_then(|p_key| {
            let prov = self.store.providers.get(p_key)?;
            prov.workspaces
                .iter()
                .filter(|(ws_key, ws_entry)| {
                    let name_lower = ws_entry.name.to_lowercase();
                    let mut candidates: Vec<&str> =
                        vec![ws_key.as_str(), &name_lower];
                    candidates.extend(ws_entry.alias.aliases.iter().map(String::as_str));
                    ["in ", "for ", "at "].iter().any(|prep| {
                        candidates.iter().any(|c| {
                            let phrase = format!("{prep}{c}");
                            word_boundary_match(&lower, &phrase)
                        })
                    })
                })
                .max_by_key(|(_, ws_entry)| {
                    ws_entry.alias.aliases.iter().map(|a| a.len()).max().unwrap_or(0)
                })
                .map(|(_, ws_entry)| ws_entry.name.clone())
        });

        let workspace_score = if workspace_match.is_some() {
            W_WORKSPACE
        } else {
            0.0
        };

        // --- Action (prefer the action whose longest matching alias is the longest) ---
        let action_match = self
            .store
            .actions
            .iter()
            .filter(|(_, entry)| entry.matches(&lower))
            .max_by_key(|(_, entry)| {
                // The length of the longest alias that actually matched.
                entry
                    .aliases
                    .iter()
                    .filter(|a| word_boundary_match(&lower, a))
                    .map(|a| a.len())
                    .max()
                    .unwrap_or(0)
            })
            .map(|(key, _)| key.clone());

        let action_score = if action_match.is_some() { W_ACTION } else { 0.0 };

        let confidence = provider_score + workspace_score + action_score;

        let mut parts = vec![];
        if let Some(ref p) = provider_match {
            parts.push(format!("provider={p}"));
        }
        if let Some(ref w) = workspace_match {
            parts.push(format!("workspace={w}"));
        }
        if let Some(ref a) = action_match {
            parts.push(format!("action={a}"));
        }

        if parts.is_empty() {
            return ResolutionOutcome::PassThrough;
        }

        if confidence >= CONFIDENT {
            ResolutionOutcome::Resolved(ResolvedIntent {
                provider: provider_match.unwrap(),
                workspace: workspace_match,
                action: action_match,
                confidence,
                reason: parts.join(", "),
            })
        } else if confidence >= NEEDS_AI {
            ResolutionOutcome::NeedsAi
        } else {
            ResolutionOutcome::PassThrough
        }
    }

    // -------------------------------------------------------------------------
    // Learning
    // -------------------------------------------------------------------------

    /// Validate an AI-produced intent against the store.
    /// Rejects entirely if the provider is unknown. Strips unknown workspace/action
    /// and applies a multiplicative confidence penalty for each removed field.
    pub fn validate_ai_intent(&self, mut intent: ResolvedIntent) -> Option<ResolvedIntent> {
        const PENALTY: f32 = 0.6;

        let prov_key = intent.provider.to_lowercase();
        let prov = self.store.providers.get(&prov_key)?;
        intent.provider = prov_key;

        if let Some(ref ws) = intent.workspace {
            if !prov.workspaces.contains_key(&ws.to_lowercase()) {
                intent.workspace = None;
                intent.confidence *= PENALTY;
            }
        }

        if let Some(ref action) = intent.action {
            if !self.store.actions.contains_key(action) {
                intent.action = None;
                intent.confidence *= PENALTY;
            }
        }

        Some(intent)
    }

    /// Boost usage scores for every entry touched by a confirmed resolution.
    pub fn learn(&mut self, intent: &ResolvedIntent) {
        if let Some(prov) = self.store.providers.get_mut(&intent.provider) {
            prov.alias.bump();
            if let Some(ws_name) = &intent.workspace {
                let ws_key = ws_name.to_lowercase();
                if let Some(ws) = prov.workspaces.get_mut(&ws_key) {
                    ws.alias.bump();
                }
            }
        }
        if let Some(action_key) = &intent.action {
            if let Some(entry) = self.store.actions.get_mut(action_key) {
                entry.bump();
            }
        }
    }

    // -------------------------------------------------------------------------
    // Store management
    // -------------------------------------------------------------------------

    pub fn add_provider(&mut self, name: &str, alias: Option<&str>) -> Result<()> {
        let key = name.to_lowercase();
        if key.is_empty() {
            bail!("Provider name cannot be empty");
        }
        let entry = self
            .store
            .providers
            .entry(key.clone())
            .or_insert_with(|| ProviderEntry::new(vec![key.clone()]));
        if let Some(a) = alias {
            let a_lower = a.to_lowercase();
            if !a_lower.is_empty() && !entry.alias.aliases.contains(&a_lower) {
                entry.alias.aliases.push(a_lower);
            }
        }
        Ok(())
    }

    pub fn add_workspace(
        &mut self,
        provider: &str,
        name: &str,
        alias: Option<&str>,
    ) -> Result<()> {
        let prov_key = provider.to_lowercase();
        let prov = self.store.providers.get_mut(&prov_key).ok_or_else(|| {
            anyhow!(
                "Provider '{}' not found. Add it first with `/resolver learn provider {}`",
                provider,
                provider
            )
        })?;
        let ws_key = name.to_lowercase();
        if ws_key.is_empty() {
            bail!("Workspace name cannot be empty");
        }
        let ws_entry = prov
            .workspaces
            .entry(ws_key.clone())
            .or_insert_with(|| WorkspaceEntry::new(name, vec![ws_key]));
        if let Some(a) = alias {
            let a_lower = a.to_lowercase();
            if !a_lower.is_empty() && !ws_entry.alias.aliases.contains(&a_lower) {
                ws_entry.alias.aliases.push(a_lower);
            }
        }
        Ok(())
    }

    pub fn add_action(&mut self, name: &str, alias: &str) -> Result<()> {
        if name.is_empty() {
            bail!("Action name cannot be empty");
        }
        if alias.is_empty() {
            bail!("Action alias cannot be empty");
        }
        let entry = self
            .store
            .actions
            .entry(name.to_string())
            .or_insert_with(|| AliasEntry::new(vec![name.to_lowercase()]));
        let a_lower = alias.to_lowercase();
        if !entry.aliases.contains(&a_lower) {
            entry.aliases.push(a_lower);
        }
        Ok(())
    }

    pub fn remove_provider(&mut self, name: &str) -> bool {
        self.store.providers.remove(&name.to_lowercase()).is_some()
    }

    pub fn remove_workspace(&mut self, provider: &str, name: &str) -> Result<bool> {
        let prov = self
            .store
            .providers
            .get_mut(&provider.to_lowercase())
            .ok_or_else(|| anyhow!("Provider '{}' not found", provider))?;
        Ok(prov.workspaces.remove(&name.to_lowercase()).is_some())
    }

    pub fn remove_action(&mut self, name: &str) -> bool {
        self.store.actions.remove(name).is_some()
    }
}

// -------------------------------------------------------------------------
// Helpers
// -------------------------------------------------------------------------

/// Extract the first `{...}` block from an LLM response.
pub fn extract_json_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end >= start {
        Some(&text[start..=end])
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_resolver() -> Resolver {
        let dir = std::env::temp_dir().join(format!(
            "fio-resolver-unit-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Resolver::load(&dir).unwrap()
    }

    fn setup() -> Resolver {
        let mut r = make_resolver();
        r.add_provider("linear", Some("ln")).unwrap();
        r.add_workspace("linear", "SAM", Some("sam")).unwrap();
        r.add_action("create_tickets", "create tickets").unwrap();
        r.add_action("list_issues", "list issues").unwrap();
        r
    }

    #[test]
    fn empty_store_passes_through() {
        let r = make_resolver();
        assert!(matches!(r.resolve("create tickets in SAM"), ResolutionOutcome::PassThrough));
    }

    #[test]
    fn resolves_all_three_components() {
        let r = setup();
        let text = "linear: create tickets in SAM - bug in checkout";
        match r.resolve(text) {
            ResolutionOutcome::Resolved(intent) => {
                assert_eq!(intent.provider, "linear");
                assert_eq!(intent.workspace.as_deref(), Some("SAM"));
                assert_eq!(intent.action.as_deref(), Some("create_tickets"));
                assert!(intent.confidence >= 0.80);
            }
            other => panic!("Expected Resolved, got {:?}", std::mem::discriminant(&other)),
        }
    }

    #[test]
    fn provider_only_passes_through() {
        let r = setup();
        // Provider-only confidence = 0.40 < NEEDS_AI (0.45), so it passes through.
        let text = "do something with linear";
        assert!(matches!(r.resolve(text), ResolutionOutcome::PassThrough));
    }

    #[test]
    fn provider_plus_action_needs_ai() {
        let r = setup();
        // Provider(0.40) + Action(0.25) = 0.65 → falls in NeedsAi range (0.45..0.80).
        let text = "linear list issues";
        assert!(matches!(r.resolve(text), ResolutionOutcome::NeedsAi));
    }

    #[test]
    fn alias_matches_provider() {
        let r = setup();
        let text = "ln: create tickets in SAM";
        match r.resolve(text) {
            ResolutionOutcome::Resolved(intent) => {
                assert_eq!(intent.provider, "linear");
            }
            other => panic!("Expected Resolved, got {:?}", std::mem::discriminant(&other)),
        }
    }

    #[test]
    fn workspace_preposition_variants() {
        let r = setup();
        for prep in ["in SAM", "for SAM", "at SAM"] {
            let text = format!("linear create tickets {prep}");
            match r.resolve(&text) {
                ResolutionOutcome::Resolved(intent) => {
                    assert_eq!(intent.workspace.as_deref(), Some("SAM"), "prep={prep}");
                }
                other => panic!(
                    "prep={prep}: Expected Resolved, got {:?}",
                    std::mem::discriminant(&other)
                ),
            }
        }
    }

    #[test]
    fn learn_bumps_scores() {
        let mut r = setup();
        let text = "linear: create tickets in SAM";
        let ResolutionOutcome::Resolved(intent) = r.resolve(text) else {
            panic!("Expected Resolved");
        };
        r.learn(&intent);
        assert_eq!(r.store.providers["linear"].alias.score, 1);
        assert_eq!(r.store.providers["linear"].workspaces["sam"].alias.score, 1);
        assert_eq!(r.store.actions["create_tickets"].score, 1);
    }

    #[test]
    fn add_remove_provider() {
        let mut r = make_resolver();
        r.add_provider("slack", Some("sl")).unwrap();
        assert!(r.store.providers.contains_key("slack"));
        let removed = r.remove_provider("slack");
        assert!(removed);
        assert!(!r.store.providers.contains_key("slack"));
    }

    #[test]
    fn rejects_empty_names() {
        let mut r = make_resolver();
        assert!(r.add_provider("", None).is_err());
        r.add_provider("linear", None).unwrap();
        assert!(r.add_workspace("linear", "", None).is_err());
        assert!(r.add_action("", "alias").is_err());
        assert!(r.add_action("name", "").is_err());
    }

    #[test]
    fn add_workspace_requires_existing_provider() {
        let mut r = make_resolver();
        let err = r.add_workspace("nonexistent", "TEAM", None).unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn preamble_format() {
        let intent = ResolvedIntent {
            provider: "linear".to_string(),
            workspace: Some("SAM".to_string()),
            action: Some("create_tickets".to_string()),
            confidence: 0.95,
            reason: "all matched".to_string(),
        };
        let preamble = intent.to_preamble();
        assert!(preamble.contains("provider=linear"));
        assert!(preamble.contains("workspace=SAM"));
        assert!(preamble.contains("action=create_tickets"));
    }

    #[test]
    fn word_boundary_prevents_false_positives() {
        use types::word_boundary_match;

        // "in" should NOT match inside "linear" — only at word boundaries.
        assert!(!word_boundary_match("linear create tickets", "in"));
        // "in" SHOULD match as a standalone word.
        assert!(word_boundary_match("in: do something", "in"));
        // Multi-word alias matches at boundaries.
        assert!(word_boundary_match("please create tickets now", "create tickets"));
        // Should NOT match partial words.
        assert!(!word_boundary_match("recreate tickets", "create tickets"));
        // Empty alias never matches.
        assert!(!word_boundary_match("anything", ""));
    }

    #[test]
    fn pathological_provider_name_doesnt_false_match() {
        let mut r = make_resolver();
        r.add_provider("in", None).unwrap();
        // "in" should NOT fire from "linear" — word boundary prevents it.
        let text = "linear create tickets";
        assert!(matches!(r.resolve(text), ResolutionOutcome::PassThrough));
    }

    #[test]
    fn short_alias_doesnt_match_inside_words() {
        let mut r = make_resolver();
        r.add_provider("linear", None).unwrap();
        r.add_workspace("linear", "S", Some("s")).unwrap();
        // "s" should NOT match inside "issues" or "status".
        // Provider-only = 0.40 < NEEDS_AI threshold, so PassThrough.
        let text = "linear list issues in status";
        assert!(matches!(r.resolve(text), ResolutionOutcome::PassThrough));
    }

    #[test]
    fn extract_json_object_finds_first_block() {
        let text = r#"Sure! {"provider":"linear","confidence":0.9} done."#;
        let extracted = extract_json_object(text).unwrap();
        assert!(extracted.starts_with('{'));
        assert!(extracted.ends_with('}'));
        assert!(extracted.contains("\"linear\""));
    }

    // --- validate_ai_intent ---

    fn ai_intent(provider: &str, workspace: Option<&str>, action: Option<&str>) -> ResolvedIntent {
        ResolvedIntent {
            provider: provider.to_string(),
            workspace: workspace.map(str::to_string),
            action: action.map(str::to_string),
            confidence: 0.95,
            reason: "AI: test".to_string(),
        }
    }

    #[test]
    fn validate_rejects_unknown_provider() {
        let r = setup();
        assert!(r.validate_ai_intent(ai_intent("nonexistent", None, None)).is_none());
    }

    #[test]
    fn validate_normalizes_provider_case() {
        let r = setup();
        let result = r.validate_ai_intent(ai_intent("Linear", None, None)).unwrap();
        assert_eq!(result.provider, "linear");
    }

    #[test]
    fn validate_passes_all_known_fields() {
        let r = setup();
        let result = r.validate_ai_intent(ai_intent("linear", Some("SAM"), Some("create_tickets"))).unwrap();
        assert_eq!(result.provider, "linear");
        assert_eq!(result.workspace.as_deref(), Some("SAM"));
        assert_eq!(result.action.as_deref(), Some("create_tickets"));
        assert!((result.confidence - 0.95).abs() < f32::EPSILON);
    }

    #[test]
    fn validate_strips_unknown_workspace() {
        let r = setup();
        let result = r.validate_ai_intent(ai_intent("linear", Some("BOGUS"), Some("create_tickets"))).unwrap();
        assert!(result.workspace.is_none());
        assert_eq!(result.action.as_deref(), Some("create_tickets"));
        assert!(result.confidence < 0.95);
    }

    #[test]
    fn validate_strips_unknown_action() {
        let r = setup();
        let result = r.validate_ai_intent(ai_intent("linear", Some("SAM"), Some("hallucinated_action"))).unwrap();
        assert_eq!(result.workspace.as_deref(), Some("SAM"));
        assert!(result.action.is_none());
        assert!(result.confidence < 0.95);
    }

    #[test]
    fn validate_strips_both_unknown_fields() {
        let r = setup();
        let result = r.validate_ai_intent(ai_intent("linear", Some("BOGUS"), Some("hallucinated"))).unwrap();
        assert!(result.workspace.is_none());
        assert!(result.action.is_none());
        // 0.95 * 0.6 * 0.6 = 0.342
        assert!(result.confidence < 0.40);
    }
}
