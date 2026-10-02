mod core_scalar_enum_return_tests {
    use super::{ScratchTarget, materialize_source_safety_fixture};
    use fe2o3_kernel_ir::{
        AddressSpace, FunctionRole, MemoryEffect, OperationKind, ScalarType, Type,
        VerifiedSimulationBundleV1, analyze_interprocedural_effects_v1, decode_module_v7,
    };
    use serde_json::json;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};

    const CRATE: &str = "fe2o3_production_source_safety_fixture";
    const SOURCE: &str = r#"#![no_std]
use fe2o3_device::{kernel, thread, DisjointSlice};

#[derive(Clone, Copy)]
#[repr(u8)]
enum ReturnedPayload {
    Absent = 3,
    Single(u64) = 11,
    Pair(u32, u8) = 29,
}

#[inline(never)]
fn make_scalar_payload(flag: u64, a: u64, b: u64) -> ReturnedPayload {
    let selected = if flag == 0 {
        ReturnedPayload::Absent
    } else if flag == 1 {
        ReturnedPayload::Single(a)
    } else if flag == 2 {
        ReturnedPayload::Single(b)
    } else {
        ReturnedPayload::Pair(a as u32, b as u8)
    };
    selected
}

#[inline(never)]
fn observe_scalar_payload(flag: u64, a: u64, b: u64) -> (u64, u64, u8, u64, u64, u8) {
    let returned = make_scalar_payload(flag, a, b);
    let copied = returned;
    let first = match returned {
        ReturnedPayload::Absent => (0_u64, 0_u64, 3_u8),
        ReturnedPayload::Single(value) => (value, 0_u64, 11_u8),
        ReturnedPayload::Pair(left, right) => (left as u64, right as u64, 29_u8),
    };
    let second = match copied {
        ReturnedPayload::Absent => (0_u64, 0_u64, 3_u8),
        ReturnedPayload::Single(value) => (value, 0_u64, 11_u8),
        ReturnedPayload::Pair(left, right) => (left as u64, right as u64, 29_u8),
    };
    (first.0, first.1, first.2, second.0, second.1, second.2)
}

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn scalar_enum_return(flag: u64, a: u64, b: u64,
                         mut left: DisjointSlice<u64>, mut right: DisjointSlice<u64>,
                         mut tag: DisjointSlice<u8>, mut copied_left: DisjointSlice<u64>,
                         mut copied_right: DisjointSlice<u64>, mut copied_tag: DisjointSlice<u8>) {
    let result = observe_scalar_payload(flag, a, b);
    if let Some(slot) = left.get_mut(thread::index_1d()) { *slot = result.0; }
    if let Some(slot) = right.get_mut(thread::index_1d()) { *slot = result.1; }
    if let Some(slot) = tag.get_mut(thread::index_1d()) { *slot = result.2; }
    if let Some(slot) = copied_left.get_mut(thread::index_1d()) { *slot = result.3; }
    if let Some(slot) = copied_right.get_mut(thread::index_1d()) { *slot = result.4; }
    if let Some(slot) = copied_tag.get_mut(thread::index_1d()) { *slot = result.5; }
}
"#;

    const POINTER_SOURCE: &str = r#"#![no_std]
use fe2o3_device::{kernel, thread, DisjointSlice};

enum ReturnedPointer<'a> { Absent, Present(&'a u64) }

#[inline(never)]
fn make_pointer_payload(flag: u64, values: &[u64]) -> ReturnedPointer<'_> {
    let selected = if flag == 0 {
        ReturnedPointer::Absent
    } else {
        match values {
            [head, ..] => ReturnedPointer::Present(head),
            [] => ReturnedPointer::Absent,
        }
    };
    selected
}

#[inline(never)]
fn observe_pointer_payload(flag: u64, values: &[u64]) -> u64 {
    match make_pointer_payload(flag, values) {
        ReturnedPointer::Absent => 0,
        ReturnedPointer::Present(pointer) => *pointer,
    }
}

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn pointer_enum_return(flag: u64, values: &[u64], mut output: DisjointSlice<u64>) {
    let result = observe_pointer_payload(flag, values);
    if let Some(slot) = output.get_mut(thread::index_1d()) { *slot = result; }
}
"#;

    const CAPABILITY_SOURCE: &str = r#"#![no_std]
use fe2o3_device::{kernel, thread, DisjointSlice};

enum ReturnedCapability { Absent, Present(thread::ThreadIndex) }

#[inline(never)]
fn make_capability_payload(flag: u64) -> ReturnedCapability {
    let selected = if flag == 0 {
        ReturnedCapability::Absent
    } else {
        ReturnedCapability::Present(thread::index_1d())
    };
    selected
}

