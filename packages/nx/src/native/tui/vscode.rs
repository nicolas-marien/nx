mod ipc_transport;
mod messaging;
mod socket_path;

use tracing::trace;

use crate::native::tui::vscode::messaging::send_terminal_message;

pub fn send_vscode_message(
    text: impl Into<String> + Send + 'static,
    workspace_root: impl AsRef<str> + Send + 'static,
) -> Result<(), anyhow::Error> {
    if !is_vscode_terminal() {
        return Ok(());
    }
    tokio::spawn(async move {
        if let Err(e) = send_terminal_message(text, workspace_root).await {
            trace!("Error sending message to VSCode terminal: {}", e);
        }
    });

    Ok(())
}

pub fn is_vscode_terminal() -> bool {
    std::env::var("TERM_PROGRAM").is_ok_and(|v| v == "vscode")
}
