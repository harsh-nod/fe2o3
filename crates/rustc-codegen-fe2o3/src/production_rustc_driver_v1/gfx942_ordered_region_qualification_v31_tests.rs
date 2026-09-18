//! Opt-in actual-source V31 -> genuine pre-ranked V16 -> CPU/LLVM observations.
//! The parent runs bounded fresh Cargo preparation and isolated rustc children.
//! No reference bindings, production selector, proof bridge or artifact owner is
//! introduced. Neither CPU results nor LLVM text claim physical GPU execution.
//! Reproduce through serialized Cargo with RUSTC pointing at the absolute
//! pinned-nightly binary and FE2O3_TEST_ORDERED_REGION_OUTPUT_V31 naming a new
//! absolute directory; select actual_ordered_region_source_ladder with --ignored.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use fe2o3_kernel_ir::{AccessMode, OperationKind, ScalarType, Type};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, BufferBackingIdV1, BufferViewArgumentV1,
    ScalarBitsV1, SharedBufferV1, SimulationArgumentV1, SimulationLimitsV1, SimulationRequestV1,
    SimulationTargetV1,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBlockIdV1, SemanticMirWireVersionV1, SemanticTerminatorKindV1,
};
use fe2o3_rustc_invocation::CARGO_METADATA_BUILD_OBSERVATION_ENV_V2;
use reserved_fe2o3_symbols::CRATE_BINDING_ID_ENV_V1;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::gfx942_inline_value_qualification_v30_tests::{
    checked, invocation_for_fixture, read_bounded, sanitized,
};
use super::{Callbacks, Compilation, Compiler, TyCtxt};

const OUTPUT_ENV: &str = "FE2O3_TEST_ORDERED_REGION_OUTPUT_V31";
const CHILD_ENV: &str = "FE2O3_TEST_ORDERED_REGION_INPUTS_V31";
const FEATURE_ENV: &str = "FE2O3_TEST_ORDERED_REGION_FEATURE_V31";
const PACKAGE: &str = "fe2o3-production-extraction-fixture";
const CRATE_NAME: &str = "fe2o3_production_extraction_fixture";
const CHILD: &str = "production_rustc_driver_v1::gfx942_ordered_region_qualification_v31_tests::actual_ordered_region_source_child";
const PREFIX: &str = "FE2O3_ORDERED_REGION_OBSERVATION_V31 ";
const FEATURES: [&str; 6] = [
    "ordered-region-v31",
    "ordered-region-unused-v31",
    "ordered-region-alias-v31",
    "ordered-region-dynamic-v31",
    "ordered-region-divergent-v31",
    "ordered-region-wrong-launch-v31",
];

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/production-extraction-device")
        .canonicalize()
        .unwrap()
}

fn source_hash(path: &Path) -> String {
    super::lower_hex_v1(&Sha256::digest(
        read_bounded(path, 16 * 1024 * 1024).unwrap(),
    ))
}

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .canonicalize()
        .unwrap()
}

fn require_current_source() {
    for (relative, expected) in [
        (
            "src/lib.rs",
            include_bytes!("../../tests/fixtures/production-extraction-device/src/lib.rs")
                .as_slice(),
        ),
        (
            "src/ordered_region_v31.rs",
            include_bytes!(
                "../../tests/fixtures/production-extraction-device/src/ordered_region_v31.rs"
            )
            .as_slice(),
        ),
        (
            "Cargo.toml",
            include_bytes!("../../tests/fixtures/production-extraction-device/Cargo.toml")
                .as_slice(),
        ),
    ] {
        assert_eq!(
            read_bounded(&fixture().join(relative), 1024 * 1024).unwrap(),
            expected,
            "rebuild this harness after changing fixture {relative}"
        );
    }
}

fn checked_feature(feature: &str) -> Result<(), &'static str> {
    FEATURES
        .contains(&feature)
        .then_some(())
        .ok_or("unknown or combined ordered-region feature")
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PreparedInvocation {
    schema: String,
    feature: String,
    args: Vec<String>,
    crate_binding: String,
    cargo_observation: String,
    source_sha256: String,
    root_source_sha256: String,
    manifest_sha256: String,
    artifacts_sha256: String,
    metadata_sha256: String,
}

