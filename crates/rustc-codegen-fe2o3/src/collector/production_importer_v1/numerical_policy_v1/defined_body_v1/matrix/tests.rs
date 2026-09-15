//! Registered source import. This gate stops before SSA, MFMA and Global loads.
use super::*;
use crate::rustc_semantic_plan_v1::{
    DebugSourceCaptureRequestV2, build_production_semantic_preflight_plan_v1,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::{Compiler, Config};
use rustc_session::config::Input;
use rustc_span::FileName;
use std::collections::BTreeSet;
use std::path::PathBuf;

mod contracts;
mod reusable_source;
mod source_scope;
mod terminal_abis;
mod accumulator_zero;
mod scoped_custody;
mod scoped_bf16;
mod scoped_bf16_source_inputs;
mod scoped_owner_negatives;

const SOURCE: &str = r#"
#![no_std]
use fe2o3_device::{KernelContext, StrictIeee, SubgroupWidth64, kernel};
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1]))]
pub fn policy_matrix_constructors(mut context: KernelContext<'_>, enabled: u32) {
    if enabled == 0 {
        return;
    }
    let policy = context.numerical_policy::<StrictIeee>();
    context.with_workgroup(|workgroup| {
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        subgroup.with_matrix(workgroup.epoch(), |matrix, _lane| {
            let bound = matrix.with_numerical_policy(&policy);
            let _narrowed = bound.gfx950();
        });
    });
}
"#;

#[derive(Clone, Copy, Eq, PartialEq)]
enum Fixture {
    Matrix,
    Global,
    GlobalLoads,
    ReusableSource,
    TerminalAbi,
    TerminalAbiPhase,
    ScopedCustody,
    ScopedCustodyDirectContext,
    ScopedBf16,
    ScopedBf16Constructors,
    ScopedBf16SourceInputs,
    ScopedBf16Repeated(bool),
}

struct Probe {
    cpu: &'static str,
    fixture: Fixture,
    completed: bool,
}

impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        let repeated = match self.fixture {
            Fixture::ScopedCustody => Some(scoped_custody::source()),
            Fixture::ScopedBf16Repeated(shared_subgroup) => Some(scoped_bf16::repeated_source(shared_subgroup)),
            Fixture::ScopedCustodyDirectContext => Some(scoped_owner_negatives::direct_context_source()),
            _ => None,
        };
        config.input = Input::Str {
            name: FileName::Custom("policy_matrix_import_source.rs".into()),
            input: match self.fixture {
                Fixture::Matrix => SOURCE,
                Fixture::Global => include_str!("tests/global_source.rs"),
                Fixture::GlobalLoads => include_str!("source/global_views/loads/fixture.rs"),
                Fixture::ReusableSource => include_str!("tests/reusable_source_fixture.rs"),
                Fixture::TerminalAbi => include_str!("tests/terminal_source.rs"),
                Fixture::TerminalAbiPhase => include_str!("tests/terminal_phase_source.rs"),
                Fixture::ScopedCustody => repeated.as_deref().unwrap(),
                Fixture::ScopedCustodyDirectContext => repeated.as_deref().unwrap(),
                Fixture::ScopedBf16 | Fixture::ScopedBf16Constructors | Fixture::ScopedBf16SourceInputs => include_str!("tests/scoped_bf16_source.rs"),
                Fixture::ScopedBf16Repeated(_) => repeated.as_deref().unwrap(),
            }
            .into(),
        };
    }

    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        use crate::collector::production_importer_v1::{
            build_identity_inventory_v1, canonical_target_layout_v1,
            construct_production_semantic_mir_v1, validate_execution_terminal_carriage_v1,
        };
        use crate::production_target_v1::RetainedProductionTargetV1;
        let target = RetainedProductionTargetV1::authenticate_live_before_collection(tcx).unwrap();
        let partitions = tcx.collect_and_partition_mono_items(());
        let closure = crate::collector::collect_authenticated_kernel_closure_v1(
            tcx,
            partitions.codegen_units,
            false,
            target,
        )
        .expect("collect registered matrix constructors as original defined helpers");
        let observed_target = RetainedProductionTargetV1::authenticate_live_before_collection(tcx)
            .unwrap()
            .authenticate_import_session(tcx)
            .unwrap();
        let inventory =
            build_identity_inventory_v1(tcx, &observed_target, &closure.collection, &closure.roots)
                .unwrap();
        let closure_types = closure
            .collection
            .functions
            .iter()
            .filter_map(|function| function.closure_plan.as_ref())
            .flat_map(|plan| plan.authenticated_closure_type_identities())
            .map(SemanticTypeIdentityV1::from_sha256)
            .collect::<BTreeSet<_>>();
        let plan = build_production_semantic_preflight_plan_v1(
            tcx,
            canonical_target_layout_v1(observed_target.rustc_layout()),
            inventory.functions,
            inventory.roots,
            inventory.sha256,
            &closure_types,
            DebugSourceCaptureRequestV2::Disabled,
        )
        .expect("retain the actual matrix producer plan, including authenticated closure types");
        if !matches!(self.fixture, Fixture::ScopedBf16 | Fixture::ScopedBf16Constructors | Fixture::ScopedBf16SourceInputs | Fixture::ScopedBf16Repeated(_)) {
            source_scope::check(tcx, &plan);
        }
        if matches!(
            self.fixture,
            Fixture::TerminalAbi | Fixture::TerminalAbiPhase
        ) {
            terminal_abis::check(tcx, &plan, self.fixture == Fixture::TerminalAbiPhase);
            self.completed = true;
            return Compilation::Stop;
        }
        if self.fixture == Fixture::ReusableSource {
            reusable_source::check(tcx, &plan);
        }
        let scoped_roots = matches!(self.fixture, Fixture::ScopedCustody | Fixture::ScopedCustodyDirectContext | Fixture::ScopedBf16 | Fixture::ScopedBf16SourceInputs | Fixture::ScopedBf16Repeated(_)).then(|| {
            crate::compiler_descriptor::typed_descriptor_roots_from_production_collection(
                tcx, &closure.collection.functions,
            ).unwrap()
        });
        let imported = construct_production_semantic_mir_v1(
            tcx,
            closure,
            DebugSourceCaptureRequestV2::Disabled,
        )
        .expect("actual matrix constructor source must complete canonical import");
        assert_eq!(imported.rustc_identity_inventory.sha256(), inventory.sha256);
        assert_eq!(
            imported.rustc_preflight_plan.canonical_transcript(),
            plan.canonical_transcript()
        );
        if self.fixture == Fixture::ScopedBf16Constructors {
            source::global_bf16_constructors::check_import(tcx, &plan, &imported);
            self.completed = true;
            return Compilation::Stop;
        }
        if let Some(roots) = scoped_roots {
            if self.fixture == Fixture::ScopedBf16SourceInputs {
                scoped_bf16_source_inputs::check(&imported, roots);
            } else if matches!(self.fixture, Fixture::ScopedBf16 | Fixture::ScopedBf16Repeated(_)) {
                let repeated = match self.fixture { Fixture::ScopedBf16Repeated(shared) => Some(shared), _ => None };
                scoped_bf16::check(&imported, roots, repeated);
            } else {
                scoped_custody::check(&imported, roots);
                if self.fixture == Fixture::ScopedCustodyDirectContext {
                    scoped_owner_negatives::check_later_context_kills(&imported);
                }
            }
            self.completed = true;
            return Compilation::Stop;
        }
        let mir = &imported.semantic_mir;
        assert_eq!(
            mir.wire_version(),
            if self.fixture == Fixture::ReusableSource {
                SemanticMirWireVersionV1::V23
            } else {
                SemanticMirWireVersionV1::V22
            }
        );
        assert_eq!(mir.roots().len(), 1);
        mir.require_complete_external_entries().unwrap();
        let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
            mir.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(decoded.functions(), mir.functions());
        assert_eq!(decoded.callables(), mir.callables());
        assert_eq!(decoded.canonical_encoding(), mir.canonical_encoding());
        assert!(
            AdmittedInertSemanticMirV1::decode_exact_v21_canonical(
                mir.canonical_encoding(),
                SemanticMirLimitsV1::default()
            )
            .is_err()
        );
        validate_execution_terminal_carriage_v1(tcx, &plan, &imported.kernel_contexts, &decoded)
            .unwrap();
        validate_carriage(tcx, &plan, &imported.kernel_contexts, &decoded).unwrap();
        contracts::check(tcx, &plan, &imported);
        if self.fixture == Fixture::ReusableSource {
            reusable_source::check_import(tcx, &plan, &imported);
        }
        if matches!(self.fixture, Fixture::Global | Fixture::GlobalLoads) {
            source::global_views::check_import(tcx, &plan, &imported);
        }
        if self.fixture == Fixture::GlobalLoads {
            source::global_views::loads::check_import(tcx, &plan, &imported);
        }
        assert_eq!(tcx.sess.opts.cg.target_cpu.as_deref(), Some(self.cpu));
        self.completed = true;
        Compilation::Stop
    }
}

