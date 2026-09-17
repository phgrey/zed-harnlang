use super::rpc::RpcMessage;
use std::str;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};

pub async fn read_message<R: AsyncRead + Unpin>(reader: &mut BufReader<R>) -> Option<RpcMessage> {
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
    serde_json::from_str::<RpcMessage>(str::from_utf8(&buf).ok()?).ok()
}

pub fn format_message(msg: &str) -> String {
    format!("Content-Length: {}\r\n\r\n{}", msg.len(), msg)
}

pub async fn write_message<W: AsyncWrite + Unpin>(
    writer: &mut W,
    msg: &RpcMessage,
) -> std::io::Result<()> {
    let json_str = serde_json::to_string(msg)?;
    let formatted = format_message(&json_str);
    writer.write_all(formatted.as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Cursor;

    #[tokio::test]
    async fn test_format_message() {
        let msg = r#"{"jsonrpc":"2.0"}"#;
        let formatted = format_message(msg);
        assert_eq!(
            formatted,
            format!("Content-Length: {}\r\n\r\n{}", msg.len(), msg)
        );
    }

    #[tokio::test]
    async fn test_write_and_read_message() {
        let mut buffer = Vec::new();
        let original_msg = RpcMessage::request(json!("req-id"), "ping", None);
        write_message(&mut buffer, &original_msg).await.unwrap();

        let mut reader = BufReader::new(Cursor::new(buffer));
        let read = read_message(&mut reader).await.unwrap();
        assert_eq!(read, original_msg);
    }

    #[tokio::test]
    async fn test_read_multiple_messages() {
        let msg1 = RpcMessage::request(json!("req-id-1"), "ping", None);
        let msg2 = RpcMessage::request(json!("req-id-2"), "pong", None);
        let s1 = serde_json::to_string(&msg1).unwrap();
        let s2 = serde_json::to_string(&msg2).unwrap();
        let raw = format!(
            "Content-Length: {}\r\n\r\n{}Content-Length: {}\r\n\r\n{}",
            s1.len(),
            s1,
            s2.len(),
            s2
        );

        let mut reader = BufReader::new(Cursor::new(raw.into_bytes()));
        assert_eq!(read_message(&mut reader).await, Some(msg1));
        assert_eq!(read_message(&mut reader).await, Some(msg2));
        assert_eq!(read_message(&mut reader).await, None);
    }

    #[tokio::test]
    async fn test_read_message_with_extra_headers() {
        let msg = RpcMessage::request(json!("req-id"), "ping", None);
        let s = serde_json::to_string(&msg).unwrap();
        let raw = format!(
            "Content-Type: application/vscode-jsonrpc; charset=utf-8\r\nContent-Length: {}\r\n\r\n{}",
            s.len(),
            s
        );

        let mut reader = BufReader::new(Cursor::new(raw.into_bytes()));
        assert_eq!(read_message(&mut reader).await, Some(msg));
    }

    #[tokio::test]
    async fn test_read_message_eof_or_empty() {
        let mut reader = BufReader::new(Cursor::new(Vec::new()));
        assert_eq!(read_message(&mut reader).await, None);

        let zero_length = "Content-Length: 0\r\n\r\n";
        let mut reader2 = BufReader::new(Cursor::new(zero_length.as_bytes()));
        assert_eq!(read_message(&mut reader2).await, None);
    }

    #[tokio::test]
    async fn test_read_message_truncated_payload() {
        let raw = "Content-Length: 20\r\n\r\nshort";
        let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));
        assert_eq!(read_message(&mut reader).await, None);
    }
}
