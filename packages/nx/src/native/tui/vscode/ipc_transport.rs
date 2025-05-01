use std::io::{Read, Write};
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
