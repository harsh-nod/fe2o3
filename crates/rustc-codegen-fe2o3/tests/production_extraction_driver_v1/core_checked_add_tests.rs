mod core_checked_add_tests {
    use super::{ScratchTarget, materialize_source_safety_fixture};
    use fe2o3_kernel_ir::{BinaryOp, CheckedBinaryOperator, OperationKind};
    use serde_json::json;
    use std::path::Path;
    use std::process::{Command, Output};

    const CRATE: &str = "fe2o3_production_source_safety_fixture";
    const UNSIGNED: [(&str, u32); 5] = [
        ("u8", 8),
        ("u16", 16),
        ("u32", 32),
        ("u64", 64),
        ("usize", 64),
    ];

    fn clean_command(fixture: &Path) -> Command {
        let mut command = Command::new(env!("CARGO"));
        command.current_dir(fixture);
        for (name, _) in std::env::vars_os() {
            let text = name.to_string_lossy();
            if text.starts_with("FE2O3_EXTRACT_")
                || text.starts_with("FE2O3_DIAGNOSTIC_")
                || matches!(
                    text.as_ref(),
                    "RUSTFLAGS"
                        | "CARGO_ENCODED_RUSTFLAGS"
                        | "CARGO_BUILD_TARGET"
                        | "RUSTC_WRAPPER"
                        | "RUSTC_WORKSPACE_WRAPPER"
                        | "CARGO_BUILD_RUSTC_WRAPPER"
                        | "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER"
                        | "FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2"
                        | "FE2O3_CRATE_BINDING_ID_V1"
                )
            {
                command.env_remove(name);
            }
        }
        command
            .env("CARGO_INCREMENTAL", "0")
            .env("CARGO_TERM_COLOR", "never");
        command
    }

    fn resolve_fixture(fixture: &Path) {
        let output = clean_command(fixture)
            .args(["metadata", "--offline", "--format-version", "1"])
            .output()
            .expect("resolve only the generated fixture lockfile");
        assert!(
            output.status.success(),
            "fixture resolution: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn extract(
        fixture: &Path,
        target: &ScratchTarget,
        cpu: &str,
        opt: u8,
        case: &str,
        bundle: &Path,
    ) -> Output {
        assert!(
            !bundle.exists(),
            "each actual extraction needs a fresh output"
        );
        clean_command(fixture)
            .env(
                "RUSTC_WORKSPACE_WRAPPER",
                env!("CARGO_BIN_EXE_fe2o3-rustc-extract"),
            )
            .env("FE2O3_EXTRACT_CRATE_V1", CRATE)
            .env("FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V1", bundle)
            .env(
                "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
                format!(
                    "-Zalways-encode-mir -Zinline-mir=no -Zmir-opt-level={} -Copt-level={opt} \
                     -Ctarget-cpu={cpu} -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32",
                    if opt == 0 { 0 } else { 2 },
                ),
            )
            .args([
                "rustc",
                "--release",
                "--offline",
                "--locked",
                "-Zbuild-std=core",
                "--lib",
                "--target",
                "amdgcn-amd-amdhsa",
                "--target-dir",
            ])
            .arg(target.path().join("cargo"))
            // The explicit case also forces the final crate to run when only
            // an observation path changes; dependencies remain reusable.
            .args(["--", "--cfg"])
            .arg(format!("checked_add_case=\"{case}\""))
            .output()
            .expect("run genuine pinned core source extraction")
    }

    fn kernel_source(expression: &str, declarations: &str) -> String {
        format!(
            r#"#![no_std]
use fe2o3_device::{{kernel, thread, DisjointSlice}};
{declarations}
fn checked_add_result(a: u64, b: u64) -> (u64, u8) {{
    match {expression} {{
        Some(value) => (value as u64, 1_u8),
        None => (0_u64, 0_u8),
    }}
}}
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn checked_add_unsigned(a: u64, b: u64, mut output: DisjointSlice<u64>,
                           mut present: DisjointSlice<u8>) {{
    let (value, flag) = checked_add_result(a, b);
    if let core::option::Option::Some(slot) = output.get_mut(thread::index_1d()) {{ *slot = value; }}
    if let core::option::Option::Some(slot) = present.get_mut(thread::index_1d()) {{ *slot = flag; }}
}}
"#
        )
    }

    #[test]
    #[ignore = "requires pinned nightly rust-src and AMD target; CPU execution only"]
    fn genuine_unsigned_checked_add_preserves_option_boundaries() {
        let target = ScratchTarget::new();
        let fixture = materialize_source_safety_fixture(&target, "");
        resolve_fixture(&fixture);
        for cpu in ["gfx942", "gfx950"] {
            for opt in [0, 3] {
                for (integer, bits) in UNSIGNED {
                    let source =
                        kernel_source(&format!("(a as {integer}).checked_add(b as {integer})"), "");
                    std::fs::write(fixture.join("src/lib.rs"), source).unwrap();
                    let case = format!("{cpu}-opt{opt}-{integer}");
                    let bundle_path = target.path().join(format!("{case}.fe2sim"));
                    let output = extract(&fixture, &target, cpu, opt, &case, &bundle_path);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    assert!(output.status.success(), "{case}: {stderr}");
                    let bundle = fe2o3_kernel_ir::VerifiedSimulationBundleV1::from_canonical_bytes(
                        std::fs::read(&bundle_path).unwrap(),
                    )
                    .expect("admit actual compiler-produced bundle");
                    assert_eq!(bundle.kernel_count(), 1);
                    assert_eq!(bundle.target(), format!("{cpu}:xnack-"));
                    assert!(
                        bundle
                            .require_canonical_compiler_execution_association()
                            .is_err()
                    );
                    assert!(
                        bundle
                            .source_lineage()
                            .rustc_identity_inventory_receipt_bytes()
                            > 0
                    );
                    let module =
                        fe2o3_kernel_ir::decode_module_v7(bundle.canonical_kir_v7()).unwrap();
                    assert!(
                        module
                            .functions
                            .iter()
                            .filter_map(|function| function.body.as_ref())
                            .flat_map(|body| &body.blocks)
                            .flat_map(|block| &block.operations)
                            .any(|operation| matches!(
                                operation.kind,
                                OperationKind::Binary {
                                    op: BinaryOp::Checked(CheckedBinaryOperator::Add),
                                    ..
                                }
                            )),
                        "{case}: actual checked arithmetic was not retained",
                    );
                    let max = ((1_u128 << bits) - 1) as u64;
                    for (a, b) in [
                        (0, 0),
                        (0, max),
                        (max, 0),
                        (max - 1, 1),
                        (max, 1),
                        (max, max),
                    ] {
                        let mathematical = u128::from(a) + u128::from(b);
                        let expected =
                            (mathematical <= u128::from(max)).then_some(mathematical as u64);
                        simulate(&target, &bundle_path, &case, a, b, expected);
                    }
                }
            }
        }
        for (case, declarations) in [
            (
                "local-safe-lookalike",
                "mod impostor { #[inline(never)] pub fn checked_add(a: u64, _: u64) -> Option<u64> { Some(a) } }",
            ),
            (
                "local-safe-fake-option",
                "mod impostor { pub enum Option<T> { Some(T), None } #[inline(never)] pub fn checked_add(a: u64, _: u64) -> Option<u64> { Option::Some(a) } } use impostor::Option::{Some, None};",
            ),
        ] {
            std::fs::write(
                fixture.join("src/lib.rs"),
                kernel_source("impostor::checked_add(a, b)", declarations),
            )
            .unwrap();
            let bundle = target.path().join(format!("{case}.fe2sim"));
            let output = extract(&fixture, &target, "gfx942", 0, case, &bundle);
            assert!(
                output.status.success(),
                "{case}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            // A safe local lookalike keeps its actual body; recognizing its
            // spelling as checked addition would incorrectly produce None.
            simulate(&target, &bundle, case, u64::MAX, 1, Some(u64::MAX));
        }
    }

    fn branch_hint_source(expression: &str, declarations: &str) -> String {
        kernel_source(
            &format!("if {expression} {{ Some(b) }} else {{ None }}"),
            declarations,
        )
    }

    #[test]
    #[ignore = "requires pinned nightly rust-src and AMD target; CPU execution only"]
    fn genuine_core_branch_hints_preserve_boolean_values() {
        let target = ScratchTarget::new();
        let fixture = materialize_source_safety_fixture(&target, "");
        resolve_fixture(&fixture);
        for hint in ["likely", "unlikely"] {
            for cpu in ["gfx942", "gfx950"] {
                for opt in [0, 3] {
                    let source = format!(
                        "#![feature(core_intrinsics)]\n{}",
                        branch_hint_source(&format!("core::intrinsics::{hint}(a != 0)"), ""),
                    );
                    std::fs::write(fixture.join("src/lib.rs"), source).unwrap();
                    let case = format!("branch-{hint}-{cpu}-opt{opt}");
                    let bundle = target.path().join(format!("{case}.fe2sim"));
                    let output = extract(&fixture, &target, cpu, opt, &case, &bundle);
                    assert!(
                        output.status.success(),
                        "{case}: {}",
                        String::from_utf8_lossy(&output.stderr),
                    );
                    for condition in [0, 1] {
                        simulate(
                            &target,
                            &bundle,
                            &case,
                            condition,
                            17,
                            (condition != 0).then_some(17),
                        );
                    }
                }
            }
            // A safe same-spelling user function keeps its opposite behavior.
            let declarations = format!(
                "mod impostor {{ #[inline(never)] pub fn {hint}(value: bool) -> bool {{ !value }} }}"
            );
            std::fs::write(
                fixture.join("src/lib.rs"),
                branch_hint_source(&format!("impostor::{hint}(a != 0)"), &declarations),
            )
            .unwrap();
            let case = format!("local-inverted-{hint}");
            let bundle = target.path().join(format!("{case}.fe2sim"));
            let output = extract(&fixture, &target, "gfx942", 0, &case, &bundle);
            assert!(
                output.status.success(),
                "{case}: {}",
                String::from_utf8_lossy(&output.stderr),
            );
            for condition in [0, 1] {
                simulate(
                    &target,
                    &bundle,
                    &case,
                    condition,
                    17,
                    (condition == 0).then_some(17),
                );
            }
        }
    }

    #[test]
    #[ignore = "requires pinned nightly rust-src and AMD target"]
    fn branch_hint_lookalikes_do_not_bypass_source_safety() {
        let target = ScratchTarget::new();
        let fixture = materialize_source_safety_fixture(&target, "");
        let provider = fixture.join("hint-lookalike");
        std::fs::create_dir_all(provider.join("src")).unwrap();
        std::fs::write(
            provider.join("Cargo.toml"),
            "[package]\nname = \"branch-hint-lookalike\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        ).unwrap();
        let manifest_path = fixture.join("Cargo.toml");
        let manifest = std::fs::read_to_string(&manifest_path).unwrap().replace(
            "[dependencies]\n",
            "[dependencies]\nhint_lookalike = { package = \"branch-hint-lookalike\", path = \"hint-lookalike\" }\n",
        );
        std::fs::write(&manifest_path, manifest).unwrap();
        std::fs::write(provider.join("src/lib.rs"), "#![no_std]\n").unwrap();
        resolve_fixture(&fixture);
        for hint in ["likely", "unlikely"] {
            for (shape, definition, expression) in [
                (
                    "bool",
                    format!("pub fn {hint}(value: bool) -> bool {{ value }}"),
                    format!("hint_lookalike::{hint}(a != 0)"),
                ),
                (
                    "wrong-input",
                    format!("pub fn {hint}(value: u8) -> bool {{ value != 0 }}"),
                    format!("hint_lookalike::{hint}(a as u8)"),
                ),
                (
                    "wrong-output",
                    format!("pub fn {hint}(value: bool) -> u8 {{ value as u8 }}"),
                    format!("hint_lookalike::{hint}(a != 0) != 0"),
                ),
                (
                    "wrong-arity",
                    format!("pub fn {hint}(value: bool, _: bool) -> bool {{ value }}"),
                    format!("hint_lookalike::{hint}(a != 0, false)"),
                ),
                (
                    "wrong-abi",
                    format!("pub extern \"C\" fn {hint}(value: bool) -> bool {{ value }}"),
                    format!("hint_lookalike::{hint}(a != 0)"),
                ),
                (
                    "generic",
                    format!("pub fn {hint}<T>(value: bool) -> bool {{ value }}"),
                    format!("hint_lookalike::{hint}::<u8>(a != 0)"),
                ),
            ] {
                std::fs::write(
                    provider.join("src/lib.rs"),
                    format!("#![no_std]\n#[inline(never)]\n{definition}\n"),
                )
                .unwrap();
                std::fs::write(
                    fixture.join("src/lib.rs"),
                    branch_hint_source(&expression, ""),
                )
                .unwrap();
                let case = format!("external-{hint}-{shape}");
                let bundle = target.path().join(format!("{case}.fe2sim"));
                let output = extract(&fixture, &target, "gfx942", 0, &case, &bundle);
                let stderr = String::from_utf8_lossy(&output.stderr);
                assert!(
                    !output.status.success(),
                    "{case}: lookalike unexpectedly admitted"
                );
                assert!(
                    stderr
                        .contains("cannot authenticate the absence of user-provided unsafe blocks")
                        && stderr.contains(&format!("branch_hint_lookalike::{hint}")),
                    "{case}: not the source-safety refusal: {stderr}",
                );
                assert!(
                    !bundle.exists(),
                    "{case}: refused source published a bundle"
                );
            }
            std::fs::write(provider.join("src/lib.rs"), "#![no_std]\n").unwrap();
            for (shape, definition, expression, diagnostic) in [
                (
                    "unsafe-body",
                    format!("pub fn {hint}(value: bool) -> bool {{ unsafe {{ value }} }}"),
                    format!("impostor::{hint}(a != 0)"),
                    "containing a user-provided unsafe block",
                ),
                (
                    "unsafe-signature",
                    format!("pub unsafe fn {hint}(value: bool) -> bool {{ value }}"),
                    format!("unsafe {{ impostor::{hint}(a != 0) }}"),
                    "reaches unsafe function instance",
                ),
            ] {
                let declarations = format!("mod impostor {{ #[inline(never)] {definition} }}");
                std::fs::write(
                    fixture.join("src/lib.rs"),
                    branch_hint_source(&expression, &declarations),
                )
                .unwrap();
                let case = format!("local-{hint}-{shape}");
                let bundle = target.path().join(format!("{case}.fe2sim"));
                let output = extract(&fixture, &target, "gfx942", 0, &case, &bundle);
                let stderr = String::from_utf8_lossy(&output.stderr);
                assert!(
                    !output.status.success(),
                    "{case}: unsafe source unexpectedly admitted"
                );
                assert!(
                    stderr.contains(diagnostic),
                    "{case}: wrong refusal: {stderr}"
                );
                assert!(
                    !bundle.exists(),
                    "{case}: refused source published a bundle"
                );
            }
        }
    }

    fn simulate(
        target: &ScratchTarget,
        bundle: &Path,
        case: &str,
        a: u64,
        b: u64,
        expected: Option<u64>,
    ) {
        let request = target.path().join("checked-add-request.json");
        std::fs::write(
            &request,
            serde_json::to_vec(&json!({
                "schema": "fe2o3-simulation-request-v1",
                "kernel": "checked_add_unsigned",
                "grid": [64, 1, 1],
                "workgroup": [64, 1, 1],
                "arguments": [
                    {"kind": "scalar", "type": "u64", "bits": format!("0x{a:016x}")},
                    {"kind": "scalar", "type": "u64", "bits": format!("0x{b:016x}")},
                    {"kind": "buffer", "element": "u64", "access": "read_write",
                     "alignment": 8, "bytes": format!("0x{}", "a5".repeat(64 * 8))},
                    {"kind": "buffer", "element": "u8", "access": "read_write",
                     "alignment": 1, "bytes": format!("0x{}", "a5".repeat(64))}
                ]
            }))
            .unwrap(),
        )
        .unwrap();
        let admitted =
            fe2o3_kir_sim_cli::load_debug_simulation_bundle_v1(bundle, &request).unwrap();
        let execution = admitted
            .input()
            .module
            .simulate(
                &admitted.input().request,
                admitted.input().simulation_target(),
                admitted.input().simulation_limits,
            )
            .unwrap_or_else(|error| panic!("{case}/{a}/{b}: {error:?}"));
        assert_eq!(execution.invocations_executed(), 64);
        let values = execution.buffer(2).unwrap().bytes();
        assert_eq!(values.len(), 64 * 8);
        let expected_value = expected.unwrap_or(0).to_le_bytes();
        assert!(
            values.chunks_exact(8).all(|value| value == expected_value),
            "{case}/{a}/{b}"
        );
        let flags = execution.buffer(3).unwrap().bytes();
        assert_eq!(
            flags,
            vec![u8::from(expected.is_some()); 64],
            "{case}/{a}/{b}"
        );
    }

    #[test]
    #[ignore = "requires pinned nightly rust-src and AMD target"]
    fn checked_add_lookalikes_do_not_bypass_source_safety() {
        let target = ScratchTarget::new();
        let fixture = materialize_source_safety_fixture(&target, "");
        let provider = fixture.join("lookalike");
        std::fs::create_dir_all(provider.join("src")).unwrap();
        std::fs::write(
            provider.join("Cargo.toml"),
            "[package]\nname = \"checked-add-lookalike\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        ).unwrap();
        let manifest_path = fixture.join("Cargo.toml");
        let manifest = std::fs::read_to_string(&manifest_path).unwrap().replace(
            "[dependencies]\n",
            "[dependencies]\nlookalike = { package = \"checked-add-lookalike\", path = \"lookalike\" }\n",
        );
        std::fs::write(&manifest_path, manifest).unwrap();
        std::fs::write(provider.join("src/lib.rs"), "#![no_std]\n").unwrap();
        resolve_fixture(&fixture);
        let external = [
            (
                "external-core-option",
                "pub fn checked_add(a: u64, _: u64) -> Option<u64> { Some(a) }",
                "lookalike::checked_add(a, b)",
                "",
            ),
            (
                "external-fake-option",
                "pub enum Option<T> { Some(T), None }\npub fn checked_add(a: u64, _: u64) -> Option<u64> { Option::Some(a) }",
                "lookalike::checked_add(a, b)",
                "use lookalike::Option::{Some, None};",
            ),
            (
                "external-wrong-payload",
                "pub fn checked_add(a: u64, _: u64) -> Option<u32> { Some(a as u32) }",
                "lookalike::checked_add(a, b)",
                "",
            ),
            (
                "external-wrong-input",
                "pub fn checked_add(a: u64, _: u32) -> Option<u64> { Some(a) }",
                "lookalike::checked_add(a, b as u32)",
                "",
            ),
            (
                "external-wrong-arity",
                "pub fn checked_add(a: u64, _: u64, _: u64) -> Option<u64> { Some(a) }",
                "lookalike::checked_add(a, b, 0)",
                "",
            ),
        ];
        for (case, definition, expression, declarations) in external {
            std::fs::write(
                provider.join("src/lib.rs"),
                format!("#![no_std]\n{definition}\n"),
            )
            .unwrap();
            std::fs::write(
                fixture.join("src/lib.rs"),
                kernel_source(expression, declarations),
            )
            .unwrap();
            let bundle = target.path().join(format!("{case}.fe2sim"));
            let output = extract(&fixture, &target, "gfx942", 0, case, &bundle);
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                !output.status.success(),
                "{case}: lookalike unexpectedly admitted"
            );
            assert!(
                stderr.contains("cannot authenticate the absence of user-provided unsafe blocks")
                    && stderr.contains("checked_add_lookalike::checked_add"),
                "{case}: not the source-safety refusal: {stderr}",
            );
            assert!(
                !bundle.exists(),
                "{case}: refused source published a bundle"
            );
        }
        std::fs::write(provider.join("src/lib.rs"), "#![no_std]\n").unwrap();
        for (case, expression, declarations, diagnostic) in [
            (
                "local-unsafe-body",
                "impostor::checked_add(a, b)",
                "mod impostor { #[inline(never)] pub fn checked_add(a: u64, _: u64) -> Option<u64> { Some(unsafe { a }) } }",
                "containing a user-provided unsafe block",
            ),
            (
                "local-unsafe-signature",
                "unsafe { impostor::checked_add(a, b) }",
                "mod impostor { #[inline(never)] pub unsafe fn checked_add(a: u64, _: u64) -> Option<u64> { Some(a) } }",
                "reaches unsafe function instance",
            ),
            (
                "signed-unsupported",
                "(a as i64).checked_add(b as i64)",
                "",
                "cannot authenticate the absence of user-provided unsafe blocks",
            ),
            (
                "wide-unsupported",
                "(a as u128).checked_add(b as u128)",
                "",
                "cannot authenticate the absence of user-provided unsafe blocks",
            ),
        ] {
            std::fs::write(
                fixture.join("src/lib.rs"),
                kernel_source(expression, declarations),
            )
            .unwrap();
            let bundle = target.path().join(format!("{case}.fe2sim"));
            let output = extract(&fixture, &target, "gfx942", 0, case, &bundle);
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                !output.status.success(),
                "{case}: unsupported source unexpectedly admitted"
            );
            assert!(
                stderr.contains(diagnostic),
                "{case}: wrong refusal: {stderr}"
            );
            assert!(
                !bundle.exists(),
                "{case}: refused source published a bundle"
            );
        }
        // These controls compile real local/external definitions. The separate
        // contract tests isolate individual shape checks, not forged DefIds.
    }
}
