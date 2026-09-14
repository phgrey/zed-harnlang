use std::fs;
use std::path::{Path, PathBuf};
use zed_extension_api::{self as zed, DownloadedFileType, LanguageServerId, Result};

pub struct HarnLspExtension;

pub fn platform_target(
    os: zed::Os,
    arch: zed::Architecture,
) -> std::result::Result<&'static str, String> {
    match (os, arch) {
        (zed::Os::Mac, zed::Architecture::Aarch64) => Ok("aarch64-apple-darwin"),
        (zed::Os::Mac, zed::Architecture::X8664) => Ok("x86_64-apple-darwin"),
        (zed::Os::Linux, zed::Architecture::X8664) => Ok("x86_64-unknown-linux-gnu"),
        (zed::Os::Linux, zed::Architecture::Aarch64) => Ok("aarch64-unknown-linux-gnu"),
        _ => Err(format!(
            "Unsupported platform for extension: {:?} {:?}",
            os, arch
        )),
    }
}

pub fn harn_download_url(target: &str) -> String {
    format!(
        "https://github.com/burin-labs/harn/releases/latest/download/harn-{}.tar.gz",
        target
    )
}

pub fn rust_analyzer_download_url(target: &str) -> String {
    format!(
        "https://github.com/rust-lang/rust-analyzer/releases/latest/download/rust-analyzer-{}.gz",
        target
    )
}

pub fn proxy_download_url(target: &str) -> String {
    format!(
        "https://github.com/phgrey/zed-harnlang/releases/latest/download/harn-lsp-proxy-{}.tar.gz",
        target
    )
}

pub const HARN_SRC_ARCHIVE_URL: &str =
    "https://github.com/burin-labs/harn/archive/refs/heads/main.tar.gz";

pub fn discover_harn_src_dir_from_path(root: &Path) -> Option<String> {
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

fn discover_harn_src_dir(worktree: &zed::Worktree) -> Option<String> {
    let root = PathBuf::from(worktree.root_path());
    discover_harn_src_dir_from_path(&root)
}

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
        let target = platform_target(os, arch)?;

        // 1. Locate or download harn-lsp
        if None == worktree.which("harn-lsp") {
            let harn_dir = "harn-bin";
            let harn_binary_path = format!("{}/harn", harn_dir);
            if !fs::metadata(&harn_binary_path).map_or(false, |stat| stat.is_file()) {
                zed::set_language_server_installation_status(
                    _language_server_id,
                    &zed::LanguageServerInstallationStatus::Downloading,
                );

                let harn_url = harn_download_url(target);

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

                let ra_url = rust_analyzer_download_url(target);

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

                let proxy_url = proxy_download_url(target);

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
                let _ = zed::download_file(HARN_SRC_ARCHIVE_URL, "./", DownloadedFileType::GzipTar);
            }
        }

        Ok(zed::Command {
            command: proxy_path,
            args: vec![],
            env,
        })
    }
}

zed::register_extension!(HarnLspExtension);

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(prefix: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let dir = std::env::temp_dir().join(format!("{}_{}_{}", prefix, std::process::id(), nanos));
            fs::create_dir_all(&dir).unwrap();
            Self { path: dir }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn test_platform_targets() {
        assert_eq!(
            platform_target(zed::Os::Mac, zed::Architecture::Aarch64).unwrap(),
            "aarch64-apple-darwin"
        );
        assert_eq!(
            platform_target(zed::Os::Mac, zed::Architecture::X8664).unwrap(),
            "x86_64-apple-darwin"
        );
        assert_eq!(
            platform_target(zed::Os::Linux, zed::Architecture::X8664).unwrap(),
            "x86_64-unknown-linux-gnu"
        );
        assert_eq!(
            platform_target(zed::Os::Linux, zed::Architecture::Aarch64).unwrap(),
            "aarch64-unknown-linux-gnu"
        );
        assert!(platform_target(zed::Os::Windows, zed::Architecture::X8664).is_err());
        assert!(platform_target(zed::Os::Windows, zed::Architecture::Aarch64).is_err());
    }

    #[test]
    fn test_download_urls() {
        let target = "aarch64-apple-darwin";
        assert_eq!(
            harn_download_url(target),
            "https://github.com/burin-labs/harn/releases/latest/download/harn-aarch64-apple-darwin.tar.gz"
        );
        assert_eq!(
            rust_analyzer_download_url(target),
            "https://github.com/rust-lang/rust-analyzer/releases/latest/download/rust-analyzer-aarch64-apple-darwin.gz"
        );
        assert_eq!(
            proxy_download_url(target),
            "https://github.com/phgrey/zed-harnlang/releases/latest/download/harn-lsp-proxy-aarch64-apple-darwin.tar.gz"
        );
        assert_eq!(
            HARN_SRC_ARCHIVE_URL,
            "https://github.com/burin-labs/harn/archive/refs/heads/main.tar.gz"
        );
    }

    #[test]
    fn test_discover_harn_src_dir_direct_match() {
        let temp = TestDir::new("harn_direct");
        let harness_file = temp.path().join("crates/harn-vm/src/harness.rs");
        fs::create_dir_all(harness_file.parent().unwrap()).unwrap();
        File::create(&harness_file).unwrap();

        let found = discover_harn_src_dir_from_path(temp.path());
        assert_eq!(found, Some(temp.path().to_str().unwrap().to_string()));
    }

    #[test]
    fn test_discover_harn_src_dir_sibling_match() {
        let parent = TestDir::new("harn_parent");
        let project = parent.path().join("my-project");
        fs::create_dir_all(&project).unwrap();

        let sibling_harness = parent.path().join("harn/crates/harn-vm/src/harness.rs");
        fs::create_dir_all(sibling_harness.parent().unwrap()).unwrap();
        File::create(&sibling_harness).unwrap();

        let found = discover_harn_src_dir_from_path(&project);
        assert_eq!(
            found,
            Some(parent.path().join("harn").to_str().unwrap().to_string())
        );
    }

    #[test]
    fn test_discover_harn_src_dir_none() {
        let temp = TestDir::new("harn_empty");
        let found = discover_harn_src_dir_from_path(temp.path());
        assert_eq!(found, None);
    }

    #[test]
    fn test_extension_initialization() {
        let ext = <HarnLspExtension as zed::Extension>::new();
        drop(ext);
    }
}
