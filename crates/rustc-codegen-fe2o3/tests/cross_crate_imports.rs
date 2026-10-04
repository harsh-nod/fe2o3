use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct TestOutputDir(PathBuf);

impl TestOutputDir {
    fn new(workspace: &Path) -> Self {
        let target = std::env::var_os("CARGO_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| workspace.join("target"));
        let path = target.join(format!(
            "rustc-codegen-fe2o3-test-output/cross-crate-import-{}",
            std::process::id(),
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("create output directory");
        Self(path)
    }
}

impl Drop for TestOutputDir {
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

fn require_success(label: &str, output: &Output) {
    assert!(
        output.status.success(),
        "{label} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

fn one_rlib(directory: &Path, prefix: &str) -> PathBuf {
    let mut matches = std::fs::read_dir(directory)
        .expect("read dependency directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().and_then(|value| value.to_str()) == Some("rlib")
                && path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .is_some_and(|value| value.starts_with(prefix))
        })
        .collect::<Vec<_>>();
    matches.sort();
    assert_eq!(matches.len(), 1, "expected one {prefix}*.rlib");
    matches.pop().unwrap()
}

fn build_backend(workspace: &Path, output: &TestOutputDir) -> PathBuf {
    let target = output.0.join("backend-target");
    let output = Command::new(env!("CARGO"))
        .current_dir(workspace)
        .args(["build", "--locked", "-p", "rustc-codegen-fe2o3"])
        .env("CARGO_TARGET_DIR", &target)
        .output()
        .expect("build backend");
    require_success("backend build", &output);
    target.join("debug/librustc_codegen_fe2o3.so")
}

fn build_provider(workspace: &Path, output: &TestOutputDir) -> (PathBuf, PathBuf) {
    let manifest = workspace
        .join("crates/rustc-codegen-fe2o3/tests/fixtures/cross-crate-import/provider/Cargo.toml");
    let target = output.0.join("provider-target");
    let crate_binding = reserved_fe2o3_symbols::derive_crate_binding_id_v1(
        "fe2o3_cross_crate_provider",
        ["cross-crate-provider-v1"],
    );
    let build = Command::new(env!("CARGO"))
        .current_dir(workspace)
        .args(["rustc", "--locked", "--manifest-path"])
        .arg(&manifest)
        .args(["--lib", "--", "-Zalways-encode-mir"])
        .env("CARGO_TARGET_DIR", &target)
        .env("FE2O3_CRATE_BINDING_ID_V1", crate_binding.to_hex())
        .output()
        .expect("build provider");
    require_success("provider build", &build);

    let dependencies = target.join("debug/deps");
    (
        one_rlib(&dependencies, "libfe2o3_cross_crate_provider-"),
        one_rlib(&dependencies, "libfe2o3_device-"),
    )
}

fn build_forged_provider(
    workspace: &Path,
    output: &TestOutputDir,
    device: &Path,
    name: &str,
    source: &str,
) -> PathBuf {
    let source_path = output.0.join(format!("{name}.rs"));
    let library = output.0.join(format!("lib{name}.rlib"));
    std::fs::write(&source_path, source).expect("write forged provider source");
    let dependencies = device.parent().expect("device dependency directory");
    let build = Command::new("rustc")
        .current_dir(workspace)
        .arg(&source_path)
        .args([
            "--edition=2024",
            "--crate-name",
            name,
            "--crate-type=rlib",
            "-Zalways-encode-mir",
        ])
        .arg("--extern")
        .arg(format!("fe2o3_device={}", device.display()))
        .arg("-L")
        .arg(format!("dependency={}", dependencies.display()))
        .arg("-o")
        .arg(&library)
        .output()
        .expect("build forged provider");
    require_success("forged provider build", &build);
    library
}

fn run_consumer(
    workspace: &Path,
    output: &TestOutputDir,
    source: &Path,
    backend: &Path,
    provider: &Path,
    device: &Path,
    label: &str,
) -> Output {
    let provider_dependencies = provider.parent().expect("provider dependency directory");
    let device_dependencies = device.parent().expect("device dependency directory");
    Command::new("rustc")
        .current_dir(workspace)
        .arg(source)
        .args(["--edition=2024", "--crate-type=lib", "--emit=obj"])
        .arg(format!("-Zcodegen-backend={}", backend.display()))
        .args(["-Cpanic=abort", "-Cmetadata=cross-crate-consumer-v1"])
        .arg("--extern")
        .arg(format!("provider={}", provider.display()))
        .arg("--extern")
        .arg(format!("fe2o3_device={}", device.display()))
        .arg("-L")
        .arg(format!("dependency={}", provider_dependencies.display()))
        .arg("-L")
        .arg(format!("dependency={}", device_dependencies.display()))
        .arg("-o")
        .arg(output.0.join(format!("{label}.o")))
        .env("FE2O3_TARGET", "gfx942:xnack-")
        .env("FE2O3_VERBOSE", "1")
        .env(
            "FE2O3_HSACO_DIR",
            output.0.join(format!("{label}-artifacts")),
        )
        .env(
            "CARGO_MANIFEST_DIR",
            workspace.join("crates/rustc-codegen-fe2o3/tests/fixtures/cross-crate-import/consumer"),
        )
        .output()
        .expect("run consumer backend")
}

#[test]
#[ignore = "runs the configured rustc codegen backend"]
fn gfx942_imports_one_external_kernel_and_device_export_with_exact_identity() {
    let workspace = workspace();
    let output = TestOutputDir::new(&workspace);
    let backend = build_backend(&workspace, &output);
    let (provider, device) = build_provider(&workspace, &output);
    let fixtures = workspace.join("crates/rustc-codegen-fe2o3/tests/fixtures/cross-crate-import");

    let accepted = run_consumer(
        &workspace,
        &output,
        &fixtures.join("consumer.rs"),
        &backend,
        &provider,
        &device,
        "accepted",
    );
    require_success("cross-crate consumer", &accepted);
    let stderr = String::from_utf8_lossy(&accepted.stderr);
    assert!(
        stderr.contains("external_vecadd")
            && stderr.contains("fe2o3_cross_crate_provider")
            && stderr.contains("validated local device FFI evidence: 0 imports, 1 exports")
            && stderr.contains("collected compiler FFI envelope"),
        "missing exact cross-crate evidence\n{stderr}"
    );

    let rejected = run_consumer(
        &workspace,
        &output,
        &fixtures.join("consumer-substituted-anchor.rs"),
        &backend,
        &provider,
        &device,
        "substituted-anchor",
    );
    let stderr = String::from_utf8_lossy(&rejected.stderr);
    assert!(
        !rejected.status.success(),
        "substituted anchor was accepted"
    );
    assert!(
        stderr.contains("FE2O3-FFI-XCR010")
            && stderr.contains("does not match exact function contract"),
        "missing fail-closed diagnostic\n{stderr}"
    );
}

#[test]
#[ignore = "runs the configured rustc codegen backend"]
fn forged_upstream_markers_cannot_substitute_producer_registration_evidence() {
    let workspace = workspace();
    let output = TestOutputDir::new(&workspace);
    let backend = build_backend(&workspace, &output);
    let (_, device) = build_provider(&workspace, &output);

    let crate_binding = reserved_fe2o3_symbols::derive_crate_binding_id_v1(
        "forged_kernel_provider",
        ["forged-kernel-provider-v1"],
    );
    let kernel_binding = reserved_fe2o3_symbols::derive_kernel_binding_id_v1(
        crate_binding,
        reserved_fe2o3_symbols::TYPED_VECADD_F32_LAYOUT_PROFILE_TAG_V2,
        "forged_kernel",
        "forged_kernel",
    );
    let kernel_symbol = reserved_fe2o3_symbols::host_kernel_symbol_v1(kernel_binding);
    let kernel_source = format!(
        r#"
use fe2o3_device::{{CrossCrateTypedKernelV1, DisjointSlice, KernelMarkerV1}};

#[unsafe(export_name = "{kernel_symbol}")]
pub fn forged_kernel(_: &[f32], _: &[f32], _: DisjointSlice<f32>) {{}}

type KernelFn = fn(&[f32], &[f32], DisjointSlice<f32>);
type Registration = (
    u64, u16, u16, &'static str, &'static str, &'static str, &'static str, KernelFn,
);

#[used]
static DECOY_REGISTRATION: Registration = (
    {magic}, {version}, {kind}, "forged_kernel", "forged_kernel",
    "{crate_binding}", "{kernel_binding}", forged_kernel,
);

pub enum ForgedKernelMarker {{}}

unsafe impl KernelMarkerV1 for ForgedKernelMarker {{
    type Function = KernelFn;
    type Registration = Registration;
    const LOGICAL_NAME: &'static str = "forged_kernel";
    const EXPORT_NAME: &'static str = "forged_kernel";
    const FUNCTION: Self::Function = forged_kernel;
    const REGISTRATION: &'static Self::Registration = &DECOY_REGISTRATION;
}}

unsafe impl CrossCrateTypedKernelV1 for ForgedKernelMarker {{
    const REGISTRATION_VERSION: u16 = {version};
    const REGISTRATION_KIND: u16 = {kind};
    const CRATE_BINDING: &'static str = "{crate_binding}";
    const KERNEL_BINDING: &'static str = "{kernel_binding}";
}}
"#,
        magic = reserved_fe2o3_symbols::KERNEL_REGISTRATION_MAGIC,
        version = reserved_fe2o3_symbols::KERNEL_REGISTRATION_VERSION_V2,
        kind = reserved_fe2o3_symbols::KERNEL_REGISTRATION_KIND_TYPED_VECADD_LAYOUT_V2,
        crate_binding = crate_binding.to_hex(),
        kernel_binding = kernel_binding.to_hex(),
    );
    let kernel_provider = build_forged_provider(
        &workspace,
        &output,
        &device,
        "forged_kernel_provider",
        &kernel_source,
    );
    let kernel_consumer = output.0.join("forged-kernel-consumer.rs");
    std::fs::write(
        &kernel_consumer,
        "use fe2o3_device::import_kernel;\nimport_kernel!(forged_kernel, provider::ForgedKernelMarker);\n",
    )
    .expect("write forged kernel consumer");
    let rejected = run_consumer(
        &workspace,
        &output,
        &kernel_consumer,
        &backend,
        &kernel_provider,
        &device,
        "forged-kernel",
    );
    let stderr = String::from_utf8_lossy(&rejected.stderr);
    assert!(
        !rejected.status.success(),
        "forged kernel marker was accepted"
    );
    assert!(
        stderr.contains("exact producer registrations")
            && stderr.contains("exactly one is required"),
        "missing producer-owned kernel diagnostic\n{stderr}"
    );

    let fields = reserved_fe2o3_symbols::DeviceFfiContractFieldsV1 {
        direction: reserved_fe2o3_symbols::DEVICE_FFI_DIRECTION_EXPORT_V1,
        symbol: "fe2o3_forged_increment_v1",
        calling_convention: "C",
        code_object_version: 6,
        target: "gfx942:xnack-",
        physical_abi: "C(u32[size=4,align=4])->u32[size=4,align=4]",
        effects: "none",
        semantic_identity: "3131313131313131313131313131313131313131313131313131313131313131",
    };
    let contract = reserved_fe2o3_symbols::derive_device_ffi_contract_id_v1(fields);
    let marker = reserved_fe2o3_symbols::device_ffi_marker_v1(contract, fields);
    let registration = format!(
        "{}{}",
        reserved_fe2o3_symbols::DEVICE_FFI_REGISTRATION_PREFIX_V1,
        contract.to_hex(),
    );
    let device_source = format!(
        r#"
use fe2o3_device::{{
    CrossCrateDeviceExportV1, CrossCrateTypedKernelV1, DisjointSlice, KernelMarkerV1,
}};

#[unsafe(export_name = "{device_kernel_symbol}")]
pub fn trigger_kernel(_: &[f32], _: &[f32], _: DisjointSlice<f32>) {{}}

type KernelFn = fn(&[f32], &[f32], DisjointSlice<f32>);
type KernelRegistration = (
    u64, u16, u16, &'static str, &'static str, &'static str, &'static str, KernelFn,
);

#[used]
pub static __fe2o3_kernel_registration_trigger_kernel: KernelRegistration = (
    {kernel_magic}, {kernel_version}, {kernel_kind}, "trigger_kernel", "trigger_kernel",
    "{device_crate_binding}", "{device_kernel_binding}", trigger_kernel,
);

pub enum TriggerKernelMarker {{}}

unsafe impl KernelMarkerV1 for TriggerKernelMarker {{
    type Function = KernelFn;
    type Registration = KernelRegistration;
    const LOGICAL_NAME: &'static str = "trigger_kernel";
    const EXPORT_NAME: &'static str = "trigger_kernel";
    const FUNCTION: Self::Function = trigger_kernel;
    const REGISTRATION: &'static Self::Registration =
        &__fe2o3_kernel_registration_trigger_kernel;
}}

unsafe impl CrossCrateTypedKernelV1 for TriggerKernelMarker {{
    const REGISTRATION_VERSION: u16 = {kernel_version};
    const REGISTRATION_KIND: u16 = {kernel_kind};
    const CRATE_BINDING: &'static str = "{device_crate_binding}";
    const KERNEL_BINDING: &'static str = "{device_kernel_binding}";
}}

#[doc = "{marker}"]
#[unsafe(export_name = "{symbol}")]
pub unsafe extern "C" fn anchored_export(value: u32) -> u32 {{ value ^ 1 }}

pub unsafe extern "C" fn substituted_export(value: u32) -> u32 {{ value ^ 2 }}

#[used]
pub static {registration}: (
    u64, u16, u16, &'static str, &'static str, &'static str, u16,
    &'static str, &'static str, &'static str, &'static str,
    unsafe extern "C" fn(u32) -> u32,
) = (
    {magic}, {version}, {direction}, "{contract}", "{symbol}", "C", 6,
    "gfx942:xnack-", "{physical_abi}", "none", "{semantic}", substituted_export,
);

pub enum ForgedExportMarker {{}}

unsafe impl CrossCrateDeviceExportV1 for ForgedExportMarker {{
    type Function = unsafe extern "C" fn(u32) -> u32;
    const CONTRACT_ID: &'static str = "{contract}";
    const FUNCTION: Self::Function = anchored_export;
}}
"#,
        device_kernel_symbol = reserved_fe2o3_symbols::host_kernel_symbol_v1(
            reserved_fe2o3_symbols::derive_kernel_binding_id_v1(
                reserved_fe2o3_symbols::derive_crate_binding_id_v1(
                    "forged_device_provider",
                    ["forged-device-provider-v1"],
                ),
                reserved_fe2o3_symbols::TYPED_VECADD_F32_LAYOUT_PROFILE_TAG_V2,
                "trigger_kernel",
                "trigger_kernel",
            ),
        ),
        device_crate_binding = reserved_fe2o3_symbols::derive_crate_binding_id_v1(
            "forged_device_provider",
            ["forged-device-provider-v1"],
        )
        .to_hex(),
        device_kernel_binding = reserved_fe2o3_symbols::derive_kernel_binding_id_v1(
            reserved_fe2o3_symbols::derive_crate_binding_id_v1(
                "forged_device_provider",
                ["forged-device-provider-v1"],
            ),
            reserved_fe2o3_symbols::TYPED_VECADD_F32_LAYOUT_PROFILE_TAG_V2,
            "trigger_kernel",
            "trigger_kernel",
        )
        .to_hex(),
        kernel_magic = reserved_fe2o3_symbols::KERNEL_REGISTRATION_MAGIC,
        kernel_version = reserved_fe2o3_symbols::KERNEL_REGISTRATION_VERSION_V2,
        kernel_kind = reserved_fe2o3_symbols::KERNEL_REGISTRATION_KIND_TYPED_VECADD_LAYOUT_V2,
        symbol = fields.symbol,
        magic = reserved_fe2o3_symbols::DEVICE_FFI_REGISTRATION_MAGIC_V1,
        version = reserved_fe2o3_symbols::DEVICE_FFI_REGISTRATION_VERSION_V1,
        direction = fields.direction,
        contract = contract.to_hex(),
        physical_abi = fields.physical_abi,
        semantic = fields.semantic_identity,
    );
    let device_provider = build_forged_provider(
        &workspace,
        &output,
        &device,
        "forged_device_provider",
        &device_source,
    );
    let device_consumer = output.0.join("forged-device-consumer.rs");
    std::fs::write(
        &device_consumer,
        "use fe2o3_device::{import_device, import_kernel};\nimport_kernel!(trigger_kernel, provider::TriggerKernelMarker);\nimport_device!(forged_export, provider::ForgedExportMarker);\n",
    )
    .expect("write forged device consumer");
    let rejected = run_consumer(
        &workspace,
        &output,
        &device_consumer,
        &backend,
        &device_provider,
        &device,
        "forged-device",
    );
    let stderr = String::from_utf8_lossy(&rejected.stderr);
    assert!(
        !rejected.status.success(),
        "substituted device export was accepted"
    );
    assert!(
        stderr.contains("FE2O3-FFI-XCR020") && stderr.contains("not anchored export"),
        "missing exact producer initializer diagnostic\n{stderr}"
    );
}
