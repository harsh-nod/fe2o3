//! Actual Rust reaches the source-owned motion owner, not a reconstructed graph.
use super::*;
use crate::production_pipeline::source_owned_v29::target_result::{
    mixed_licm_v28::{
        tests::genuine_mixed_case,
        worker_input_v26::tests::{Mode as WorkerMode, genuine_worker_input_case},
    },
    tests::Mode as TargetMode,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
use std::panic::{AssertUnwindSafe, catch_unwind};

#[path = "production_rustc_driver_mixed_worker_v28_tests.rs"]
mod worker_orchestration_tests;

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::original_source_tests::mixed_licm_tests::mixed_licm_child";
const STRAIGHT: &str = "*slot = (value ^ seed) | seed;";
const LOOP: &str = r#"
    let mut iteration = 0u32;
    while iteration < trips {
        *slot = (value ^ seed) | seed;
        iteration += 1;
    }
"#;

fn program(body: &str) -> String {
    let control_flow = match body {
        STRAIGHT => "",
        LOOP => ", control_flow(loop_bounds(4294967295))",
        _ => panic!("unregistered source-owned motion fixture"),
    };
    let kernel = |name: &str| {
        format!(
            r#"
#[kernel(typed{control_flow}, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [3, 1, 1]))]
pub fn {name}(input: &[u32], mut output: DisjointSlice<u32>, _unused: &[u32], seed: u32, trips: u32) {{
    let index = thread::index_1d();
    let i = index.get();
    let Some(slot) = output.get_mut(index) else {{ return; }};
    if i >= input.len() {{ return; }}
    let value = input[i];
    {body}
}}
"#
        )
    };
    format!(
        "use fe2o3_device::{{DisjointSlice, kernel, thread}};\n{}{}",
        kernel("mixed_first"),
        kernel("mixed_second")
    )
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Observation {
    policy: u16,
    roots: usize,
    original: [u8; 32],
    prefix: [u8; 32],
    output: [u8; 32],
    moved: usize,
    changed_definitions: usize,
    actual_pair_checked: bool,
    target: u16,
    callbacks: [usize; 5],
    early_callbacks: usize,
    target_controls_checked: bool,
    worker_input_controls_checked: bool,
}

#[derive(Default)]
struct MotionCallbacks {
    result: Option<Result<Observation, String>>,
}

impl Callbacks for MotionCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = || {
                transaction_in_active_session_v1(
                    tcx,
                    crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
                )
            };
            let foreign = transaction()?
                .original_source_ssa_for_test_v18()
                .map_err(|error| format!("foreign motion source: {error:?}"))?;
            let mut callbacks = [0; 5];
            let mut observation = transaction()?
                .with_original_source_mixed_licm_v28(|source, relocation, roots, target, budget| {
                    callbacks[0] += 1;
                    relocation.check_original_source(source.source_ssa(budget)?, budget)?;
                    let prefix = relocation.prefix(budget)?;
                    prefix.check_original_argument_abi_v26(AbiInput { roots }, budget)?;
                    let original = source.canonical(budget)?;
                    let optimized = prefix.output(budget)?;
                    let tail = relocation.tail(budget)?;
                    assert_eq!(optimized.input_audit_bytes(), original.canonical_bytes());
                    assert_eq!(optimized.execution().policy_version(), 10);
                    assert_eq!(
                        source.source_semantic(budget)?.wire_version(),
                        SemanticMirWireVersionV1::V35
                    );
                    assert_eq!(source.root_count(budget)?, roots.len());
                    assert_eq!(roots.len(), 2);
                    assert_eq!(original.module().kernels.len(), roots.len());
                    assert_eq!(
                        tail.output().module().kernels,
                        optimized.owner().module().kernels
                    );
                    assert!(!relocation.final_native_completion_is_complete());
                    assert!(!relocation.grants_artifact_or_launch_authority());
                    let bytes = original
                        .canonical_bytes()
                        .len()
                        .checked_add(optimized.owner().canonical_bytes().len())
                        .and_then(|n| n.checked_add(tail.output().canonical_bytes().len()))
                        .ok_or(Resource::Arithmetic)?;
                    budget.charge_work(bytes)?;
                    let digest_original = Sha256::digest(original.canonical_bytes()).into();
                    let digest_prefix = Sha256::digest(optimized.owner().canonical_bytes()).into();
                    let digest_output = Sha256::digest(tail.output().canonical_bytes()).into();
                    let definitions = relocation.definition_projection(budget)?;
                    budget.charge_work(
                        definitions
                            .len()
                            .checked_add(tail.origins().len())
                            .ok_or(Resource::Arithmetic)?,
                    )?;
                    let changed_definitions = definitions
                        .iter()
                        .filter(|row| row.input != row.output)
                        .count();
                    let moved = tail
                        .origins()
                        .iter()
                        .filter(|row| row.hoist.is_some())
                        .count();
                    let (pair, receipt) = tail.replay_against(optimized.owner(), budget).map_err(
                        fe2o3_lower_mir_kernel::ProductionMixedLicmRelocationErrorV28::from,
                    )?;
                    budget.reserve_storage(receipt.retained_storage())?;
                    assert!(std::ptr::eq(pair.input(), optimized.owner()));
                    assert!(std::ptr::eq(pair.output(), tail.output()));
                    assert_eq!(pair.origins(), tail.origins());
                    drop(pair);
                    budget.release_storage(receipt.retained_storage())?;
                    Ok(Observation {
                        policy: optimized.execution().policy_version(),
                        roots: roots.len(),
                        original: digest_original,
                        prefix: digest_prefix,
                        output: digest_output,
                        moved,
                        changed_definitions,
                        actual_pair_checked: true,
                        target: if target.device_target().starts_with("gfx942") {
                            942
                        } else {
                            assert!(target.device_target().starts_with("gfx950"));
                            950
                        },
                        callbacks: [0; 5],
                        early_callbacks: 0,
                        target_controls_checked: false,
                        worker_input_controls_checked: false,
                    })
                })
                .map_err(|error| format!("source-owned motion continuation: {error:?}"))?
                .into_observation();

            for mode in [
                TargetMode::Success,
                TargetMode::ExactStorage,
                TargetMode::ShortStorage,
                TargetMode::WorkRefusal,
                TargetMode::ForeignEntry,
                TargetMode::RestoredFloor,
            ] {
                let result = transaction()?.with_original_source_mixed_licm_v28(
                    |source, relocation, _, target, budget| {
                        let native = relocation
                            .complete_native_v28(budget)
                            .map_err(Error::MixedLicmCompletion)?;
                        let selected = genuine_mixed_case(source, &native, target, budget, mode)
                            .map_err(Error::from);
                        let released = native.discard(budget).map_err(Error::from);
                        selected.and(released)
                    },
                );
                if matches!(mode, TargetMode::Success | TargetMode::ExactStorage) {
                    result
                        .map_err(|error| format!("final LICM target {mode:?}: {error:?}"))?
                        .into_observation();
                } else {
                    assert!(matches!(
                        result.err().expect("final LICM target refusal"),
                        Error::TargetLlvm(_)
                    ));
                }
            }
            observation.target_controls_checked = true;
            for mode in [
                WorkerMode::Success,
                WorkerMode::WrongTarget,
                WorkerMode::WrongBinding,
                WorkerMode::WrongType,
                WorkerMode::IncompleteAbi,
                WorkerMode::ForeignLedger,
                WorkerMode::ExactStorage,
                WorkerMode::ShortStorage,
                WorkerMode::WorkRefusal,
                WorkerMode::RestoredFloor,
            ] {
                let result = transaction()?.with_original_source_mixed_licm_v28(
                    |source, relocation, roots, target, budget| {
                        let native = relocation
                            .complete_native_v28(budget)
                            .map_err(Error::MixedLicmCompletion)?;
                        let selected =
                            genuine_worker_input_case(source, &native, roots, target, budget, mode)
                                .map_err(Error::from);
                        let released = native.discard(budget).map_err(Error::from);
                        selected.and(released)
                    },
                );
                if matches!(mode, WorkerMode::Success | WorkerMode::ExactStorage) {
                    result
                        .map_err(|error| format!("final LICM Worker input {mode:?}: {error:?}"))?
                        .into_observation();
                } else {
                    assert!(matches!(
                        result.err().expect("final LICM Worker refusal"),
                        Error::MixedLicmWorkerInput(_)
                    ));
                }
            }
            observation.worker_input_controls_checked = true;
            let foreign_result = transaction()?.with_original_source_mixed_licm_v28::<(), _>(
                |_, relocation, _, _, budget| {
                    callbacks[1] += 1;
                    relocation.check_original_source(&foreign, budget)?;
                    Ok(())
                },
            );
            assert!(matches!(
                foreign_result,
                Err(Error::Source(SourceError::Binding(
                    "foreign original SSA owner"
                )))
            ));
            let abi_result = transaction()?.with_original_source_mixed_licm_v28::<(), _>(
                |_, relocation, _, _, budget| {
                    callbacks[2] += 1;
                    relocation
                        .prefix(budget)?
                        .check_original_argument_abi_v26(AbiInput { roots: &[] }, budget)?;
                    Ok(())
                },
            );
            assert!(matches!(
                abi_result,
                Err(Error::Source(SourceError::Binding(
                    "kernel argument ABI profile differs from its original descriptor/source contract"
                )))
            ));
            let selected =
                transaction()?.with_original_source_mixed_licm_v28::<(), _>(|_, _, _, _, _| {
                    callbacks[3] += 1;
                    Err(Error::Unsupported("selected actual motion consumer"))
                });
            assert!(matches!(
                selected,
                Err(Error::Unsupported("selected actual motion consumer"))
            ));
            let transaction = transaction()?;
            let unwind = catch_unwind(AssertUnwindSafe(|| {
                transaction.with_original_source_mixed_licm_v28::<(), _>(|_, _, _, _, _| {
                    callbacks[4] += 1;
                    std::panic::panic_any(731u32);
                })
            }));
            let payload = match unwind {
                Err(payload) => payload,
                Ok(_) => panic!("selected raw motion panic was swallowed"),
            };
            assert_eq!(*payload.downcast::<u32>().unwrap(), 731);
            assert_eq!(callbacks, [1; 5]);
            observation.callbacks = callbacks;
            // Reconstruct a fresh transaction after the selected raw unwind.
            let mut early_callbacks = 0;
            let early = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?
            .with_original_source_mixed_licm_test_limits_v28::<(), _>(
                0,
                20_000_000,
                |_, _, _, _, _| {
                    early_callbacks += 1;
                    Ok(())
                },
            );
            assert!(
                matches!(early, Err(Error::Resource(Resource::Work(error))) if error.limit() == 0 && error.actual() > 0)
            );
            assert_eq!(early_callbacks, 0);
            observation.early_callbacks = early_callbacks;
            Ok(observation)
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "subprocess helper for the genuine original Rust motion parent"]
fn mixed_licm_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = MotionCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("actual motion callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("motion result"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "actual source-owned motion: {result:?}");
}

#[test]
#[ignore = "requires pinned nightly rust-src/rustc-dev and authentic AMD dependencies"]
fn actual_original_mixed_licm_preserves_source_and_checks_final_loop_and_nonloop_graphs() {
    run_actual_sources::<Observation>(
        &[("straight", STRAIGHT), ("loop", LOOP)],
        &[(0, 0)],
        CHILD,
        "ORIGINAL_SOURCE_MIXED_LICM_V28",
        program,
        |_, _, case, report, _| {
            assert_eq!((report.policy, report.roots), (10, 2));
            assert_eq!(report.callbacks, [1; 5]);
            assert_eq!(report.early_callbacks, 0);
            assert!(report.actual_pair_checked);
            assert!(report.target_controls_checked && report.worker_input_controls_checked);
            assert!(matches!(report.target, 942 | 950));
            assert_ne!(report.original, [0; 32]);
            if case == "loop" {
                assert!(report.moved >= 4);
                assert!(report.changed_definitions >= 4);
                assert_ne!(report.prefix, report.output);
            } else {
                assert_eq!(report.moved, 0);
                assert_eq!(report.changed_definitions, 0);
                assert_eq!(report.prefix, report.output);
            }
        },
    );
}

#[test]
fn mixed_licm_actual_source_matrix_has_two_roots_and_dynamic_zero_trip_control() {
    for body in [STRAIGHT, LOOP] {
        let source = program(body);
        assert_eq!(source.matches("#[kernel(").count(), 2);
        assert_eq!(source.matches(body).count(), 2);
        assert!(!source.contains("unsafe"));
        assert_eq!(source.matches("trips: u32").count(), 2);
        assert_eq!(source.matches("_unused: &[u32]").count(), 2);
        assert_eq!(
            source
                .matches("control_flow(loop_bounds(4294967295))")
                .count(),
            if body == LOOP { 2 } else { 0 }
        );
        assert_eq!(
            source.matches("while iteration < trips").count(),
            if body == LOOP { 2 } else { 0 }
        );
        assert!(!source.contains("integer_switches"));
    }
    assert!(LOOP.contains("while iteration < trips"));
    assert!(CHILD.ends_with("::mixed_licm_tests::mixed_licm_child"));
}
