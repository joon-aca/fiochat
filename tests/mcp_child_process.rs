use aichat::mcp::{McpManager, McpServerConfig};
use serde_json::json;

#[tokio::test]
async fn mcp_child_process_tool_discovery_and_call() {
    // Cargo sets this env var for package binaries when running tests.
    let server_exe = env!("CARGO_BIN_EXE_mcp_test_server");

    let manager = McpManager::new();
    manager
        .initialize(vec![McpServerConfig {
            name: "test".to_string(),
            command: server_exe.to_string(),
            args: vec![],
            env: Default::default(),
            enabled: true,
            trusted: false,
            description: Some("test server".to_string()),
        }])
        .await
        .unwrap();

    // Connect + discover tools.
    tokio::time::timeout(std::time::Duration::from_secs(5), manager.connect("test"))
        .await
        .expect("connect timed out")
        .unwrap();

    let tools = manager.get_all_tools().await;
    assert!(
        tools.iter()
            .any(|t| t.name == "mcp__test__echo_structured" && t.parameters.type_value.as_deref() == Some("object")),
        "expected MCP tool not found in discovered tools: {tools:?}"
    );

    // Call tool end-to-end.
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        manager.call_tool(
            "mcp__test__echo_structured",
            json!({"text": "hello", "count": 2}),
        ),
    )
    .await
    .expect("call_tool timed out")
    .unwrap();

    // rmcp serializes CallToolResult with camelCase keys.
    assert_eq!(result["isError"], json!(false));
    assert_eq!(result["structuredContent"]["echoed"], json!("hello"));
    assert_eq!(result["structuredContent"]["count"], json!(2));

    // Disconnect should succeed and clear tools.
    manager.disconnect("test").await.unwrap();
}
