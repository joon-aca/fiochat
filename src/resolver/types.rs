use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Check whether `alias` appears in `text` at a word boundary.
/// Both inputs must already be lowercased.
pub(crate) fn word_boundary_match(text: &str, alias: &str) -> bool {
    if alias.is_empty() {
        return false;
    }
    let text_bytes = text.as_bytes();
    let mut search_from = 0;
    while let Some(pos) = text[search_from..].find(alias) {
        let abs = search_from + pos;
        let end = abs + alias.len();
        let before_ok = abs == 0 || !text_bytes[abs - 1].is_ascii_alphanumeric();
        let after_ok = end >= text.len() || !text_bytes[end].is_ascii_alphanumeric();
        if before_ok && after_ok {
            return true;
        }
        search_from = abs + 1;
    }
    false
}

/// A named thing with aliases, a usage score, and a last-used timestamp.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AliasEntry {
    pub aliases: Vec<String>,
    pub score: u32,
    pub last_used_secs: Option<u64>,
}

impl AliasEntry {
    pub fn new(aliases: Vec<String>) -> Self {
        Self {
            aliases,
            score: 0,
            last_used_secs: None,
        }
    }

    /// True if any alias appears in `text` at a word boundary (case-insensitive).
    pub fn matches(&self, text: &str) -> bool {
        let lower = text.to_lowercase();
        self.aliases
            .iter()
            .any(|a| word_boundary_match(&lower, a))
    }

    pub fn bump(&mut self) {
        self.score = self.score.saturating_add(1);
        self.last_used_secs = Some(now_secs());
    }
}

/// A workspace within a provider (e.g. a Linear team).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkspaceEntry {
    /// Display-form canonical name (e.g. "SAM").
    pub name: String,
    #[serde(flatten)]
    pub alias: AliasEntry,
}

impl WorkspaceEntry {
    pub fn new(name: &str, aliases: Vec<String>) -> Self {
        Self {
            name: name.to_string(),
            alias: AliasEntry::new(aliases),
        }
    }
}

/// A provider (e.g. "linear") with its workspaces.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderEntry {
    #[serde(flatten)]
    pub alias: AliasEntry,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub workspaces: HashMap<String, WorkspaceEntry>,
}

impl ProviderEntry {
    pub fn new(aliases: Vec<String>) -> Self {
        Self {
            alias: AliasEntry::new(aliases),
            workspaces: HashMap::new(),
        }
    }
}

/// The persisted resolver state.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResolverStore {
    /// Providers keyed by canonical name (lowercase, e.g. "linear").
    #[serde(default)]
    pub providers: HashMap<String, ProviderEntry>,
    /// Actions keyed by canonical snake_case name (e.g. "create_tickets").
    #[serde(default)]
    pub actions: HashMap<String, AliasEntry>,
}

/// The result of a successful resolution.
#[derive(Debug, Clone)]
pub struct ResolvedIntent {
    pub provider: String,
    pub workspace: Option<String>,
    pub action: Option<String>,
    /// Confidence in [0, 1].
    pub confidence: f32,
    pub reason: String,
}

impl ResolvedIntent {
    /// Build the context preamble that is prepended to the user message.
    pub fn to_preamble(&self) -> String {
        let workspace = self.workspace.as_deref().unwrap_or("-");
        let action = self.action.as_deref().unwrap_or("-");
        format!(
            "[Resolver: provider={} workspace={} action={} confidence={:.2} reason={}]",
            self.provider, workspace, action, self.confidence, self.reason
        )
    }
}

/// The outcome returned by the deterministic resolution pass.
pub enum ResolutionOutcome {
    Resolved(ResolvedIntent),
    /// Confidence is in the medium range — worth asking the AI.
    NeedsAi,
    /// Not enough signal; pass the input through unchanged.
    PassThrough,
}
