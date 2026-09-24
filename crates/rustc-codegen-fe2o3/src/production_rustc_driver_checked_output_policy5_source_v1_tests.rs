//! A separate fixed Policy5 child, not a compiler pass-policy selector.
use super::*;
use fe2o3_kernel_ir::{AddressSpace, Module, OperationKind};

pub(super) const CHILD_TEST: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::policy5_source::checked_output_policy5_source_child";
const CHILD_REPORT: &str = "FE2O3_TEST_POLICY5_SOURCE_REPORT";

#[derive(Debug, Serialize, Deserialize)]
struct Report {
    source: [u8; 32],
    intermediate: [u8; 32],
    output: [u8; 32],
    store_rows: usize,
    load_rows: usize,
    before_private_reads: usize,
    after_private_reads: usize,
    llvm_private_reads: usize,
    llvm_digest: [u8; 32],
}

fn private_reads(module: &Module) -> usize {
    module
        .functions
        .iter()
        .filter_map(|function| function.body.as_ref())
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(&operation.kind, OperationKind::Load { access, .. }
            if access.address_space == AddressSpace::Private)
        })
        .count()
}

pub(super) fn configure_child(command: &mut Command, report: &Path) {
    command.env(CHILD_REPORT, report);
}

pub(super) fn check(report: &Path, observation: &Observation, retained_mir: bool, barrier: bool) {
    let report: Report = serde_json::from_slice(&std::fs::read(report).unwrap()).unwrap();
    assert_eq!(observation.policy, 5);
    assert_eq!(report.output, observation.output_digest);
    assert_ne!(report.source, [0; 32]);
    assert_ne!(report.intermediate, [0; 32]);
    assert_eq!(
        report.before_private_reads,
        report.after_private_reads + report.load_rows
    );
    assert_eq!(report.llvm_private_reads, report.after_private_reads);
    assert_eq!(report.after_private_reads, observation.private_reads);
    assert_eq!(
        report.store_rows, 0,
        "fixture must exercise S/O, not C/S forwarding"
    );
    if barrier {
        assert_eq!(
            report.load_rows, 0,
            "intervening global store clears the load seed"
        );
        assert_eq!(report.intermediate, report.output);
        assert!(
            report.after_private_reads >= 2,
            "opt0 barrier control must retain both reads"
        );
    } else if retained_mir {
        assert!(
            report.load_rows > 0,
            "explicit MIR opt0 must retain a genuine load pair"
        );
        assert_ne!(report.intermediate, report.output);
        assert!(
            report.after_private_reads > 0,
            "first potentially trapping read remains"
        );
    } else if report.load_rows == 0 {
        assert_eq!(report.intermediate, report.output);
    } else {
        assert_ne!(report.intermediate, report.output);
    }
    eprintln!(
        "ordinary fixed Policy5; explicit MIR opt0={retained_mir}, store barrier={barrier}: {report:?}"
    );
}

#[derive(Default)]
struct CallbacksV1 {
    result: Option<Result<Observation, SourceFailure>>,
    progress: progress::CallbackProgress,
    endpoint_directory: Option<PathBuf>,
}

