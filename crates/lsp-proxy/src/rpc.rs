use crate::config::extension_path;
use crate::text::rank_symbol_locations;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct RpcMessage {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,
}

impl RpcMessage {
    pub fn new(id: Option<Value>, method: Option<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            method,
            params,
            result: None,
            error: None,
        }
    }

    pub fn request(id: Value, method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            method: Some(method.into()),
            params,
            result: None,
            error: None,
        }
    }

    pub fn response(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            method: None,
            params: None,
            result: Some(result),
            error: None,
        }
    }

    pub fn notification(method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: Some(method.into()),
            params,
            result: None,
            error: None,
        }
    }

    pub fn error(id: Option<Value>, code: i64, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            method: None,
            params: None,
            result: None,
            error: Some(json!({
                "code": code,
                "message": message.into(),
            })),
        }
    }

    pub fn clone_second(&self) -> Option<Self> {
        match self.method.as_deref() {
            Some("initialize") => {
                let ext_path = extension_path();
                let uri = format!("file://{}/harn-main", ext_path);
                let root_path = format!("{}/harn-main", ext_path);
                let mut cloned = self.clone();
                let params = cloned.params.as_mut()?.as_object_mut()?;
                params.insert("rootUri".to_string(), json!(uri));
                params.insert("rootPath".to_string(), json!(root_path));
                params.insert(
                    "workspaceFolders".to_string(),
                    json!([{"uri": uri, "name": "harn-main"}]),
                );
                Some(cloned)
            }
            Some("initialized") => Some(self.clone()),
            Some("exit") => Some(self.clone()),
            _ => None,
        }
    }

    pub fn is_empty_result(&self) -> bool {
        match self.result.as_ref() {
            None | Some(Value::Null) => true,
            Some(Value::Array(arr)) => arr.is_empty(),
            _ => false,
        }
    }

    /// Rewrites a `workspace/symbol` response from rust-analyzer into a ranked list of definition Locations for Zed.
    pub fn into_symbol_response(mut self, word: &str) -> Self {
        let locations = self
            .result
            .as_ref()
            .and_then(|r| r.as_array())
            .map(|arr| rank_symbol_locations(arr, word))
            .unwrap_or_default();

        self.result = if locations.is_empty() {
            Some(Value::Null)
        } else {
            Some(Value::Array(locations))
        };
        self
    }

    /// Safe identifier string extraction for both numeric and string IDs.
    /// Returns `None` for notifications (which have no ID).
    pub fn id_str(&self) -> Option<String> {
        self.id.as_ref().map(|v| match v {
            Value::String(val) => val.clone(),
            other => other.to_string(),
        })
    }

    pub fn idd(&self) -> Option<String> {
        self.id_str()
    }

    pub fn to_json_string(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn from_json_str(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rpc_request_serialization() {
        let req = RpcMessage::request(json!(1), "initialize", Some(json!({"processId": 123})));
        let json_str = req.to_json_string().unwrap();
        assert!(json_str.contains("\"jsonrpc\":\"2.0\""));
        assert!(json_str.contains("\"id\":1"));
        assert!(json_str.contains("\"method\":\"initialize\""));

        let deserialized = RpcMessage::from_json_str(&json_str).unwrap();
        assert_eq!(req, deserialized);
    }

    #[test]
    fn test_rpc_response_serialization() {
        let resp = RpcMessage::response(json!("req-42"), json!({"capabilities": {}}));
        let json_str = resp.to_json_string().unwrap();
        assert!(json_str.contains("\"id\":\"req-42\""));
        assert!(json_str.contains("\"result\":{\"capabilities\":{}}"));
        assert!(!json_str.contains("\"error\""));

        let deserialized = RpcMessage::from_json_str(&json_str).unwrap();
        assert_eq!(resp, deserialized);
    }

    #[test]
    fn test_rpc_notification() {
        let notif = RpcMessage::notification("initialized", None);
        assert!(notif.id.is_none());
        assert_eq!(notif.method.as_deref(), Some("initialized"));
        let json_str = notif.to_json_string().unwrap();
        assert!(!json_str.contains("\"id\""));
    }

    #[test]
    fn test_rpc_error() {
        let err = RpcMessage::error(Some(json!(5)), -32600, "Invalid Request");
        assert_eq!(err.id, Some(json!(5)));
        let json_str = err.to_json_string().unwrap();
        assert!(json_str.contains("\"code\":-32600"));
        assert!(json_str.contains("\"message\":\"Invalid Request\""));
    }

    #[test]
    fn test_rpc_invalid_json() {
        assert!(RpcMessage::from_json_str("not a json").is_err());
    }

    #[test]
    fn test_rpc_new_constructor() {
        let msg = RpcMessage::new(
            Some(json!(99)),
            Some("custom/method".to_string()),
            Some(json!({"a": 1})),
        );
        assert_eq!(msg.jsonrpc, "2.0");
        assert_eq!(msg.id, Some(json!(99)));
        assert_eq!(msg.method.as_deref(), Some("custom/method"));
        assert_eq!(msg.params, Some(json!({"a": 1})));
    }

    #[test]
    fn test_id_str_variants() {
        let num_id = RpcMessage::request(json!(123), "test", None);
        assert_eq!(num_id.id_str(), Some("123".to_string()));

        let str_id = RpcMessage::request(json!("req-abc"), "test", None);
        assert_eq!(str_id.id_str(), Some("req-abc".to_string()));

        let notif = RpcMessage::notification("test", None);
        assert_eq!(notif.id_str(), None);
    }

    #[test]
    fn test_is_empty_result() {
        let null_resp = RpcMessage::response(json!(1), json!(null));
        assert!(null_resp.is_empty_result());

        let empty_arr = RpcMessage::response(json!(1), json!([]));
        assert!(empty_arr.is_empty_result());

        let non_empty = RpcMessage::response(json!(1), json!([{"uri": "file:///a"}]));
        assert!(!non_empty.is_empty_result());

        let scalar = RpcMessage::response(json!(1), json!({"uri": "file:///a"}));
        assert!(!scalar.is_empty_result());
    }

    #[test]
    fn test_into_symbol_response() {
        let ra_resp = RpcMessage::response(
            json!(1),
            json!([
                {
                    "name": "Harness",
                    "kind": 22,
                    "location": {"uri": "file:///path/to/harn.rs"}
                }
            ]),
        );

        let zed_resp = ra_resp.into_symbol_response("Harness");
        assert_eq!(
            zed_resp.result,
            Some(json!([{"uri": "file:///path/to/harn.rs"}]))
        );
    }

    #[test]
    fn test_into_symbol_response_not_found() {
        let ra_resp = RpcMessage::response(json!(1), json!([]));
        let zed_resp = ra_resp.into_symbol_response("Harness");
        assert_eq!(zed_resp.result, Some(json!(null)));
    }

    #[test]
    fn test_clone_second() {
        let init_req = RpcMessage::request(
            json!(1),
            "initialize",
            Some(json!({
                "processId": 1234,
                "rootUri": "file:///original",
                "capabilities": {}
            })),
        );
        let second = init_req.clone_second();
        assert!(second.is_some());
        let sec = second.unwrap();
        assert_eq!(sec.method.as_deref(), Some("initialize"));
        let sec_params = sec.params.unwrap();
        assert!(
            sec_params["rootUri"]
                .as_str()
                .unwrap()
                .ends_with("/harn-main")
        );

        let notif_init = RpcMessage::notification("initialized", None);
        assert_eq!(notif_init.clone_second(), Some(notif_init.clone()));

        let notif_exit = RpcMessage::notification("exit", None);
        assert_eq!(notif_exit.clone_second(), Some(notif_exit.clone()));

        let text_change = RpcMessage::notification("textDocument/didChange", None);
        assert_eq!(text_change.clone_second(), None);
    }
}