const CRATE_NAME: &str = "policy_matrix_import_source";

#[test]
#[ignore = "requires existing authenticated AMD metadata; no Cargo"]
fn scoped_bf16_original_constructors_gfx942() {
    run("gfx942", "scoped_bf16_original_constructors_gfx942", Fixture::ScopedBf16Constructors);
}

#[test]
#[ignore = "requires existing authenticated AMD metadata; no Cargo"]
fn scoped_bf16_complete_source_inputs_gfx942() {
    run("gfx942", "scoped_bf16_complete_source_inputs_gfx942", Fixture::ScopedBf16SourceInputs);
}

#[test]
#[ignore = "requires existing authenticated AMD metadata; no Cargo"]
fn scoped_bf16_complete_source_inputs_gfx950() {
    run("gfx950", "scoped_bf16_complete_source_inputs_gfx950", Fixture::ScopedBf16SourceInputs);
}

#[test]
#[ignore = "requires existing authenticated AMD metadata; no Cargo"]
fn scoped_bf16_original_constructors_gfx950() {
    run("gfx950", "scoped_bf16_original_constructors_gfx950", Fixture::ScopedBf16Constructors);
}

#[test]
#[ignore = "requires existing authenticated AMD metadata; no Cargo"]
fn policy_gfx950_terminal_abis_gfx942() {
    run(
        "gfx942",
        "policy_gfx950_terminal_abis_gfx942",
        Fixture::TerminalAbi,
    );
}

#[test]
#[ignore = "requires existing authenticated AMD metadata; no Cargo"]
fn policy_gfx950_terminal_abis_gfx950() {
    run(
        "gfx950",
        "policy_gfx950_terminal_abis_gfx950",
        Fixture::TerminalAbi,
    );
}
#[test]
#[ignore = "requires existing authenticated AMD metadata; no Cargo"]
fn policy_gfx950_homogeneous_phase_abis_gfx942() {
    run(
        "gfx942",
        "policy_gfx950_homogeneous_phase_abis_gfx942",
        Fixture::TerminalAbiPhase,
    );
}

#[test]
#[ignore = "requires existing authenticated AMD metadata; no Cargo"]
fn policy_gfx950_homogeneous_phase_abis_gfx950() {
    run(
        "gfx950",
        "policy_gfx950_homogeneous_phase_abis_gfx950",
        Fixture::TerminalAbiPhase,
    );
}

const METADATA: &str = "fe2o3-policy-matrix-import-v1";
const CHILD: &str = "FE2O3_POLICY_MATRIX_IMPORT_CHILD";

