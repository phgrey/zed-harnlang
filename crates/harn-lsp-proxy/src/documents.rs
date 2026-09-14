use serde_json::Value;
use std::collections::HashMap;

#[derive(Default, Debug, Clone)]
pub struct DocumentStore {
    documents: HashMap<String, String>,
}

impl DocumentStore {
    pub fn new() -> Self {
        Self {
            documents: HashMap::new(),
        }
    }

    pub fn insert(&mut self, uri: impl Into<String>, text: impl Into<String>) {
        self.documents.insert(uri.into(), text.into());
    }

    pub fn get(&self, uri: &str) -> Option<&str> {
        self.documents.get(uri).map(|s| s.as_str())
    }

    pub fn remove(&mut self, uri: &str) -> Option<String> {
        self.documents.remove(uri)
    }

    pub fn len(&self) -> usize {
        self.documents.len()
    }

    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }

    pub fn handle_did_open(&mut self, params: Option<&Value>) -> bool {
        if let Some(params) = params {
            if let (Some(uri), Some(text)) = (
                params.pointer("/textDocument/uri").and_then(|v| v.as_str()),
                params.pointer("/textDocument/text").and_then(|v| v.as_str()),
            ) {
                self.documents.insert(uri.to_string(), text.to_string());
                return true;
            }
        }
        false
    }

    pub fn handle_did_change(&mut self, params: Option<&Value>) -> bool {
        if let Some(params) = params {
            if let (Some(uri), Some(changes)) = (
                params.pointer("/textDocument/uri").and_then(|v| v.as_str()),
                params.pointer("/contentChanges").and_then(|v| v.as_array()),
            ) {
                if let Some(first_change) = changes.first() {
                    if let Some(text) = first_change.get("text").and_then(|v| v.as_str()) {
                        self.documents.insert(uri.to_string(), text.to_string());
                        return true;
                    }
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_document_store_basic() {
        let mut store = DocumentStore::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);

        store.insert("file:///a.harn", "let x = 1;");
        assert_eq!(store.len(), 1);
        assert!(!store.is_empty());
        assert_eq!(store.get("file:///a.harn"), Some("let x = 1;"));

        assert_eq!(store.remove("file:///a.harn"), Some("let x = 1;".to_string()));
        assert!(store.is_empty());
    }

    #[test]
    fn test_handle_did_open() {
        let mut store = DocumentStore::new();
        let valid_params = json!({
            "textDocument": {
                "uri": "file:///test.harn",
                "text": "fn main() { Harness::init(); }"
            }
        });

        assert!(store.handle_did_open(Some(&valid_params)));
        assert_eq!(
            store.get("file:///test.harn"),
            Some("fn main() { Harness::init(); }")
        );

        let invalid_params = json!({
            "textDocument": {
                "uri": "file:///test2.harn"
            }
        });
        assert!(!store.handle_did_open(Some(&invalid_params)));
        assert!(!store.handle_did_open(None));
    }

    #[test]
    fn test_handle_did_change() {
        let mut store = DocumentStore::new();
        store.insert("file:///test.harn", "old text");

        let change_params = json!({
            "textDocument": {
                "uri": "file:///test.harn"
            },
            "contentChanges": [
                {
                    "text": "updated text"
                }
            ]
        });

        assert!(store.handle_did_change(Some(&change_params)));
        assert_eq!(store.get("file:///test.harn"), Some("updated text"));

        let empty_changes = json!({
            "textDocument": {
                "uri": "file:///test.harn"
            },
            "contentChanges": []
        });
        assert!(!store.handle_did_change(Some(&empty_changes)));
    }
}