fn derive_record(directory: &Path, feature: &str) -> PreparedInvocation {
    checked_feature(feature).unwrap();
    let fixture = fixture();
    let (args, crate_binding, cargo_observation) =
        invocation_for_fixture(directory, &fixture, PACKAGE, CRATE_NAME, Some(feature));
    PreparedInvocation {
        schema: "fe2o3-test-source-ordered-region-invocation-v31".into(),
        feature: feature.into(),
        args,
        crate_binding,
        cargo_observation,
        source_sha256: source_hash(&fixture.join("src/ordered_region_v31.rs")),
        root_source_sha256: source_hash(&fixture.join("src/lib.rs")),
        manifest_sha256: source_hash(&fixture.join("Cargo.toml")),
        artifacts_sha256: source_hash(&directory.join("dependencies.stdout")),
        metadata_sha256: source_hash(&directory.join("metadata.stdout")),
    }
}

// Exact source-profile failures, not an arbitrary later unsupported-operation
// failure or a compiler/process failure counted as a successful negative.
fn expected_rejection(feature: &str, diagnostic: &str) -> Result<(), &'static str> {
    checked_feature(feature)?;
    if diagnostic.len() > 64 * 1024 {
        return Err("diagnostic exceeds bound");
    }
    let expected = match feature {
        "ordered-region-alias-v31" => "ordered region physical roles must be distinct v0..v63",
        "ordered-region-dynamic-v31" => {
            "ordered region physical role is not an actual MIR constant"
        }
        "ordered-region-divergent-v31" => {
            "ordered region must precede every conditional source edge"
        }
        "ordered-region-wrong-launch-v31" => "ordered region requires an explicit 64x1x1 workgroup",
        _ => return Err("positive source must not be refused"),
    };
    diagnostic
        .contains(expected)
        .then_some(())
        .ok_or("wrong source rejection boundary")
}

