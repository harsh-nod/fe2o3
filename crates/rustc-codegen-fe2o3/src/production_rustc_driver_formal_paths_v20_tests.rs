//! Actual original-source memory/report/path composition, not final admission.
use super::*;
use fe2o3_kernel_analysis::FormalPaidPathDecisionV20;
use fe2o3_kernel_ir::{
    CanonicalFormalLaunchInputV19, ExplicitLaunchExtent, FormalIndexWidth, OperationKind,
};
use fe2o3_lower_mir_kernel::ProductionFormalPathSideV20 as Side;
use std::cell::Cell;

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::original_source_tests::formal_paths_tests::formal_paths_child";
const REFUSAL: &str = "actual paired paths requested observation refusal";

fn program(body: &str) -> String {
    let alpha = if body == "memory" {
        r#"
#[inline(never)]
fn store(output: &mut DisjointSlice<u32, GridExclusive>, leader: &GridLeader, value: u32) {
    let Some(slot) = output.get_mut_exclusive(leader, 0) else { fe2o3_device::trap(); };
    *slot = value + 0;
}
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [3, 1, 1]))]
pub fn paths_alpha(mut output: DisjointSlice<u32, GridExclusive>, value: u32, _word: usize) {
    let Some(leader) = thread::grid_leader() else { return; };
    store(&mut output, &leader, value);
}
"#
    } else {
        assert_eq!(body, "empty");
        r#"#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [3, 1, 1]))]
pub fn paths_alpha(_word: usize) {}
"#
    };
    format!(
        "use fe2o3_device::{{DisjointSlice, GridExclusive, GridLeader, kernel, thread}};\n{alpha}\n#[kernel(typed, launch(required = [32, 1, 1], max = [32, 1, 1], max_grid = [5, 1, 1]))]\npub fn paths_beta(_word: u32) {{}}\n"
    )
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Observation {
    roots: usize,
    callbacks: usize,
    extents: [[u64; 3]; 2],
    original: [u8; 32],
    output: [u8; 32],
    memory_operations: usize,
    conflict_rows: usize,
    observations: usize,
    excluded: usize,
    not_proved: usize,
    reasons: usize,
    complete: bool,
    exact_owner_report_rows: bool,
    prepared: bool,
    materialized: bool,
    exact_consumer_refusal: bool,
}

#[derive(Default)]
struct PathCallbacks {
    result: Option<Result<Observation, String>>,
}

