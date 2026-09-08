use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use fe2o3_rustc_front::{
    KERNEL_CONTEXT_FRONTEND_REGISTRATION_KIND_V1, KERNEL_CONTEXT_FRONTEND_REGISTRATION_MAGIC_V1,
    KERNEL_CONTEXT_FRONTEND_REGISTRATION_VERSION_V1,
    encode_generated_kernel_context_frontend_contract_v1,
};
use reserved_fe2o3_symbols::{
    KERNEL_REGISTRATION_KIND_KERNEL, KERNEL_REGISTRATION_MAGIC, KERNEL_REGISTRATION_VERSION_V1,
};

struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fe2o3-context-abi-{label}-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(path.join("src")).expect("create ABI fixture directory");
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace")
}

fn pinned_rustc() -> PathBuf {
    let rustc = Path::new(env!("CARGO")).with_file_name("rustc");
    assert!(
        rustc.is_file(),
        "pinned rustc is missing: {}",
        rustc.display()
    );
    rustc
}

fn fixture_source(
    logical_name: &str,
    helper_safety: &str,
    root_safety: &str,
    helper_argument: &str,
    root_argument: &str,
    call_argument: &str,
    helper_return: &str,
    root_return: &str,
    helper_value: &str,
    root_value: &str,
) -> String {
    let root_name = format!("fe2o3_kernel_{logical_name}");
    let helper_name = format!("__fe2o3_kernel_body_v1_{logical_name}");
    let marker_name = format!("__fe2o3_kernel_marker_{logical_name}");
    let contract = encode_generated_kernel_context_frontend_contract_v1(
        &root_name,
        &helper_name,
        &marker_name,
    )
    .expect("encode generated context contract");
    let contract = contract
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    let helper_call = if helper_safety.is_empty() {
        format!("{helper_name}(context, {call_argument})")
    } else {
        format!("unsafe {{ {helper_name}(context, {call_argument}) }}")
    };

    format!(
        r#"#![no_std]

use fe2o3_device::{{CurrentTarget, KernelContext, RegisteredLaunch}};

pub enum {marker_name} {{}}

#[inline(always)]
{helper_safety} fn {helper_name}(
    context: KernelContext<'_, {marker_name}, CurrentTarget, RegisteredLaunch>,
    value: {helper_argument},
) {helper_return} {{
    let _ = context.invocation();
    let _ = value;
    {helper_value}
}}

#[unsafe(no_mangle)]
pub {root_safety} fn {root_name}(value: {root_argument}) {root_return} {{
    let context = unsafe {{ KernelContext::__compiler_issue() }};
    let result = {helper_call};
    let _ = result;
    {root_value}
}}

#[used]
static __fe2o3_kernel_registration_{logical_name}: (
    u64,
    u16,
    u16,
    &'static str,
    &'static str,
    {root_safety} fn({root_argument}) {root_return},
) = (
    {KERNEL_REGISTRATION_MAGIC},
    {KERNEL_REGISTRATION_VERSION_V1},
    {KERNEL_REGISTRATION_KIND_KERNEL},
    "{logical_name}",
    "{logical_name}",
    {root_name},
);

#[used]
static __fe2o3_kernel_context_contract_v1_{logical_name}: (
    u64,
    u16,
    u16,
    &'static str,
    &'static [u8],
    {root_safety} fn({root_argument}) {root_return},
) = (
    {KERNEL_CONTEXT_FRONTEND_REGISTRATION_MAGIC_V1},
    {KERNEL_CONTEXT_FRONTEND_REGISTRATION_VERSION_V1},
    {KERNEL_CONTEXT_FRONTEND_REGISTRATION_KIND_V1},
    "{logical_name}",
    &[{contract}],
    {root_name},
);
"#,
    )
}

fn run_fixture(label: &str, source: &str) -> Output {
    let scratch = Scratch::new(label);
    let root = workspace();
    let manifest = format!(
        r#"[package]
name = "fe2o3-context-abi-{label}"
version = "0.0.0"
edition = "2024"
publish = false

[workspace]

[lib]
path = "src/lib.rs"

[dependencies]
fe2o3-device = {{ path = "{}" }}
"#,
        root.join("crates/fe2o3-device").display(),
    );
    std::fs::write(scratch.0.join("Cargo.toml"), manifest).expect("write ABI fixture manifest");
    std::fs::copy(root.join("Cargo.lock"), scratch.0.join("Cargo.lock"))
        .expect("copy workspace lockfile");
    std::fs::write(scratch.0.join("src/lib.rs"), source).expect("write ABI fixture source");

    Command::new(env!("CARGO"))
        .current_dir(&scratch.0)
        .env("RUSTC", pinned_rustc())
        .env(
            "RUSTC_WORKSPACE_WRAPPER",
            env!("CARGO_BIN_EXE_fe2o3-rustc-extract"),
        )
        .env(
            "FE2O3_EXTRACT_CRATE_V1",
            format!("fe2o3_context_abi_{}", label.replace('-', "_")),
        )
        .env("FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2", "55".repeat(32))
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env(
            "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
            "-Zalways-encode-mir -Zinline-mir=no -Copt-level=0 -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32",
        )
        .args([
            "check",
            "--offline",
            "--locked",
            "-Zbuild-std=core",
            "--target",
            "amdgcn-amd-amdhsa",
            "--target-dir",
        ])
        .arg(scratch.0.join("target"))
        .output()
        .expect("run ABI fixture through production extractor")
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn collector_rejects_each_logical_physical_abi_substitution() {
    let cases = [
        (
            "physical-argument",
            fixture_source(
                "physical_argument",
                "",
                "",
                "u64",
                "u32",
                "u64::from(value)",
                "",
                "",
                "",
                "",
            ),
            "physical/logical argument 0 violates the exact identity-or-capability-carrier relation",
        ),
        (
            "safety",
            fixture_source(
                "safety", "", "unsafe", "u32", "u32", "value", "", "", "", "",
            ),
            "physical/logical safety qualifiers differ",
        ),
        (
            "return",
            fixture_source(
                "return_type",
                "",
                "",
                "u32",
                "u32",
                "value",
                "-> u32",
                "",
                "17",
                "",
            ),
            "physical/logical return types differ outside the exact trusted KernelResult-to-unit entry normalization",
        ),
    ];

    for (label, source, detail) in cases {
        let output = run_fixture(label, &source);
        let stderr = String::from_utf8(output.stderr).expect("rustc diagnostics are UTF-8");
        assert!(
            !output.status.success(),
            "hostile ABI fixture {label} compiled"
        );
        assert!(
            stderr.contains("[FE2O3-CAP-ABI001]") && stderr.contains(detail),
            "hostile ABI fixture {label} changed its diagnostic:\n{stderr}",
        );
    }
}
