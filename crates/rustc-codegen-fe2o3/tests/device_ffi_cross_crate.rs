use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct TestOutputDir(PathBuf);

impl TestOutputDir {
    fn new(workspace: &Path, label: &str) -> Self {
        let path = workspace.join(format!(
            "target/fe2o3/test-output/device-ffi-{label}-{}",
            std::process::id()
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

fn marker(
    direction: u16,
    symbol: &str,
    semantic_identity: &str,
) -> (reserved_fe2o3_symbols::DeviceFfiContractIdV1, String) {
    let fields = reserved_fe2o3_symbols::DeviceFfiContractFieldsV1 {
        direction,
        symbol,
        calling_convention: "C",
        code_object_version: 5,
        target: "gfx1100",
        physical_abi: "C(u32[size=4,align=4])->u32[size=4,align=4]",
        effects: "none",
        semantic_identity,
    };
    let contract = reserved_fe2o3_symbols::derive_device_ffi_contract_id_v1(fields);
    (
        contract,
        reserved_fe2o3_symbols::device_ffi_marker_v1(contract, fields),
    )
}

fn compile_rlib(source: &Path, crate_name: &str, output: &Path) {
    compile_rlib_with_metadata(source, crate_name, None, output);
}

fn compile_rlib_with_metadata(
    source: &Path,
    crate_name: &str,
    metadata: Option<&str>,
    output: &Path,
) {
    let mut command = Command::new("rustc");
    command
        .arg(source)
        .args([
            "--edition=2024",
            "--crate-type=rlib",
            "-Zalways-encode-mir",
            "--crate-name",
        ])
        .arg(crate_name)
        .arg("-o")
        .arg(output);
    if let Some(metadata) = metadata {
        command.arg(format!("-Cmetadata={metadata}"));
    }
    let compile = command
        .output()
        .unwrap_or_else(|error| panic!("compile {crate_name}: {error}"));
    require_success(crate_name, &compile);
}

fn build_backend(workspace: &Path) -> PathBuf {
    let build = Command::new(env!("CARGO"))
        .current_dir(workspace)
        .args(["build", "--locked", "-p", "rustc-codegen-fe2o3"])
        .output()
        .expect("build codegen backend");
    require_success("codegen backend", &build);
    let backend = workspace.join("target/debug/librustc_codegen_fe2o3.so");
    assert!(
        backend.is_file(),
        "missing backend at {}",
        backend.display()
    );
    backend
}

fn run_backend(source: &Path, backend: &Path, output: &Path, externs: &[(&str, &Path)]) -> Output {
    let mut command = Command::new("rustc");
    command
        .arg(source)
        .args(["--edition=2024", "--crate-type=lib", "--emit=obj"])
        .arg(format!("-Zcodegen-backend={}", backend.display()))
        .arg("-Cpanic=abort")
        .arg("-o")
        .arg(output.join("fixture.o"))
        .env("FE2O3_TARGET", "gfx1100")
        .env("FE2O3_VERBOSE", "1")
        .env("FE2O3_HSACO_DIR", output.join("artifacts"));
    for (name, path) in externs {
        command
            .arg("--extern")
            .arg(format!("{name}={}", path.display()));
    }
    command.output().expect("run codegen backend")
}

#[test]
fn reviewed_export_survives_cross_crate_metadata() {
    let workspace = workspace();
    let fixtures = workspace.join("crates/rustc-codegen-fe2o3/tests/fixtures/device-ffi");
    let output = TestOutputDir::new(&workspace, "cross-crate-metadata");
    let fields = reserved_fe2o3_symbols::DeviceFfiContractFieldsV1 {
        direction: reserved_fe2o3_symbols::DEVICE_FFI_DIRECTION_EXPORT_V1,
        symbol: "cross_crate_device_helper_v1",
        calling_convention: "C",
        code_object_version: 5,
        target: "gfx1100",
        physical_abi: "C(u32[size=4,align=4])->u32[size=4,align=4]",
        effects: "none",
        semantic_identity: "3333333333333333333333333333333333333333333333333333333333333333",
    };
    let contract = reserved_fe2o3_symbols::derive_device_ffi_contract_id_v1(fields);
    let marker = reserved_fe2o3_symbols::device_ffi_marker_v1(contract, fields);
    let export_source = include_str!("fixtures/device-ffi/export-lib.rs")
        .replace("__MARKER__", &marker)
        .replace("__CONTRACT__", &contract.to_hex())
        .replace(
            "registration_v1_fixture",
            &format!("registration_v1_{}", contract.to_hex()),
        );
    let export_source_path = output.0.join("export-lib.rs");
    std::fs::write(&export_source_path, export_source).expect("write generated export fixture");
    let ffi_export = output.0.join("libffi_export.rlib");

    let export = Command::new("rustc")
        .arg(&export_source_path)
        .args([
            "--edition=2024",
            "--crate-type=rlib",
            "--crate-name=ffi_export",
        ])
        .arg("-o")
        .arg(&ffi_export)
        .output()
        .expect("compile export fixture");
    require_success("cross-crate export", &export);

    let app = Command::new("rustc")
        .arg(fixtures.join("app.rs"))
        .args(["--edition=2024", "--emit=metadata"])
        .arg("--extern")
        .arg(format!("ffi_export={}", ffi_export.display()))
        .arg("-o")
        .arg(output.0.join("app.rmeta"))
        .output()
        .expect("compile app fixture");
    require_success("cross-crate app", &app);
}

#[test]
#[ignore = "runs the configured rustc codegen backend"]
fn reachable_cross_crate_import_is_retained_in_the_closed_device_graph() {
    let workspace = workspace();
    let fixtures = workspace.join("crates/rustc-codegen-fe2o3/tests/fixtures/device-ffi");
    let output = TestOutputDir::new(&workspace, "reachable-import");
    let backend = build_backend(&workspace);
    let (contract, marker) = marker(
        reserved_fe2o3_symbols::DEVICE_FFI_DIRECTION_IMPORT_V1,
        "cross_crate_external_add_v1",
        "4444444444444444444444444444444444444444444444444444444444444444",
    );
    let source = include_str!("fixtures/device-ffi/import-lib.rs")
        .replace("__MARKER__", &marker)
        .replace("__CONTRACT__", &contract.to_hex())
        .replace(
            "registration_v1_fixture",
            &format!("registration_v1_{}", contract.to_hex()),
        );
    let source_path = output.0.join("import-lib.rs");
    std::fs::write(&source_path, source).expect("write import fixture");
    let rlib = output.0.join("libffi_import.rlib");
    compile_rlib(&source_path, "ffi_import", &rlib);

    let compile = run_backend(
        &fixtures.join("import-app.rs"),
        &backend,
        &output.0,
        &[("ffi_import", &rlib)],
    );
    let stderr = String::from_utf8_lossy(&compile.stderr);
    assert!(
        stderr.contains("external device import:")
            && stderr.contains("cross_crate_external_add_v1"),
        "reachable import was not observed\n{stderr}"
    );
    assert!(
        stderr.contains("closed device FFI: 1 imports, 0 exports, target gfx1100, code object v5"),
        "closed FFI summary was not emitted\n{stderr}"
    );
    assert!(!stderr.contains("host-only or unreachable"), "{stderr}");
}

#[test]
#[ignore = "runs the configured rustc codegen backend"]
fn host_only_import_is_rejected_with_source_ownership() {
    let workspace = workspace();
    let output = TestOutputDir::new(&workspace, "host-only-import");
    let backend = build_backend(&workspace);
    let (contract, marker) = marker(
        reserved_fe2o3_symbols::DEVICE_FFI_DIRECTION_IMPORT_V1,
        "cross_crate_external_add_v1",
        "4444444444444444444444444444444444444444444444444444444444444444",
    );
    let source = include_str!("fixtures/device-ffi/import-lib.rs")
        .replace("__MARKER__", &marker)
        .replace("__CONTRACT__", &contract.to_hex())
        .replace(
            "registration_v1_fixture",
            &format!("registration_v1_{}", contract.to_hex()),
        );
    let source_path = output.0.join("host-only.rs");
    std::fs::write(&source_path, source).expect("write host-only fixture");

    let compile = run_backend(&source_path, &backend, &output.0, &[]);
    let stderr = String::from_utf8_lossy(&compile.stderr);
    assert!(!compile.status.success(), "host-only import was accepted");
    assert!(
        stderr.contains("import `cross_crate_external_add_v1`")
            && stderr.contains("host-only or unreachable")
            && stderr.contains("host_only::cross_crate_external_add"),
        "missing stable source-owned diagnostic\n{stderr}"
    );
}

#[test]
#[ignore = "runs the configured rustc codegen backend"]
fn generic_cross_crate_export_is_rejected_at_its_concrete_instance() {
    let workspace = workspace();
    let fixtures = workspace.join("crates/rustc-codegen-fe2o3/tests/fixtures/device-ffi");
    let output = TestOutputDir::new(&workspace, "generic-export");
    let backend = build_backend(&workspace);
    let (contract, marker) = marker(
        reserved_fe2o3_symbols::DEVICE_FFI_DIRECTION_EXPORT_V1,
        "cross_crate_device_helper_v1",
        "3333333333333333333333333333333333333333333333333333333333333333",
    );
    let source = include_str!("fixtures/device-ffi/export-lib.rs")
        .replace("__MARKER__", &marker)
        .replace("__CONTRACT__", &contract.to_hex())
        .replace(
            "registration_v1_fixture",
            &format!("registration_v1_{}", contract.to_hex()),
        )
        .replace(
            "fn cross_crate_device_helper(value: u32)",
            "fn cross_crate_device_helper<const N: usize>(value: u32)",
        )
        .replace(
            "    cross_crate_device_helper,",
            "    cross_crate_device_helper::<1>,",
        );
    let source_path = output.0.join("generic-export.rs");
    std::fs::write(&source_path, source).expect("write generic export fixture");
    let rlib = output.0.join("libffi_export.rlib");
    compile_rlib(&source_path, "ffi_export", &rlib);
    let app = std::fs::read_to_string(fixtures.join("app.rs"))
        .expect("read app fixture")
        .replace(
            "cross_crate_device_helper(7)",
            "cross_crate_device_helper::<1>(7)",
        );
    let app_path = output.0.join("app.rs");
    std::fs::write(&app_path, app).expect("write generic app fixture");

    let compile = run_backend(&app_path, &backend, &output.0, &[("ffi_export", &rlib)]);
    let stderr = String::from_utf8_lossy(&compile.stderr);
    assert!(!compile.status.success(), "generic FFI export was accepted");
    assert!(
        stderr.contains("ffi_export::cross_crate_device_helper")
            && stderr.contains("is generic")
            && stderr.contains("concrete nongeneric identity"),
        "missing stable generic diagnostic\n{stderr}"
    );
}

#[test]
#[ignore = "runs the configured rustc codegen backend"]
fn duplicate_cross_crate_providers_report_both_source_owners() {
    let workspace = workspace();
    let output = TestOutputDir::new(&workspace, "duplicate-providers");
    let backend = build_backend(&workspace);
    let (contract, marker) = marker(
        reserved_fe2o3_symbols::DEVICE_FFI_DIRECTION_EXPORT_V1,
        "cross_crate_device_helper_v1",
        "3333333333333333333333333333333333333333333333333333333333333333",
    );
    let source = include_str!("fixtures/device-ffi/export-lib.rs")
        .replace("__MARKER__", &marker)
        .replace("__CONTRACT__", &contract.to_hex())
        .replace(
            "registration_v1_fixture",
            &format!("registration_v1_{}", contract.to_hex()),
        );
    let first_source = output.0.join("provider-a.rs");
    let second_source = output.0.join("provider-b.rs");
    std::fs::write(&first_source, &source).expect("write first provider");
    std::fs::write(&second_source, source).expect("write second provider");
    let first = output.0.join("libprovider_a.rlib");
    let second = output.0.join("libprovider_b.rlib");
    compile_rlib(&first_source, "provider_a", &first);
    compile_rlib(&second_source, "provider_b", &second);
    let app = r#"
#[unsafe(no_mangle)]
pub fn fe2o3_kernel_duplicate_providers() {
    unsafe {
        let _ = provider_a::cross_crate_device_helper(1);
        let _ = provider_b::cross_crate_device_helper(2);
    }
}

#[used]
static __fe2o3_kernel_registration_duplicate_providers: (
    u64, u16, u16, &'static str, &'static str, fn(),
) = (
    0x4e52_4b33_4f32_4546, 1, 1, "duplicate_providers",
    "duplicate_providers", fe2o3_kernel_duplicate_providers,
);
"#;
    let app_path = output.0.join("app.rs");
    std::fs::write(&app_path, app).expect("write duplicate-provider app");

    let compile = run_backend(
        &app_path,
        &backend,
        &output.0,
        &[("provider_a", &first), ("provider_b", &second)],
    );
    let stderr = String::from_utf8_lossy(&compile.stderr);
    assert!(
        !compile.status.success(),
        "duplicate providers were accepted"
    );
    assert!(
        stderr.contains("duplicate device FFI contract")
            && stderr.contains("provider_a::cross_crate_device_helper")
            && stderr.contains("provider_b::cross_crate_device_helper"),
        "missing stable duplicate-provider ownership diagnostic\n{stderr}"
    );
}

#[test]
#[ignore = "runs adversarial rustc codegen backend probes"]
fn reserved_registration_prefix_items_fail_without_ice() {
    let workspace = workspace();
    let output = TestOutputDir::new(&workspace, "reserved-prefix-kinds");
    let backend = build_backend(&workspace);
    let prefix = reserved_fe2o3_symbols::DEVICE_FFI_REGISTRATION_PREFIX_V1;
    let valid_suffix = "1111111111111111111111111111111111111111111111111111111111111111";
    let cases = [
        (
            "struct",
            format!("pub struct {prefix}spoof;"),
            "FE2O3-FFI-REG001",
        ),
        (
            "function",
            format!("pub fn {prefix}spoof() {{}}"),
            "FE2O3-FFI-REG001",
        ),
        (
            "module",
            format!("pub mod {prefix}spoof {{}}"),
            "FE2O3-FFI-REG001",
        ),
        (
            "const",
            format!("pub const {prefix}spoof: u32 = 0;"),
            "FE2O3-FFI-REG001",
        ),
        (
            "malformed-static",
            format!("#[used]\nstatic {prefix}{valid_suffix}: u32 = 0;"),
            "FE2O3-FFI-REG010",
        ),
        (
            "mutable-static",
            format!("#[used]\nstatic mut {prefix}{valid_suffix}: u32 = 0;"),
            "FE2O3-FFI-REG002",
        ),
    ];

    for (label, source, diagnostic) in cases {
        let case_output = output.0.join(label);
        std::fs::create_dir_all(&case_output).expect("create adversarial case directory");
        let source_path = case_output.join("probe.rs");
        std::fs::write(&source_path, source).expect("write reserved-prefix probe");
        let compile = run_backend(&source_path, &backend, &case_output, &[]);
        let stderr = String::from_utf8_lossy(&compile.stderr);
        assert!(!compile.status.success(), "{label} probe was accepted");
        assert!(
            stderr.contains(diagnostic),
            "{label} omitted stable diagnostic {diagnostic}\n{stderr}"
        );
        assert_no_ice(label, &stderr);
    }
}

#[test]
#[ignore = "runs adversarial rustc codegen backend probes"]
fn registration_initializer_is_exactly_bound_to_its_marker_function() {
    let workspace = workspace();
    let output = TestOutputDir::new(&workspace, "registration-initializer");
    let backend = build_backend(&workspace);
    let (contract, marker) = marker(
        reserved_fe2o3_symbols::DEVICE_FFI_DIRECTION_IMPORT_V1,
        "cross_crate_external_add_v1",
        "4444444444444444444444444444444444444444444444444444444444444444",
    );
    let valid = include_str!("fixtures/device-ffi/import-lib.rs")
        .replace("__MARKER__", &marker)
        .replace("__CONTRACT__", &contract.to_hex())
        .replace(
            "registration_v1_fixture",
            &format!("registration_v1_{}", contract.to_hex()),
        );
    let cases = [
        (
            "wrong-magic",
            valid.replacen("0x4946_4633_4f32_4546", "0", 1),
            "FE2O3-FFI-REG011",
        ),
        (
            "function-cast",
            valid.replacen(
                "    cross_crate_external_add,",
                "    cross_crate_external_add as unsafe extern \"C\" fn(u32) -> u32,",
                1,
            ),
            "FE2O3-FFI-REG007",
        ),
        (
            "unmarked-function",
            format!(
                "unsafe extern \"C\" {{ fn unrelated(value: u32) -> u32; }}\n{}",
                valid.replacen("    cross_crate_external_add,", "    unrelated,", 1)
            ),
            "FE2O3-FFI-REG009",
        ),
    ];

    for (label, source, diagnostic) in cases {
        let case_output = output.0.join(label);
        std::fs::create_dir_all(&case_output).expect("create registration case directory");
        let source_path = case_output.join("probe.rs");
        std::fs::write(&source_path, source).expect("write registration probe");
        let compile = run_backend(&source_path, &backend, &case_output, &[]);
        let stderr = String::from_utf8_lossy(&compile.stderr);
        assert!(!compile.status.success(), "{label} probe was accepted");
        assert!(
            stderr.contains(diagnostic),
            "{label} omitted stable diagnostic {diagnostic}\n{stderr}"
        );
        assert_no_ice(label, &stderr);
    }
}

#[test]
#[ignore = "runs adversarial rustc codegen backend probes"]
fn function_pointer_calls_to_device_imports_fail_closed() {
    let workspace = workspace();
    let output = TestOutputDir::new(&workspace, "indirect-import");
    let backend = build_backend(&workspace);
    let (contract, marker) = marker(
        reserved_fe2o3_symbols::DEVICE_FFI_DIRECTION_IMPORT_V1,
        "cross_crate_external_add_v1",
        "4444444444444444444444444444444444444444444444444444444444444444",
    );
    let import_source = include_str!("fixtures/device-ffi/import-lib.rs")
        .replace("__MARKER__", &marker)
        .replace("__CONTRACT__", &contract.to_hex())
        .replace(
            "registration_v1_fixture",
            &format!("registration_v1_{}", contract.to_hex()),
        );
    let import_path = output.0.join("import.rs");
    std::fs::write(&import_path, import_source).expect("write import provider");
    let rlib = output.0.join("libffi_import.rlib");
    compile_rlib(&import_path, "ffi_import", &rlib);

    let kernel_case = indirect_import_app(false);
    let helper_case = indirect_import_app(true);
    let constant_case = constant_import_app();
    for (label, source, diagnostic) in [
        ("kernel", kernel_case, "FE2O3-FFI-CALL001"),
        ("helper", helper_case, "FE2O3-FFI-CALL001"),
        ("constant", constant_case, "FE2O3-FFI-CALL002"),
    ] {
        let case_output = output.0.join(label);
        std::fs::create_dir_all(&case_output).expect("create indirect-call directory");
        let source_path = case_output.join("app.rs");
        std::fs::write(&source_path, source).expect("write indirect-call app");
        let compile = run_backend(
            &source_path,
            &backend,
            &case_output,
            &[("ffi_import", &rlib)],
        );
        let stderr = String::from_utf8_lossy(&compile.stderr);
        assert!(
            !compile.status.success(),
            "{label} indirect call was accepted"
        );
        assert!(
            stderr.contains(diagnostic),
            "{label} omitted stable indirect-call diagnostic\n{stderr}"
        );
        if label == "helper" {
            assert!(stderr.contains("call_import_indirect"), "{stderr}");
        }
        assert_no_ice(label, &stderr);
    }
}

#[test]
#[ignore = "runs adversarial rustc codegen backend probes"]
fn same_name_crate_versions_do_not_collapse_provider_identity() {
    let workspace = workspace();
    let output = TestOutputDir::new(&workspace, "same-name-provider-versions");
    let backend = build_backend(&workspace);
    let (contract, marker) = marker(
        reserved_fe2o3_symbols::DEVICE_FFI_DIRECTION_EXPORT_V1,
        "cross_crate_device_helper_v1",
        "3333333333333333333333333333333333333333333333333333333333333333",
    );
    let source = include_str!("fixtures/device-ffi/export-lib.rs")
        .replace("__MARKER__", &marker)
        .replace("__CONTRACT__", &contract.to_hex())
        .replace(
            "registration_v1_fixture",
            &format!("registration_v1_{}", contract.to_hex()),
        );
    let first_source = output.0.join("first.rs");
    let second_source = output.0.join("second.rs");
    std::fs::write(&first_source, &source).expect("write first same-name provider");
    std::fs::write(&second_source, source).expect("write second same-name provider");
    let first = output.0.join("libsame_provider_a.rlib");
    let second = output.0.join("libsame_provider_b.rlib");
    compile_rlib_with_metadata(&first_source, "same_provider", Some("version-a"), &first);
    compile_rlib_with_metadata(&second_source, "same_provider", Some("version-b"), &second);
    let app = duplicate_provider_app();
    let app_path = output.0.join("app.rs");
    std::fs::write(&app_path, app).expect("write same-name provider app");

    let compile = run_backend(
        &app_path,
        &backend,
        &output.0,
        &[("provider_a", &first), ("provider_b", &second)],
    );
    let stderr = String::from_utf8_lossy(&compile.stderr);
    assert!(
        !compile.status.success(),
        "same-name providers were collapsed"
    );
    assert!(
        stderr.contains("duplicate device FFI contract")
            && stderr
                .matches("same_provider::cross_crate_device_helper")
                .count()
                >= 2
            && stderr.matches("def-path-hash=").count() >= 2,
        "same-name crate versions lacked distinct stable ownership\n{stderr}"
    );
    assert_no_ice("same-name providers", &stderr);
}

fn assert_no_ice(label: &str, stderr: &str) {
    for signature in [
        "compiler unexpectedly panicked",
        "query stack during panic",
        "do not use `optimized_mir` for constants",
        "rustc-ice-",
    ] {
        assert!(
            !stderr.contains(signature),
            "{label} triggered ICE signature `{signature}`\n{stderr}"
        );
    }
}

fn indirect_import_app(helper: bool) -> String {
    let call = if helper {
        "unsafe { call_import_indirect(7) }"
    } else {
        r#"unsafe {
        let function: unsafe extern "C" fn(u32) -> u32 =
            ffi_import::cross_crate_external_add;
        function(7)
    }"#
    };
    let helper = if helper {
        r#"
#[inline(always)]
unsafe fn call_import_indirect(value: u32) -> u32 {
    let function: unsafe extern "C" fn(u32) -> u32 =
        ffi_import::cross_crate_external_add;
    unsafe { function(value) }
}
"#
    } else {
        ""
    };
    format!(
        r#"
{helper}
#[unsafe(no_mangle)]
pub fn fe2o3_kernel_indirect_import() {{
    let _ = {call};
}}

#[used]
static __fe2o3_kernel_registration_indirect_import: (
    u64, u16, u16, &'static str, &'static str, fn(),
) = (
    0x4e52_4b33_4f32_4546, 1, 1, "indirect_import",
    "indirect_import", fe2o3_kernel_indirect_import,
);
"#
    )
}

fn constant_import_app() -> String {
    r#"
const IMPORT: unsafe extern "C" fn(u32) -> u32 = ffi_import::cross_crate_external_add;

#[unsafe(no_mangle)]
pub fn fe2o3_kernel_constant_import() {
    let _ = unsafe { IMPORT(7) };
}

#[used]
static __fe2o3_kernel_registration_constant_import: (
    u64, u16, u16, &'static str, &'static str, fn(),
) = (
    0x4e52_4b33_4f32_4546, 1, 1, "constant_import",
    "constant_import", fe2o3_kernel_constant_import,
);
"#
    .to_owned()
}

fn duplicate_provider_app() -> &'static str {
    r#"
#[unsafe(no_mangle)]
pub fn fe2o3_kernel_duplicate_providers() {
    unsafe {
        let _ = provider_a::cross_crate_device_helper(1);
        let _ = provider_b::cross_crate_device_helper(2);
    }
}

#[used]
static __fe2o3_kernel_registration_duplicate_providers: (
    u64, u16, u16, &'static str, &'static str, fn(),
) = (
    0x4e52_4b33_4f32_4546, 1, 1, "duplicate_providers",
    "duplicate_providers", fe2o3_kernel_duplicate_providers,
);
"#
}
