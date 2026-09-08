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

fn fixture_target(manifest_dir: &Path, name: &str) -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("../../target"))
        .join(name)
}

#[test]
fn typed_kernel_resolves_renamed_host_dependency() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manifest = manifest_dir.join("tests/fixtures/renamed-typed-host/Cargo.toml");
    let target_dir = fixture_target(&manifest_dir, "renamed-typed-host-test");
    let output = cargo_check(&manifest, &target_dir, Some("renamed-typed-host-fixture"));

    assert!(
        output.status.success(),
        "renamed typed-host fixture failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn typed_kernel_authenticates_logical_kernel_context_types() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manifest = manifest_dir.join("tests/fixtures/renamed-typed-host/Cargo.toml");
    let target_dir = fixture_target(&manifest_dir, "renamed-typed-host-test");

    let output = cargo_check(&manifest, &target_dir, Some("kernel_context"));
    assert!(
        output.status.success(),
        "genuine renamed KernelContext fixture failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output = cargo_check(&manifest, &target_dir, Some("kernel_context_lookalike"));
    assert!(!output.status.success(), "KernelContext lookalike compiled");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("KernelContextTypeV1"),
        "lookalike rejection omitted the sealed context diagnostic:\n{stderr}"
    );

    let cases: &[(&str, &str)] = &[
        (
            "kernel_context_explicit_brands",
            "kernel, target, and launch brands are compiler-issued",
        ),
        (
            "kernel_context_named_lifetime",
            "KernelContext's lifetime is compiler-bound and must be written as '_",
        ),
    ];
    for (bin, expected) in cases {
        let output = cargo_check(&manifest, &target_dir, Some(bin));
        assert!(!output.status.success(), "{bin} unexpectedly compiled");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(expected),
            "{bin} omitted diagnostic `{expected}`:\n{stderr}",
        );
    }
}

#[test]
fn typed_global_keeps_physical_abi_and_rejects_source_substitution() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manifest = manifest_dir.join("tests/fixtures/renamed-typed-host/Cargo.toml");
    let target_dir = fixture_target(&manifest_dir, "renamed-typed-host-test");

    let output = cargo_check(&manifest, &target_dir, Some("capability_global"));
    assert!(
        output.status.success(),
        "typed Global fixture failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let cases: &[(&str, &str)] = &[
        (
            "capability_global_explicit_brand",
            "Brand is compiler-bound and must not be written explicitly",
        ),
        ("capability_global_lookalike", "CapabilityMemoryViewTypeV1"),
        (
            "capability_global_missing_context",
            "require KernelContext<'_> as the first parameter",
        ),
        (
            "capability_global_unsupported_element",
            "Global supports only i8/u8/i16/u16/i32/u32/i64/u64/f32/f64",
        ),
        (
            "capability_global_wrong_role",
            "Global's role must be ReadOnly, DisjointWrite<IndexSpace>, ExclusiveReadWrite, or AtomicReadWrite<Scope>",
        ),
        ("capability_global_role_substitution", "ExclusiveReadWrite"),
        (
            "capability_global_wrong_space",
            "support only global address space in V1",
        ),
    ];
    for (bin, expected) in cases {
        let output = cargo_check(&manifest, &target_dir, Some(bin));
        assert!(!output.status.success(), "{bin} unexpectedly compiled");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(expected),
            "{bin} omitted diagnostic `{expected}`:\n{stderr}",
        );
    }
}

