use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

use super::super::McpManager;

/// Typed wrapper for the cron-mcp tools exposed via MCP.
#[derive(Clone)]
pub struct CronMcpClient {
    manager: Arc<McpManager>,
    server_name: String,
}

impl CronMcpClient {
    pub fn new(manager: Arc<McpManager>, server_name: impl Into<String>) -> Self {
        Self {
            manager,
            server_name: server_name.into(),
        }
    }

    async fn call_tool_raw(&self, tool: &str, args: Option<Value>) -> Result<ToolEnvelope> {
        let prefixed = format!("mcp__{}__{}", self.server_name, tool);
        let args_val = args.unwrap_or(Value::Null);
        let raw = self.manager.call_tool(&prefixed, args_val).await?;
        parse_tool_envelope(raw)
    }

    pub async fn list_jobs(&self, enabled: Option<bool>, source: Option<&str>) -> Result<Vec<CronJob>> {
        let mut obj = serde_json::Map::new();
        if let Some(e) = enabled {
            obj.insert("enabled".to_string(), Value::Bool(e));
        }
        if let Some(s) = source {
            obj.insert("source".to_string(), Value::String(s.to_string()));
        }
        let args = if obj.is_empty() { None } else { Some(Value::Object(obj)) };
        let env = self.call_tool_raw("cron_list_jobs", args).await?;
        if !env.ok {
            return Err(anyhow!(env
                .error_message()
                .unwrap_or_else(|| "cron_list_jobs failed".to_string())));
        }
        let result = env
            .result
            .ok_or_else(|| anyhow!("Missing result in cron_list_jobs response"))?;
        let jobs_val = result.get("jobs").cloned().unwrap_or(Value::Array(vec![]));
        let jobs: Vec<CronJob> = serde_json::from_value(jobs_val)?;
        Ok(jobs)
    }

    pub async fn create_or_update_job(
        &self,
        command: &str,
        schedule: &str,
        description: Option<&str>,
        force_update: bool,
        dry_run: bool,
    ) -> Result<MutationResponse> {
        let mut map = serde_json::Map::new();
        map.insert("command".into(), Value::String(command.into()));
        map.insert("schedule".into(), Value::String(schedule.into()));
        if let Some(d) = description {
            map.insert("description".into(), Value::String(d.into()));
        }
        if force_update {
            map.insert("forceUpdate".into(), Value::Bool(true));
        }
        if dry_run {
            map.insert("dryRun".into(), Value::Bool(true));
        }
        let env = self
            .call_tool_raw("cron_create_or_update_job", Some(Value::Object(map)))
            .await?;
        if !env.ok {
            return Err(anyhow!(env
                .error_message()
                .unwrap_or_else(|| "create_or_update blocked".to_string())));
        }
        let result = env
            .result
            .ok_or_else(|| anyhow!("Missing result in create_or_update_job response"))?;
        let mr: MutationResponse = serde_json::from_value(result)?;
        Ok(mr)
    }

    async fn simple_mutation(
        &self,
        tool: &str,
        selector: serde_json::Map<String, Value>,
    ) -> Result<MutationResponse> {
        let args = Value::Object(selector);
        let env = self.call_tool_raw(tool, Some(args)).await?;
        if !env.ok {
            return Err(anyhow!(env
                .error_message()
                .unwrap_or_else(|| format!("{} blocked", tool))));
        }
        let result = env
            .result
            .ok_or_else(|| anyhow!("Missing result in mutation response"))?;
        let mr: MutationResponse = serde_json::from_value(result)?;
        Ok(mr)
    }

    pub async fn disable_job_by_command(&self, command: &str, dry_run: bool) -> Result<MutationResponse> {
        let mut map = serde_json::Map::new();
        map.insert("command".into(), Value::String(command.into()));
        if dry_run {
            map.insert("dryRun".into(), Value::Bool(true));
        }
        self.simple_mutation("cron_disable_job", map).await
    }

