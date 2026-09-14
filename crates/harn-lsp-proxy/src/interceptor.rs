use crate::documents::DocumentStore;
use crate::rpc::RpcMessage;
use crate::symbols::find_symbol_location;
use crate::text::get_word_at_position;
use serde_json::{Value, json};
use std::path::Path;

pub fn rewrite_initialize_response(msg: &str) -> String {
    if let Ok(mut rpc) = serde_json::from_str::<RpcMessage>(msg) {
        if let Some(result) = rpc.result.as_mut() {
            if let Some(caps) = result.get_mut("capabilities") {
                if caps.get("textDocumentSync").is_some() {
                    caps["textDocumentSync"] = json!(1); // Full sync
                    if let Ok(rewritten) = serde_json::to_string(&rpc) {
                        return rewritten;
                    }
                }
            }
        }
    }
    msg.to_string()
}

pub fn build_ra_initialize_request(original_msg: &str, extension_dir: &Path) -> Option<String> {
    let mut ra_init = serde_json::from_str::<RpcMessage>(original_msg).ok()?;
    let harn_main_dir = extension_dir.join("harn-main");
    if let Some(fpath) = harn_main_dir.to_str() {
        if let Some(params) = ra_init.params.as_mut().and_then(|p| p.as_object_mut()) {
            let uri = format!("file://{}", fpath);
            params.insert("rootUri".to_string(), json!(uri));
            params.insert("rootPath".to_string(), json!(fpath));
            params.insert(
                "workspaceFolders".to_string(),
                json!([
                    {
                        "uri": uri,
                        "name": "harn-main"
                    }
                ]),
            );
            return serde_json::to_string(&ra_init).ok();
        }
    }
    None
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum DefinitionIntercept {
    Intercepted {
        id: Value,
        word: String,
        ra_query_message: String,
    },
    Forward,
}

pub fn check_definition_request(
    rpc: &RpcMessage,
    documents: &DocumentStore,
) -> DefinitionIntercept {
    if rpc.method.as_deref() != Some("textDocument/definition") {
        return DefinitionIntercept::Forward;
    }

    if let Some(params) = rpc.params.as_ref() {
        if let (Some(uri), Some(line), Some(col)) = (
            params.pointer("/textDocument/uri").and_then(|v| v.as_str()),
            params.pointer("/position/line").and_then(|v| v.as_u64()),
            params
                .pointer("/position/character")
                .and_then(|v| v.as_u64()),
        ) {
            if let Some(text) = documents.get(uri) {
                if let Some(word) = get_word_at_position(text, line as usize, col as usize) {
                    if word.starts_with("Harness") {
                        if let Some(id) = rpc.id.clone() {
                            let query_msg = json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "method": "workspace/symbol",
                                "params": {
                                    "query": word
                                }
                            });
                            if let Ok(ra_msg) = serde_json::to_string(&query_msg) {
                                return DefinitionIntercept::Intercepted {
                                    id,
                                    word,
                                    ra_query_message: ra_msg,
                                };
                            }
                        }
                    }
                }
            }
        }
    }

    DefinitionIntercept::Forward
}

pub fn rewrite_symbol_response(rpc: &RpcMessage, word: &str) -> String {
    let mut rewritten_rpc = rpc.clone();
    let mut found_loc = None;

    if let Some(result_arr) = rpc.result.as_ref().and_then(|r| r.as_array()) {
        found_loc = find_symbol_location(result_arr, word);
    }

    rewritten_rpc.result = Some(found_loc.unwrap_or(json!(null)));
    serde_json::to_string(&rewritten_rpc).unwrap_or_else(|_| "{}".to_string())
}

pub fn resolve_harn_lsp_command(extension_dir: &Path) -> String {
    let harn_lsp_path = extension_dir.join("harn-bin/harn");
    if harn_lsp_path.exists() {
        harn_lsp_path.to_str().unwrap_or("harn-lsp").to_string()
    } else {
        "harn-lsp".to_string()
    }
}

