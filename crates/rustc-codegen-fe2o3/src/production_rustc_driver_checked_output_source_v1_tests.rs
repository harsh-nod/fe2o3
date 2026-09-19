//! Ordinary Rust through the checked-output stage, without a shipping selector.
use super::*;
use crate::production_pipeline::checked_output_policy4_v1::snapshots;
use fe2o3_kernel_ir::OperationKind;
use fe2o3_rustc_invocation::{
    CARGO_METADATA_BUILD_OBSERVATION_ENV_V2, PortablePackageIdentityV1, RustcInvocationV2,
    classify_rustc_invocation_v2, derive_cargo_metadata_build_observation_v2,
    ordered_rustc_codegen_metadata_v1, portable_rustc_metadata_v1,
};
use reserved_fe2o3_symbols::{CRATE_BINDING_ID_ENV_V1, derive_crate_binding_id_v1};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::ffi::OsString;
use std::process::Command;

#[path = "production_context_source_v29_tests.rs"]
mod context_source_v29_tests;

#[path = "production_rustc_driver_conditional_coverage_v1_tests.rs"]
mod conditional_coverage;
#[path = "production_rustc_driver_conditional_ranked_output_v1_tests.rs"]
mod conditional_ranked_output;
#[path = "production_rustc_driver_fixed_census_lifecycle_v1_tests.rs"]
mod fixed_census_lifecycle;
#[path = "production_rustc_driver_fixed_census_observation_v1_tests.rs"]
mod fixed_census_observation;
#[path = "production_rustc_driver_helper_reference_source_v1_tests.rs"]
mod helper_reference_source;
#[path = "production_rustc_driver_integer_identity_source_v1_tests.rs"]
mod integer_identity_source;
#[path = "production_rustc_driver_loop_capture_source_v1_tests.rs"]
mod loop_capture_source;
#[path = "production_rustc_driver_redundant_store_source_v1_tests.rs"]
mod redundant_store_source;
#[path = "production_rustc_driver_wave64_capture_source_v1_tests.rs"]
mod wave64_capture_source;

const CHILD_ARGS: &str = "FE2O3_TEST_CHECKED_OUTPUT_ARGS_V1";
const CHILD_RESULT: &str = "FE2O3_TEST_CHECKED_OUTPUT_RESULT_V1";
const CHILD_PROOF_PROBE: &str = "FE2O3_TEST_CHECKED_OUTPUT_PROOF_PROBE_V1";
const CHILD_TEST: &str =
    "production_rustc_driver_v1::checked_output_source_v1_tests::checked_output_source_child";

