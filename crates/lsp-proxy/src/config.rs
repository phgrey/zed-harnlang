pub struct Config<'a> {
    pub cmd: &'a str,
    pub repo: &'a str,
}

pub const SERVERS: [Config<'static>; 2] = [
    Config {
        cmd: "harn-lsp",
        repo: "https://github.com/burin-labs/harn",
    },
    Config {
        cmd: "rust-analyzer",
        repo: "https://github.com/rust-lang/rust-analyzer",
    },
];

pub fn extension_path() -> String {
    let current_exe = std::env::current_exe().expect("Failed to get current executable path");
    current_exe.parent().unwrap().to_str().unwrap().to_string()
}
