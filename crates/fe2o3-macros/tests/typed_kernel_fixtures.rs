use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn cargo_check(manifest: &Path, target_dir: &Path, bin: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO"));
    command
        .arg("check")
        .arg("--offline")
        .arg("--locked")
        .arg("--manifest-path")
        .arg(manifest)
        .arg("--target-dir")
        .arg(target_dir);
    if let Some(bin) = bin {
        command.arg("--bin").arg(bin);
    }

    command.output().expect("failed to run cargo check fixture")
}

#[test]
fn typed_kernel_resolves_renamed_host_dependency() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manifest = manifest_dir.join("tests/fixtures/renamed-typed-host/Cargo.toml");
    let target_dir = manifest_dir.join("../../target/renamed-typed-host-test");
    let output = cargo_check(&manifest, &target_dir, Some("renamed-typed-host-fixture"));

    assert!(
        output.status.success(),
        "renamed typed-host fixture failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn generated_arguments_retain_source_borrows() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manifest = manifest_dir.join("tests/fixtures/renamed-typed-host/Cargo.toml");
    let target_dir = manifest_dir.join("../../target/renamed-typed-host-test");
    let cases: &[(&str, &str)] = &[
        (
            "arguments_lifetime_escape",
            "lifetime may not live long enough",
        ),
        (
            "arguments_mutable_alias",
            "cannot borrow `*output` as mutable more than once at a time",
        ),
    ];

    for (bin, expected_diagnostic) in cases {
        let output = cargo_check(&manifest, &target_dir, Some(bin));
        assert!(!output.status.success(), "{bin} unexpectedly compiled");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(expected_diagnostic),
            "{bin} omitted diagnostic `{expected_diagnostic}`:\n{stderr}"
        );
    }
}

#[test]
fn exact_alpha_zeta_generated_adapter_compiles_downstream() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manifest = manifest_dir.join("tests/fixtures/alpha-zeta-adapter/Cargo.toml");
    let target_dir = manifest_dir.join("../../target/alpha-zeta-adapter-test");
    let output = cargo_check(&manifest, &target_dir, Some("pass"));

    assert!(
        output.status.success(),
        "alpha/zeta generated adapter fixture failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn exact_alpha_zeta_generated_adapter_rejects_unsafe_escape_hatches() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manifest = manifest_dir.join("tests/fixtures/alpha-zeta-adapter/Cargo.toml");
    let target_dir = manifest_dir.join("../../target/alpha-zeta-adapter-test");
    let cases: &[(&str, &[&str])] = &[
        (
            "wrong_role",
            &[
                "error[E0277]",
                "zeta_gpu::Arguments<'static>",
                "unsatisfied trait bound",
                "is not implemented",
            ],
        ),
        (
            "wrong_signature",
            &[
                "error[E0277]",
                "alpha_gpu::Arguments<'static>",
                "unsatisfied trait bound",
                "is not implemented",
            ],
        ),
        (
            "wrong_mutability",
            &[
                "error[E0277]",
                "alpha_gpu::Arguments<'static>",
                "unsatisfied trait bound",
                "is not implemented",
            ],
        ),
        ("lifetime_escape", &["lifetime may not live long enough"]),
        ("private_fields", &["private"]),
        ("non_clone", &["no method named `clone`"]),
        ("raw_pointer_escape", &["field `input`"]),
    ];

    for (bin, expected_diagnostics) in cases {
        let output = cargo_check(&manifest, &target_dir, Some(bin));
        assert!(!output.status.success(), "{bin} unexpectedly compiled");
        let stderr = String::from_utf8_lossy(&output.stderr);
        for expected_diagnostic in *expected_diagnostics {
            assert!(
                stderr.contains(expected_diagnostic),
                "{bin} omitted diagnostic `{expected_diagnostic}`:\n{stderr}"
            );
        }
    }
}

#[test]
fn typed_kernel_compile_fail_diagnostics_are_stable() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manifest = manifest_dir.join("tests/fixtures/typed-invalid/Cargo.toml");
    let target_dir = manifest_dir.join("../../target/typed-kernel-invalid-test");
    let cases: &[(&str, &[&str])] = &[
        (
            "invalid_attribute",
            &["#[kernel] accepts only #[kernel], #[kernel(typed)]"],
        ),
        (
            "missing_namespace",
            &[
                "#[kernel(typed)] requires the cargo-fe2o3 rustc wrapper or an explicit 256-bit namespace",
            ],
        ),
        (
            "invalid_signatures",
            &[
                "#[kernel(typed)] requires a public kernel function",
                "#[kernel(typed)] requires a safe kernel function",
                "#[kernel(typed)] does not support generic kernel functions",
                "#[kernel(typed)] requires the unit return type",
                "#[kernel(typed)] requires `pub fn(&[f32], &[f32], DisjointSlice<f32>)`",
                "#[kernel(typed)] argument 1 must have exact type `&[f32]`",
                "#[kernel(typed)] argument 2 must have exact type `&[f32]`",
                "#[kernel(typed)] argument 3 must have exact type `DisjointSlice<f32>`",
            ],
        ),
        (
            "invalid_symbol_stems",
            &[
                "#[kernel(typed)] kernel name must be 1 to 128 ASCII identifier bytes for backend artifact symbols",
            ],
        ),
        (
            "invalid_launch",
            &[
                "workgroup dimensions must be nonzero",
                "required workgroup dimensions exceed max dimensions",
                "min_workgroups_per_compute_unit requires max workgroup dimensions",
                "launch maximum dimensions are duplicated",
                "general typed V1 supports only an exact 256x1x1 launch contract",
            ],
        ),
        (
            "invalid_unsafe_asm",
            &[
                "unsafe_asm supports only target = \"gfx942\" in V1",
                "unsafe_asm effects conflict with its memory/control-flow options",
                "unsafe_asm(...) requires an unsafe kernel function",
            ],
        ),
        (
            "undeclared_asm",
            &[
                "asm! reachable directly from a kernel requires an explicit unsafe_asm(...) declaration",
            ],
        ),
        (
            "invalid_control_flow",
            &[
                "direct kernel loop requires control_flow",
                "control_flow loop bounds must be nonzero",
                "declares 2 loop bounds but the kernel contains 1 direct loops",
                "integer_switches supports only fixed-width",
                "not a fixed-width integer switch",
                "range patterns are unsupported in V1",
                "guarded match arms are unsupported",
                "break with a value is unsupported",
                "unsafe assembly with control_flow effects cannot participate",
            ],
        ),
    ];

    for (bin, expected_diagnostics) in cases {
        let output = cargo_check(&manifest, &target_dir, Some(bin));
        assert!(!output.status.success(), "{bin} unexpectedly compiled");
        let stderr = String::from_utf8_lossy(&output.stderr);
        for expected in *expected_diagnostics {
            assert!(
                stderr.contains(expected),
                "{bin} omitted diagnostic `{expected}`:\n{stderr}"
            );
        }
        assert!(
            !stderr.contains("could not resolve the fe2o3-host crate"),
            "{bin} resolved host support before rejecting invalid syntax:\n{stderr}"
        );
    }
}
