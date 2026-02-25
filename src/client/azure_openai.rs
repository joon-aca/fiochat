use super::openai::*;
use super::*;

use anyhow::Result;
use serde::Deserialize;

const DEFAULT_CHAT_API_VERSION: &str = "2024-12-01-preview";
const DEFAULT_EMBEDDINGS_API_VERSION: &str = "2024-10-21";

#[derive(Debug, Clone, Deserialize)]
pub struct AzureOpenAIConfig {
    pub name: Option<String>,
    pub api_base: Option<String>,
    pub api_key: Option<String>,
    pub api_version: Option<String>,
    #[serde(default)]
    pub models: Vec<ModelData>,
    pub patch: Option<RequestPatch>,
    pub extra: Option<ExtraConfig>,
}

impl AzureOpenAIClient {
    config_get_fn!(api_base, get_api_base);
    config_get_fn!(api_key, get_api_key);

    pub const PROMPTS: [PromptAction<'static>; 3] = [
        (
            "api_base",
            "API Base",
            Some("e.g. https://{RESOURCE}.openai.azure.com"),
        ),
        ("api_key", "API Key", None),
        ("api_version", "API Version", Some("e.g. 2025-01-01-preview")),
    ];
}

impl_client_trait!(
    AzureOpenAIClient,
    (
        prepare_chat_completions,
        openai_chat_completions,
        openai_chat_completions_streaming
    ),
    (prepare_embeddings, openai_embeddings),
    (noop_prepare_rerank, noop_rerank),
);

fn prepare_chat_completions(
    self_: &AzureOpenAIClient,
    data: ChatCompletionsData,
) -> Result<RequestData> {
    let api_base = self_.get_api_base()?;
    let api_key = self_.get_api_key()?;
    let api_version = self_
        .config
        .api_version
        .clone()
        .unwrap_or_else(|| DEFAULT_CHAT_API_VERSION.to_string());

    let url = format!(
        "{}/openai/deployments/{}/chat/completions?api-version={}",
        &api_base,
        self_.model.real_name(),
        api_version
    );

    let body = openai_build_chat_completions_body(data, &self_.model);

    let mut request_data = RequestData::new(url, body);

    request_data.header("api-key", api_key);

    Ok(request_data)
}

fn prepare_embeddings(self_: &AzureOpenAIClient, data: &EmbeddingsData) -> Result<RequestData> {
    let api_base = self_.get_api_base()?;
    let api_key = self_.get_api_key()?;
    let api_version = self_
        .config
        .api_version
        .clone()
        .unwrap_or_else(|| DEFAULT_EMBEDDINGS_API_VERSION.to_string());

    let url = format!(
        "{}/openai/deployments/{}/embeddings?api-version={}",
        &api_base,
        self_.model.real_name(),
        api_version
    );

    let body = openai_build_embeddings_body(data, &self_.model);

    let mut request_data = RequestData::new(url, body);

    request_data.header("api-key", api_key);

    Ok(request_data)
}
