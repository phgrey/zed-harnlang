use std::env;
use std::fs;
use std::path::PathBuf;
use zed_extension_api::{self as zed, LanguageServerId, Result, DownloadedFileType};

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
        
        let (os, arch) = zed::current_platform();
        
        // 1. Download harn-lsp
        let harn_target = match (os, arch) {
            (zed::Os::Mac, zed::Architecture::Aarch64) => "aarch64-apple-darwin",
            (zed::Os::Mac, zed::Architecture::X8664) => "x86_64-apple-darwin",
            (zed::Os::Linux, zed::Architecture::X8664) => "x86_64-unknown-linux-gnu",
            (zed::Os::Linux, zed::Architecture::Aarch64) => "aarch64-unknown-linux-gnu",
            _ => return Err(format!("Unsupported platform for harn-lsp: {:?} {:?}", os, arch)),
        };
        
        let harn_dir = "harn-bin";
        let harn_binary_path = format!("{}/harn", harn_dir);
        if !fs::metadata(&harn_binary_path).map_or(false, |stat| stat.is_file()) {
            zed::set_language_server_installation_status(
                _language_server_id,
                &zed::LanguageServerInstallationStatus::Downloading,
            );

            // TODO: Fetch latest version dynamically if needed. For now, assuming latest tag or hardcoded.
            let harn_url = format!(
                "https://github.com/burin-labs/harn/releases/latest/download/harn-{}.tar.gz",
                harn_target
            );
            
            zed::download_file(
                &harn_url,
                harn_dir,
                DownloadedFileType::GzipTar,
            ).map_err(|e| format!("Failed to download harn: {}", e))?;

            zed::make_file_executable(&harn_binary_path)
                .map_err(|e| format!("Failed to make harn executable: {}", e))?;
        }

        // 2. Download rust-analyzer
        let ra_target = match (os, arch) {
            (zed::Os::Mac, zed::Architecture::Aarch64) => "aarch64-apple-darwin",
            (zed::Os::Mac, zed::Architecture::X8664) => "x86_64-apple-darwin",
            (zed::Os::Linux, zed::Architecture::X8664) => "x86_64-unknown-linux-gnu",
            (zed::Os::Linux, zed::Architecture::Aarch64) => "aarch64-unknown-linux-gnu",
            _ => return Err(format!("Unsupported platform for rust-analyzer: {:?} {:?}", os, arch)),
        };
        
        let ra_binary_path = "rust-analyzer";
        if !fs::metadata(ra_binary_path).map_or(false, |stat| stat.is_file()) {
            zed::set_language_server_installation_status(
                _language_server_id,
                &zed::LanguageServerInstallationStatus::Downloading,
            );

            let ra_url = format!(
                "https://github.com/rust-lang/rust-analyzer/releases/latest/download/rust-analyzer-{}.gz",
                ra_target
            );
            
            zed::download_file(
                &ra_url,
                ra_binary_path,
                DownloadedFileType::Gzip,
            ).map_err(|e| format!("Failed to download rust-analyzer: {}", e))?;

            zed::make_file_executable(ra_binary_path)
                .map_err(|e| format!("Failed to make rust-analyzer executable: {}", e))?;
        }
        
        zed::set_language_server_installation_status(
            _language_server_id,
            &zed::LanguageServerInstallationStatus::None,
        );

        let mut env = worktree.shell_env();

        if let Some(src_dir) = discover_harn_src_dir(worktree) {
            env.push(("HARN_SRC_DIR".into(), src_dir));
        }
        
        // Let the proxy know where the downloaded binaries are
        // zed extensions run in the extension directory, so relative paths work
        env.push(("HARN_LSP_PATH".into(), format!("./{}", harn_binary_path)));
        env.push(("RUST_ANALYZER_PATH".into(), format!("./{}", ra_binary_path)));

        // We run the proxy via cargo since it's locally in the zed-harnlang workspace
        // When published, this would need to point to a downloaded proxy binary.
        let proxy_manifest = worktree.root_path() + "/crates/harn-lsp-proxy/Cargo.toml";

        Ok(zed::Command {
            command: "cargo".to_string(),
            args: vec![
                "run".to_string(),
                "--quiet".to_string(),
                "--manifest-path".to_string(),
                proxy_manifest,
            ],
            env,
        })
    }
}

fn discover_harn_src_dir(worktree: &zed::Worktree) -> Option<String> {
    if let Ok(dir) = std::env::var("HARN_SRC_DIR") {
        return Some(dir);
    }
    let root = std::path::PathBuf::from(worktree.root_path());
    if root.join("crates/harn-vm/src/harness.rs").exists() {
        return root.to_str().map(String::from);
    }
    if let Some(parent) = root.parent() {
        let sibling = parent.join("harn");
        if sibling.join("crates/harn-vm/src/harness.rs").exists() {
            return sibling.to_str().map(String::from);
        }
    }
    None
}

zed::register_extension!(HarnLspExtension);