#[derive(Debug, Serialize, Deserialize)]
struct Observation {
    roots: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source_route: Option<dispatch::RouteObservation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    transparent_result_wrappers: Option<usize>,
    internal_helpers: usize,
    helper_calls: usize,
    reads: usize,
    writes: usize,
    global_reads: usize,
    global_writes: usize,
    private_reads: usize,
    private_writes: usize,
    other_reads: usize,
    other_writes: usize,
    formal_accesses: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    runtime_domains: Option<runtime_domains::RuntimeDomainObservation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    simulation: Option<simulation::SimulationObservation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    constant_shift: Option<shift_source::ShiftObservation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    masked_shift: Option<masked_shift_source::ShiftObservation>,
    policy: u16,
    output_digest: [u8; 32],
    llvm_bytes: usize,
    descriptor_roots: usize,
    missing_proof_refused: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum SourceStage {
    Manifest,
    CargoMetadata,
    CargoDependencies,
    Invocation,
    Rustc,
    SourceCollection,
    RankedChecks,
    Policy4,
    Policy5,
    NativeSourceProof,
    NativeHandoff,
    Simulation,
    Observation,
}

#[derive(Debug, Serialize, Deserialize)]
struct SourceFailure {
    stage: SourceStage,
    detail: String,
}

impl SourceFailure {
    fn new(stage: SourceStage, detail: impl std::fmt::Display) -> Self {
        Self {
            stage,
            detail: detail.to_string(),
        }
    }
}

#[derive(Default)]
struct CheckedOutputCallbacks {
    result: Option<Result<Observation, SourceFailure>>,
    probe_missing_proof: bool,
    progress: progress::CallbackProgress,
    endpoint_directory: Option<PathBuf>,
}

impl Callbacks for CheckedOutputCallbacks {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.progress.end(progress::Outcome::Complete);
        self.result = Some((|| {
            let transaction = self
                .progress
                .run(SourceStage::SourceCollection, || {
                    transaction_in_active_session_v1(
                        tcx,
                        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
                    )
                })
                .map_err(|e| SourceFailure::new(SourceStage::SourceCollection, e))?;
            let ranked = self
                .progress
                .run(SourceStage::RankedChecks, || {
                    transaction.verify_general_kernel_checks()
                })
                .map_err(|e| SourceFailure::new(SourceStage::RankedChecks, format!("{e:?}")))?;
            assert!(ranked.all_kernel_checks_are_clean());
            assert!(!ranked.grants_artifact_or_launch_authority());
            // This call contains B/C admission and fresh O checks; an error alone
            // does not identify which internal endpoint failed.
            let stage = snapshots::with_directory(self.endpoint_directory.as_deref(), || {
                self.progress
                    .run(SourceStage::Policy4, || dispatch::Stage::lower(ranked))
            })
            .map_err(|e| SourceFailure::new(SourceStage::Policy4, format!("{e:?}")))?;
            assert!(std::ptr::eq(stage.output(), stage.checked_output().owner()));
            let module = stage.output().module();
            let semantic = stage.semantic();
            let mut transparent_result_wrappers = 0;
            for root in semantic.roots() {
                let selection = semantic
                    .select_kernel_body_for_root_v1(*root)
                    .filter(|selection| selection.root() == *root)
                    .ok_or_else(|| {
                        SourceFailure::new(SourceStage::Observation, "exact source body selection")
                    })?;
                transparent_result_wrappers +=
                    usize::from(selection.has_transparent_result_wrapper());
            }
            let helpers = module
                .functions
                .iter()
                .filter(|function| function.role == fe2o3_kernel_ir::FunctionRole::InternalHelper)
                .map(|function| &function.id)
                .collect::<std::collections::BTreeSet<_>>();
            let mut observation = Observation {
                roots: module
                    .kernels
                    .iter()
                    .map(|k| k.id.as_str().to_owned())
                    .collect(),
                source_route: Some(stage.observe_route()),
                transparent_result_wrappers: Some(transparent_result_wrappers),
                internal_helpers: helpers.len(),
                helper_calls: 0,
                reads: 0,
                writes: 0,
                global_reads: 0,
                global_writes: 0,
                private_reads: 0,
                private_writes: 0,
                other_reads: 0,
                other_writes: 0,
                formal_accesses: stage.kernels().iter().map(|k| k.accesses().len()).sum(),
                runtime_domains: Some(runtime_domains::observe(stage.kernels())?),
                simulation: None,
                constant_shift: None,
                masked_shift: None,
                policy: stage.checked_output().execution().policy_version(),
                output_digest: *stage.output().canonical().identity().digest(),
                llvm_bytes: 0,
                descriptor_roots: 0,
                missing_proof_refused: false,
            };
            for operation in module
                .functions
                .iter()
                .filter_map(|f| f.body.as_ref())
                .flat_map(|body| &body.blocks)
                .flat_map(|block| &block.operations)
            {
                if let OperationKind::Call { callee, .. } = &operation.kind {
                    observation.helper_calls += usize::from(helpers.contains(callee));
                }
                let (write, access) = match &operation.kind {
                    OperationKind::Load { access, .. }
                    | OperationKind::GuardedLoad { access, .. } => (false, access),
                    OperationKind::Store { access, .. }
                    | OperationKind::GuardedStore { access, .. } => (true, access),
                    _ => continue,
                };
                use fe2o3_kernel_ir::AddressSpace;
                let count = match (write, access.address_space) {
                    (false, AddressSpace::Global) => &mut observation.global_reads,
                    (true, AddressSpace::Global) => &mut observation.global_writes,
                    (false, AddressSpace::Private) => &mut observation.private_reads,
                    (true, AddressSpace::Private) => &mut observation.private_writes,
                    (false, _) => &mut observation.other_reads,
                    (true, _) => &mut observation.other_writes,
                };
                *count += 1;
                if write {
                    observation.writes += 1;
                } else {
                    observation.reads += 1;
                }
            }
            let simulation_case = simulation::requested()?;
            observation.constant_shift = shift_source::check_actual(&stage, simulation_case)?;
            observation.masked_shift = masked_shift_source::check_actual(&stage, simulation_case)?;
            let exp_source_identity = exp_source::check_actual_if_requested(&stage)?;
            if let Some(case) = simulation_case {
                observation.simulation =
                    Some(self.progress.run(SourceStage::Simulation, || {
                        simulation::observe(stage.output().canonical(), case)
                    })?);
            }
            if self.probe_missing_proof {
                use crate::production_native_source_lineage_v1::NativeSourceLineageErrorV1;
                use crate::production_pipeline::{
                    ProductionPipelineError, checked_output_policy4_v1::CheckedOutputStageErrorV1,
                };
                let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(
                    usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT)
                        .map_err(|e| SourceFailure::new(SourceStage::NativeSourceProof, e))?,
                );
                let mut budget =
                    fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(
                        &mut work,
                        crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
                    );
                let floor = stage.retained_storage_floor_v1();
                budget
                    .reserve_storage(floor)
                    .map_err(|e| SourceFailure::new(SourceStage::NativeSourceProof, e))?;
                let refused = self.progress.run(SourceStage::NativeSourceProof, || {
                    stage.probe_native_source_lineage_v1(&mut budget)
                });
                assert_eq!(budget.storage(), floor);
                match refused {
                    Err(ProductionPipelineError::CheckedOutputStage(
                        CheckedOutputStageErrorV1::NativeSource(error),
                    )) if matches!(
                        *error,
                        NativeSourceLineageErrorV1::MissingSignedRankedReceipt { .. }
                    ) =>
                    {
                        observation.missing_proof_refused = true
                    }
                    Err(error) => {
                        return Err(SourceFailure::new(
                            SourceStage::NativeSourceProof,
                            format!("unexpected native proof refusal: {error:?}"),
                        ));
                    }
                    Ok(_) => {
                        return Err(SourceFailure::new(
                            SourceStage::NativeSourceProof,
                            "unsigned source acquired native proof custody",
                        ));
                    }
                }
                return Ok(observation);
            }
            let (handoff, descriptor) = self
                .progress
                .run(SourceStage::NativeHandoff, || {
                    stage.into_worker_handoff_extraction_v1()
                })
                .map_err(|e| SourceFailure::new(SourceStage::NativeHandoff, format!("{e:?}")))?;
            assert!(!descriptor.grants_launch_authority());
            masked_shift_source::check_descriptor(observation.masked_shift.as_ref(), &descriptor)?;
            let llvm = std::str::from_utf8(handoff.module_bytes())
                .map_err(|e| SourceFailure::new(SourceStage::NativeHandoff, e))?;
            assert!(llvm.contains("amdgpu_kernel"));
            exp_source::check_handoff_if_requested(
                &handoff,
                exp_source_identity,
                &observation.output_digest,
            )?;
            shift_source::check_native(observation.constant_shift.as_ref(), llvm)?;
            masked_shift_source::check_native(observation.masked_shift.as_ref(), llvm)?;
            if let Some(case) = simulation_case
                && !matches!(
                    case,
                    simulation::Case::ConstantShift(_) | simulation::Case::MaskedShift(_)
                )
            {
                simulation::check_native_arithmetic(case, llvm)?;
            }
            observation.llvm_bytes = llvm.len();
            observation.descriptor_roots = descriptor.table().kernels().len();
            Ok(observation)
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "subprocess helper; invoked with an exact source/dependency request by its parent"]
fn checked_output_source_child() {
    let Some(path) = env::var_os(CHILD_ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    if loop_capture_source::requested() {
        loop_capture_source::run_child(&args);
        return;
    }
    let mut callbacks = CheckedOutputCallbacks {
        probe_missing_proof: env::var_os(CHILD_PROOF_PROBE).is_some(),
        progress: progress::CallbackProgress::from_environment(),
        endpoint_directory: snapshots::child_directory(),
        ..CheckedOutputCallbacks::default()
    };
    callbacks.progress.begin(SourceStage::Rustc);
    let completed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        rustc_driver::run_compiler(&args, &mut callbacks);
    }));
    let result = if completed.is_err() {
        Err(SourceFailure::new(
            SourceStage::Rustc,
            "rustc or callback panicked; see captured diagnostics",
        ))
    } else {
        callbacks.result.unwrap_or_else(|| {
            Err(SourceFailure::new(
                SourceStage::Rustc,
                "actual-source callback did not run",
            ))
        })
    };
    callbacks.progress.finish(if completed.is_err() {
        progress::Outcome::Panicked
    } else if result.is_ok() {
        progress::Outcome::Complete
    } else {
        progress::Outcome::Refused
    });
    std::fs::write(
        env::var_os(CHILD_RESULT).expect("child result path"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    masked_shift_source::record_child_refusal(&args, &result);
    assert!(result.is_ok(), "checked native source route: {result:?}");
}

fn clean_command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut command = Command::new(program);
    command.env_clear();
    for key in [
        "HOME",
        "PATH",
        "CARGO_HOME",
        "RUSTUP_HOME",
        "RUSTUP_TOOLCHAIN",
        "LD_LIBRARY_PATH",
        "TMPDIR",
    ] {
        if let Some(value) = env::var_os(key) {
            command.env(key, value);
        }
    }
    command
        .env("CARGO_BUILD_JOBS", "1")
        .env("CARGO_INCREMENTAL", "0")
        .env("CARGO_NET_OFFLINE", "true")
        .env("CARGO_PROFILE_RELEASE_DEBUG", "0")
        .env("FE2O3_HIP_SYS_DISABLE", "1")
        .env("FE2O3_HSA_RUNTIME_DISABLE", "1");
    command
}

fn output(command: &mut Command) -> std::process::Output {
    let result = command
        .output()
        .expect("execute source qualification command");
    assert!(
        result.status.success(),
        "{command:?}\n{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    result
}

fn artifact(messages: &[serde_json::Value], name: &str) -> PathBuf {
    let artifacts: Vec<_> = messages
        .iter()
        .filter(|m| m["reason"] == "compiler-artifact" && m["target"]["name"] == name)
        .collect();
    assert_eq!(
        artifacts.len(),
        1,
        "one actual Cargo artifact for {name}: {artifacts:?}"
    );
    let filenames = artifacts[0]["filenames"]
        .as_array()
        .expect("Cargo artifact filenames");
    // build-std can emit both for one artifact. This callback needs metadata;
    // use the standalone metadata when present, otherwise its library container.
    for extension in ["rmeta", "rlib"] {
        let matches: Vec<_> = filenames
            .iter()
            .filter_map(|p| p.as_str())
            .map(PathBuf::from)
            .filter(|p| p.extension().is_some_and(|ext| ext == extension))
            .collect();
        assert!(
            matches.len() <= 1,
            "ambiguous {extension} for {name}: {matches:?}"
        );
        if let Some(path) = matches.into_iter().next() {
            return path;
        }
    }
    panic!("missing metadata for actual Cargo artifact {name}");
}

#[test]
#[ignore = "requires pinned nightly rust-src, AMD dependencies, and ordinary-source compilation"]
fn ordinary_rust_fill_and_vecadd_reach_checked_native_output() {
    ordinary_rust_checked_output_cases(&[OrdinarySourceCase::Fill, OrdinarySourceCase::Vecadd]);
}

#[test]
#[ignore = "requires pinned nightly rust-src, AMD dependencies, and ordinary-source compilation"]
fn ordinary_rust_result_wrapped_fill_reaches_checked_native_output() {
    ordinary_rust_checked_output_cases(&[
        OrdinarySourceCase::RetainedWrappedFill,
        OrdinarySourceCase::WrappedFill,
    ]);
}

#[test]
#[ignore = "requires pinned nightly rust-src and admitted Verus runtime for the actual-source helper effect join"]
fn ordinary_rust_shared_unit_helper_reaches_checked_native_output() {
    ordinary_rust_checked_output_cases(&[OrdinarySourceCase::SharedUnitHelper]);
}

#[test]
#[ignore = "requires pinned nightly rust-src, AMD dependencies, and ordinary-source compilation"]
fn ordinary_rust_private_unit_helper_reaches_checked_native_output() {
    ordinary_rust_checked_output_cases(&[
        OrdinarySourceCase::PrivateUnitHelper,
        OrdinarySourceCase::RetainedPrivateUnitHelper,
    ]);
}

enum OrdinarySourceCase {
    ConditionalDescriptorPair,
    ReferenceFill,
    MaskedShift(masked_shift_source::Config),
    ConstantShift(shift_source::Config),
    ScalarBorrowPolicy5,
    RetainedScalarBorrowPolicy5,
    ScalarBorrowPolicy5Barrier,
    NumericCast(numeric_cast_source::Config),
    F32Exp,
    RetainedF32Exp,
    SaturatingInteger(saturating_source::Config),
    Fill,
    Vecadd,
    WrappedFill,
    RetainedWrappedFill,
    SharedUnitHelper,
    PrivateUnitHelper,
    RetainedPrivateUnitHelper,
    F32Negate,
    F32Divide,
    RetainedF32Negate,
    RetainedF32Divide,
}

fn ordinary_rust_checked_output_cases(cases: &[OrdinarySourceCase]) {
    ordinary_rust_checked_output_cases_for_profile(
        cases,
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
    );
}

fn ordinary_rust_checked_output_cases_for_profile(
    cases: &[OrdinarySourceCase],
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
) {
    ordinary_rust_checked_output_cases_for_profile_with_fixed_facade(cases, profile, false);
}

fn ordinary_rust_checked_output_cases_for_profile_with_fixed_facade(
    cases: &[OrdinarySourceCase],
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    fixed_facade: bool,
) {
    ordinary_rust_source_cases(cases, profile, fixed_facade, None);
}

struct SourceObserver {
    child_test: &'static str,
    check: fn(&Path, &[&str]),
}

fn ordinary_rust_source_cases(
    cases: &[OrdinarySourceCase],
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    fixed_facade: bool,
    observer: Option<SourceObserver>,
) {
    assert!(!fixed_facade || observer.is_none());
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-checked-output-source");
    let target = scratch.path().join("target");
    let cargo = env!("CARGO");
    let metadata: serde_json::Value = serde_json::from_slice(
        &output(clean_command(cargo).current_dir(&workspace).args([
            "metadata",
            "--offline",
            "--locked",
            "--no-deps",
            "--format-version",
            "1",
        ]))
        .stdout,
    )
    .unwrap();
    let built = output(clean_command(cargo).current_dir(&workspace)
        .args(["check", "--offline", "--locked", "--release", "-Zbuild-std=core",
            "-p", "fe2o3-device", "--target", "amdgcn-amd-amdhsa",
            "--message-format=json-render-diagnostics", "--target-dir"])
        .arg(&target)
        .env("CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
            format!("-Zalways-encode-mir -Ctarget-cpu={} -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32", profile.cpu())));
    let messages: Vec<serde_json::Value> = built
        .stdout
        .split(|b| *b == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).unwrap())
        .collect();
    let device = artifact(&messages, "fe2o3_device");
    let core = artifact(&messages, "core");
    let builtins = artifact(&messages, "compiler_builtins");
    assert!(device.starts_with(&target));
    let rustc = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let sysroot = output(clean_command(&rustc).args(["--print", "sysroot"]));
    let sysroot = String::from_utf8(sysroot.stdout).unwrap();
    let mut private_helper_needs_retained_mir = true;
    let mut fixed_erased_observed = false;
    for case in cases {
        if matches!(case, OrdinarySourceCase::RetainedPrivateUnitHelper)
            && !private_helper_needs_retained_mir
        {
            continue;
        }
        let configuration_name = match case {
            OrdinarySourceCase::MaskedShift(config) => config.name(),
            OrdinarySourceCase::ConstantShift(config) => config.name(),
            OrdinarySourceCase::NumericCast(config) => config.name(),
            OrdinarySourceCase::SaturatingInteger(config) => config.name(),
            _ => String::new(),
        };
        let (name, package_path, feature, roots, reads, writes, calls) = match case {
            OrdinarySourceCase::MaskedShift(config) => (
                configuration_name.as_str(),
                "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device",
                Some("masked-shift"),
                config.roots(),
                0,
                config.roots().len(),
                if config.batch.retained {
                    config.roots().len()
                } else {
                    0
                },
            ),
            OrdinarySourceCase::ConstantShift(config) => (
                configuration_name.as_str(),
                "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device",
                Some("constant-shift"),
                config.roots(),
                0,
                config.roots().len(),
                if config.batch.retained {
                    config.roots().len()
                } else {
                    0
                },
            ),
            OrdinarySourceCase::ScalarBorrowPolicy5
            | OrdinarySourceCase::RetainedScalarBorrowPolicy5
            | OrdinarySourceCase::ScalarBorrowPolicy5Barrier => (
                if matches!(case, OrdinarySourceCase::ScalarBorrowPolicy5Barrier) {
                    "scalar-borrow-policy5-opt0-barrier"
                } else if matches!(case, OrdinarySourceCase::RetainedScalarBorrowPolicy5) {
                    "scalar-borrow-policy5-opt0"
                } else {
                    "scalar-borrow-policy5"
                },
                "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device",
                Some("scalar-borrow-policy5"),
                &["scalar_borrow_policy5"][..],
                0,
                0,
                0,
            ),
            OrdinarySourceCase::F32Exp => (
                "f32-exp",
                "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device",
                Some("f32-exp"),
                &["f32_exp"][..],
                0,
                1,
                0,
            ),
            OrdinarySourceCase::RetainedF32Exp => (
                "retained-f32-exp",
                "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device",
                Some("f32-helper-exp"),
                &["f32_helper_exp"][..],
                0,
                1,
                1,
            ),
            OrdinarySourceCase::NumericCast(config) => (
                configuration_name.as_str(),
                "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device",
                Some("numeric-cast"),
                &["numeric_cast"][..],
                0,
                1,
                usize::from(config.retained),
            ),
            OrdinarySourceCase::SaturatingInteger(config) => (
                configuration_name.as_str(),
                "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device",
                Some("saturating-integer"),
                &["saturating_integer"][..],
                0,
                1,
                usize::from(config.retained),
            ),
            OrdinarySourceCase::Fill => ("fill", "examples/fill", None, &["fill"][..], 0, 1, 0),
            OrdinarySourceCase::ReferenceFill => (
                "reference-fill",
                "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device",
                Some("reference-positive"),
                &["fill"][..],
                0,
                1,
                0,
            ),
            OrdinarySourceCase::ConditionalDescriptorPair => (
                "conditional-descriptor-pair",
                "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device",
                Some("conditional-descriptor-pair"),
                &["binding_first", "binding_second"][..],
                0,
                2,
                0,
            ),
            OrdinarySourceCase::Vecadd => {
                ("vecadd", "examples/vecadd", None, &["vecadd"][..], 2, 1, 0)
            }
            OrdinarySourceCase::WrappedFill => (
                "wrapped-fill",
                "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device",
                Some("wrapped-fill"),
                &["wrapped_fill"][..],
                0,
                1,
                0,
            ),
            OrdinarySourceCase::RetainedWrappedFill => (
                "retained-wrapped-fill",
                "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device",
                Some("wrapped-fill"),
                &["wrapped_fill"][..],
                0,
                1,
                0,
            ),
            OrdinarySourceCase::SharedUnitHelper => (
                "shared-unit-helper",
                "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device",
                Some("multi-root-target-lineage"),
                &["alpha", "zeta"][..],
                0,
                2,
                2,
            ),
            OrdinarySourceCase::F32Negate => (
                "f32-negate",
                "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device",
                Some("f32-negate"),
                &["f32_negate"][..],
                0,
                1,
                0,
            ),
            OrdinarySourceCase::F32Divide => (
                "f32-divide",
                "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device",
                Some("f32-divide"),
                &["f32_divide"][..],
                0,
                1,
                0,
            ),
            OrdinarySourceCase::RetainedF32Negate => (
                "retained-f32-negate",
                "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device",
                Some("f32-helper-negate"),
                &["f32_helper_negate"][..],
                0,
                1,
                1,
            ),
            OrdinarySourceCase::RetainedF32Divide => (
                "retained-f32-divide",
                "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device",
                Some("f32-helper-divide"),
                &["f32_helper_divide"][..],
                0,
                1,
                1,
            ),
            OrdinarySourceCase::PrivateUnitHelper
            | OrdinarySourceCase::RetainedPrivateUnitHelper => (
                if matches!(case, OrdinarySourceCase::RetainedPrivateUnitHelper) {
                    "retained-private-unit-helper"
                } else {
                    "private-unit-helper"
                },
                "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device",
                Some("private-unit-helper"),
                &["private_helper_fill"][..],
                0,
                1,
                0,
            ),
        };
        let package_dir = workspace.join(package_path);
        let manifest = package_dir.join("Cargo.toml");
        let package = metadata["packages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| Path::new(p["manifest_path"].as_str().unwrap()) == manifest)
            .unwrap();
        let package_name = package["name"].as_str().unwrap();
        let version = package["version"].as_str().unwrap();
        let crate_name = package_name.replace('-', "_");
        let identity = PortablePackageIdentityV1::new(
            package_name,
            version,
            Sha256::digest(std::fs::read(&manifest).unwrap()).into(),
        )
        .unwrap();
        let mut args = vec![
            rustc.to_str().unwrap().to_owned(),
            "--crate-name".into(),
            crate_name.clone(),
            "--crate-type=lib".into(),
            format!("--edition={}", package["edition"].as_str().unwrap()),
            "--target=amdgcn-amd-amdhsa".into(),
            format!("-Ctarget-cpu={}", profile.cpu()),
            "-Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32".into(),
            "-Copt-level=3".into(),
            "-Cdebug-assertions=off".into(),
            "-Coverflow-checks=on".into(),
            "-Zalways-encode-mir".into(),
            "-Zunstable-options".into(),
            "--emit=metadata".into(),
            "--sysroot".into(),
            sysroot.trim().into(),
            format!("--extern=fe2o3_device={}", device.display()),
            format!("--extern=noprelude:core={}", core.display()),
            format!(
                "--extern=noprelude:compiler_builtins={}",
                builtins.display()
            ),
            format!("-Ldependency={}", device.parent().unwrap().display()),
            format!("-Ldependency={}", target.join("release/deps").display()),
            "--out-dir".into(),
            scratch.path().display().to_string(),
            package_dir.join("src/lib.rs").display().to_string(),
        ];
        if let Some(feature) = feature {
            args.push(format!("--cfg=feature=\"{feature}\""));
        }
        if let OrdinarySourceCase::SaturatingInteger(config) = case {
            config.configure(&mut args);
        }
        if let OrdinarySourceCase::NumericCast(config) = case {
            config.configure(&mut args);
        }
        if let OrdinarySourceCase::ConstantShift(config) = case {
            config.configure(&mut args);
        }
        if let OrdinarySourceCase::MaskedShift(config) = case {
            config.configure(&mut args);
        }
        if matches!(
            case,
            OrdinarySourceCase::RetainedScalarBorrowPolicy5
                | OrdinarySourceCase::ScalarBorrowPolicy5Barrier
        ) {
            args.push("-Zmir-opt-level=0".into());
        }
        if matches!(case, OrdinarySourceCase::ScalarBorrowPolicy5Barrier) {
            args.push("--cfg=feature=\"scalar-borrow-policy5-barrier\"".into());
        }
        // Qualify both real frontend shapes. This changes only rustc's test
        // invocation, never the fixed fe2o3 optimizer or its admission policy.
        if matches!(
            case,
            OrdinarySourceCase::RetainedWrappedFill | OrdinarySourceCase::ConditionalDescriptorPair
        ) {
            args.push("-Zinline-mir=no".into());
        }
        if matches!(
            case,
            OrdinarySourceCase::RetainedF32Negate
                | OrdinarySourceCase::RetainedF32Divide
                | OrdinarySourceCase::RetainedF32Exp
        ) {
            args.push("-Zinline-mir=no".into());
        }
        if matches!(case, OrdinarySourceCase::RetainedPrivateUnitHelper) {
            args.push("-Zinline-mir=no".into());
            args.push("-Zmir-opt-level=0".into());
        }
        let original: Vec<OsString> = args.iter().map(OsString::from).collect();
        let RustcInvocationV2::Compile(compile) = classify_rustc_invocation_v2(&original).unwrap()
        else {
            panic!("expected source compile")
        };
        let portable = portable_rustc_metadata_v1(compile, &identity).unwrap();
        args.push(format!("-Cmetadata={portable}"));
        let actual: Vec<OsString> = args.iter().map(OsString::from).collect();
        let RustcInvocationV2::Compile(compile) = classify_rustc_invocation_v2(&actual).unwrap()
        else {
            panic!("expected bound source compile")
        };
        let observation = derive_cargo_metadata_build_observation_v2(
            &ordered_rustc_codegen_metadata_v1(compile).unwrap(),
        );
        let binding = derive_crate_binding_id_v1(&crate_name, [portable.as_str()]);
        let request = scratch.path().join(format!("{name}-args.json"));
        let response = scratch.path().join(format!("{name}-result.json"));
        std::fs::write(&request, serde_json::to_vec(&args).unwrap()).unwrap();
        let mut command = clean_command(env::current_exe().unwrap());
        let policy5_case = matches!(
            case,
            OrdinarySourceCase::ScalarBorrowPolicy5
                | OrdinarySourceCase::RetainedScalarBorrowPolicy5
                | OrdinarySourceCase::ScalarBorrowPolicy5Barrier
        );
        let checked_policy5 = policy5_case || fixed_facade;
        let child_test = if let Some(observer) = &observer {
            observer.child_test
        } else if checked_policy5 {
            policy5_source::CHILD_TEST
        } else {
            CHILD_TEST
        };
        let policy5_report = scratch.path().join(format!("{name}-policy5.json"));
        if checked_policy5 {
            policy5_source::configure_child(&mut command, &policy5_report);
        }
        command
            .current_dir(&workspace)
            .args(["--exact", child_test, "--ignored", "--nocapture"])
            .env(CHILD_ARGS, &request)
            .env(CHILD_RESULT, &response)
            .env("CARGO_MANIFEST_DIR", &package_dir)
            .env("CARGO_PKG_NAME", package_name)
            .env("CARGO_PKG_VERSION", version)
            .env("CARGO_PRIMARY_PACKAGE", "1")
            .env(CRATE_BINDING_ID_ENV_V1, binding.to_hex())
            .env(
                CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
                observation.to_hex(),
            );
        progress::clear_inherited_jobserver(&mut command);
        let simulation_case = match case {
            OrdinarySourceCase::MaskedShift(config) => {
                Some(simulation::Case::MaskedShift(config.batch))
            }
            OrdinarySourceCase::ConstantShift(config) => {
                (!config.dynamic).then_some(simulation::Case::ConstantShift(config.batch))
            }
            OrdinarySourceCase::ScalarBorrowPolicy5
            | OrdinarySourceCase::RetainedScalarBorrowPolicy5
            | OrdinarySourceCase::ScalarBorrowPolicy5Barrier => {
                Some(simulation::Case::ScalarBorrow)
            }
            OrdinarySourceCase::F32Exp | OrdinarySourceCase::RetainedF32Exp => None,
            OrdinarySourceCase::NumericCast(config) => {
                Some(simulation::Case::NumericCast(config.operation))
            }
            OrdinarySourceCase::SaturatingInteger(config) => {
                Some(simulation::Case::SaturatingInteger(config.operation))
            }
            OrdinarySourceCase::Fill
            | OrdinarySourceCase::WrappedFill
            | OrdinarySourceCase::RetainedWrappedFill
            | OrdinarySourceCase::PrivateUnitHelper
            | OrdinarySourceCase::RetainedPrivateUnitHelper => Some(simulation::Case::Fill),
            OrdinarySourceCase::Vecadd => Some(simulation::Case::Vecadd),
            OrdinarySourceCase::SharedUnitHelper
            | OrdinarySourceCase::ReferenceFill
            | OrdinarySourceCase::ConditionalDescriptorPair => None,
            OrdinarySourceCase::F32Negate | OrdinarySourceCase::RetainedF32Negate => {
                Some(simulation::Case::F32Negate)
            }
            OrdinarySourceCase::F32Divide | OrdinarySourceCase::RetainedF32Divide => {
                Some(simulation::Case::F32Divide)
            }
        };
        simulation::configure_child(&mut command, simulation_case);
        exp_source::configure_child(
            &mut command,
            matches!(
                case,
                OrdinarySourceCase::F32Exp | OrdinarySourceCase::RetainedF32Exp
            ),
        );
        let diagnostic_name = format!("{}-{name}", profile.cpu());
        snapshots::configure_child(&mut command, &diagnostic_name);
        if let Some(observer) = &observer {
            let child = output(&mut command);
            eprintln!(
                "actual-source observation {diagnostic_name}:\n{}",
                String::from_utf8_lossy(&child.stdout),
            );
            (observer.check)(&response, roots);
            continue;
        }
        let (child, result) = match case {
            OrdinarySourceCase::ConstantShift(config) if config.dynamic => {
                shift_source::expected_refusal_child(&mut command, &response)
            }
            OrdinarySourceCase::MaskedShift(config) if config.discovery => {
                config.invoke_discovery(&mut command, &args, &response)
            }
            _ => {
                let child = output(&mut command);
                let result: Result<Observation, SourceFailure> =
                    serde_json::from_slice(&std::fs::read(&response).unwrap()).unwrap();
                (child, result)
            }
        };
        if let OrdinarySourceCase::MaskedShift(config) = case
            && config.record_refusal(profile, &result)
        {
            eprintln!("{}", String::from_utf8_lossy(&child.stdout));
            eprintln!("{}", String::from_utf8_lossy(&child.stderr));
            continue;
        }
        if let OrdinarySourceCase::ConstantShift(config) = case
            && config.dynamic
        {
            config.check_refusal(result);
            eprintln!("ordinary source {diagnostic_name}: exact dynamic-count source refusal");
            continue;
        }
        let result = result.unwrap();
        if let OrdinarySourceCase::ConstantShift(config) = case {
            config.check_root_roster(&result.roots).unwrap();
        } else if let OrdinarySourceCase::MaskedShift(config) = case {
            config.check_root_roster(&result.roots).unwrap();
        } else {
            assert_eq!(result.roots, roots);
        }
        assert_eq!(
            result.transparent_result_wrappers,
            Some(usize::from(matches!(
                case,
                OrdinarySourceCase::RetainedWrappedFill
            )))
        );
        if matches!(
            case,
            OrdinarySourceCase::PrivateUnitHelper | OrdinarySourceCase::RetainedPrivateUnitHelper
        ) {
            let route = dispatch::check_private_helper_route(
                &result,
                matches!(case, OrdinarySourceCase::RetainedPrivateUnitHelper),
            )
            .unwrap();
            if matches!(case, OrdinarySourceCase::PrivateUnitHelper) {
                private_helper_needs_retained_mir = route != dispatch::Route::SilentUnitLocal;
            }
            fixed_erased_observed |= fixed_facade && route == dispatch::Route::SilentUnitLocal;
            eprintln!(
                "ordinary private helper observed route: {route:?}; retained-MIR test configuration: {}",
                matches!(case, OrdinarySourceCase::RetainedPrivateUnitHelper)
            );
        } else {
            assert_eq!(result.helper_calls, calls);
            assert_eq!(result.internal_helpers == 0, calls == 0);
        }
        if matches!(
            case,
            OrdinarySourceCase::F32Negate
                | OrdinarySourceCase::F32Divide
                | OrdinarySourceCase::RetainedF32Negate
                | OrdinarySourceCase::RetainedF32Divide
        ) {
            assert_eq!(
                dispatch::check_private_helper_route(&result, false).unwrap(),
                dispatch::Route::DirectRawEmpty
            );
            eprintln!(
                "ordinary F32 source {name}: actual O helpers={}, calls={}; -Zinline-mir=no={}",
                result.internal_helpers,
                result.helper_calls,
                matches!(
                    case,
                    OrdinarySourceCase::RetainedF32Negate | OrdinarySourceCase::RetainedF32Divide
                )
            );
        }
        if policy5_case {
            policy5_source::check(
                &policy5_report,
                &result,
                matches!(
                    case,
                    OrdinarySourceCase::RetainedScalarBorrowPolicy5
                        | OrdinarySourceCase::ScalarBorrowPolicy5Barrier
                ),
                matches!(case, OrdinarySourceCase::ScalarBorrowPolicy5Barrier),
            );
            assert_eq!(
                (result.global_reads, result.other_reads, result.other_writes),
                (0, 0, 0)
            );
            assert!(result.global_writes > 0);
        } else {
            assert_eq!((result.reads, result.writes), (reads, writes));
        }
        if let OrdinarySourceCase::SaturatingInteger(config) = case {
            config.check(&result);
        }
        if let OrdinarySourceCase::ConstantShift(config) = case {
            config.check(&result);
        }
        if let OrdinarySourceCase::MaskedShift(config) = case {
            config.check(&result);
        }
        assert_eq!(
            result.formal_accesses,
            if policy5_case {
                result.global_reads + result.global_writes
            } else {
                reads + writes
            }
        );
        assert_eq!(
            result.runtime_domains,
            Some(runtime_domains::RuntimeDomainObservation::default())
        );
        assert_eq!(result.policy, if checked_policy5 { 5 } else { 4 });
        assert_eq!(result.descriptor_roots, roots.len());
        assert_ne!(result.output_digest, [0; 32]);
        assert!(result.llvm_bytes > 0);
        assert!(!result.missing_proof_refused);
        if let Some(case) = simulation_case {
            simulation::check_observation(&result, case).unwrap();
        } else {
            assert!(result.simulation.is_none());
        }
        eprintln!(
            "actual-source checked native output {diagnostic_name}: {result:?}\n{}",
            String::from_utf8_lossy(&child.stdout)
        );
        if let OrdinarySourceCase::MaskedShift(config) = case {
            config.record_qualified(profile, &result);
        }
        if fixed_facade {
            let extracted = scratch.path().join(format!("{name}-fixed-extractor.ll"));
            let extractor_response = scratch.path().join(format!("{name}-fixed-extractor.json"));
            policy5_source::configure_real_extractor_child(&mut command, Some(&extracted));
            let extracted_child = output(command.env(CHILD_RESULT, &extractor_response));
            policy5_source::check_real_extraction(&policy5_report, &extractor_response, &extracted);
            assert_eq!(
                std::fs::metadata(&extracted).unwrap().len(),
                result.llvm_bytes as u64
            );
            eprintln!(
                "real fixed checked-output extractor matched actual O for {diagnostic_name}:\n{}",
                String::from_utf8_lossy(&extracted_child.stderr),
            );
            policy5_source::configure_real_extractor_child(&mut command, None);
            command.env(CHILD_RESULT, &response);
        }
        let probe_private = if fixed_facade {
            matches!(
                case,
                OrdinarySourceCase::PrivateUnitHelper
                    | OrdinarySourceCase::RetainedPrivateUnitHelper
            ) && dispatch::check_private_helper_route(&result, false)
                == Ok(dispatch::Route::SilentUnitLocal)
        } else {
            matches!(case, OrdinarySourceCase::PrivateUnitHelper)
        };
        if matches!(case, OrdinarySourceCase::Fill) || probe_private {
            let erased_probe = matches!(
                case,
                OrdinarySourceCase::PrivateUnitHelper
                    | OrdinarySourceCase::RetainedPrivateUnitHelper
            );
            if erased_probe {
                assert_eq!(
                    dispatch::check_private_helper_route(&result, true).unwrap(),
                    dispatch::Route::SilentUnitLocal,
                    "normal-MIR source must reach genuine silent Unit erasure before its proof probe"
                );
            }
            let expected_output = result.output_digest;
            let expected_route = result.source_route;
            let proof_response = scratch.path().join(format!("{name}-proof-refusal.json"));
            simulation::configure_child(&mut command, None);
            snapshots::configure_child(&mut command, &format!("{diagnostic_name}-missing-proof"));
            let probe = output(
                command
                    .env(CHILD_PROOF_PROBE, "1")
                    .env(CHILD_RESULT, &proof_response),
            );
            let result: Result<Observation, SourceFailure> =
                serde_json::from_slice(&std::fs::read(proof_response).unwrap()).unwrap();
            let result = result.unwrap();
            assert!(result.missing_proof_refused);
            assert_eq!(result.source_route, expected_route);
            if erased_probe {
                assert_eq!(
                    dispatch::check_private_helper_route(&result, true).unwrap(),
                    dispatch::Route::SilentUnitLocal
                );
            }
            assert_eq!(result.transparent_result_wrappers, Some(0));
            assert!(result.simulation.is_none());
            assert_eq!(
                result.runtime_domains,
                Some(runtime_domains::RuntimeDomainObservation::default())
            );
            assert_eq!(result.policy, if checked_policy5 { 5 } else { 4 });
            assert_eq!(result.output_digest, expected_output);
            assert_eq!((result.llvm_bytes, result.descriptor_roots), (0, 0));
            eprintln!(
                "actual-source native proof correctly refused missing signed receipt: {}",
                String::from_utf8_lossy(&probe.stdout)
            );
        }
    }
    if fixed_facade
        && cases.iter().any(|case| {
            matches!(
                case,
                OrdinarySourceCase::PrivateUnitHelper
                    | OrdinarySourceCase::RetainedPrivateUnitHelper
            )
        })
    {
        assert!(
            fixed_erased_observed,
            "fixed dispatcher never consumed a genuine UnitLocal source owner"
        );
    }
}

#[path = "production_rustc_driver_checked_output_corpus_v1_tests.rs"]
mod corpus;
#[path = "production_rustc_driver_checked_output_cargo_v1_tests.rs"]
mod corpus_cargo;
#[path = "production_rustc_driver_checked_output_dispatch_v1_tests.rs"]
mod dispatch;
#[path = "production_rustc_driver_checked_output_exp_source_v1_tests.rs"]
mod exp_source;
#[path = "production_rustc_driver_checked_output_f32_source_v1_tests.rs"]
mod f32_source;
#[path = "production_rustc_driver_checked_output_masked_shift_source_v1_tests.rs"]
mod masked_shift_source;
#[path = "production_rustc_driver_checked_output_numeric_cast_source_v1_tests.rs"]
mod numeric_cast_source;
#[path = "production_rustc_driver_checked_output_policy5_source_v1_tests.rs"]
mod policy5_source;
#[path = "production_rustc_driver_checked_output_progress_v1_tests.rs"]
mod progress;
#[path = "production_rustc_driver_checked_output_runtime_domains_v1_tests.rs"]
mod runtime_domains;
#[path = "production_rustc_driver_checked_output_saturating_source_v1_tests.rs"]
mod saturating_source;
#[path = "production_rustc_driver_checked_output_shift_source_v1_tests.rs"]
mod shift_source;
#[path = "production_rustc_driver_checked_output_simulation_v1_tests.rs"]
mod simulation;