#[inline(never)]
fn observe_capability_payload(flag: u64) -> u64 {
    match make_capability_payload(flag) {
        ReturnedCapability::Absent => 0,
        ReturnedCapability::Present(witness) => witness.get() as u64,
    }
}

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn capability_enum_return(flag: u64, mut output: DisjointSlice<u64>) {
    let result = observe_capability_payload(flag);
    if let Some(slot) = output.get_mut(thread::index_1d()) { *slot = result; }
}
"#;

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
            String::from_utf8_lossy(&output.stderr),
        );
    }

    fn extract(
        fixture: &Path,
        target: &ScratchTarget,
        cpu: &str,
        opt: u8,
        case: &str,
        source: &str,
    ) -> (PathBuf, Output) {
        let bundle = target.path().join(format!("{case}.fe2sim"));
        assert!(!bundle.exists(), "an export cannot reuse an old bundle");
        assert_eq!(
            std::fs::read_to_string(fixture.join("src/lib.rs")).unwrap(),
            source,
            "{case}: source changed before extraction",
        );
        let mut command = clean_command(fixture);
        command
            .env(
                "RUSTC_WORKSPACE_WRAPPER",
                env!("CARGO_BIN_EXE_fe2o3-rustc-extract"),
            )
            .env("FE2O3_EXTRACT_CRATE_V1", CRATE)
            .env("FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V1", &bundle)
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
            .args(["--", "--cfg"])
            .arg(format!("scalar_enum_return_case=\"{case}\""));
        // Optional MIR replay retains the original failure, flags and status.
        let output = super::source_failure_mir_v1::run(
            &mut command,
            &[
                "make_scalar_payload",
                "observe_scalar_payload",
                "make_pointer_payload",
                "observe_pointer_payload",
                "make_capability_payload",
                "observe_capability_payload",
            ],
        );
        assert_eq!(
            std::fs::read_to_string(fixture.join("src/lib.rs")).unwrap(),
            source,
            "{case}: source changed during extraction",
        );
        (bundle, output)
    }

    fn inspect_returned_enum(bundle: &VerifiedSimulationBundleV1, case: &str) {
        let module =
            decode_module_v7(bundle.canonical_kir_v7()).expect("decode this source export");
        let helpers: Vec<_> = module
            .functions
            .iter()
            .filter(|function| {
                function.role == FunctionRole::InternalHelper
                    && function.signature.parameters
                        == [
                            Type::Scalar(ScalarType::U64),
                            Type::Scalar(ScalarType::U64),
                            Type::Scalar(ScalarType::U64),
                        ]
                    && function.signature.results.len() == 4
                    && matches!(function.signature.results.first(), Some(Type::Scalar(_)))
                    && function.signature.results[1..]
                        == [
                            Type::Scalar(ScalarType::U64),
                            Type::Scalar(ScalarType::U32),
                            Type::Scalar(ScalarType::U8),
                        ]
            })
            .collect();
        assert_eq!(
            helpers.len(),
            1,
            "{case}: enum-return helper and variant-qualified scalar results not retained",
        );
        let helper = helpers[0];
        let effects = analyze_interprocedural_effects_v1(&module)
            .expect("analyze the unchanged source-produced call graph");
        for internal in module
            .functions
            .iter()
            .filter(|function| function.role == FunctionRole::InternalHelper)
        {
            let body = internal.body.as_ref().expect("retained helper has a body");
            assert!(!body.blocks.is_empty(), "{case}: empty retained helper");
            for operation in body.blocks.iter().flat_map(|block| &block.operations) {
                assert!(
                    !operation.memory_effects().iter().any(|effect| matches!(
                        effect,
                        MemoryEffect::Allocate(AddressSpace::Private)
                            | MemoryEffect::Read(AddressSpace::Private)
                            | MemoryEffect::Write(AddressSpace::Private)
                    )),
                    "{case}: enum-result transport introduced a Private effect: {:?}",
                    operation.kind,
                );
            }
            assert!(
                effects
                    .function(&internal.id)
                    .is_some_and(|decision| decision.is_complete_and_pure()),
                "{case}: retained helper {:?} violates the existing purity contract",
                internal.id,
            );
        }
        assert!(
            module
                .functions
                .iter()
                .filter_map(|function| function.body.as_ref())
                .flat_map(|body| &body.blocks)
                .flat_map(|block| &block.operations)
                .any(|operation| matches!(
                    &operation.kind,
                    OperationKind::Call { callee, arguments }
                        if callee == &helper.id && arguments.len() == 3
                            && operation.results.iter().map(|result| &result.ty)
                                .eq(helper.signature.results.iter())
                )),
            "{case}: enum-return helper was not invoked by the actual graph",
        );
        assert!(
            module.functions.iter().all(|function| {
                function.role != FunctionRole::DeviceFfiExport
                    && function.role != FunctionRole::ExternalImport
            }),
            "{case}: internal enum transport became an external ABI",
        );
    }

    fn simulate(target: &ScratchTarget, bundle: &Path, case: &str, flag: u64, a: u64, b: u64) {
        let request = target.path().join("scalar-enum-return-request.json");
        let widths = [8, 8, 1, 8, 8, 1];
        let mut arguments = vec![
            json!({"kind": "scalar", "type": "u64", "bits": format!("0x{flag:016x}")}),
            json!({"kind": "scalar", "type": "u64", "bits": format!("0x{a:016x}")}),
            json!({"kind": "scalar", "type": "u64", "bits": format!("0x{b:016x}")}),
        ];
        arguments.extend(widths.map(|width| {
            json!({
                "kind": "buffer", "element": if width == 8 { "u64" } else { "u8" },
                "access": "read_write", "alignment": width,
                "bytes": format!("0x{}", "a5".repeat(64 * width)),
            })
        }));
        std::fs::write(
            &request,
            serde_json::to_vec(&json!({
                "schema": "fe2o3-simulation-request-v1",
                "kernel": "scalar_enum_return",
                "grid": [64, 1, 1],
                "workgroup": [64, 1, 1],
                "arguments": arguments,
            }))
            .unwrap(),
        )
        .unwrap();
        let admitted = fe2o3_kir_sim_cli::load_debug_simulation_bundle_v1(bundle, &request)
            .expect("admit the newly produced bundle and exact request");
        let execution = admitted
            .input()
            .module
            .simulate(
                &admitted.input().request,
                admitted.input().simulation_target(),
                admitted.input().simulation_limits,
            )
            .unwrap_or_else(|error| panic!("{case}/{flag}/{a}/{b}: {error:?}"));
        assert_eq!(execution.invocations_executed(), 64);
        // Independent arithmetic oracle: no Rust enum, helper or KIR evaluation.
        let left = match flag {
            0 => 0,
            1 => a,
            2 => b,
            _ => a & 0xffff_ffff,
        };
        let right = if flag > 2 { b & 0xff } else { 0 };
        let tag = if flag == 0 {
            3
        } else if flag <= 2 {
            11
        } else {
            29
        };
        let expected = [left, right, tag, left, right, tag];
        for (field, (width, value)) in widths.into_iter().zip(expected).enumerate() {
            let actual = execution.buffer(field + 3).unwrap().bytes();
            assert_eq!(actual.len(), 64 * width, "{case}: field {field} extent");
            let bits = value.to_le_bytes();
            assert!(
                actual
                    .chunks_exact(width)
                    .all(|lane| lane == &bits[..width]),
                "{case}/{flag}/{a}/{b}: field {field} differs from independent oracle",
            );
        }
    }

    #[test]
    #[ignore = "requires pinned nightly rust-src and AMD target; CPU execution only"]
    fn genuine_scalar_enum_helper_returns_preserve_source_semantics() {
        let target = ScratchTarget::new();
        let fixture = materialize_source_safety_fixture(&target, SOURCE);
        resolve_fixture(&fixture);
        for cpu in ["gfx942", "gfx950"] {
            for opt in [0, 3] {
                let case = format!("scalar-enum-return-{cpu}-opt{opt}");
                let (path, output) = extract(&fixture, &target, cpu, opt, &case, SOURCE);
                assert!(
                    output.status.success(),
                    "{case}: {}",
                    String::from_utf8_lossy(&output.stderr),
                );
                let original = std::fs::read(&path).unwrap();
                let bundle = VerifiedSimulationBundleV1::from_canonical_bytes(original.clone())
                    .expect("admit the original compiler-produced export");
                assert_eq!(bundle.canonical_bytes(), original);
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
                inspect_returned_enum(&bundle, &case);
                for flag in [0, 1, 2, 3, u64::MAX] {
                    for (a, b) in [
                        (0, 0),
                        (1, 255),
                        (u64::MAX, 256),
                        (1_u64 << 32, 1_u64 << 32),
                        (1_u64 << 63, u64::MAX),
                        (0x0123_4567_89ab_cdef, 0xfedc_ba98_7654_3210),
                    ] {
                        simulate(&target, &path, &case, flag, a, b);
                    }
                }
                assert_eq!(
                    std::fs::read(&path).unwrap(),
                    original,
                    "{case}: simulation must not replace the compiler-produced export",
                );
            }
        }
    }

    #[test]
    #[ignore = "requires pinned nightly rust-src and AMD target; no GPU execution"]
    fn pointer_and_capability_enum_returns_remain_refused() {
        const RESULT_REFUSAL: &str =
            "scalar enum helper result requires ordinary scalar-data payloads";
        let target = ScratchTarget::new();
        let fixture = materialize_source_safety_fixture(&target, POINTER_SOURCE);
        resolve_fixture(&fixture);
        // This scalar-data result contract does not qualify provenance-aware
        // Option<&T> returns. The existing positive slice-get parent stays required.
        for (kind, source) in [
            ("pointer", POINTER_SOURCE),
            ("capability", CAPABILITY_SOURCE),
        ] {
            std::fs::write(fixture.join("src/lib.rs"), source).unwrap();
            for cpu in ["gfx942", "gfx950"] {
                for opt in [0, 3] {
                    let case = format!("{kind}-enum-return-{cpu}-opt{opt}");
                    let (path, output) = extract(&fixture, &target, cpu, opt, &case, source);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    assert!(
                        !output.status.success(),
                        "{case}: non-data enum result unexpectedly admitted",
                    );
                    assert!(
                        stderr.contains(RESULT_REFUSAL),
                        "{case}: failed outside the enum-result payload boundary: {stderr}",
                    );
                    assert!(
                        !path.exists(),
                        "{case}: refused enum result published a simulation bundle",
                    );
                }
            }
        }
    }
}