fn observe_materialized(
    retained: &crate::production_pipeline::ordered_region_qualification_v31::OrderedRegionObservationOwnerV31,
    feature: &str,
    output: &Path,
) -> Value {
    let owner = retained.materialized();
    assert!(!owner.grants_artifact_or_launch_authority());
    let (inventory, plan) = retained.authenticated_source_identities();
    assert_ne!(inventory, [0; 32]);
    assert_ne!(plan, [0; 32]);
    let semantic = owner.semantic_ssa().source_semantic();
    assert_eq!(semantic.wire_version(), SemanticMirWireVersionV1::V31);
    assert_eq!(semantic.roots().len(), 1);
    assert_eq!(semantic.functions().len(), 1);
    let root = semantic.roots()[0];
    let function = &semantic.functions()[root.index() as usize];
    let calls = function
        .blocks()
        .iter()
        .enumerate()
        .filter_map(|(index, block)| match block.terminator().kind() {
            SemanticTerminatorKindV1::Call(call) if call.ordered_region_source_v31().is_some() => {
                Some((index, call))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 1);
    let (block, call) = calls[0];
    assert_eq!(call.arguments().len(), 8);
    let checked_call = semantic
        .checked_gfx942_ordered_region_call_v31(
            root,
            SemanticBlockIdV1::from_index(u32::try_from(block).unwrap()),
        )
        .unwrap();
    let source = checked_call.source();
    assert_eq!(source.function(), function.identity());
    let executable = owner.executable();
    let module = executable.module();
    assert_eq!(module.functions.len(), 1);
    assert_eq!(module.kernels.len(), 1);
    let operations = module.functions[0]
        .body
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    let regions = operations
        .iter()
        .filter_map(|operation| match &operation.kind {
            OperationKind::Gfx942OrderedRegion(region) => Some((operation, region)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        regions.len(),
        1,
        "even an unused region must survive source materialization"
    );
    let (operation, region) = regions[0];
    assert_eq!(operation.results.len(), 1);
    assert_eq!(operation.results[0].ty, Type::Scalar(ScalarType::U32));
    assert_eq!(
        region.source(),
        fe2o3_kernel_ir::AssemblySourceIdentity::new(
            source.frontend_unit(),
            *source.function().as_bytes(),
            source.contract(),
            source.statement()
        )
    );
    assert_eq!(region.registers().scratch(), 32);
    assert_eq!(region.registers().output(), 33);
    assert_eq!(region.registers().inputs(), [34, 35, 36]);
    assert!(operations.iter().any(|operation|
        matches!(&operation.kind, OperationKind::InlineAssembly(asm) if asm.mnemonic == "v_mov_b32")),
        "V31 composes the existing V30 source marker without relabeling it");
    fs::write(
        output.join("canonical-v16.bin"),
        executable.canonical_bytes(),
    )
    .unwrap();
    let llvm =
        fe2o3_amdgcn_model::lower_canonical_v16_compiler_module_to_gfx942_xnack_minus_llvm_ir(
            executable,
        )
        .unwrap();
    assert_eq!(llvm.matches("v_xor_b32_e32").count(), 1);
    assert_eq!(llvm.matches("v_add_u32_e32").count(), 1);
    let line = llvm
        .lines()
        .find(|line| line.contains("v_xor_b32_e32"))
        .unwrap();
    assert!(line.contains("asm sideeffect") && line.contains("v_add_u32_e32"));
    assert!(line.contains("=&{v33},{v34},{v35},{v36},~{v32}"));
    assert!(line.find("v_xor_b32_e32").unwrap() < line.find("v_add_u32_e32").unwrap());
    fs::write(output.join("observation.ll"), &llvm).unwrap();
    let cases = observe_cpu(executable, feature == FEATURES[1]);
    json!({
        "stage": "actual_source_v31_exact_v16_cpu_and_llvm_observed",
        "semantic_sha256": super::lower_hex_v1(semantic.semantic_sha256().as_bytes()),
        "canonical_v16_identity": super::lower_hex_v1(executable.identity().digest()),
        "canonical_bytes_sha256": super::lower_hex_v1(&Sha256::digest(executable.canonical_bytes())),
        "canonical_v16_length": executable.canonical_bytes().len(),
        "llvm_sha256": super::lower_hex_v1(&Sha256::digest(llvm.as_bytes())),
        "rustc_identity_inventory_sha256": super::lower_hex_v1(&inventory),
        "rustc_preflight_plan_sha256": super::lower_hex_v1(&plan),
        "region_source_ids": ([source.frontend_unit(), *source.function().as_bytes(), source.contract(), source.statement()]
            .map(|id| super::lower_hex_v1(&id))),
        "region_count": regions.len(), "cpu_cases": cases, "lanes_per_case": 64,
        "canaries_unchanged": true, "unused_result_retained": feature == FEATURES[1],
    })
}

fn observe_cpu(
    executable: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV16,
    unused: bool,
) -> usize {
    let admitted =
        AdmittedSimulationModuleV1::admit_v16(executable, SimulationLimitsV1::default()).unwrap();
    let target = SimulationTargetV1::amdgpu_64();
    let cases = [
        (0, 0, 0),
        (u32::MAX, 0, 1),
        (u32::MAX, 1, 2),
        (0x8000_0000, 0, 0x8000_0000),
        (0xaaaa_5555, 0x5555_aaaa, 19),
        (19, 23, 42),
    ];
    let parameters = &executable.module().functions[0].signature.parameters;
    assert_eq!(parameters.len(), 4);
    assert!(matches!(parameters[0], Type::Slice(_)));
    assert!(
        parameters[1..]
            .iter()
            .all(|ty| *ty == Type::Scalar(ScalarType::U32))
    );
    for (a, b, c) in cases {
        let backing = BufferBackingIdV1(1);
        let buffer = BufferArgumentV1::new(
            ScalarType::U32,
            AccessMode::ReadWrite,
            4,
            vec![0xa5; 66 * 4],
            vec![false; 66 * 4],
            target,
        )
        .unwrap();
        let view = BufferViewArgumentV1::new(
            backing,
            ScalarType::U32,
            AccessMode::ReadWrite,
            4,
            4,
            64,
            target,
        )
        .unwrap();
        let mut arguments = vec![SimulationArgumentV1::BufferView(view)];
        arguments.extend([a, b, c].map(|value| {
            SimulationArgumentV1::Scalar(
                ScalarBitsV1::new(ScalarType::U32, u128::from(value), target).unwrap(),
            )
        }));
        let request = SimulationRequestV1::new(
            executable.module().kernels[0].id.clone(),
            [64, 1, 1],
            [64, 1, 1],
            arguments,
        )
        .with_shared_buffers(vec![SharedBufferV1 {
            id: backing,
            buffer,
        }]);
        let original = request.clone();
        let execution = admitted
            .simulate(&request, target, SimulationLimitsV1::default())
            .unwrap();
        assert_eq!(request, original);
        assert!(!execution.grants_execution_authority());
        assert_eq!(execution.identity().wire_version(), 16);
        assert_eq!(
            execution.identity().digest(),
            executable.identity().digest()
        );
        let output = execution.shared_buffer(backing).unwrap();
        // Independent Rust arithmetic, not the region descriptor's evaluator.
        let expected = if unused { a } else { (a ^ b).wrapping_add(c) };
        for lane in 0..64 {
            assert_eq!(
                &output.bytes()[4 + lane * 4..8 + lane * 4],
                &expected.to_le_bytes()
            );
        }
        assert!(output.initialized()[4..65 * 4].iter().all(|value| *value));
        for range in [0..4, 65 * 4..66 * 4] {
            assert_eq!(&output.bytes()[range.clone()], &[0xa5; 4]);
            assert_eq!(&output.initialized()[range], &[false; 4]);
        }
    }
    cases.len()
}

struct RegionCallbacks<'a> {
    feature: &'a str,
    output: &'a Path,
    calls: usize,
    result: Option<Result<Value, String>>,
}

impl Callbacks for RegionCallbacks<'_> {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        self.result = Some(
            super::transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )
            .and_then(|transaction| transaction.observe_ordered_region_v31())
            .map(|owner| observe_materialized(&owner, self.feature, self.output)),
        );
        Compilation::Stop
    }
}

