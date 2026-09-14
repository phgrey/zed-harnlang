use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};

pub async fn read_message<R: AsyncRead + Unpin>(reader: &mut BufReader<R>) -> Option<String> {
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

pub fn format_message(msg: &str) -> String {
    format!("Content-Length: {}\r\n\r\n{}", msg.len(), msg)
}

pub async fn write_message<W: AsyncWrite + Unpin>(writer: &mut W, msg: &str) -> std::io::Result<()> {
    let payload = format_message(msg);
    writer.write_all(payload.as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let original_msg = r#"{"jsonrpc":"2.0","method":"ping"}"#;
        write_message(&mut buffer, original_msg).await.unwrap();

        let mut reader = BufReader::new(Cursor::new(buffer));
        let read = read_message(&mut reader).await.unwrap();
        assert_eq!(read, original_msg);
    }

    #[tokio::test]
    async fn test_read_multiple_messages() {
        let msg1 = r#"{"msg":1}"#;
        let msg2 = r#"{"msg":2}"#;
        let raw = format!(
            "Content-Length: {}\r\n\r\n{}Content-Length: {}\r\n\r\n{}",
            msg1.len(),
            msg1,
            msg2.len(),
            msg2
        );

        let mut reader = BufReader::new(Cursor::new(raw.into_bytes()));
        assert_eq!(read_message(&mut reader).await.as_deref(), Some(msg1));
        assert_eq!(read_message(&mut reader).await.as_deref(), Some(msg2));
        assert_eq!(read_message(&mut reader).await, None);
    }

    #[tokio::test]
    async fn test_read_message_with_extra_headers() {
        let msg = r#"{"hello":"world"}"#;
        let raw = format!(
            "Content-Type: application/vscode-jsonrpc; charset=utf-8\r\nContent-Length: {}\r\n\r\n{}",
            msg.len(),
            msg
        );

        let mut reader = BufReader::new(Cursor::new(raw.into_bytes()));
        assert_eq!(read_message(&mut reader).await.as_deref(), Some(msg));
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
        // Declares 20 bytes, but stream ends after 5 bytes
        let raw = "Content-Length: 20\r\n\r\nshort";
        let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));
        assert_eq!(read_message(&mut reader).await, None);
    }
}
