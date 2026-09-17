use super::transport::write_message;
use crate::rpc::RpcMessage;
use std::process::Stdio;
use tokio::io::AsyncWrite;
use tokio::process::{ChildStdin, ChildStdout, Command};
use tokio::sync::mpsc::{Sender, channel};

pub struct Server {
    pub stdin: ChildStdin,
    pub stdout: ChildStdout,
}

/* Struct for running underlying servers */
impl Server {
    pub fn lsp(cmd: &str) -> Self {
        let mut process = Command::new(cmd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap_or_else(|e| panic!("Failed to start {}: {}", cmd, e));

        Self {
            stdin: process.stdin.take().unwrap(),
            stdout: process.stdout.take().unwrap(),
        }
    }
}

pub fn tx_for_std<T>(mut stdout: T) -> Sender<RpcMessage>
where
    T: AsyncWrite + Unpin + Send + 'static,
{
    let (tx_to_out, mut rx_to_out) = channel::<RpcMessage>(32);

    tokio::spawn(async move {
        while let Some(msg) = rx_to_out.recv().await {
            let _ = write_message(&mut stdout, &msg).await;
        }
    });
    tx_to_out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn test_tx_for_std_channel_writes() {
        let buffer = Arc::new(Mutex::new(Vec::<u8>::new()));
        let writer = TestWriter {
            buf: buffer.clone(),
        };

        let tx = tx_for_std(writer);
        let msg = RpcMessage::request(json!(1), "test", None);
        tx.send(msg.clone()).await.unwrap();

        // Drop tx so the writer task finishes
        drop(tx);
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let locked = buffer.lock().await;
        let written = String::from_utf8(locked.clone()).unwrap();
        assert!(written.contains("Content-Length:"));
        assert!(written.contains("\"method\":\"test\""));
    }

    struct TestWriter {
        buf: Arc<Mutex<Vec<u8>>>,
    }

    impl tokio::io::AsyncWrite for TestWriter {
        fn poll_write(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            buf: &[u8],
        ) -> std::task::Poll<std::io::Result<usize>> {
            let mut guard = self.buf.try_lock().unwrap();
            guard.extend_from_slice(buf);
            std::task::Poll::Ready(Ok(buf.len()))
        }

        fn poll_flush(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            std::task::Poll::Ready(Ok(()))
        }

        fn poll_shutdown(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            std::task::Poll::Ready(Ok(()))
        }
    }
}
