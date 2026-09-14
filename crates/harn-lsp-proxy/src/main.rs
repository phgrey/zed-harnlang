use harn_lsp_proxy::{
    build_ra_initialize_request, check_definition_request, read_message,
    resolve_harn_lsp_command, resolve_rust_analyzer_command, rewrite_initialize_response,
    rewrite_symbol_response, write_message, DefinitionIntercept, DocumentStore, RpcMessage,
    build_ra_symbol_query,
};
use log::info;
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use tokio::io::BufReader;
use tokio::process::Command;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    env_logger::init();
    info!("Starting harn-lsp-proxy");

    let current_exe = env::current_exe().expect("Failed to get current executable path");
    let extension_dir = current_exe.parent().unwrap(); // Go up from proxy-bin to extension root

    let harn_lsp_cmd = resolve_harn_lsp_command(extension_dir);
    let mut harn_child = Command::new(&harn_lsp_cmd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("Failed to start harn-lsp");
    let mut harn_stdin = harn_child.stdin.take().unwrap();
    let harn_stdout = harn_child.stdout.take().unwrap();

    let ra_cmd = resolve_rust_analyzer_command(extension_dir);
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

    let (tx_to_ra, mut rx_to_ra) = mpsc::channel::<String>(32);
    let tx_to_ra_clone1 = tx_to_ra.clone();

    let (tx_to_harn, mut rx_to_harn) = mpsc::channel::<String>(32);

    tokio::spawn(async move {
        while let Some(msg) = rx_to_ra.recv().await {
            let _ = write_message(&mut ra_stdin, &msg).await;
        }
    });

    tokio::spawn(async move {
        while let Some(msg) = rx_to_harn.recv().await {
            let _ = write_message(&mut harn_stdin, &msg).await;
        }
    });

    let pending_harn_defs: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));
    let pending_harn_defs_clone1 = pending_harn_defs.clone();
    let pending_harn_defs_clone2 = pending_harn_defs.clone();

    let pending_ra_defs: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));
    let pending_ra_defs_clone1 = pending_ra_defs.clone();
    let pending_ra_defs_clone2 = pending_ra_defs.clone();

    let mut documents = DocumentStore::new();

    // Read from harn-lsp -> Zed
    tokio::spawn(async move {
        let mut reader = BufReader::new(harn_stdout);
        while let Some(msg) = read_message(&mut reader).await {
            if let Ok(rpc) = serde_json::from_str::<RpcMessage>(&msg) {
                // Check if this is a response to a tracked definition request
                if let Some(id) = rpc.id.as_ref() {
                    let id_str = id.to_string();
                    let queried_word = {
                        let mut map = pending_harn_defs_clone1.lock().unwrap();
                        map.remove(&id_str)
                    };

                    if let Some(word) = queried_word {
                        let is_empty = match rpc.result.as_ref() {
                            None | Some(Value::Null) => true,
                            Some(Value::Array(arr)) if arr.is_empty() => true,
                            _ => false,
                        };

                        if is_empty {
                            // Harn-lsp didn't find it. Fallback to RA!
                            {
                                let mut map = pending_ra_defs_clone1.lock().unwrap();
                                map.insert(id_str, word.clone());
                            }
                            let query_msg = build_ra_symbol_query(id, &word);
                            let _ = tx_to_ra_clone1.send(query_msg).await;
                            continue; // Do NOT send the empty response to Zed yet
                        }
                    }
                }
            }
            
            let out_msg = rewrite_initialize_response(&msg);
            let _ = tx_to_zed_clone1.send(out_msg).await;
        }
    });

    // Read from rust-analyzer -> Zed
    tokio::spawn(async move {
        let mut reader = BufReader::new(ra_stdout);
        while let Some(msg) = read_message(&mut reader).await {
            if let Ok(rpc) = serde_json::from_str::<RpcMessage>(&msg) {
                if let Some(id) = rpc.id.as_ref() {
                    let id_str = id.to_string();
                    let queried_word = {
                        let mut map = pending_ra_defs_clone2.lock().unwrap();
                        map.remove(&id_str)
                    };

                    if let Some(word) = queried_word {
                        let out_msg = rewrite_symbol_response(&rpc, &word);
                        let _ = tx_to_zed_clone2.send(out_msg).await;
                        continue;
                    }
                }
            }
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
                if let Some(ra_msg) = build_ra_initialize_request(&msg, extension_dir) {
                    let _ = tx_to_ra.send(ra_msg).await;
                } else {
                    info!(
                        "Failed to resolve absolute path for Harn source directory: {:?}",
                        extension_dir.join("harn-main")
                    );
                }

                let _ = tx_to_harn.send(msg).await;
                continue;
            }

            if method == "initialized" {
                let _ = tx_to_ra.send(msg.clone()).await;
            }

            // 2. Track open documents
            if method == "textDocument/didOpen" {
                documents.handle_did_open(rpc.params.as_ref());
            }

            // 3. Track changes (Full sync required)
            if method == "textDocument/didChange" {
                documents.handle_did_change(rpc.params.as_ref());
            }

            // 4. Intercept Definition
            if let DefinitionIntercept::Tracked { id, word } = check_definition_request(&rpc, &documents) {
                {
                    let mut map = pending_harn_defs_clone2.lock().unwrap();
                    map.insert(id.to_string(), word);
                }
            }

            // Pass everything else to harn-lsp (including didOpen/didChange and tracked definitions)
            let _ = tx_to_harn.send(msg.clone()).await;

            if method == "exit" {
                let _ = tx_to_ra.send(msg).await;
                break;
            }
        }
    }

    let _ = harn_child.wait().await;
    let _ = ra_child.wait().await;
    info!("Proxy exiting");
}
