//! Genuine Rust exercises the final-native, source-bound expression entrance.
use super::*;
#[path = "production_rustc_driver_mixed_relocation_cfg_v28_tests.rs"]
mod cfg_tests;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_lower_mir_kernel::{
    ProductionMixedLicmRelocationV28 as Relocation, ProductionSourceOwnedViewV18 as Source,
};
use fe2o3_verifier::{
    MixedOptimizerRelocationErrorV28 as RequestError,
    prepare_mixed_relocation_expressions_v28 as prepare,
};
use std::cell::Cell;

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::original_source_tests::mixed_relocation_tests::mixed_relocation_child";
const WORK: usize = 500_000_000;
const STORAGE: usize = 20_000_000;
const LOOP: &str = r#"
    let mut iteration = 0u32;
    while iteration < trips {
        *slot = (value ^ seed) | seed;
        iteration += 1;
    }
"#;
const NESTED: &str = r#"
    let mut outer = 0u32;
    while outer < trips {
        let mut inner = 0u32;
        while inner < seed {
            *slot = (value ^ outer) | seed;
            inner += 1;
        }
        outer += 1;
    }
"#;
fn program(body: &str) -> String {
    let kernel = |name: &str| {
        format!(
            r#"
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [3, 1, 1]))]
pub fn {name}(input: &[u32], mut output: DisjointSlice<u32>, seed: u32, trips: u32) {{
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
        kernel("relocation_first"),
        kernel("relocation_second")
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct Measurement {
    work: usize,
    peak: usize,
}
#[derive(Clone, Copy)]
enum Bounds {
    Measure,
    Exact(Measurement),
    WorkShort(Measurement),
    StorageShort(Measurement),
}
thread_local! {
    static BOUNDS: Cell<Bounds> = const { Cell::new(Bounds::Measure) };
    static ENTERED: Cell<usize> = const { Cell::new(0) };
}

fn resource(error: &Error) -> Resource {
    let mut cause: &(dyn std::error::Error + 'static) = error;
    loop {
        if let Some(resource) = cause.downcast_ref::<Resource>() {
            return *resource;
        }
        if let Some(work) = cause.downcast_ref::<fe2o3_kernel_ir::CanonicalKernelIrWorkLimitV1>() {
            return Resource::Work(*work);
        }
        cause = cause
            .source()
            .unwrap_or_else(|| panic!("not a typed resource refusal: {error:?}"));
    }
}
fn selected<T: Copy>(
    result: Result<T, Error>,
    settled: Result<(), SourceError>,
) -> Result<T, Error> {
    match result {
        Ok(value) => settled.map(|()| value).map_err(Error::from),
        Err(error) => Err(error),
    }
}
fn measure<'p, 'v, 's>(
    source: &'v Source<'s>,
    relocation: &Relocation<'p, 'v, 's>,
    _: &[AbiRoot<'_>],
    _: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    budget: &mut Budget<'_>,
) -> Result<Measurement, Error> {
    let native = relocation
        .complete_native_v28(budget)
        .map_err(Error::MixedLicmCompletion)?;
    let result = (|| {
        ENTERED.with(|entered| entered.set(entered.get() + 1));
        let original = budget.storage();
        let bounds = BOUNDS.with(Cell::get);
        let floor = match bounds {
            Bounds::Measure => budget.peak_storage(),
            Bounds::Exact(measured) | Bounds::WorkShort(measured) => {
                budget.storage_limit() - measured.peak
            }
            Bounds::StorageShort(measured) => budget.storage_limit() - (measured.peak - 1),
        };
        let padding = floor.checked_sub(original).unwrap();
        budget.reserve_storage(padding)?;
        if let Bounds::Exact(measured)
        | Bounds::WorkShort(measured)
        | Bounds::StorageShort(measured) = bounds
        {
            let allowed = measured.work - usize::from(matches!(bounds, Bounds::WorkShort(_)));
            budget.charge_work(WORK - budget.work() - allowed)?;
        }
        let before = budget.work();
        let result = match prepare(source, &native, budget) {
            Ok(request) => {
                let inspected = (|| {
                    request.replay(budget)?;
                    let subject = request.subject(budget)?;
                    assert_eq!(subject.prefix_policy_version(), 10);
                    assert!(subject.moved_operations() >= 4);
                    assert!(subject.moved_results() >= subject.moved_operations());
                    assert!(subject.cut_bindings() > 0);
                    assert!(subject.runtime_occurrences() >= 4);
                    assert_eq!(subject.slice_premises(), 4);
                    assert!(request.retained_storage(budget)? > 0);
                    assert!(!request.authenticates_executed_proof());
                    assert!(!request.proves_cfg_refinement());
                    assert!(!request.grants_artifact_or_launch_authority());
                    Ok::<(), RequestError>(())
                })();
                let settled = request.discard(budget);
                inspected
                    .and(settled)
                    .map_err(Error::MixedRelocationExpressions)
            }
            Err(error) => Err(Error::MixedRelocationExpressions(error)),
        };
        assert_eq!(
            budget.storage(),
            floor,
            "request settles before caller padding"
        );
        let measured = Measurement {
            work: budget.work() - before,
            peak: budget.peak_storage() - floor,
        };
        budget.release_storage(padding)?;
        assert_eq!(budget.storage(), original);
        result.map(|()| measured)
    })();
    let settled = native.discard(budget);
    selected(result, settled)
}

#[derive(Debug, Serialize, Deserialize)]
struct Observation {
    production_stage: bool,
    measured: Measurement,
    exact_short: usize,
    foreign_source: usize,
    foreign_ledger: usize,
    restored_floor: usize,
}
#[derive(Default)]
struct RelocationCallbacks {
    result: Option<Result<Observation, String>>,
}
impl Callbacks for RelocationCallbacks {
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
                .map_err(|error| format!("foreign original relocation owner: {error:?}"))?;
            let stage = transaction()?
                .analyze_original_source_mixed_relocation_v28()
                .map_err(|error| format!("production relocation expression stage: {error:?}"))?
                .into_observation();
            assert_eq!(stage.prefix_policy_version(), 10);
            assert!(stage.moved_operations() >= 4 && stage.cut_bindings() > 0);
            assert_eq!(stage.slice_premises(), 4);
            assert_ne!(stage.prefix(), stage.output());
            BOUNDS.with(|mode| mode.set(Bounds::Measure));
            ENTERED.with(|entered| entered.set(0));
            let measured = transaction()?
                .with_original_source_mixed_licm_test_limits_v28(WORK, STORAGE, measure)
                .map_err(|error| format!("relocation request measure: {error:?}"))?
                .into_observation();
            assert_eq!(ENTERED.with(Cell::get), 1);
            assert!(measured.work > 0 && measured.peak > 0);
            for bounds in [
                Bounds::Exact(measured),
                Bounds::WorkShort(measured),
                Bounds::StorageShort(measured),
            ] {
                BOUNDS.with(|mode| mode.set(bounds));
                ENTERED.with(|entered| entered.set(0));
                let result = transaction()?
                    .with_original_source_mixed_licm_test_limits_v28(WORK, STORAGE, measure);
                assert_eq!(
                    ENTERED.with(Cell::get),
                    1,
                    "short limit must reach final-native request"
                );
                match bounds {
                    Bounds::Exact(_) => assert_eq!(
                        result
                            .map_err(|error| format!("exact relocation request: {error:?}"))?
                            .into_observation(),
                        measured
                    ),
                    Bounds::WorkShort(_) => assert!(
                        matches!(resource(&refused(result)), Resource::Work(error) if error.limit() == WORK && error.actual() > WORK)
                    ),
                    Bounds::StorageShort(_) => assert!(
                        matches!(resource(&refused(result)), Resource::Storage(error) if error.limit() == STORAGE && error.actual() > STORAGE)
                    ),
                    Bounds::Measure => unreachable!(),
                }
            }
            let reached = Cell::new(false);
            let error = refused(transaction()?.with_original_source_mixed_licm_v28::<(), _>(
                |source, relocation, roots, _, budget| {
                    let native = relocation
                        .complete_native_v28(budget)
                        .map_err(Error::MixedLicmCompletion)?;
                    let result = (|| {
                        native.check_original_argument_abi_v26(AbiInput { roots }, budget)?;
                        let floor = budget.storage();
                        let request = prepare(source, &native, budget)
                            .map_err(Error::MixedRelocationExpressions)?;
                        let inspected = request.check_original_source(&foreign, budget);
                        assert!(matches!(
                            inspected,
                            Err(RequestError::Source(SourceError::Binding(
                                "foreign original SSA owner"
                            )))
                        ));
                        let _ = request.discard(budget);
                        assert_eq!(budget.storage(), floor);
                        reached.set(true);
                        inspected.map_err(Error::MixedRelocationExpressions)
                    })();
                    let settled = native.discard(budget);
                    selected(result, settled)
                },
            ));
            assert!(reached.get());
            assert!(matches!(
                error,
                Error::MixedRelocationExpressions(RequestError::Source(SourceError::Binding(
                    "foreign original SSA owner"
                )))
            ));
            for undercut in [false, true] {
                let reached = Cell::new(false);
                let error = refused(transaction()?.with_original_source_mixed_licm_v28::<(), _>(
                    |source, relocation, _, _, budget| {
                        let native = relocation
                            .complete_native_v28(budget)
                            .map_err(Error::MixedLicmCompletion)?;
                        let result = (|| {
                            let request = prepare(source, &native, budget)
                                .map_err(Error::MixedRelocationExpressions)?;
                            let required = budget.storage();
                            let query = if undercut {
                                budget.release_storage(1)?;
                                let query = request.subject(budget);
                                budget.reserve_storage(1)?;
                                query
                            } else {
                                let mut work = Work::new(WORK);
                                let mut foreign = Budget::new(&mut work, STORAGE);
                                foreign.reserve_storage(required)?;
                                let query = request.subject(&foreign);
                                assert_eq!(foreign.storage(), required);
                                query
                            };
                            let error = query.expect_err("lost request custody was accepted");
                            assert!(matches!(
                                error,
                                RequestError::Source(SourceError::Resource(Resource::Accounting))
                            ));
                            assert!(request.subject(budget).is_err());
                            assert!(request.discard(budget).is_err());
                            assert_eq!(
                                budget.storage(),
                                required,
                                "restored balances cannot regain refund"
                            );
                            reached.set(true);
                            Err(Error::MixedRelocationExpressions(error))
                        })();
                        let before = budget.storage();
                        assert!(native.discard(budget).is_err());
                        assert_eq!(
                            budget.storage(),
                            before,
                            "linked native credit stays denied"
                        );
                        result
                    },
                ));
                assert!(reached.get());
                assert!(matches!(resource(&error), Resource::Accounting));
            }
            Ok(Observation {
                production_stage: true,
                measured,
                exact_short: 3,
                foreign_source: 1,
                foreign_ledger: 1,
                restored_floor: 1,
            })
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "subprocess child requires actual rustc arguments"]
fn mixed_relocation_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = RelocationCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("actual source-bound relocation callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("relocation result"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(
        result.is_ok(),
        "actual relocation expression request: {result:?}"
    );
}

#[test]
#[ignore = "requires pinned rust-src/rustc-dev and actual AMD dependencies; no Verus execution"]
fn actual_original_relocation_expressions_retain_final_native_source_and_exact_custody() {
    run_actual_sources::<Observation>(
        &[("loop", LOOP), ("nested", NESTED)],
        &[(0, 0)],
        CHILD,
        "MIXED_RELOCATION_EXPRESSIONS_V28",
        program,
        |_, _, _, report, _| {
            assert!(report.production_stage);
            assert_eq!(
                (
                    report.exact_short,
                    report.foreign_source,
                    report.foreign_ledger,
                    report.restored_floor
                ),
                (3, 1, 1, 1)
            );
            assert!(report.measured.work > 0 && report.measured.peak > 0);
        },
    );
}
