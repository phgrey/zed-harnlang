use zed_extension_api::{self as zed, LanguageServerId, Result};

struct HarnLspExtension;

impl zed::Extension for HarnLspExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let path = worktree
            .which("harn-lsp")
            .ok_or_else(|| "Binary harn-lsp not found in PATH".to_string())?;
        Ok(zed::Command {
            command: path,
            args: vec![],
            env: worktree.shell_env(),
        })
    }
}

zed::register_extension!(HarnLspExtension);
