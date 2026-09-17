use log::info;
use lsp_proxy::rpc::RpcMessage;
use lsp_proxy::server::{Server, tx_for_std};
use lsp_proxy::text::word_at_cursor;
use lsp_proxy::transport::read_message;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::io::BufReader;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("lsp-proxy initialized");

    let ha_lsp = Server::lsp("harn-lsp");
    let ra_lsp = Server::lsp("rust-analyzer");

    let tx_to_zed = tx_for_std(tokio::io::stdout());
    let tx_to_harn = tx_for_std(ha_lsp.stdin);
    let tx_to_ra = tx_for_std(ra_lsp.stdin);

    // Track original definition request params: id -> params
    let pending_harn: Arc<Mutex<HashMap<String, Value>>> = Arc::new(Mutex::new(HashMap::new()));
    // Track forwarded definition requests to RA: id -> word
    let pending_ra: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));

    // 1. Harn LSP stdout -> Zed (or fallback to RA if definition is empty)
    let tx_to_zed_for_harn = tx_to_zed.clone();
    let tx_to_ra_for_harn = tx_to_ra.clone();
    let pending_harn_clone = pending_harn.clone();
    let pending_ra_clone = pending_ra.clone();
    let harn_task = tokio::spawn(async move {
        let mut stdout = BufReader::new(ha_lsp.stdout);

        while let Some(msg) = read_message(&mut stdout).await {
            let id_opt = msg.id_str();

            // Check if this is an empty definition result that should fall back to RA
            if msg.is_empty_result()
                && let Some(ref id) = id_opt
            {
                let word = {
                    let pend = pending_harn_clone.lock().unwrap();
                    pend.get(id).and_then(word_at_cursor)
                };

                match word {
                    Some(word) => {
                        info!(
                            "Harn empty result for req {id}, falling back to RA for [{word}] - {msg:?}"
                        );
                        {
                            let mut map = pending_ra_clone.lock().unwrap();
                            map.insert(id.clone(), word.clone());
                        }
                        let _ = tx_to_ra_for_harn
                            .send(RpcMessage::symbol_request(msg.id.unwrap_or_default(), word))
                            .await;
                        continue; // Do NOT send empty response to Zed yet
                    }
                    None => {
                        let has_pending = pending_harn_clone.lock().unwrap().contains_key(id);
                        info!(
                            "Harn empty result for req {id} (saved params present: {has_pending}), but could not extract word at cursor"
                        );
                    }
                }
            }

            // Clean up from pending_harn if this request is finished
            if let Some(ref id) = id_opt {
                let mut pend = pending_harn_clone.lock().unwrap();
                pend.remove(id);
            }

            let _ = tx_to_zed_for_harn.send(msg).await;
        }
    });

    // 2. Rust Analyzer stdout -> Zed (only for fallback symbol responses)
    let tx_to_zed_for_ra = tx_to_zed.clone();
    let pending_ra_clone2 = pending_ra.clone();
    let ra_task = tokio::spawn(async move {
        let mut stdout = BufReader::new(ra_lsp.stdout);

        while let Some(msg) = read_message(&mut stdout).await {
            if let Some(ref id) = msg.id_str() {
                let queried_word = {
                    let mut map = pending_ra_clone2.lock().unwrap();
                    map.remove(id)
                };

                if let Some(word) = queried_word {
                    let out_msg = msg.into_symbol_response(&word);
                    info!("got from ra for [{word}] {out_msg:?}");
                    let _ = tx_to_zed_for_ra.send(out_msg).await;
                }
            }
        }
    });

    // 3. Zed stdin -> Harn LSP (and mirror initialize/exit to RA)
    let mut stdin = BufReader::new(tokio::io::stdin());
    while let Some(msg) = read_message(&mut stdin).await {
        if let Some(msg2) = msg.clone_second() {
            let _ = tx_to_ra.send(msg2).await;
        }
        if let Some(id) = msg.id_str()
            && let Some(params) = msg.params.clone()
        {
            let mut pend = pending_harn.lock().unwrap();
            pend.insert(id, params);
        }
        let _ = tx_to_harn.send(msg).await;
    }

    // Zed disconnected (EOF on stdin). Wait for tasks.
    let _ = harn_task.await;
    let _ = ra_task.await;
}