pub fn resolve_rust_analyzer_command(extension_dir: &Path) -> String {
    let ra_path = extension_dir.join("rust-analyzer");
    if ra_path.exists() {
        ra_path.to_str().unwrap_or("rust-analyzer").to_string()
    } else {
        "rust-analyzer".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};

    #[test]
    fn test_rewrite_initialize_response_sync_capability() {
        let msg = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "capabilities": {
                    "textDocumentSync": 2
                }
            }
        })
        .to_string();

        let rewritten = rewrite_initialize_response(&msg);
        let parsed: Value = serde_json::from_str(&rewritten).unwrap();
        assert_eq!(
            parsed["result"]["capabilities"]["textDocumentSync"],
            json!(1)
        );
    }

    #[test]
    fn test_rewrite_initialize_response_no_change_when_not_applicable() {
        let msg = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "capabilities": {}
            }
        })
        .to_string();

        assert_eq!(rewrite_initialize_response(&msg), msg);
        assert_eq!(rewrite_initialize_response("invalid json"), "invalid json");
    }

    #[test]
    fn test_build_ra_initialize_request() {
        let msg = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {}
        })
        .to_string();

        let ext_dir = Path::new("/tmp/extension");
        let ra_init = build_ra_initialize_request(&msg, ext_dir).unwrap();
        let parsed: Value = serde_json::from_str(&ra_init).unwrap();

        assert_eq!(parsed["params"]["rootPath"], "/tmp/extension/harn-main");
        assert_eq!(
            parsed["params"]["rootUri"],
            "file:///tmp/extension/harn-main"
        );
        assert_eq!(parsed["params"]["workspaceFolders"][0]["name"], "harn-main");
    }

    #[test]
    fn test_check_definition_request_intercepted() {
        let mut docs = DocumentStore::new();
        let uri = "file:///sample.harn";
        docs.insert(uri, "let h = Harness::new();");

        let def_req = RpcMessage::request(
            json!(10),
            "textDocument/definition",
            Some(json!({
                "textDocument": { "uri": uri },
                "position": { "line": 0, "character": 9 }
            })),
        );

        match check_definition_request(&def_req, &docs) {
            DefinitionIntercept::Intercepted {
                id,
                word,
                ra_query_message,
            } => {
                assert_eq!(id, json!(10));
                assert_eq!(word, "Harness");
                let parsed: Value = serde_json::from_str(&ra_query_message).unwrap();
                assert_eq!(parsed["method"], "workspace/symbol");
                assert_eq!(parsed["params"]["query"], "Harness");
            }
            DefinitionIntercept::Forward => panic!("Expected intercepted definition request"),
        }
    }

    #[test]
    fn test_check_definition_request_forward() {
        let mut docs = DocumentStore::new();
        let uri = "file:///sample.harn";
        docs.insert(uri, "let h = NonHarness::new();");

        let def_req = RpcMessage::request(
            json!(11),
            "textDocument/definition",
            Some(json!({
                "textDocument": { "uri": uri },
                "position": { "line": 0, "character": 9 }
            })),
        );

        assert_eq!(
            check_definition_request(&def_req, &docs),
            DefinitionIntercept::Forward
        );

        // Different method
        let hover_req = RpcMessage::request(
            json!(12),
            "textDocument/hover",
            Some(json!({
                "textDocument": { "uri": uri },
                "position": { "line": 0, "character": 9 }
            })),
        );
        assert_eq!(
            check_definition_request(&hover_req, &docs),
            DefinitionIntercept::Forward
        );
    }

    #[test]
    fn test_rewrite_symbol_response() {
        let ra_resp = RpcMessage::response(
            json!(10),
            json!([
                {
                    "name": "Harness",
                    "kind": 22,
                    "location": {
                        "uri": "file:///crates/harn-vm/src/harness.rs",
                        "range": { "start": { "line": 1, "character": 0 } }
                    }
                }
            ]),
        );

        let rewritten = rewrite_symbol_response(&ra_resp, "Harness");
        let parsed: Value = serde_json::from_str(&rewritten).unwrap();
        assert_eq!(
            parsed["result"]["uri"],
            "file:///crates/harn-vm/src/harness.rs"
        );
    }

    #[test]
    fn test_resolve_commands() {
        let temp_dir =
            std::env::temp_dir().join(format!("test_resolve_cmds_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        // Without binaries present -> returns default commands
        assert_eq!(resolve_harn_lsp_command(&temp_dir), "harn-lsp");
        assert_eq!(resolve_rust_analyzer_command(&temp_dir), "rust-analyzer");

        // With binaries present
        let harn_bin_dir = temp_dir.join("harn-bin");
        fs::create_dir_all(&harn_bin_dir).unwrap();
        File::create(harn_bin_dir.join("harn")).unwrap();
        File::create(temp_dir.join("rust-analyzer")).unwrap();

        assert_eq!(
            resolve_harn_lsp_command(&temp_dir),
            harn_bin_dir.join("harn").to_str().unwrap()
        );
        assert_eq!(
            resolve_rust_analyzer_command(&temp_dir),
            temp_dir.join("rust-analyzer").to_str().unwrap()
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }
}

#[test]
fn test_build_ra_initialize_request_non_object_params() {
    let msg = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": "not-an-object"
    })
    .to_string();

    let ext_dir = Path::new("/tmp/ext");
    assert_eq!(build_ra_initialize_request(&msg, ext_dir), None);
}

#[test]
fn test_rewrite_symbol_response_empty_or_no_match() {
    let ra_resp = RpcMessage::response(json!(5), json!([]));
    let rewritten = rewrite_symbol_response(&ra_resp, "Harness");
    let parsed: Value = serde_json::from_str(&rewritten).unwrap();
    assert_eq!(parsed["result"], json!(null));
}
