use zed_extension_api::{self as zed, LanguageServerId, Result};

struct HarmExtension;

impl zed::Extension for HarmExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &LanguageServerId,
        _worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        Ok(zed::Command {
            command: "harn-lsp".to_string(),
            args: vec![],
            env: vec![],
        })
    }
}

zed::register_extension!(HarmExtension);
