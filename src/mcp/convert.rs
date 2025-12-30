use anyhow::{Context, Result};
use serde_json::Value;

use crate::function::{FunctionDeclaration, JsonSchema};

/// Convert MCP tool schema to fiochat `FunctionDeclaration`.
pub fn mcp_tool_to_function(
    server_name: &str,
    tool_name: &str,
    tool_description: &str,
    input_schema: &Value,
) -> Result<FunctionDeclaration> {
    // Prefix name to avoid conflicts. The double-underscore sentinel allows server names with
    // underscores without ambiguity.
    let prefixed_name = format!("mcp__{}__{}", server_name, tool_name);

    let parameters = convert_json_schema(input_schema)
        .with_context(|| format!("Failed to convert schema for MCP tool '{tool_name}'"))?;

    Ok(FunctionDeclaration {
        name: prefixed_name,
        description: tool_description.to_string(),
        parameters,
        agent: false,
    })
}

fn convert_json_schema(schema: &Value) -> Result<JsonSchema> {
    let mut json_schema = JsonSchema {
        type_value: schema
            .get("type")
            .and_then(|v| v.as_str())
            .map(String::from),
        description: schema
            .get("description")
            .and_then(|v| v.as_str())
            .map(String::from),
        properties: None,
        items: None,
        any_of: None,
        enum_value: None,
        default: schema.get("default").cloned(),
        required: None,
    };

    if let Some(properties) = schema.get("properties").and_then(|v| v.as_object()) {
        let mut converted_props = indexmap::IndexMap::new();
        for (key, value) in properties {
            converted_props.insert(key.clone(), convert_json_schema(value)?);
        }
        json_schema.properties = Some(converted_props);
    }

    if let Some(required) = schema.get("required").and_then(|v| v.as_array()) {
        json_schema.required = Some(
            required
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
        );
    }

    if let Some(items) = schema.get("items") {
        json_schema.items = Some(Box::new(convert_json_schema(items)?));
    }

    if let Some(any_of) = schema.get("anyOf").and_then(|v| v.as_array()) {
        let mut converted = vec![];
        for item in any_of {
            converted.push(convert_json_schema(item)?);
        }
        json_schema.any_of = Some(converted);
    }

    if let Some(enum_values) = schema.get("enum").and_then(|v| v.as_array()) {
        json_schema.enum_value = Some(
            enum_values
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
        );
    }

    Ok(json_schema)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_mcp_tool_conversion() {
        let schema = json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" }
            }
        });
        let func =
            mcp_tool_to_function("filesystem", "read_file", "Read a file", &schema).unwrap();
        assert_eq!(func.name, "mcp__filesystem__read_file");
    }
}


