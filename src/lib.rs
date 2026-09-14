use std::fs;
use zed_extension_api::{self as zed, DownloadedFileType, LanguageServerId, Result};

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

        // 1. Locate or download harn-lsp
        if None == worktree.which("harn-lsp") {
            let harn_dir = "harn-bin";
            let harn_binary_path = format!("{}/harn", harn_dir);
            if !fs::metadata(&harn_binary_path).map_or(false, |stat| stat.is_file()) {
                zed::set_language_server_installation_status(
                    _language_server_id,
                    &zed::LanguageServerInstallationStatus::Downloading,
                );

                let harn_target = match (os, arch) {
                    (zed::Os::Mac, zed::Architecture::Aarch64) => "aarch64-apple-darwin",
                    (zed::Os::Mac, zed::Architecture::X8664) => "x86_64-apple-darwin",
                    (zed::Os::Linux, zed::Architecture::X8664) => "x86_64-unknown-linux-gnu",
                    (zed::Os::Linux, zed::Architecture::Aarch64) => "aarch64-unknown-linux-gnu",
                    _ => {
                        return Err(format!(
                            "Unsupported platform for harn-lsp: {:?} {:?}",
                            os, arch
                        ))
                    }
                };

                let harn_url = format!(
                    "https://github.com/burin-labs/harn/releases/latest/download/harn-{}.tar.gz",
                    harn_target
                );

                zed::download_file(&harn_url, harn_dir, DownloadedFileType::GzipTar)
                    .map_err(|e| format!("Failed to download harn: {}", e))?;

                zed::make_file_executable(&harn_binary_path)
                    .map_err(|e| format!("Failed to make harn executable: {}", e))?;
            }
        };

        // 2. Locate or download rust-analyzer
        if None == worktree.which("rust-analyzer") {
            let ra_binary_path = "rust-analyzer";
            if !fs::metadata(ra_binary_path).map_or(false, |stat| stat.is_file()) {
                zed::set_language_server_installation_status(
                    _language_server_id,
                    &zed::LanguageServerInstallationStatus::Downloading,
                );

                let ra_target = match (os, arch) {
                    (zed::Os::Mac, zed::Architecture::Aarch64) => "aarch64-apple-darwin",
                    (zed::Os::Mac, zed::Architecture::X8664) => "x86_64-apple-darwin",
                    (zed::Os::Linux, zed::Architecture::X8664) => "x86_64-unknown-linux-gnu",
                    (zed::Os::Linux, zed::Architecture::Aarch64) => "aarch64-unknown-linux-gnu",
                    _ => {
                        return Err(format!(
                            "Unsupported platform for rust-analyzer: {:?} {:?}",
                            os, arch
                        ))
                    }
                };

                let ra_url = format!(
                    "https://github.com/rust-lang/rust-analyzer/releases/latest/download/rust-analyzer-{}.gz",
                    ra_target
                );

                zed::download_file(&ra_url, ra_binary_path, DownloadedFileType::Gzip)
                    .map_err(|e| format!("Failed to download rust-analyzer: {}", e))?;

                zed::make_file_executable(ra_binary_path)
                    .map_err(|e| format!("Failed to make rust-analyzer executable: {}", e))?;
            }
        };

        // 3. Locate or download harn-lsp-proxy
        let proxy_path = if let Some(path) = worktree.which("harn-lsp-proxy") {
            path
        } else {
            let proxy_binary_path = "harn-lsp-proxy";
            if !fs::metadata(&proxy_binary_path).map_or(false, |stat| stat.is_file()) {
                zed::set_language_server_installation_status(
                    _language_server_id,
                    &zed::LanguageServerInstallationStatus::Downloading,
                );

                let proxy_target = match (os, arch) {
                    (zed::Os::Mac, zed::Architecture::Aarch64) => "aarch64-apple-darwin",
                    (zed::Os::Mac, zed::Architecture::X8664) => "x86_64-apple-darwin",
                    (zed::Os::Linux, zed::Architecture::X8664) => "x86_64-unknown-linux-gnu",
                    (zed::Os::Linux, zed::Architecture::Aarch64) => "aarch64-unknown-linux-gnu",
                    _ => {
                        return Err(format!(
                            "Unsupported platform for proxy: {:?} {:?}",
                            os, arch
                        ))
                    }
                };

                let proxy_url = format!(
                    "https://github.com/phgrey/zed-harnlang/releases/latest/download/harn-lsp-proxy-{}.tar.gz",
                    proxy_target
                );

                zed::download_file(&proxy_url, "./", DownloadedFileType::GzipTar)
                    .map_err(|e| format!("Failed to download proxy: {}", e))?;

                zed::make_file_executable(&proxy_binary_path)
                    .map_err(|e| format!("Failed to make proxy executable: {}", e))?;
            }
            format!("./{}", proxy_binary_path)
        };

        zed::set_language_server_installation_status(
            _language_server_id,
            &zed::LanguageServerInstallationStatus::None,
        );

        let env = worktree.shell_env();

        if None == discover_harn_src_dir(worktree) {
            let src_marker = "harn-main/Cargo.toml";
            if !fs::metadata(&src_marker).map_or(false, |stat| stat.is_file()) {
                zed::set_language_server_installation_status(
                    _language_server_id,
                    &zed::LanguageServerInstallationStatus::Downloading,
                );

                // Fallback: download the harn source repository for rust-analyzer
                let src_url = "https://github.com/burin-labs/harn/archive/refs/heads/main.tar.gz";
                let _ = zed::download_file(src_url, "./", DownloadedFileType::GzipTar);
            }
        }

        Ok(zed::Command {
            command: proxy_path,
            args: vec![],
            env,
        })
    }
}

fn discover_harn_src_dir(worktree: &zed::Worktree) -> Option<String> {
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