    pub async fn enable_job_by_command(&self, command: &str, dry_run: bool) -> Result<MutationResponse> {
        let mut map = serde_json::Map::new();
        map.insert("command".into(), Value::String(command.into()));
        if dry_run {
            map.insert("dryRun".into(), Value::Bool(true));
        }
        self.simple_mutation("cron_enable_job", map).await
    }

    pub async fn delete_job_by_command(&self, command: &str, dry_run: bool) -> Result<MutationResponse> {
        let mut map = serde_json::Map::new();
        map.insert("command".into(), Value::String(command.into()));
        if dry_run {
            map.insert("dryRun".into(), Value::Bool(true));
        }
        self.simple_mutation("cron_delete_job", map).await
    }

    pub async fn explain_schedule(&self, schedule: &str, occurrences: Option<u8>) -> Result<Value> {
        let mut map = serde_json::Map::new();
        map.insert("schedule".into(), Value::String(schedule.into()));
        if let Some(n) = occurrences {
            map.insert("showNextOccurrences".into(), Value::Number(serde_json::Number::from(n)));
        }
        let env = self.call_tool_raw("cron_explain_schedule", Some(Value::Object(map))).await?;
        if !env.ok {
            return Err(anyhow!(env
                .error_message()
                .unwrap_or_else(|| "cron_explain_schedule blocked".to_string())));
        }
        Ok(env.result.unwrap_or(Value::Null))
    }

    pub async fn nl_to_cron(&self, text: &str) -> Result<Value> {
        let args = json!({ "text": text });
        let env = self.call_tool_raw("cron_nl_to_cron", Some(args)).await?;
        if !env.ok {
            return Err(anyhow!(env
                .error_message()
                .unwrap_or_else(|| "cron_nl_to_cron blocked".to_string())));
        }
        Ok(env.result.unwrap_or(Value::Null))
    }
}

fn parse_tool_envelope(raw: Value) -> Result<ToolEnvelope> {
    // case A: the MCP server already returned the envelope object
    if raw.get("tool").is_some() {
        let te: ToolEnvelope = serde_json::from_value(raw)?;
        return Ok(te);
    }

    // case B: the server returned a CallToolResult-like object with `content[0].text` containing JSON
    if let Some(content) = raw.get("content").and_then(|v| v.as_array()) {
        if content.is_empty() {
            return Err(anyhow!("Empty content in MCP response"));
        }
        if let Some(text) = content[0].get("text").and_then(|t| t.as_str()) {
            let parsed: ToolEnvelope = serde_json::from_str(text)?;
            return Ok(parsed);
        }
    }

    Err(anyhow!("Unrecognized MCP tool result shape"))
}

#[derive(Debug, Deserialize, Serialize)]
struct ToolEnvelope {
    pub tool: String,
    pub ok: bool,
    pub status: String,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub error: Option<ToolError>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ToolError {
    message: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CronJob {
    pub id: String,
    pub schedule: String,
    pub command: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub source: String,
    #[serde(rename = "rawLines")]
    pub raw_lines: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DiffChange {
    #[serde(rename = "type")]
    pub change_type: String,
    pub before: Option<String>,
    pub after: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CronDiff {
    #[serde(rename = "isNoop")]
    pub is_noop: bool,
    pub before: String,
    pub after: String,
    pub changes: Vec<DiffChange>,
    pub unified: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SafetyIssue {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub details: Option<Value>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SafetyReport {
    pub issues: Vec<SafetyIssue>,
    #[serde(rename = "canProceed")]
    pub can_proceed: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MutationResponse {
    pub status: String,
    #[serde(rename = "dryRun")]
    pub dry_run: Option<bool>,
    pub job: Option<CronJob>,
    pub jobs: Option<Vec<CronJob>>,
    pub diff: Option<CronDiff>,
    pub safety: SafetyReport,
    #[serde(rename = "backupPath")]
    pub backup_path: Option<String>,
}

impl ToolEnvelope {
    fn error_message(&self) -> Option<String> {
        self.error.as_ref().map(|e| e.message.clone())
    }
}
