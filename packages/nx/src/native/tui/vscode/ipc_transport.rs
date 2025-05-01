use std::path::PathBuf;
use std::sync::Arc;

use anyhow::anyhow;
use interprocess::{
    bound_util::{RefTokioAsyncRead, RefTokioAsyncWrite},
    local_socket::{
        tokio::{prelude::*, Stream},
        GenericFilePath, ToFsName,
    },
};
use jsonrpsee::core::client::{ReceivedMessage, TransportReceiverT, TransportSenderT};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Debug, Error)]
#[error(transparent)]
pub struct IpcError(#[from] anyhow::Error);

pub struct IpcTransport {
    _stream: Arc<Stream>,
    pub reader: IpcTransportReceiver,
    pub writer: IpcTransportSender,
}
impl IpcTransport {
    pub async fn new(socket_path: PathBuf) -> Result<Self, anyhow::Error> {
        let socket_path = socket_path.to_fs_name::<GenericFilePath>()?;
        let conn = Stream::connect(socket_path).await?;
        let stream = Arc::new(conn);
        let writer = IpcTransportSender(Arc::clone(&stream));
        let reader = IpcTransportReceiver(Arc::clone(&stream));
        Ok(Self {
            _stream: stream,
            reader,
            writer,
        })
    }
}

pub struct IpcTransportSender(Arc<Stream>);
pub struct IpcTransportReceiver(Arc<Stream>);

const NEW_LINE: &str = "\r\n";

impl TransportSenderT for IpcTransportSender {
    type Error = IpcError;

    async fn send(&mut self, msg: String) -> Result<(), Self::Error> {
        let mut stream = self.0.as_tokio_async_write();
        let headers = format!("content-length: {}{}{}", msg.len(), NEW_LINE, NEW_LINE);
        stream
            .write_all(headers.as_bytes())
            .await
            .map_err(anyhow::Error::from)?;
        stream.flush().await.map_err(anyhow::Error::from)?;
        let mut msg = msg;
        msg.push_str(NEW_LINE);
        msg.push_str(NEW_LINE);
        stream
            .write_all(msg.as_bytes())
            .await
            .map_err(anyhow::Error::from)?;
        stream.flush().await.map_err(anyhow::Error::from)?;
        Ok(())
    }
}

impl TransportReceiverT for IpcTransportReceiver {
    type Error = IpcError;

    async fn receive(&mut self) -> Result<ReceivedMessage, Self::Error> {
        let mut stream = self.0.as_tokio_async_read();
        let mut response_data = Vec::new();
        let mut buffer = [0u8; 1024];

        loop {
            let bytes_read = stream
                .read(buffer.as_mut())
                .await
                .map_err(anyhow::Error::from)?;
            if bytes_read == 0 {
                break;
            }
            response_data.extend_from_slice(&buffer[..bytes_read]);

            if let Ok(response_str) = String::from_utf8(response_data.clone()) {
                if response_str.contains('\n') {
                    let parts: Vec<&str> = response_str.split('\n').collect();
                    if let Some(response_part) = parts.first() {
                        return Ok(ReceivedMessage::Text(response_part.to_string()));
                    }
                }
            }
        }

        Err(anyhow!("Failed to read from IPC stream").into())
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::time::Duration;
    use tempfile::NamedTempFile;
    use tokio::net::UnixListener;
    use tokio::task;

    #[tokio::test]
    async fn test_ipc_transport_connection() {
        let temp_file = NamedTempFile::new().unwrap();
        let socket_path = temp_file.path().to_path_buf();
        std::fs::remove_file(&socket_path).unwrap_or(());

        // Create a mock server
        let listener = UnixListener::bind(&socket_path).unwrap();

        // Connect in a separate task
        let socket_path_clone = socket_path.clone();
        let client_task = task::spawn(async move {
            // Small delay to ensure server is ready
            tokio::time::sleep(Duration::from_millis(100)).await;
            IpcTransport::new(socket_path_clone).await
        });

        // Accept the connection
        let (_, _) = listener.accept().await.unwrap();

        let result = client_task.await.unwrap();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_transport_sender_send() {
        let temp_file = NamedTempFile::new().unwrap();
        let socket_path = temp_file.path().to_path_buf();
        std::fs::remove_file(&socket_path).unwrap_or(());

        let listener = UnixListener::bind(&socket_path).unwrap();

        // Start client in background
        let socket_path_clone = socket_path.clone();
        let client_task = task::spawn(async move {
            let mut transport = IpcTransport::new(socket_path_clone).await.unwrap();
            transport.writer.send("test message".to_string()).await
        });

        // Accept the connection and read the message
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 1024];
        let n = socket.read(&mut buf).await.unwrap();
        let received = String::from_utf8_lossy(&buf[0..n]);

        assert!(received.contains("content-length: 12"));

        let result = client_task.await.unwrap();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_transport_receiver_receive() {
        let temp_file = NamedTempFile::new().unwrap();
        let socket_path = temp_file.path().to_path_buf();
        std::fs::remove_file(&socket_path).unwrap_or(());

        let listener = UnixListener::bind(&socket_path).unwrap();

        // Start client in background
        let socket_path_clone = socket_path.clone();
        let client_task = task::spawn(async move {
            let mut transport = IpcTransport::new(socket_path_clone).await.unwrap();
            transport.reader.receive().await
        });

        // Accept connection and send a message
        let (mut socket, _) = listener.accept().await.unwrap();
        let message = "test\nresponse";
        socket.write_all(message.as_bytes()).await.unwrap();
        socket.flush().await.unwrap();

        let result = client_task.await.unwrap();
        assert!(result.is_ok());
        if let Ok(ReceivedMessage::Text(text)) = result {
            assert_eq!(text, "test");
        } else {
            panic!("Expected Text message");
        }
    }
}

#[cfg(all(test, windows))]
mod tests_windows {
    use super::*;
    use std::time::Duration;
    use tokio::net::windows::named_pipe::ServerOptions;
    use tokio::task;

    #[tokio::test]
    async fn test_ipc_transport_connection() {
        let pipe_name = format!(r"\\.\pipe\test-{}", uuid::Uuid::new_v4());

        // Create a mock server
        let server = ServerOptions::new()
            .first_pipe_instance(true)
            .create(&pipe_name)
            .unwrap();

        // Connect in a separate task
        let pipe_name_clone = pipe_name.clone();
        let client_task = task::spawn(async move {
            // Small delay to ensure server is ready
            tokio::time::sleep(Duration::from_millis(100)).await;
            IpcTransport::new(pipe_name_clone.into()).await
        });

        // Accept the connection
        server.connect().await.unwrap();

        let result = client_task.await.unwrap();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_transport_sender_send() {
        let pipe_name = format!(r"\\.\pipe\test-{}", uuid::Uuid::new_v4());

        // Create a mock server
        let mut server = ServerOptions::new()
            .first_pipe_instance(true)
            .create(&pipe_name)
            .unwrap();

        // Start client in background
        let pipe_name_clone = pipe_name.clone();
        let client_task = task::spawn(async move {
            let mut transport = IpcTransport::new(pipe_name_clone.into()).await.unwrap();
            transport.writer.send("test message".to_string()).await
        });

        // Accept connection and read the message
        server.connect().await.unwrap();
        let mut buf = [0u8; 1024];
        let n = server.read(&mut buf).await.unwrap();
        let received = String::from_utf8_lossy(&buf[0..n]);

        assert!(received.contains("content-length: 12"));

        let result = client_task.await.unwrap();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_transport_receiver_receive() {
        let pipe_name = format!(r"\\.\pipe\test-{}", uuid::Uuid::new_v4());

        // Create a mock server
        let mut server = ServerOptions::new()
            .first_pipe_instance(true)
            .create(&pipe_name)
            .unwrap();

        // Start client in background
        let pipe_name_clone = pipe_name.clone();
        let client_task = task::spawn(async move {
            let mut transport = IpcTransport::new(pipe_name_clone.into()).await.unwrap();
            transport.reader.receive().await
        });

        // Accept connection and send a message
        server.connect().await.unwrap();
        let message = "test\nresponse";
        server.write_all(message.as_bytes()).await.unwrap();
        server.flush().await.unwrap();

        let result = client_task.await.unwrap();
        assert!(result.is_ok());
        if let Ok(ReceivedMessage::Text(text)) = result {
            assert_eq!(text, "test");
        } else {
            panic!("Expected Text message");
        }
    }
}