impl Callbacks for CallbacksV1 {
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
                .map_err(|error| SourceFailure::new(SourceStage::SourceCollection, error))?;
            let ranked = self
                .progress
                .run(SourceStage::RankedChecks, || {
                    transaction.verify_general_kernel_checks()
                })
                .map_err(|error| {
                    SourceFailure::new(SourceStage::RankedChecks, format!("{error:?}"))
                })?;
            assert!(ranked.all_kernel_checks_are_clean());
            assert!(!ranked.grants_artifact_or_launch_authority());
            let stage = snapshots::with_directory(self.endpoint_directory.as_deref(), || {
                self.progress.run(SourceStage::Policy5, || {
                    ranked.lower_fixed_checked_output_v1()
                })
            })
            .map_err(|error| SourceFailure::new(SourceStage::Policy5, format!("{error:?}")))?;
            let checked = stage.checked_output();
            assert!(std::ptr::eq(stage.output(), checked.owner()));
            assert!(!stage.original_canonical_bytes().is_empty());
            let intermediate = checked.intermediate_policy4().owner();
            let mut report = Report {
                source: stage.original_digest(),
                intermediate: *intermediate.canonical().identity().digest(),
                output: *stage.output().canonical().identity().digest(),
                store_rows: checked.intermediate_policy4().forwarding_rows().len(),
                load_rows: checked.load_forwarding_rows().len(),
                before_private_reads: private_reads(intermediate.module()),
                after_private_reads: private_reads(stage.output().module()),
                llvm_private_reads: 0,
                llvm_digest: [0; 32],
            };
            let module = stage.output().module();
            let semantic = stage.semantic();
            let wrappers = semantic
                .roots()
                .iter()
                .map(|root| {
                    let selected = semantic.select_kernel_body_for_root_v1(*root).unwrap();
                    assert_eq!(selected.root(), *root);
                    usize::from(selected.has_transparent_result_wrapper())
                })
                .sum();
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
                    .map(|root| root.id.as_str().to_owned())
                    .collect(),
                source_route: Some(dispatch::observe_fixed(&stage)),
                transparent_result_wrappers: Some(wrappers),
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
                formal_accesses: stage
                    .kernels()
                    .iter()
                    .map(|kernel| kernel.accesses().len())
                    .sum(),
                runtime_domains: Some(runtime_domains::observe(stage.kernels())?),
                simulation: None,
                constant_shift: None,
                masked_shift: None,
                policy: checked.execution().policy_version(),
                output_digest: report.output,
                llvm_bytes: 0,
                descriptor_roots: 0,
                missing_proof_refused: false,
            };
            for operation in module
                .functions
                .iter()
                .filter_map(|function| function.body.as_ref())
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
            if let Some(case) = simulation::requested()? {
                observation.simulation =
                    Some(self.progress.run(SourceStage::Simulation, || {
                        simulation::observe(stage.output().canonical(), case)
                    })?);
            }
            if env::var_os(super::CHILD_PROOF_PROBE).is_some() {
                use crate::production_native_source_lineage_v1::NativeSourceLineageErrorV1;
                use crate::production_pipeline::{
                    ProductionPipelineError,
                    checked_output_policy5_v1::CheckedOutputPolicy5StageErrorV1,
                };
                let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(
                    usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT)
                        .map_err(|error| {
                            SourceFailure::new(SourceStage::NativeSourceProof, error)
                        })?,
                );
                let mut budget =
                    fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(
                        &mut work,
                        crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
                    );
                let floor = stage.retained_storage_floor_v1();
                budget
                    .reserve_storage(floor)
                    .map_err(|error| SourceFailure::new(SourceStage::NativeSourceProof, error))?;
                let result = self.progress.run(SourceStage::NativeSourceProof, || {
                    stage.probe_native_source_lineage_v1(&mut budget)
                });
                assert_eq!(budget.storage(), floor);
                match result {
                    Err(ProductionPipelineError::CheckedOutputPolicy5Stage(
                        CheckedOutputPolicy5StageErrorV1::NativeSource(error),
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
                            format!("unexpected fixed-output native proof refusal: {error:?}"),
                        ));
                    }
                    Ok(()) => {
                        return Err(SourceFailure::new(
                            SourceStage::NativeSourceProof,
                            "unsigned fixed-output source acquired native proof custody",
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
                .map_err(|error| {
                    SourceFailure::new(SourceStage::NativeHandoff, format!("{error:?}"))
                })?;
            assert!(!descriptor.grants_launch_authority());
            let llvm = std::str::from_utf8(handoff.module_bytes())
                .map_err(|error| SourceFailure::new(SourceStage::NativeHandoff, error))?;
            assert!(llvm.contains("amdgpu_kernel"));
            report.llvm_private_reads = llvm
                .lines()
                .filter(|line| line.contains(" = load i32, ptr addrspace(5) "))
                .count();
            observation.llvm_bytes = llvm.len();
            report.llvm_digest = Sha256::digest(handoff.module_bytes()).into();
            observation.descriptor_roots = descriptor.table().kernels().len();
            std::fs::write(
                env::var_os(CHILD_REPORT).expect("explicit Policy5 report path"),
                serde_json::to_vec(&report).unwrap(),
            )
            .unwrap();
            Ok(observation)
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "subprocess helper; fixed Policy5 source path, not the legacy default"]
fn checked_output_policy5_source_child() {
    let Some(path) = env::var_os(super::CHILD_ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    if run_real_extractor_child_if_requested(&args) {
        return;
    }
    let mut callbacks = CallbacksV1 {
        progress: progress::CallbackProgress::from_environment(),
        endpoint_directory: snapshots::child_directory(),
        ..CallbacksV1::default()
    };
    callbacks.progress.begin(SourceStage::Rustc);
    let completed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        rustc_driver::run_compiler(&args, &mut callbacks);
    }));
    let result = if completed.is_err() {
        Err(SourceFailure::new(
            SourceStage::Rustc,
            "Policy5 source callback panicked",
        ))
    } else {
        callbacks.result.unwrap_or_else(|| {
            Err(SourceFailure::new(
                SourceStage::Rustc,
                "actual Policy5 source callback did not run",
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
        env::var_os(super::CHILD_RESULT).expect("child result path"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(
        result.is_ok(),
        "fixed Policy5 native source route: {result:?}"
    );
}

#[test]
#[ignore = "requires pinned nightly rust-src and ordinary-source compilation; explicit opt0 rewrite qualification"]
fn ordinary_rust_scalar_borrow_policy5_normal_and_opt0_both_profiles() {
    for profile in [
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950,
    ] {
        ordinary_rust_checked_output_cases_for_profile(
            &[
                OrdinarySourceCase::ScalarBorrowPolicy5,
                OrdinarySourceCase::RetainedScalarBorrowPolicy5,
                OrdinarySourceCase::ScalarBorrowPolicy5Barrier,
            ],
            profile,
        );
    }
}

const CHILD_FIXED_EXTRACTOR_OUTPUT: &str = "FE2O3_TEST_FIXED_CHECKED_EXTRACTOR_OUTPUT";

pub(super) fn configure_real_extractor_child(command: &mut Command, output: Option<&Path>) {
    command.env_remove(CHILD_FIXED_EXTRACTOR_OUTPUT);
    if let Some(output) = output {
        command.env(CHILD_FIXED_EXTRACTOR_OUTPUT, output);
    }
}

pub(super) fn check_real_extraction(report: &Path, result: &Path, output: &Path) {
    let result: Result<(), String> =
        serde_json::from_slice(&std::fs::read(result).unwrap()).unwrap();
    result.unwrap();
    let report: Report = serde_json::from_slice(&std::fs::read(report).unwrap()).unwrap();
    let bytes = std::fs::read(output).unwrap();
    assert!(!bytes.is_empty());
    assert_eq!(<[u8; 32]>::from(Sha256::digest(&bytes)), report.llvm_digest);
    assert!(
        std::str::from_utf8(&bytes)
            .unwrap()
            .contains("amdgpu_kernel")
    );
}

fn run_real_extractor_child_if_requested(args: &[String]) -> bool {
    let Some(output) = env::var_os(CHILD_FIXED_EXTRACTOR_OUTPUT) else {
        return false;
    };
    let result =
        crate::run_production_fixed_checked_output_extraction_driver_v1(args, Path::new(&output));
    std::fs::write(
        env::var_os(super::CHILD_RESULT).expect("explicit real-extractor result path"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    true
}

#[test]
#[ignore = "requires pinned nightly rust-src, AMD dependencies, and ordinary-source compilation"]
fn ordinary_fixed_dispatcher_preserves_source_routes_and_unsigned_proof_refusals() {
    for profile in [
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950,
    ] {
        ordinary_rust_source_cases(
            &[
                OrdinarySourceCase::UnannotatedFill,
                OrdinarySourceCase::PrivateUnitHelper,
                OrdinarySourceCase::RetainedPrivateUnitHelper,
            ],
            profile,
            true,
            None,
        );
    }
}
