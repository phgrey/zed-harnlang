use std::process::Command;
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
        let mut path = worktree.which("harn-lsp");
        if path == None {
            let _st1 = Command::new("wget")
                .arg("https://harnlang.com/install.sh")
                .status()
                .expect("Failed to wget 'https://harnlang.com/install.sh'");
            let _st2 = Command::new("sh")
                .arg("install.sh")
                .status()
                .expect("Failed to run sh install.sh");
            path = worktree.which("harn-lsp");
        }
        // 2. Otherwise, download the latest release binary for the OS/arch

        Ok(zed::Command {
            command: path.ok_or_else(|| "Binary harn-lsp not found in PATH".to_string())?,
            args: vec![],
            env: worktree.shell_env(),
        })
    }
}

zed::register_extension!(HarnLspExtension);
