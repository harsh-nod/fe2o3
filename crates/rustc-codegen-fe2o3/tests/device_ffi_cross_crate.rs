use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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

#[test]
fn reviewed_export_survives_cross_crate_metadata() {
    let workspace = workspace();
    let fixtures = workspace.join("crates/rustc-codegen-fe2o3/tests/fixtures/device-ffi");
    let output = workspace.join(format!(
        "target/fe2o3/test-output/device-ffi-cross-crate-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&output);
    std::fs::create_dir_all(&output).expect("create output directory");
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
        .replace("__CONTRACT__", &contract.to_hex());
    let export_source_path = output.join("export-lib.rs");
    std::fs::write(&export_source_path, export_source).expect("write generated export fixture");
    let ffi_export = output.join("libffi_export.rlib");

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
        .arg(output.join("app.rmeta"))
        .output()
        .expect("compile app fixture");
    require_success("cross-crate app", &app);

    let _ = std::fs::remove_dir_all(output);
}