#[test]
#[ignore = "isolated actual AMD rustc child; use actual_ordered_region_source_ladder"]
fn actual_ordered_region_source_child() {
    let directory =
        PathBuf::from(std::env::var_os(CHILD_ENV).expect("missing preparation directory"));
    let feature = std::env::var(FEATURE_ENV).expect("missing feature");
    checked_feature(&feature).unwrap();
    require_current_source();
    let actual = derive_record(&directory, &feature);
    let retained: PreparedInvocation = serde_json::from_slice(
        &read_bounded(
            &directory.join(format!("{feature}.invocation.json")),
            64 * 1024,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(actual, retained, "stale or substituted preparation");
    assert_eq!(
        std::env::var(CRATE_BINDING_ID_ENV_V1).unwrap(),
        actual.crate_binding
    );
    assert_eq!(
        std::env::var(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2).unwrap(),
        actual.cargo_observation
    );
    assert_eq!(
        std::env::var("CARGO_MANIFEST_DIR").unwrap(),
        fixture().to_str().unwrap()
    );
    assert_eq!(std::env::var("CARGO_PKG_NAME").unwrap(), PACKAGE);
    assert_eq!(std::env::var("CARGO_PKG_VERSION").unwrap(), "0.1.0");
    assert_eq!(std::env::var("CARGO_CRATE_NAME").unwrap(), CRATE_NAME);
    super::require_canonical_overflow_checks_v1(&actual.args).unwrap();
    let output = directory.join(&feature);
    fs::create_dir(&output).unwrap();
    let mut callbacks = RegionCallbacks {
        feature: &feature,
        output: &output,
        calls: 0,
        result: None,
    };
    rustc_driver::run_compiler(&actual.args, &mut callbacks);
    assert_eq!(callbacks.calls, 1);
    let result = callbacks.result.expect("actual callback not reached");
    let observation = if FEATURES[..2].contains(&feature.as_str()) {
        result.unwrap()
    } else {
        let diagnostic = result.expect_err("invalid source unexpectedly materialized");
        expected_rejection(&feature, &diagnostic).unwrap();
        assert!(fs::read_dir(&output).unwrap().next().is_none());
        json!({"stage": "actual_source_profile_refused", "diagnostic": diagnostic})
    };
    require_current_source();
    assert!(
        fs::read_dir(directory.join("analysis-output"))
            .unwrap()
            .next()
            .is_none()
    );
    println!(
        "\n{PREFIX}{}",
        serde_json::to_string(&json!({
            "schema": "fe2o3-test-source-ordered-region-observation-v31", "feature": feature,
            "invocation": actual, "observation": observation, "actual_rustc_callback": true,
            "source_unchanged": true, "proof_executed": false, "final_production_admitted": false,
            "grants_artifact_or_launch_authority": false, "hardware_observed": false,
        }))
        .unwrap()
    );
}

#[test]
#[ignore = "pinned-nightly real-source ladder; serialize Cargo and provide a fresh absolute output directory"]
fn actual_ordered_region_source_ladder() {
    let directory = PathBuf::from(
        std::env::var_os(OUTPUT_ENV).expect("set a fresh task-owned output directory"),
    );
    assert!(directory.is_absolute());
    fs::create_dir(&directory).unwrap();
    require_current_source();
    let rustc_path = PathBuf::from(
        std::env::var_os("RUSTC")
            .expect("set RUSTC to the absolute pinned-nightly compiler; no manager auto-install"),
    );
    assert!(rustc_path.is_absolute());
    let mut rustc = Command::new(rustc_path);
    let sysroot = checked(
        sanitized(&mut rustc).args(["--print", "sysroot"]),
        &directory,
        "sysroot",
        None,
    );
    let sysroot = PathBuf::from(std::str::from_utf8(&sysroot).unwrap().trim_end());
    assert!(
        sysroot
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("nightly-2026-04-03-")
    );
    let mut metadata = Command::new(sysroot.join("bin/cargo"));
    checked(
        sanitized(&mut metadata)
            .args([
                "metadata",
                "--locked",
                "--offline",
                "--no-deps",
                "--format-version=1",
                "--manifest-path",
            ])
            .arg(fixture().join("Cargo.toml")),
        &directory,
        "metadata",
        None,
    );
    let target = directory.join("dependencies");
    let mut cargo = Command::new(sysroot.join("bin/cargo"));
    checked(sanitized(&mut cargo).current_dir(repository()).args(["check", "--release", "--locked", "--offline", "-Zbuild-std=core", "-p", "fe2o3-device", "--target", "amdgcn-amd-amdhsa", "--message-format=json", "--manifest-path"])
        .arg(fixture().join("Cargo.toml")).arg("--target-dir").arg(&target)
        .env("RUSTC", sysroot.join("bin/rustc"))
        .env("CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS", "-Zalways-encode-mir -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32 -Coverflow-checks=on"),
        &directory, "dependencies", Some(&target));
    fs::create_dir(directory.join("analysis-output")).unwrap();
    let mut observations = Vec::new();
    for feature in FEATURES {
        let record = derive_record(&directory, feature);
        fs::write(
            directory.join(format!("{feature}.invocation.json")),
            serde_json::to_vec_pretty(&record).unwrap(),
        )
        .unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap());
        let stdout = checked(
            sanitized(&mut child)
                // Preserve real workspace-relative provider paths without
                // changing the running process directory or environment.
                .current_dir(repository())
                .args(["--exact", CHILD, "--ignored", "--nocapture"])
                .env(CHILD_ENV, &directory)
                .env(FEATURE_ENV, feature)
                .env(CRATE_BINDING_ID_ENV_V1, &record.crate_binding)
                .env(
                    CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
                    &record.cargo_observation,
                )
                .env("CARGO_MANIFEST_DIR", fixture())
                .env("CARGO_PKG_NAME", PACKAGE)
                .env("CARGO_PKG_VERSION", "0.1.0")
                .env("CARGO_CRATE_NAME", CRATE_NAME),
            &directory,
            feature,
            None,
        );
        let stdout = std::str::from_utf8(&stdout).unwrap();
        let lines = stdout
            .lines()
            .filter_map(|line| line.strip_prefix(PREFIX))
            .collect::<Vec<_>>();
        assert_eq!(lines.len(), 1);
        assert_eq!(
            stdout
                .lines()
                .filter(|line| line.starts_with("test result: ok. 1 passed; 0 failed; 0 ignored;"))
                .count(),
            1
        );
        let observation: Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(observation["feature"], feature);
        assert_eq!(
            observation["invocation"],
            serde_json::to_value(record).unwrap()
        );
        observations.push(observation);
    }
    require_current_source();
    fs::write(
        directory.join("observation.json"),
        serde_json::to_vec_pretty(&json!({
            "schema": "fe2o3-test-source-ordered-region-ladder-v31", "observations": observations,
            "grants_artifact_or_launch_authority": false, "hardware_observed": false,
        }))
        .unwrap(),
    )
    .unwrap();
    eprintln!(
        "ordered-region actual source observations retained at {}",
        directory.display()
    );
}

#[test]
fn ordered_region_source_controls_reject_wrong_stage_or_feature() {
    for feature in &FEATURES[2..] {
        assert!(expected_rejection(feature, "generic unsupported KIR operation").is_err());
        assert!(expected_rejection(feature, "actual compiler process failed").is_err());
    }
    assert!(checked_feature("ordered-region-v31,ordered-region-alias-v31").is_err());
    assert!(checked_feature("../other").is_err());
    assert!(
        expected_rejection(
            FEATURES[0],
            "ordered region physical roles must be distinct v0..v63"
        )
        .is_err()
    );
    assert!(serde_json::from_str::<PreparedInvocation>("{}").is_err());
}
