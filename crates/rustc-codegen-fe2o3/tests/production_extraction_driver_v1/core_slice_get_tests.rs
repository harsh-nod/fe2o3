mod core_slice_get_tests {
    use super::{ScratchTarget, materialize_source_safety_fixture};
    use serde_json::json;
    use std::path::Path;
    use std::process::{Command, Output};

    const CRATE: &str = "fe2o3_production_source_safety_fixture";

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
            .arg(format!("slice_get_case=\"{case}\""))
            .output()
            .expect("run genuine pinned core source extraction")
    }

    fn kernel_source(element: &str, expression: &str, value: &str, declarations: &str) -> String {
        format!(
            r#"#![no_std]
use fe2o3_device::{{kernel, thread, DisjointSlice}};
{declarations}
fn slice_get_result(input: &[{element}], index: u64) -> (u64, u8) {{
    match {expression} {{
        Some(value) => ({value}, 1_u8),
        None => (0_u64, 0_u8),
    }}
}}
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn slice_get_shared(input: &[{element}], index: u64, mut output: DisjointSlice<u64>,
                        mut present: DisjointSlice<u8>) {{
    let (value, flag) = slice_get_result(input, index);
    if let core::option::Option::Some(slot) = output.get_mut(thread::index_1d()) {{ *slot = value; }}
    if let core::option::Option::Some(slot) = present.get_mut(thread::index_1d()) {{ *slot = flag; }}
}}
"#
        )
    }

    fn admit_actual_source(
        fixture: &Path,
        target: &ScratchTarget,
        source: &str,
        cpu: &str,
        opt: u8,
        case: &str,
    ) -> std::path::PathBuf {
        let source_path = fixture.join("src/lib.rs");
        std::fs::write(&source_path, source).unwrap();
        let path = target.path().join(format!("{case}.fe2sim"));
        let output = extract(fixture, target, cpu, opt, case, &path);
        assert_eq!(std::fs::read_to_string(source_path).unwrap(), source);
        assert!(
            output.status.success(),
            "{case}: {}",
            String::from_utf8_lossy(&output.stderr),
        );
        let bundle = fe2o3_kernel_ir::VerifiedSimulationBundleV1::from_canonical_bytes(
            std::fs::read(&path).unwrap(),
        )
        .expect("admit this run's actual compiler-produced bundle");
        assert_eq!(bundle.kernel_count(), 1);
        assert_eq!(bundle.target(), format!("{cpu}:xnack-"));
        assert!(
            bundle
                .require_canonical_compiler_execution_association()
                .is_err()
        );
        let lineage = bundle.source_lineage();
        assert!(lineage.rustc_identity_inventory_receipt_bytes() > 0);
        assert_ne!(lineage.rustc_identity_inventory_receipt_sha256(), [0; 32]);
        assert!(lineage.rustc_preflight_plan_receipt_bytes() > 0);
        assert_ne!(lineage.rustc_preflight_plan_receipt_sha256(), [0; 32]);
        path
    }

    #[test]
    #[ignore = "requires pinned nightly rust-src and AMD target; CPU execution only"]
    fn genuine_core_shared_slice_get_preserves_boundaries() {
        let target = ScratchTarget::new();
        let fixture = materialize_source_safety_fixture(&target, "");
        resolve_fixture(&fixture);
        for cpu in ["gfx942", "gfx950"] {
            for opt in [0, 3] {
                for (element, bytes, values) in [
                    ("u8", 1, [7, 129, 255]),
                    ("u32", 4, [7, 65_537, u64::from(u32::MAX)]),
                    ("u64", 8, [7, 4_294_967_297, u64::MAX]),
                ] {
                    for owner in ["inherent", "trait"] {
                        let expression = if owner == "inherent" {
                            "input.get(index as usize)".to_owned()
                        } else {
                            format!(
                                "<usize as core::slice::SliceIndex<[{element}]>>::get(index as usize, input)"
                            )
                        };
                        let mut source = kernel_source(element, &expression, "*value as u64", "");
                        if owner == "trait" {
                            source = format!("#![feature(slice_index_methods)]\n{source}");
                        }
                        let case = format!("{owner}-{element}-{cpu}-opt{opt}");
                        let bundle =
                            admit_actual_source(&fixture, &target, &source, cpu, opt, &case);
                        for input in [&values[..0], &values[..1], &values[..]] {
                            for index in [0, 1, 2, 3, u64::MAX] {
                                // Independent positional oracle; never call Rust slice get.
                                let expected =
                                    input.iter().enumerate().find_map(|(position, value)| {
                                        (position as u64 == index).then_some(*value)
                                    });
                                simulate(
                                    &target, &bundle, &case, element, bytes, input, index, expected,
                                );
                            }
                        }
                    }
                }
            }
        }
        // Safe user code with the same spelling is not replaced by a core operation.
        for (case, declarations) in [
            (
                "local-safe-lookalike",
                "mod impostor { #[inline(never)] pub fn get(_: &[u32], _: usize) -> Option<&u32> { None } }",
            ),
            (
                "local-safe-fake-option",
                "mod impostor { pub enum Option<T> { Some(T), None } #[inline(never)] pub fn get(_: &[u32], _: usize) -> Option<&u32> { Option::None } } use impostor::Option::{Some, None};",
            ),
        ] {
            let source = kernel_source(
                "u32",
                "impostor::get(input, index as usize)",
                "*value as u64",
                declarations,
            );
            let bundle = admit_actual_source(&fixture, &target, &source, "gfx942", 0, case);
            simulate(&target, &bundle, case, "u32", 4, &[17, 29], 0, None);
        }
    }

    fn simulate(
        target: &ScratchTarget,
        bundle: &Path,
        case: &str,
        element: &str,
        element_bytes: usize,
        input: &[u64],
        index: u64,
        expected: Option<u64>,
    ) {
        use std::fmt::Write as _;
        let mut input_bytes = Vec::new();
        let mut encoded = String::from("0x");
        for value in input {
            for byte in &value.to_le_bytes()[..element_bytes] {
                input_bytes.push(*byte);
                write!(&mut encoded, "{byte:02x}").unwrap();
            }
        }
        let request = target.path().join("slice-get-request.json");
        std::fs::write(
            &request,
            serde_json::to_vec(&json!({
                "schema": "fe2o3-simulation-request-v1",
                "kernel": "slice_get_shared",
                "grid": [64, 1, 1],
                "workgroup": [64, 1, 1],
                "arguments": [
                    {"kind": "buffer", "element": element, "access": "read_only",
                     "alignment": element_bytes, "bytes": encoded},
                    {"kind": "scalar", "type": "u64", "value": index},
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
            .unwrap_or_else(|error| panic!("{case}/{index}: {error:?}"));
        assert_eq!(execution.invocations_executed(), 64);
        assert_eq!(execution.buffer(0).unwrap().bytes(), input_bytes);
        let values = execution.buffer(2).unwrap().bytes();
        assert_eq!(values.len(), 64 * 8);
        let expected_value = expected.unwrap_or(0).to_le_bytes();
        assert!(
            values.chunks_exact(8).all(|value| value == expected_value),
            "{case}/{index}"
        );
        assert_eq!(
            execution.buffer(3).unwrap().bytes(),
            vec![u8::from(expected.is_some()); 64],
            "{case}/{index}"
        );
    }

    fn refuse_source(
        fixture: &Path,
        target: &ScratchTarget,
        source: &str,
        case: &str,
        diagnostic: &str,
        owner: &str,
    ) {
        std::fs::write(fixture.join("src/lib.rs"), source).unwrap();
        let bundle = target.path().join(format!("{case}.fe2sim"));
        let output = extract(fixture, target, "gfx942", 0, case, &bundle);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !output.status.success(),
            "{case}: source unexpectedly admitted"
        );
        assert!(
            stderr.contains(diagnostic) && stderr.contains(owner),
            "{case}: wrong refusal: {stderr}"
        );
        assert!(
            !bundle.exists(),
            "{case}: refused source published a bundle"
        );
    }

    #[test]
    #[ignore = "requires pinned nightly rust-src and AMD target"]
    fn slice_get_lookalikes_and_other_owners_do_not_bypass_source_safety() {
        let target = ScratchTarget::new();
        let fixture = materialize_source_safety_fixture(&target, "");
        let provider = fixture.join("lookalike");
        std::fs::create_dir_all(provider.join("src")).unwrap();
        std::fs::write(
            provider.join("Cargo.toml"),
            "[package]\nname = \"slice-get-lookalike\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        let manifest_path = fixture.join("Cargo.toml");
        let manifest = std::fs::read_to_string(&manifest_path).unwrap().replace(
            "[dependencies]\n",
            "[dependencies]\nlookalike = { package = \"slice-get-lookalike\", path = \"lookalike\" }\n",
        );
        std::fs::write(&manifest_path, manifest).unwrap();
        std::fs::write(provider.join("src/lib.rs"), "#![no_std]\n").unwrap();
        resolve_fixture(&fixture);
        const EXTERNAL: &str = "cannot authenticate the absence of user-provided unsafe blocks";
        for (case, definition, expression, declarations) in [
            (
                "external-core-option",
                "pub fn get(_: &[u32], _: usize) -> Option<&u32> { None }",
                "lookalike::get(input, index as usize)",
                "",
            ),
            (
                "external-fake-option",
                "pub enum Option<T> { Some(T), None } pub fn get(_: &[u32], _: usize) -> Option<&u32> { Option::None }",
                "lookalike::get(input, index as usize)",
                "use lookalike::Option::{Some, None};",
            ),
            (
                "external-wrong-index",
                "pub fn get(_: &[u32], _: u64) -> Option<&u32> { None }",
                "lookalike::get(input, index)",
                "",
            ),
            (
                "external-wrong-payload",
                "pub fn get(_: &[u32], _: usize) -> Option<&u64> { None }",
                "lookalike::get(input, index as usize)",
                "",
            ),
            (
                "external-wrong-arity",
                "pub fn get(_: &[u32], _: usize, _: u32) -> Option<&u32> { None }",
                "lookalike::get(input, index as usize, 0)",
                "",
            ),
            (
                "external-wrong-abi",
                "pub extern \"C\" fn get(_: &[u32], _: usize) -> Option<&u32> { None }",
                "lookalike::get(input, index as usize)",
                "",
            ),
            (
                "external-raw-input",
                "pub fn get(_: *const u32, _: usize) -> Option<&'static u32> { None }",
                "lookalike::get(input as *const [u32] as *const u32, index as usize)",
                "",
            ),
            (
                "external-trait-impostor",
                "pub trait SliceIndex<T: ?Sized> { fn get(self, input: &T) -> Option<&u32>; } impl SliceIndex<[u32]> for usize { #[inline(never)] fn get(self, _: &[u32]) -> Option<&u32> { None } }",
                "<usize as lookalike::SliceIndex<[u32]>>::get(index as usize, input)",
                "",
            ),
        ] {
            std::fs::write(
                provider.join("src/lib.rs"),
                format!("#![no_std]\n{definition}\n"),
            )
            .unwrap();
            let source = kernel_source("u32", expression, "*value as u64", declarations);
            refuse_source(
                &fixture,
                &target,
                &source,
                case,
                EXTERNAL,
                "slice_get_lookalike",
            );
        }
        std::fs::write(provider.join("src/lib.rs"), "#![no_std]\n").unwrap();
        for (case, expression, value, declarations, diagnostic, owner) in [
            (
                "core-range",
                "input.get(index as usize..)",
                "value.len() as u64",
                "",
                EXTERNAL,
                "::get",
            ),
            (
                "core-get-mut",
                "{ let mut values = [7_u32, 13_u32]; match values.get_mut(index as usize) { Some(value) => Some(*value), None => None } }",
                "value as u64",
                "",
                EXTERNAL,
                "::get_mut",
            ),
            (
                "core-get-unchecked",
                "Some(unsafe { input.get_unchecked(index as usize) })",
                "*value as u64",
                "",
                "reaches unsafe function instance",
                "::get_unchecked",
            ),
            (
                "local-unsafe-body",
                "impostor::get(input, index as usize)",
                "*value as u64",
                "mod impostor { #[inline(never)] pub fn get(_: &[u32], _: usize) -> Option<&u32> { unsafe { None } } }",
                "containing a user-provided unsafe block",
                "impostor::get",
            ),
            (
                "local-unsafe-signature",
                "unsafe { impostor::get(input, index as usize) }",
                "*value as u64",
                "mod impostor { #[inline(never)] pub unsafe fn get(_: &[u32], _: usize) -> Option<&u32> { None } }",
                "reaches unsafe function instance",
                "impostor::get",
            ),
        ] {
            let source = kernel_source("u32", expression, value, declarations);
            refuse_source(&fixture, &target, &source, case, diagnostic, owner);
        }
    }
}
