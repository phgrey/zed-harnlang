use harn_lsp_proxy::{
    build_ra_initialize_request, check_definition_request, rewrite_initialize_response,
    rewrite_symbol_response, DefinitionIntercept, DocumentStore, RpcMessage,
};
use serde_json::json;
use std::path::Path;

#[test]
fn test_proxy_initialize_flow() {
    let client_init = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "capabilities": {}
        }
    }).to_string();

    let ext_dir = Path::new("/Users/example/extension");
    let ra_init = build_ra_initialize_request(&client_init, ext_dir).expect("should build ra init");
    let ra_val: serde_json::Value = serde_json::from_str(&ra_init).unwrap();
    assert_eq!(
        ra_val["params"]["rootUri"],
        "file:///Users/example/extension/harn-main"
    );

    // Harn response arrives: capabilities must be rewritten to textDocumentSync = 1
    let harn_resp = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "capabilities": {
                "hoverProvider": true,
                "textDocumentSync": 2
            }
        }
    }).to_string();

    let rewritten = rewrite_initialize_response(&harn_resp);
    let rewritten_val: serde_json::Value = serde_json::from_str(&rewritten).unwrap();
    assert_eq!(
        rewritten_val["result"]["capabilities"]["textDocumentSync"],
        json!(1)
    );
    assert_eq!(
        rewritten_val["result"]["capabilities"]["hoverProvider"],
        json!(true)
    );
}

#[test]
fn test_proxy_document_sync_and_definition_cycle() {
    let mut docs = DocumentStore::new();
    let uri = "file:///project/main.harn";

    // 1. didOpen
    let did_open_params = json!({
        "textDocument": {
            "uri": uri,
            "text": "fn main(h: Harness) {\n  let x = h.env;\n}"
        }
    });
    assert!(docs.handle_did_open(Some(&did_open_params)));
    assert_eq!(docs.get(uri).unwrap(), "fn main(h: Harness) {\n  let x = h.env;\n}");

    // 2. Definition on Harness
    let def_req = RpcMessage::request(
        json!(100),
        "textDocument/definition",
        Some(json!({
            "textDocument": { "uri": uri },
            "position": { "line": 0, "character": 12 }
        })),
    );

    let action = check_definition_request(&def_req, &docs);
    let (id, word, ra_msg) = match action {
        DefinitionIntercept::Intercepted { id, word, ra_query_message } => (id, word, ra_query_message),
        DefinitionIntercept::Forward => panic!("Expected definition to be intercepted"),
    };

    assert_eq!(id, json!(100));
    assert_eq!(word, "Harness");
    let query_val: serde_json::Value = serde_json::from_str(&ra_msg).unwrap();
    assert_eq!(query_val["method"], "workspace/symbol");
    assert_eq!(query_val["params"]["query"], "Harness");

    // 3. Rust-analyzer returns symbol search results
    let ra_resp = RpcMessage::response(
        json!(100),
        json!([
            {
                "name": "Harness_unused",
                "kind": 1,
                "location": { "uri": "file:///harn/crates/unused.rs" }
            },
            {
                "name": "Harness",
                "kind": 22,
                "location": {
                    "uri": "file:///harn/crates/harn-vm/src/harness.rs",
                    "range": { "start": { "line": 15, "character": 0 } }
                }
            }
        ]),
    );

    let final_resp_str = rewrite_symbol_response(&ra_resp, &word);
    let final_resp: serde_json::Value = serde_json::from_str(&final_resp_str).unwrap();
    assert_eq!(final_resp["id"], json!(100));
    assert_eq!(
        final_resp["result"]["uri"],
        "file:///harn/crates/harn-vm/src/harness.rs"
    );

    // 4. didChange
    let did_change_params = json!({
        "textDocument": { "uri": uri },
        "contentChanges": [{ "text": "fn main(h: Custom) {}" }]
    });
    assert!(docs.handle_did_change(Some(&did_change_params)));
    assert_eq!(docs.get(uri).unwrap(), "fn main(h: Custom) {}");

    // 5. Definition on non-Harness should forward
    let non_harness_def = RpcMessage::request(
        json!(101),
        "textDocument/definition",
        Some(json!({
            "textDocument": { "uri": uri },
            "position": { "line": 0, "character": 12 }
        })),
    );
    assert_eq!(
        check_definition_request(&non_harness_def, &docs),
        DefinitionIntercept::Forward
    );
}
