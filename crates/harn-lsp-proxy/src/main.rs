use log::info;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::env;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct RpcMessage {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<Value>,
}

async fn read_message<R: AsyncRead + Unpin>(reader: &mut BufReader<R>) -> Option<String> {
    let mut length = 0;
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line).await {
            Ok(0) => return None,
            Ok(_) => {
                if line == "\r\n" {
                    break;
                }
                if line.starts_with("Content-Length: ") {
                    length = line
                        .trim_start_matches("Content-Length: ")
                        .trim()
                        .parse()
                        .unwrap_or(0);
                }
            }
            Err(_) => return None,
        }
    }
    if length == 0 {
        return None;
    }
    let mut buf = vec![0; length];
    if reader.read_exact(&mut buf).await.is_err() {
        return None;
    }
    String::from_utf8(buf).ok()
}

async fn write_message<W: AsyncWrite + Unpin>(writer: &mut W, msg: &str) -> std::io::Result<()> {
    let payload = format!("Content-Length: {}\r\n\r\n{}", msg.len(), msg);
    writer.write_all(payload.as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}

fn get_word_at_position(text: &str, line: usize, col: usize) -> Option<String> {
    let line_str = text.lines().nth(line)?;

    // Find the word boundaries around `col`
    let mut start = col;
    while start > 0 && line_str.is_char_boundary(start - 1) {
        let c = line_str[..start].chars().last().unwrap();
        if !c.is_alphanumeric() && c != '_' {
            break;
        }
        start -= c.len_utf8();
    }

    let mut end = col;
    while end < line_str.len() {
        let c = line_str[end..].chars().next().unwrap();
        if !c.is_alphanumeric() && c != '_' {
            break;
        }
        end += c.len_utf8();
    }

    if start < end {
        Some(line_str[start..end].to_string())
    } else {
        None
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();
    info!("Starting harn-lsp-proxy");

    let current_exe = env::current_exe().expect("Failed to get current executable path");
    let extension_dir = current_exe.parent().unwrap(); // Go up from proxy-bin to extension root

    let harn_lsp_path = extension_dir.join("harn-bin/harn");
    let harn_lsp_cmd = if harn_lsp_path.exists() {
        harn_lsp_path.to_str().unwrap().to_string()
    } else {
        "harn-lsp".to_string()
    };

    let mut harn_child = Command::new(&harn_lsp_cmd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("Failed to start harn-lsp");
    let mut harn_stdin = harn_child.stdin.take().unwrap();
    let harn_stdout = harn_child.stdout.take().unwrap();

    let ra_path = extension_dir.join("rust-analyzer");
    let ra_cmd = if ra_path.exists() {
        ra_path.to_str().unwrap().to_string()
    } else {
        "rust-analyzer".to_string()
    };

    let mut ra_child = Command::new(&ra_cmd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("Failed to start rust-analyzer");
    let mut ra_stdin = ra_child.stdin.take().unwrap();
    let ra_stdout = ra_child.stdout.take().unwrap();

    let (tx_to_zed, mut rx_to_zed) = mpsc::channel::<String>(32);
    let tx_to_zed_clone1 = tx_to_zed.clone();
    let tx_to_zed_clone2 = tx_to_zed.clone();

    // Map to keep track of pending RA queries to ensure exact name matching
    use std::sync::{Arc, Mutex};
    let pending_queries: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));
    let pending_queries_clone1 = pending_queries.clone();
    let pending_queries_clone2 = pending_queries.clone();

    let mut documents = HashMap::new();

    // Read from harn-lsp -> Zed
    tokio::spawn(async move {
        let mut reader = BufReader::new(harn_stdout);
        while let Some(msg) = read_message(&mut reader).await {
            // We need to intercept harn-lsp's initialize response to force TextDocumentSyncKind::Full (1)
            let mut out_msg = msg.clone();
            if let Ok(mut rpc) = serde_json::from_str::<RpcMessage>(&msg) {
                if let Some(result) = rpc.result.as_mut() {
                    if let Some(caps) = result.get_mut("capabilities") {
                        if caps.get("textDocumentSync").is_some() {
                            caps["textDocumentSync"] = json!(1); // Full sync
                            out_msg = serde_json::to_string(&rpc).unwrap();
                        }
                    }
                }
            }
            let _ = tx_to_zed_clone1.send(out_msg).await;
        }
    });

    // Read from rust-analyzer -> Zed
    tokio::spawn(async move {
        let mut reader = BufReader::new(ra_stdout);
        while let Some(msg) = read_message(&mut reader).await {
            if let Ok(rpc) = serde_json::from_str::<RpcMessage>(&msg) {
                // If it's a response to workspace/symbol, we intercept it!
                if let Some(id) = rpc.id.as_ref() {
                    let id_str = id.to_string();
                    let queried_word = {
                        let mut map = pending_queries_clone1.lock().unwrap();
                        map.remove(&id_str)
                    };

                    if let Some(word) = queried_word {
                        let mut rewritten_rpc = rpc.clone();
                        let mut found_loc = None;

                        if let Some(result_arr) = rpc.result.as_ref().and_then(|r| r.as_array()) {
                            // First pass: look for exact name match + kind struct/enum (22/13)
                            for sym in result_arr {
                                if let (Some(name), Some(kind)) = (sym.get("name").and_then(|n| n.as_str()), sym.get("kind").and_then(|k| k.as_u64())) {
                                    if name == word && (kind == 22 || kind == 13 || kind == 5 || kind == 11) {
                                        found_loc = sym.get("location").cloned();
                                        break;
                                    }
                                }
                            }
                            // Second pass: just exact name match
                            if found_loc.is_none() {
                                for sym in result_arr {
                                    if let Some(name) = sym.get("name").and_then(|n| n.as_str()) {
                                        if name == word {
                                            found_loc = sym.get("location").cloned();
                                            break;
                                        }
                                    }
                                }
                            }
                            // Fallback: first item
                            if found_loc.is_none() && !result_arr.is_empty() {
                                found_loc = result_arr[0].get("location").cloned();
                            }
                        }
                        
                        rewritten_rpc.result = Some(found_loc.unwrap_or(json!(null)));
                        let out_msg = serde_json::to_string(&rewritten_rpc).unwrap();
                        let _ = tx_to_zed_clone2.send(out_msg).await;
                        continue;
                    }
                }
            }
            // For other RA messages, we might ignore or log them, but we only intercept pending queries
        }
    });

    // Write to Zed
    tokio::spawn(async move {
        let mut stdout = tokio::io::stdout();
        while let Some(msg) = rx_to_zed.recv().await {
            let _ = write_message(&mut stdout, &msg).await;
        }
    });

    let mut stdin = BufReader::new(tokio::io::stdin());
    
    while let Some(msg) = read_message(&mut stdin).await {
        if let Ok(rpc) = serde_json::from_str::<RpcMessage>(&msg) {
            let method = rpc.method.as_deref().unwrap_or("");

            // 1. Initialize both servers
            if method == "initialize" {
                let mut ra_init = rpc.clone();

                if let Some(fpath) = extension_dir.join("harn-main").to_str()
                    && let Some(params) = ra_init.params.as_mut().and_then(|p| p.as_object_mut())
                {
                    let uri: String = format!("file://{}", fpath);
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
                } else {
                    info!(
                        "Failed to resolve absolute path for Harn source directory: {:?}",
                        extension_dir.join("harn-main")
                    );
                }

                let ra_msg = serde_json::to_string(&ra_init).unwrap();
                let _ = write_message(&mut ra_stdin, &ra_msg).await;
                let _ = write_message(&mut harn_stdin, &msg).await;
                continue;
            }

            if method == "initialized" {
                let _ = write_message(&mut ra_stdin, &msg).await;
            }

            // 2. Track open documents
            if method == "textDocument/didOpen" {
                if let Some(params) = rpc.params.as_ref() {
                    if let (Some(uri), Some(text)) = (
                        params.pointer("/textDocument/uri").and_then(|v| v.as_str()),
                        params
                            .pointer("/textDocument/text")
                            .and_then(|v| v.as_str()),
                    ) {
                        documents.insert(uri.to_string(), text.to_string());
                    }
                }
            }

            // 3. Track changes (Full sync required)
            if method == "textDocument/didChange" {
                if let Some(params) = rpc.params.as_ref() {
                    if let (Some(uri), Some(changes)) = (
                        params.pointer("/textDocument/uri").and_then(|v| v.as_str()),
                        params.pointer("/contentChanges").and_then(|v| v.as_array()),
                    ) {
                        if let Some(first_change) = changes.first() {
                            if let Some(text) = first_change.get("text").and_then(|v| v.as_str()) {
                                documents.insert(uri.to_string(), text.to_string());
                            }
                        }
                    }
                }
            }

            // 4. Intercept Definition
            if method == "textDocument/definition" {
                let mut intercepted = false;
                if let Some(params) = rpc.params.as_ref() {
                    if let (Some(uri), Some(line), Some(col)) = (
                        params.pointer("/textDocument/uri").and_then(|v| v.as_str()),
                        params.pointer("/position/line").and_then(|v| v.as_u64()),
                        params
                            .pointer("/position/character")
                            .and_then(|v| v.as_u64()),
                    ) {
                        if let Some(text) = documents.get(uri) {
                            if let Some(word) =
                                get_word_at_position(text, line as usize, col as usize)
                            {
                                // If the word is a Harness entity, ask rust-analyzer!
                                if word.starts_with("Harness") {
                                    intercepted = true;
                                    
                                    if let Some(id) = rpc.id.as_ref() {
                                        let mut map = pending_queries_clone2.lock().unwrap();
                                        map.insert(id.to_string(), word.clone());
                                    }
                                    
                                    // Send workspace/symbol query to RA, reusing Zed's request ID
                                    let query_msg = json!({
                                        "jsonrpc": "2.0",
                                        "id": rpc.id,
                                        "method": "workspace/symbol",
                                        "params": {
                                            "query": word
                                        }
                                    });
                                    let ra_msg = serde_json::to_string(&query_msg).unwrap();
                                    let _ = write_message(&mut ra_stdin, &ra_msg).await;
                                }
                            }
                        }
                    }
                }

                if intercepted {
                    continue; // Skip forwarding to harn-lsp
                }
            }

            // Pass everything else to harn-lsp (including didOpen/didChange)
            let _ = write_message(&mut harn_stdin, &msg).await;

            if method == "exit" {
                let _ = write_message(&mut ra_stdin, &msg).await;
                break;
            }
        }
    }

    let _ = harn_child.wait().await;
    let _ = ra_child.wait().await;
    info!("Proxy exiting");
}
