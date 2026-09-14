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
}

#[test]
fn test_rpc_new_constructor() {
    let msg = RpcMessage::new(Some(json!(99)), Some("custom/method".to_string()), Some(json!({"a": 1})));
    assert_eq!(msg.jsonrpc, "2.0");
    assert_eq!(msg.id, Some(json!(99)));
    assert_eq!(msg.method.as_deref(), Some("custom/method"));
    assert_eq!(msg.params, Some(json!({"a": 1})));
}
