use std::process::Command;
use zed_extension_api::{self as zed, LanguageServerId, Result};

pub struct HarnLspExtension;

impl zed::Extension for HarnLspExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let lsp_cmd = if let Some(path) = worktree.which("harn-lsp") {
            path
        } else {
            let _st1 = Command::new("wget")
                .arg("https://harnlang.com/install.sh")
                .status()
                .expect("Failed to wget 'https://harnlang.com/install.sh'");
            let _st2 = Command::new("sh")
                .arg("install.sh")
                .status()
                .expect("Failed to run sh install.sh");
            worktree
                .which("harn-lsp")
                .unwrap_or_else(|| panic!("cannot install harn-lsp"))
        };

        Ok(zed::Command {
            command: lsp_cmd,
            args: vec![],
            env: worktree.shell_env(),
        })
    }
}

zed::register_extension!(HarnLspExtension);
