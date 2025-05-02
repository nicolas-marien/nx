use std::sync::Arc;

use tracing::trace;

use jsonrpsee::{
    async_client::{Client, ClientBuilder},
    proc_macros::rpc,
};

use crate::native::{
    tui::nx_console::ipc_transport::IpcTransport,
    utils::socket_path::get_full_nx_console_socket_path,
};

#[rpc(client, namespace = "nx", namespace_separator = "/")]
pub trait ConsoleRpc {
    #[method(name = "terminalMessage")]
    fn terminal_message(&self, text: String);
}

pub struct NxConsoleMessageConnection {
    client: Arc<Client>,
}

impl NxConsoleMessageConnection {
    pub async fn new(workspace_root: &str) -> Option<Self> {
        let socket_path = get_full_nx_console_socket_path(workspace_root);
        let client = IpcTransport::new(socket_path)
            .await
            .map(|transport| {
                ClientBuilder::new().build_with_tokio(transport.writer, transport.reader)
            })
            .inspect_err(|e| {
                trace!("Could not connect to Nx Console: {}", e);
            })
            .ok()
            .map(Arc::new)?;

        Some(Self { client })
    }

    pub fn send_terminal_string(&self, message: impl Into<String>) {
        let message = message.into();
        let client = self.client.clone();
        tokio::spawn(async move {
            if let Err(e) = client.terminal_message(message).await {
                trace!("Failed to send terminal message: {}", e);
            }
        });
    }
}