#[test]
fn generated_arguments_retain_source_borrows() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manifest = manifest_dir.join("tests/fixtures/renamed-typed-host/Cargo.toml");
    let target_dir = fixture_target(&manifest_dir, "renamed-typed-host-test");
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
fn generated_global_mut_arguments_reject_forgery_and_substitution() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manifest = manifest_dir.join("tests/fixtures/renamed-typed-host/Cargo.toml");
    let target_dir = fixture_target(&manifest_dir, "renamed-typed-host-test");
    let cases: &[(&str, &[&str])] = &[
        (
            "global_mut_alias",
            &["cannot borrow `*target` as mutable more than once"],
        ),
        ("global_mut_forgery", &["field `region`", "is private"]),
        (
            "global_mut_wrong_type",
            &[
                "expected `GlobalMut<'_, u32>`",
                "found `GlobalMut<'_, f32>`",
            ],
        ),
        (
            "global_mut_wrong_mutability",
            &[
                "expected `GeneratedKfdReadWriteSlice",
                "found `GeneratedKfdReadSlice",
            ],
        ),
        ("global_mut_raw_escape", &["no method named `as_raw`"]),
        (
            "global_mut_wrong_address_space",
            &["requires `pub fn(&[f32], &[f32], DisjointSlice<f32>)`"],
        ),
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
fn generated_worker_v3_adapter_compiles_downstream() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manifest = manifest_dir.join("tests/fixtures/generic-worker-v3-adapter/Cargo.toml");
    let target_dir = fixture_target(&manifest_dir, "generic-worker-v3-adapter-test");
    let output = cargo_check(&manifest, &target_dir, Some("pass"));

    assert!(
        output.status.success(),
        "generic generated adapter fixture failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn generated_worker_v3_adapter_rejects_unsafe_escape_hatches() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manifest = manifest_dir.join("tests/fixtures/generic-worker-v3-adapter/Cargo.toml");
    let target_dir = fixture_target(&manifest_dir, "generic-worker-v3-adapter-test");
    let cases: &[(&str, &[&str])] = &[
        ("lifetime_escape", &["lifetime may not live long enough"]),
        ("private_fields", &["private"]),
        ("non_clone", &["no method named `clone`"]),
        ("raw_pointer_escape", &["field `source`"]),
        (
            "multi_lifetime_escape",
            &["lifetime may not live long enough"],
        ),
        (
            "multi_private_fields",
            &["cannot construct", "private fields"],
        ),
        ("multi_raw_pointer_escape", &["field `first`", "is private"]),
        (
            "multi_capability_raw_pointer_escape",
            &["no method named `device_pointer`"],
        ),
        ("multi_non_clone", &["no method named `clone`"]),
        (
            "multi_abi_substitution",
            &[
                "error[E0277]",
                "CompilerGeneratedKfdArguments",
                "is not implemented",
            ],
        ),
        (
            "kfd_trait_requires_unsafe_impl",
            &["error[E0200]", "CompilerGeneratedKfdArguments"],
        ),
        (
            "kfd_mutable_alias",
            &["cannot borrow `*output` as mutable more than once"],
        ),
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
    let target_dir = fixture_target(&manifest_dir, "typed-kernel-invalid-test");
    let cases: &[(&str, &[&str])] = &[
        (
            "invalid_safe_kernel",
            &[
                "ordinary #[kernel] functions must be safe",
                "unsafe blocks are not allowed in ordinary #[kernel] bodies",
            ],
        ),
        (
            "invalid_attribute",
            &["#[kernel] accepts only #[kernel], #[kernel(typed)]"],
        ),
        (
            "missing_namespace",
            &[
                "#[kernel(typed)] must be compiled through cargo fe2o3 build, check, test, or clippy",
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
                "general typed V1 explicit launch requires required dimensions",
                "general typed V1 explicit launch requires identical required and max dimensions",
                "general typed V1 requires exact [N, 1, 1] launch dimensions with N in 1..=256",
                "the typed vecadd V2 profile requires an exact 256x1x1 launch contract",
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
                "kernel match has no corresponding control_flow integer switch type",
                "integer switch case suffix `u32` does not match declared `u64`",
                "range patterns are unsupported in V1",
                "guarded match arms are unsupported",
                "break with a value is unsupported",
                "async expressions are unsupported by the V1 kernel sidecar",
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

#[test]
fn ordinary_kernel_profile_accepts_safe_only_source() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manifest = manifest_dir.join("tests/fixtures/typed-invalid/Cargo.toml");
    let target_dir = fixture_target(&manifest_dir, "typed-kernel-invalid-test");
    let output = cargo_check(&manifest, &target_dir, Some("safe_kernel"));

    assert!(
        output.status.success(),
        "safe-only kernel fixture failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
