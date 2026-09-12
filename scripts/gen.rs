#! /usr/bin/env -S cargo +nightly -Zscript
---cargo
[dependencies]
harn-builtin-meta = "0.10.134"
---

use harn_builtin_meta::CapabilityId;
use std::env;

fn rust_source_file(cap: CapabilityId) -> &'static str {
    match cap {
        CapabilityId::Stdio => "crates/harn-vm/src/stdlib/io.rs",
        CapabilityId::Term => "crates/harn-vm/src/term.rs",
        CapabilityId::Clock => "crates/harn-vm/src/stdlib/clock.rs",
        CapabilityId::Fs => "crates/harn-vm/src/stdlib/fs.rs",
        CapabilityId::Env => "crates/harn-vm/src/stdlib/process.rs",
        CapabilityId::Random => "crates/harn-vm/src/stdlib/crypto.rs",
        CapabilityId::Net => "crates/harn-vm/src/harness_net.rs",
        CapabilityId::Process => "crates/harn-vm/src/stdlib/process.rs",
        CapabilityId::Channels => "crates/harn-vm/src/stdlib/channels.rs",
        CapabilityId::System => "crates/harn-vm/src/harness_system.rs",
        CapabilityId::Secrets => "crates/harn-vm/src/stdlib/secret_scan.rs",
        CapabilityId::Llm => "crates/harn-vm/src/llm/",
        CapabilityId::Agent => "crates/harn-vm/src/stdlib/agents.rs",
        CapabilityId::Tenant => "crates/harn-vm/src/harness_tenant.rs",
        CapabilityId::Auth => "crates/harn-vm/src/harness_auth.rs",
        CapabilityId::Observability => "crates/harn-vm/src/stdlib/observability.rs",
        CapabilityId::Verdict => "crates/harn-vm/src/harness.rs",
        CapabilityId::Tools => "crates/harn-vm/src/stdlib/tools.rs",
        CapabilityId::Ast => "crates/harn-parser/src/lib.rs",
        CapabilityId::CodeIndex => "crates/harn-vm/src/harness.rs",
        CapabilityId::Computer => "crates/harn-vm/src/harness.rs",
        CapabilityId::Embed => "crates/harn-cli/src/local_embed.rs",
        CapabilityId::Memory => "crates/harn-vm/src/stdlib/memory.rs",
        CapabilityId::Sqlite => "crates/harn-vm/src/stdlib/sqlite/mod.rs",
        CapabilityId::Postgres => "crates/harn-vm/src/stdlib/postgres/mod.rs",
        CapabilityId::FsWatch => "crates/harn-vm/src/stdlib/files.rs",
        CapabilityId::HostLease => "crates/harn-vm/src/stdlib/host.rs",
        CapabilityId::Scanner => "crates/harn-vm/src/stdlib/secret_scan.rs",
        CapabilityId::SecretStore => "crates/harn-vm/src/stdlib/secret_scan.rs",
        CapabilityId::TerminalSession => "crates/harn-vm/src/term.rs",
        CapabilityId::Rules => "crates/harn-rules/src/lib.rs",
        CapabilityId::Lint => "crates/harn-lint/src/lib.rs",
        CapabilityId::Runtime => "crates/harn-vm/src/harness.rs",
        CapabilityId::Interaction => "crates/harn-vm/src/stdlib/hitl.rs",
        CapabilityId::Project => "crates/harn-vm/src/stdlib/project_enrich.rs",
        CapabilityId::Dashboard => "crates/harn-vm/src/harness.rs",
        CapabilityId::Workspace => "crates/harn-vm/src/workspace_anchor.rs",
        CapabilityId::MergeCaptain => "crates/harn-vm/src/harness.rs",
        CapabilityId::Session => "crates/harn-vm/src/stdlib/agent_sessions.rs",
        CapabilityId::Permission => "crates/harn-vm/src/harness.rs",
        CapabilityId::Text => "crates/harn-vm/src/text_diff.rs",
        CapabilityId::Lsp => "crates/harn-lsp/src/lib.rs",
        CapabilityId::Credentials => "crates/harn-vm/src/harness_auth.rs",
        CapabilityId::PrMonitor => "crates/harn-vm/src/harness.rs",
        CapabilityId::Workflow => "crates/harn-vm/src/orchestration/workflow.rs",
        CapabilityId::Testing => "crates/harn-vm/src/stdlib/testing.rs",
    }
}

fn generate_harness_stubs(repo_root: &str) -> String {
    let mut out = String::new();
    let root = repo_root.trim_end_matches('/');

    out.push_str("/**\n");
    out.push_str(" * Root capability handle threaded into `fn main(harness: Harness)`.\n");
    out.push_str(" *\n");
    out.push_str(&format!(" * Rust source: [crates/harn-vm/src/harness.rs](file://{root}/crates/harn-vm/src/harness.rs)\n"));
    out.push_str(" */\n");
    out.push_str("pub struct Harness {\n");

    for cap in CapabilityId::ALL {
        let rel_path = rust_source_file(*cap);
        out.push_str(&format!(
            "  /** Rust source: [{rel_path}](file://{root}/{rel_path}) */\n  {}: {},\n",
            cap.field_name(),
            cap.type_name()
        ));
    }
    out.push_str("}\n\n");

    for cap in CapabilityId::ALL {
        let rel_path = rust_source_file(*cap);
        let full_path = format!("{root}/{rel_path}");
        out.push_str("/**\n");
        out.push_str(&format!(
            " * `{}` capability handle (`harness.{}`, `{}`).\n",
            cap.type_name(),
            cap.field_name(),
            cap.variant_name()
        ));
        out.push_str(" *\n");
        out.push_str(&format!(
            " * Rust source: [{rel_path}](file://{full_path})\n"
        ));
        out.push_str(" */\n");
        out.push_str(&format!("pub struct {} {{}}\n\n", cap.type_name()));
    }

    out
}

fn main() {
    let repo_root = env::var("HARN_REPO_ROOT")
        .unwrap_or_else(|_| "/Users/ssemenov/work/zedpack/harn".to_string());
    let text = generate_harness_stubs(&repo_root);
    println!("{}", text);
}
