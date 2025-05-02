use std::sync::OnceLock;

mod ipc_transport;
pub mod messaging;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SupportedEditor {
    VSCode,
    Cursor,
    Windsurf,
    JetBrains,
    Unknown,
}

static SUPPORTED_EDITORS: &[(&str, SupportedEditor)] = &[
    ("vscode", SupportedEditor::VSCode),
    ("cursor", SupportedEditor::Cursor),
    ("windsurf", SupportedEditor::Windsurf),
    ("jetbrains", SupportedEditor::JetBrains),
];

static CURRENT_EDITOR: OnceLock<SupportedEditor> = OnceLock::new();

pub fn get_editor() -> &'static SupportedEditor {
    CURRENT_EDITOR.get_or_init(detect_editor)
}

fn detect_editor() -> SupportedEditor {
    let term_editor = if let Ok(term) = std::env::var("TERM_PROGRAM") {
        let term_lower = term.to_lowercase();
        SUPPORTED_EDITORS
            .iter()
            .find(|&&(name, _)| term_lower == name)
            .map(|(_, editor)| editor.clone())
    } else {
        None
    };

    // If TERM_PROGRAM is not found or is not recognized, return Unknown
    let Some(term_ed) = term_editor else {
        return SupportedEditor::Unknown;
    };

    // For JetBrains, we don't need any additional checks
    if matches!(term_ed, SupportedEditor::JetBrains) {
        return term_ed;
    }

    if matches!(term_ed, SupportedEditor::VSCode) {
        if let Ok(askpass_node) = std::env::var("VSCODE_GIT_ASKPASS_NODE") {
            let askpass_lower = askpass_node.to_lowercase();

            if askpass_lower.contains("cursor") {
                return SupportedEditor::Cursor;
            } else if askpass_lower.contains("windsurf") {
                return SupportedEditor::Windsurf;
            } else {
                return SupportedEditor::VSCode;
            }
        } else {
            return term_ed;
        }
    }

    SupportedEditor::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn setup() {
        // Clear relevant environment variables before each test
        env::remove_var("TERM_PROGRAM");
        env::remove_var("VSCODE_GIT_ASKPASS_NODE");
    }

    #[test]
    fn test_detect_vscode() {
        setup();
        env::set_var("TERM_PROGRAM", "vscode");
        env::set_var("VSCODE_GIT_ASKPASS_NODE", "some/path/with/vscode/in/it");
        assert_eq!(detect_editor(), SupportedEditor::VSCode);
    }

    #[test]
    fn test_detect_cursor() {
        setup();
        env::set_var("TERM_PROGRAM", "vscode");
        env::set_var("VSCODE_GIT_ASKPASS_NODE", "some/path/with/cursor/in/it");
        assert_eq!(detect_editor(), SupportedEditor::Cursor);
    }

    #[test]
    fn test_detect_windsurf() {
        setup();
        env::set_var("TERM_PROGRAM", "vscode");
        env::set_var("VSCODE_GIT_ASKPASS_NODE", "some/path/with/windsurf/in/it");
        assert_eq!(detect_editor(), SupportedEditor::Windsurf);
    }

    #[test]
    fn test_detect_jetbrains() {
        setup();
        env::set_var("TERM_PROGRAM", "jetbrains");
        assert_eq!(detect_editor(), SupportedEditor::JetBrains);
    }

    #[test]
    fn test_term_program_missing() {
        setup();
        assert_eq!(detect_editor(), SupportedEditor::Unknown);
    }

    #[test]
    fn test_term_program_unknown() {
        setup();
        env::set_var("TERM_PROGRAM", "some-unknown-editor");
        assert_eq!(detect_editor(), SupportedEditor::Unknown);
    }

    #[test]
    fn test_vscode_without_askpass_confirmation() {
        setup();
        env::set_var("TERM_PROGRAM", "vscode");
        // No VSCODE_GIT_ASKPASS_NODE set or doesn't contain "vscode"
        assert_eq!(detect_editor(), SupportedEditor::VSCode);
    }

    #[test]
    fn test_vscode_with_wrong_askpass() {
        setup();
        env::set_var("TERM_PROGRAM", "vscode");
        env::set_var(
            "VSCODE_GIT_ASKPASS_NODE",
            "some/path/with/no/matching/editor",
        );
        assert_eq!(detect_editor(), SupportedEditor::VSCode);
    }

    #[test]
    fn test_case_insensitivity() {
        setup();
        env::set_var("TERM_PROGRAM", "VSCode");
        env::set_var("VSCODE_GIT_ASKPASS_NODE", "some/path/with/VSCODE/in/it");
        assert_eq!(detect_editor(), SupportedEditor::VSCode);
    }

    #[test]
    fn test_cursor_without_askpass_confirmation() {
        setup();
        env::set_var("TERM_PROGRAM", "cursor");
        // No VSCODE_GIT_ASKPASS_NODE set
        assert_eq!(detect_editor(), SupportedEditor::Unknown);
    }

    #[test]
    fn test_cursor_with_wrong_askpass() {
        setup();
        env::set_var("TERM_PROGRAM", "cursor");
        env::set_var(
            "VSCODE_GIT_ASKPASS_NODE",
            "some/path/with/no/matching/editor",
        );
        assert_eq!(detect_editor(), SupportedEditor::Unknown);
    }

    #[test]
    fn test_windsurf_without_askpass_confirmation() {
        setup();
        env::set_var("TERM_PROGRAM", "windsurf");
        // No VSCODE_GIT_ASKPASS_NODE set
        assert_eq!(detect_editor(), SupportedEditor::Unknown);
    }

    #[test]
    fn test_windsurf_with_wrong_askpass() {
        setup();
        env::set_var("TERM_PROGRAM", "windsurf");
        env::set_var(
            "VSCODE_GIT_ASKPASS_NODE",
            "some/path/with/no/matching/editor",
        );
        assert_eq!(detect_editor(), SupportedEditor::Unknown);
    }
}
