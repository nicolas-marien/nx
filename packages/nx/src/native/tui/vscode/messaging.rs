use jsonrpsee::async_client::ClientBuilder;
use jsonrpsee::proc_macros::rpc;

use crate::native::tui::vscode::socket_path::get_full_nx_console_socket_path;

use super::ipc_transport::IpcTransport;

#[rpc(client, namespace = "nx", namespace_separator = "/")]
pub trait VscodeMessaging {
    #[method(name = "terminalMessage")]
    fn terminal_message(&self, text: String);
}

pub async fn send_terminal_message(
    message: impl Into<String>,
    workspace_root: impl AsRef<str>,
) -> Result<(), anyhow::Error> {
    let transport =
        IpcTransport::new(get_full_nx_console_socket_path(workspace_root.as_ref())).await?;
    let client = ClientBuilder::new().build_with_tokio(transport.writer, transport.reader);

    client.terminal_message(message.into()).await?;
    Ok(())
}