fn configured(name: &str) -> PathBuf {
    let path = PathBuf::from(
        std::env::var_os(name)
            .unwrap_or_else(|| panic!("set {name}; this test never builds dependencies")),
    );
    path.canonicalize()
        .unwrap_or_else(|error| panic!("{name}: {error}"))
}

fn run(cpu: &'static str, name: &str, fixture: Fixture) {
    use fe2o3_rustc_invocation::{
        CARGO_METADATA_BUILD_OBSERVATION_ENV_V2, derive_cargo_metadata_build_observation_v2,
    };
    let device = configured("FE2O3_CORE_TRY_DEVICE_RMETA");
    let host = configured("FE2O3_CORE_TRY_HOST_DEPS");
    let core = configured("FE2O3_CORE_TRY_AMDGPU_CORE");
    let builtins = configured("FE2O3_CORE_TRY_AMDGPU_BUILTINS");
    let observation = derive_cargo_metadata_build_observation_v2(&[METADATA]).to_hex();
    let binding =
        reserved_fe2o3_symbols::derive_crate_binding_id_v1(CRATE_NAME, [METADATA]).to_hex();
    let key = format!("{cpu}:{observation}");
    if std::env::var(CHILD).ok().as_deref() != Some(key.as_str()) {
        let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-policy-matrix-source");
        let device_source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../fe2o3-device");
        std::fs::write(scratch.path().join("Cargo.toml"), format!(
            "[package]\nname = \"policy-matrix-import-source\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[workspace]\n[dependencies]\nfe2o3-device = {{ path = {device_source:?} }}\n"
        )).unwrap();
        let test = format!(
            "collector::production_importer_v1::numerical_policy_v1::defined_body_v1::matrix::tests::{name}"
        );
        let output = crate::process_execution::capture_output(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    &test,
                    "--ignored",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
                .env(CHILD, &key)
                .env(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2, &observation)
                .env("FE2O3_CORE_TRY_DEVICE_RMETA", &device)
                .env("FE2O3_CORE_TRY_HOST_DEPS", &host)
                .env("FE2O3_CORE_TRY_AMDGPU_CORE", &core)
                .env("FE2O3_CORE_TRY_AMDGPU_BUILTINS", &builtins)
                .env("CARGO_MANIFEST_DIR", scratch.path())
                .env("CARGO_PKG_NAME", "policy-matrix-import-source")
                .env(reserved_fe2o3_symbols::CRATE_BINDING_ID_ENV_V1, &binding)
                .env("FE2O3_SIMULATION_MODE_V1", "1")
                .env(
                    "FE2O3_SIMULATION_ATTEMPT_V1",
                    "92929292929292929292929292929292",
                ),
        )
        .unwrap();
        assert!(
            output.status.success(),
            "matrix import child:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed"));
        return;
    }
    assert_eq!(
        std::env::var(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2).unwrap(),
        observation
    );
    assert_eq!(
        std::env::var(reserved_fe2o3_symbols::CRATE_BINDING_ID_ENV_V1).unwrap(),
        binding
    );
    let sysroot = crate::process_execution::capture_output(
        std::process::Command::new("rustc").args(["--print", "sysroot"]),
    )
    .unwrap();
    assert!(sysroot.status.success());
    let args = vec![
        "rustc".into(),
        format!("--crate-name={CRATE_NAME}"),
        format!("--out-dir={}", std::env::var("CARGO_MANIFEST_DIR").unwrap()),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout).unwrap().trim().into(),
        "--target=amdgcn-amd-amdhsa".into(),
        format!("-Ctarget-cpu={cpu}"),
        "-Ctarget-feature=-wavefrontsize32,+wavefrontsize64,-xnack".into(),
        "-Zno-codegen".into(),
        "-Zalways-encode-mir".into(),
        "-Zinline-mir=no".into(),
        "-Zmir-enable-passes=-JumpThreading".into(),
        "-Zunstable-options".into(),
        "-Copt-level=0".into(),
        "-Cpanic=abort".into(),
        "-Cdebuginfo=2".into(),
        format!("-Cmetadata={METADATA}"),
        "--extern".into(),
        format!("fe2o3_device={}", device.display()),
        "-L".into(),
        format!("dependency={}", device.parent().unwrap().display()),
        "-L".into(),
        format!("dependency={}", host.display()),
        "--extern".into(),
        format!("noprelude,nounused:core={}", core.display()),
        "--extern".into(),
        format!(
            "noprelude,nounused:compiler_builtins={}",
            builtins.display()
        ),
        "-".into(),
    ];
    let repeated_cases = [Fixture::ScopedBf16, Fixture::ScopedBf16Repeated(false), Fixture::ScopedBf16Repeated(true)];
    let context_cases = [Fixture::ScopedCustody, Fixture::ScopedCustodyDirectContext];
    let cases = if fixture == Fixture::ScopedBf16 && cpu == "gfx950" {
        &repeated_cases[..]
    } else if fixture == Fixture::ScopedCustody {
        &context_cases[..]
    } else {
        std::slice::from_ref(&fixture)
    };
    for &fixture in cases {
        let mut probe = Probe { cpu, fixture, completed: false };
        rustc_driver::run_compiler(&args, &mut probe);
        assert!(probe.completed, "canonical matrix import and substitutions must complete");
    }
}