impl Callbacks for PathCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = || {
                transaction_in_active_session_v1(
                    tcx,
                    crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
                )
            };
            start_preparation_observation_v29();
            let mut observation = Observation {
                roots: 0,
                callbacks: 0,
                extents: [[0; 3]; 2],
                original: [0; 32],
                output: [0; 32],
                memory_operations: 0,
                conflict_rows: 0,
                observations: 0,
                excluded: 0,
                not_proved: 0,
                reasons: 0,
                complete: true,
                exact_owner_report_rows: false,
                prepared: false,
                materialized: false,
                exact_consumer_refusal: false,
            };
            let continuation = transaction()?
                .with_original_source_formal_paths_v20(|side, before, after, path, _| {
                    let root = observation.callbacks / 2;
                    assert_eq!(before.root_index(), root);
                    assert_eq!(after.root_index(), root);
                    assert_eq!(path.root_index(), root);
                    assert_eq!(
                        side,
                        if observation.callbacks % 2 == 0 {
                            Side::Original
                        } else {
                            Side::Optimized
                        }
                    );
                    assert!(!std::ptr::eq(
                        before.original_owner(),
                        after.original_owner()
                    ));
                    let report = if side == Side::Original {
                        before.analysis()
                    } else {
                        after.analysis()
                    };
                    let owner = if side == Side::Original {
                        before.original_owner()
                    } else {
                        after.original_owner()
                    };
                    assert!(std::ptr::eq(path.original_owner(), owner));
                    assert!(std::ptr::eq(path.analysis(), report));
                    assert_eq!(path.index_width(), FormalIndexWidth::Bits64);
                    assert_eq!(path.launch_input(), before.launch_input());
                    assert_eq!(path.launch_input(), after.launch_input());
                    let CanonicalFormalLaunchInputV19::PhysicalEnvelope(
                        ExplicitLaunchExtent::Exact { rank: 1, extents },
                    ) = path.launch_input()
                    else {
                        panic!("original physical coordinate envelope required");
                    };
                    assert_eq!(&extents[1..], &[1, 1]);
                    assert!(matches!(extents[0], 160 | 192));
                    if side == Side::Original {
                        observation.extents[root] = extents;
                        observation.roots += 1;
                    } else {
                        assert_eq!(observation.extents[root], extents);
                    }
                    let original: [u8; 32] =
                        Sha256::digest(before.original_owner().canonical_bytes()).into();
                    let output: [u8; 32] =
                        Sha256::digest(after.original_owner().canonical_bytes()).into();
                    if observation.callbacks == 0 {
                        observation.original = original;
                        observation.output = output;
                    } else {
                        assert_eq!(observation.original, original);
                        assert_eq!(observation.output, output);
                    }
                    let conflicts = report.obligations().inter_invocation_conflicts();
                    assert_eq!(path.observations().len(), conflicts.len());
                    for (ordinal, row) in path.observations().iter().enumerate() {
                        assert_eq!(row.ordinal(), ordinal);
                        assert_eq!(row.requirement(), conflicts[ordinal]);
                        match row.decision() {
                            FormalPaidPathDecisionV20::ExcludedForAllU64Coordinates => {
                                observation.excluded += 1
                            }
                            FormalPaidPathDecisionV20::NotProved => observation.not_proved += 1,
                        }
                    }
                    observation.conflict_rows += conflicts.len();
                    observation.observations += path.observations().len();
                    observation.reasons += report.incomplete_reasons().len();
                    observation.complete &= report.is_complete();
                    if root == 0 {
                        observation.memory_operations += path
                            .original_owner()
                            .module()
                            .functions
                            .iter()
                            .filter_map(|function| function.body.as_ref())
                            .flat_map(|body| &body.blocks)
                            .flat_map(|block| &block.operations)
                            .filter(|op| {
                                matches!(
                                    op.kind,
                                    OperationKind::Load { .. } | OperationKind::Store { .. }
                                )
                            })
                            .count();
                    }
                    observation.callbacks += 1;
                    Ok(())
                })
                .map_err(|error| format!("actual paired path composition: {error:?}"))?;
            continuation.into_observation();
            let preparation =
                take_preparation_observation_v29().expect("genuine owning source preparation");
            assert!(preparation.materialized);
            assert_eq!((observation.roots, observation.callbacks), (2, 4));
            observation.extents.sort();
            assert_eq!(observation.extents, [[160, 1, 1], [192, 1, 1]]);
            assert_eq!(observation.conflict_rows, observation.observations);
            assert_eq!(
                observation.excluded + observation.not_proved,
                observation.observations
            );
            observation.exact_owner_report_rows = true;
            observation.prepared = true;
            observation.materialized = preparation.materialized;

            let completed = Cell::new(false);
            let error = refused(transaction()?.with_original_source_formal_paths_v20(
                |side, before, after, path, _| {
                    assert_eq!(side, Side::Original);
                    assert_eq!(
                        (before.root_index(), after.root_index(), path.root_index()),
                        (0, 0, 0)
                    );
                    assert!(std::ptr::eq(path.analysis(), before.analysis()));
                    completed.set(true);
                    Err(Error::Unsupported(REFUSAL))
                },
            ));
            assert!(completed.get());
            let mut cause: &(dyn std::error::Error + 'static) = &error;
            loop {
                if matches!(
                    cause.downcast_ref::<Error>(),
                    Some(Error::Unsupported(REFUSAL))
                ) {
                    observation.exact_consumer_refusal = true;
                    break;
                }
                let Some(next) = cause.source() else {
                    break;
                };
                cause = next;
            }
            assert!(
                observation.exact_consumer_refusal,
                "lost typed consumer refusal: {error:?}"
            );
            Ok(observation)
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "process helper; requires an exact actual-source request from its parent"]
fn formal_paths_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = PathCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("actual paired path callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("paired path observation result"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "paired path observation failed: {result:?}");
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn actual_paired_paths_bind_original_memory_roots_and_preserve_every_residual() {
    run_actual_sources::<Observation>(
        &[("paths_empty", "empty"), ("paths_memory", "memory")],
        &[(0, 0)],
        CHILD,
        "ORIGINAL_SOURCE_FORMAL_PATHS_V20",
        program,
        |_, _, label, report, _| {
            assert_eq!((report.roots, report.callbacks), (2, 4));
            assert_eq!(report.extents, [[160, 1, 1], [192, 1, 1]]);
            assert_eq!(report.conflict_rows, report.observations);
            assert_eq!(report.excluded + report.not_proved, report.observations);
            assert!(
                report.exact_owner_report_rows
                    && report.prepared
                    && report.materialized
                    && report.exact_consumer_refusal
            );
            if label == "paths_memory" {
                assert!(report.memory_operations > 0);
            } else {
                assert_eq!(label, "paths_empty");
                assert_eq!(report.memory_operations, 0);
                assert_eq!(report.reasons, 0);
                assert!(report.complete);
            }
        },
    );
}

#[test]
fn actual_paired_path_fixture_keeps_memory_and_scalar_roots_with_nominal_input() {
    for kind in ["empty", "memory"] {
        let source = program(kind);
        assert_eq!(source.matches("#[kernel(").count(), 2);
        assert!(source.contains("_word: usize"));
        assert!(source.contains("max_grid = [3, 1, 1]"));
        assert!(source.contains("max_grid = [5, 1, 1]"));
        assert_eq!(
            source.contains("output.get_mut_exclusive(leader, 0)"),
            kind == "memory"
        );
    }
    assert!(CHILD.ends_with("::formal_paths_tests::formal_paths_child"));
}
