use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredOAuthToken {
    pub access_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(default = "default_token_type")]
    pub token_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at_unix: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

impl StoredOAuthToken {
    pub fn has_refresh_token(&self) -> bool {
        self.refresh_token
            .as_ref()
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeviceCodeStart {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification_uri_complete: Option<String>,
    pub expires_in: i64,
    pub interval: i64,
}

fn default_token_type() -> String {
    "Bearer".to_string()
}