#[test]
#[ignore = "requires cached FE2O3_CORE_TRY_* AMD metadata; no Cargo or dependency build"]
fn policy_matrix_full_import_gfx942() {
    run(
        "gfx942",
        "policy_matrix_full_import_gfx942",
        Fixture::Matrix,
    );
}

#[test]
#[ignore = "requires cached FE2O3_CORE_TRY_* AMD metadata; no Cargo or dependency build"]
fn policy_matrix_full_import_gfx950() {
    run(
        "gfx950",
        "policy_matrix_full_import_gfx950",
        Fixture::Matrix,
    );
}

#[test]
#[ignore = "requires cached FE2O3_CORE_TRY_* AMD metadata; no Cargo or dependency build"]
fn policy_global_matrix_full_import_gfx942() {
    run(
        "gfx942",
        "policy_global_matrix_full_import_gfx942",
        Fixture::Global,
    );
}

#[test]
#[ignore = "requires cached FE2O3_CORE_TRY_* AMD metadata; no Cargo or dependency build"]
fn policy_global_matrix_full_import_gfx950() {
    run(
        "gfx950",
        "policy_global_matrix_full_import_gfx950",
        Fixture::Global,
    );
}

#[test]
#[ignore = "requires cached FE2O3_CORE_TRY_* AMD metadata; no Cargo or dependency build"]
fn policy_global_fp4_fp8_load_full_import_gfx942() {
    run(
        "gfx942",
        "policy_global_fp4_fp8_load_full_import_gfx942",
        Fixture::GlobalLoads,
    );
}

#[test]
#[ignore = "requires cached FE2O3_CORE_TRY_* AMD metadata; no Cargo or dependency build"]
fn policy_global_fp4_fp8_load_full_import_gfx950() {
    run(
        "gfx950",
        "policy_global_fp4_fp8_load_full_import_gfx950",
        Fixture::GlobalLoads,
    );
}

#[test]
#[ignore = "requires cached FE2O3_CORE_TRY_* AMD metadata; full V23 import, no Cargo"]
fn policy_reusable_matrix_source_contract_gfx942() {
    run(
        "gfx942",
        "policy_reusable_matrix_source_contract_gfx942",
        Fixture::ReusableSource,
    );
}

#[test]
#[ignore = "requires cached FE2O3_CORE_TRY_* AMD metadata; full V23 import, no Cargo"]
fn policy_reusable_matrix_source_contract_gfx950() {
    run(
        "gfx950",
        "policy_reusable_matrix_source_contract_gfx950",
        Fixture::ReusableSource,
    );
}
