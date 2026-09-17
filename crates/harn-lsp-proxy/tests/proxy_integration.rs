use harn_lsp_proxy::rpc::RpcMessage;
use harn_lsp_proxy::text::{find_symbol_location, get_word_at_position, rank_symbol_locations};
use serde_json::json;

#[test]
fn test_proxy_initialize_mirroring() {
    let client_init = RpcMessage::request(
        json!(1),
        "initialize",
        Some(json!({
            "processId": 1234,
            "rootUri": "file:///workspace/project",
            "capabilities": {}
        })),
    );

    // RA clone gets updated workspace and rootUri directed at harn-main
    let ra_init = client_init.clone_second().expect("should clone initialize");
    let ra_params = ra_init.params.as_ref().unwrap();
    assert!(
        ra_params["rootUri"]
            .as_str()
            .unwrap()
            .ends_with("/harn-main")
    );
    assert_eq!(ra_params["workspaceFolders"][0]["name"], "harn-main");
}

#[test]
fn test_proxy_definition_fallback_flow() {
    // 1. Cursor is on "Harness" in the code
    let code = "fn run(h: Harness) {\n  h.execute();\n}";
    let word = get_word_at_position(code, 0, 12).expect("should extract word");
    assert_eq!(word, "Harness");

    // 2. Harn-lsp returns empty/null result
    let harn_resp = RpcMessage::response(json!(42), json!(null));
    assert!(harn_resp.is_empty_result());

    // 3. Proxy forms a workspace/symbol query to rust-analyzer
    let ra_query = RpcMessage::request(
        json!(42),
        "workspace/symbol",
        Some(json!({
            "query": word
        })),
    );
    assert_eq!(ra_query.id, Some(json!(42)));
    assert_eq!(ra_query.method.as_deref(), Some("workspace/symbol"));

    // 4. Rust-analyzer returns candidate symbols (including unrelated substring matches)
    let ra_resp = RpcMessage::response(
        json!(42),
        json!([
            {
                "name": "HarnessHelper",
                "kind": 12,
                "location": { "uri": "file:///path/to/helper.rs" }
            },
            {
                "name": "Harness",
                "kind": 22,
                "location": {
                    "uri": "file:///path/to/harness.rs",
                    "range": { "start": { "line": 10, "character": 0 } }
                }
            }
        ]),
    );

    // 5. Proxy transforms RA response into Zed definition response with strictly filtered locations
    let zed_resp = ra_resp.into_symbol_response(&word);
    assert_eq!(zed_resp.id, Some(json!(42)));
    let result = zed_resp.result.unwrap();
    let locations = result
        .as_array()
        .expect("result should be an array of locations");

    // Only exact match (Harness) is kept; HarnessHelper is filtered out
    assert_eq!(locations.len(), 1);
    assert_eq!(locations[0]["uri"], "file:///path/to/harness.rs");
    assert_eq!(locations[0]["range"]["start"]["line"], 10);
}

#[test]
fn test_proxy_definition_found_by_harn_no_fallback() {
    let harn_resp = RpcMessage::response(
        json!(50),
        json!({
            "uri": "file:///workspace/project/local.harn",
            "range": { "start": { "line": 5, "character": 4 } }
        }),
    );

    assert!(!harn_resp.is_empty_result());
}

#[test]
fn test_find_and_rank_symbol_location_heuristics() {
    let symbols = vec![
        json!({ "name": "foo", "kind": 12, "location": { "uri": "file:///foo_fn.rs" } }),
        json!({ "name": "Target", "kind": 22, "location": { "uri": "file:///target_struct.rs" } }),
    ];

    // Priority kind match (struct = 22)
    let loc = find_symbol_location(&symbols, "Target").unwrap();
    assert_eq!(loc["uri"], "file:///target_struct.rs");

    // Exact name match for function
    let loc_foo = find_symbol_location(&symbols, "foo").unwrap();
    assert_eq!(loc_foo["uri"], "file:///foo_fn.rs");

    // Non-exact matches return None
    let loc_unmatched = find_symbol_location(&symbols, "unknown");
    assert_eq!(loc_unmatched, None);

    // Ranking returns only exact matches for "Target"
    let ranked = rank_symbol_locations(&symbols, "Target");
    assert_eq!(ranked.len(), 1);
    assert_eq!(ranked[0]["uri"], "file:///target_struct.rs");
}
