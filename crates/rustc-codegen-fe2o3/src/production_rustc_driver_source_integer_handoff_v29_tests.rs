//! Genuine live-rustc integer ownership with an explicit closed finalizer.
use super::*;

const INTEGER_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::integer_handoff_tests::source_owned_integer_child";

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct IntegerResult {
    original: [u8; 32],
    output: [u8; 32],
    changed: bool,
    input_binary: usize,
    output_binary: usize,
    roots: usize,
    foreign_source_refused: bool,
    foreign_ledger_refused: bool,
    callback_error_preserved: bool,
    raw_panic_preserved: bool,
    finalizer_refused: bool,
}

#[derive(Default)]
struct IntegerCallbacks {
    result: Option<Result<IntegerResult, String>>,
}

impl Callbacks for IntegerCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = || {
                transaction_in_active_session_v1(
                    tcx,
                    crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
                )
            };
            let foreign = transaction()?
                .source_owned_ssa_for_test_v29(true)
                .map_err(|error| format!("independent source: {error:?}"))?;
            let mut report = transaction()?
                .with_source_owned_integer_handoff_v29(|source, handoff, budget| {
                    handoff.check_original_source(source.source_ssa(budget)?, budget)?;
                    let input = source.canonical(budget)?;
                    let output = handoff.output(budget)?;
                    assert_eq!(output.input_audit_bytes(), input.canonical_bytes());
                    assert_eq!(source.source_ssa(budget)?.identity(), foreign.identity());
                    assert!(!std::ptr::eq(source.source_ssa(budget)?, &foreign));
                    assert_eq!(output.report().passes().len(), 2);
                    assert_eq!(
                        output.report().passes()[0].pass(),
                        fe2o3_pliron::PlironOptimizationPassV1::IntegerNeutralCanonicalization
                    );
                    assert_eq!(
                        output.report().passes()[1].pass(),
                        fe2o3_pliron::PlironOptimizationPassV1::DeadCodeElimination
                    );
                    let binary_count =
                        |owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18| {
                            owner
                                .module()
                                .functions
                                .iter()
                                .filter_map(|function| function.body.as_ref())
                                .flat_map(|body| &body.blocks)
                                .flat_map(|block| &block.operations)
                                .filter(|operation| {
                                    matches!(
                                        operation.kind,
                                        fe2o3_kernel_ir::OperationKind::Binary { .. }
                                    )
                                })
                                .count()
                        };
                    assert!(handoff.retained_storage(budget)? > 0);
                    Ok(IntegerResult {
                        original: Sha256::digest(input.canonical_bytes()).into(),
                        output: Sha256::digest(output.owner().canonical_bytes()).into(),
                        changed: input.canonical_bytes() != output.owner().canonical_bytes(),
                        input_binary: binary_count(input),
                        output_binary: binary_count(output.owner()),
                        roots: input.module().kernels.len(),
                        foreign_source_refused: false,
                        foreign_ledger_refused: false,
                        callback_error_preserved: false,
                        raw_panic_preserved: false,
                        finalizer_refused: false,
                    })
                })
                .map_err(|error| format!("real integer output: {error:?}"))?;
            let error = transaction()?
                .with_source_owned_integer_handoff_v29::<(), _>(|_, handoff, budget| {
                    handoff.check_original_source(&foreign, budget)?;
                    panic!("foreign owner reached the integer continuation")
                })
                .unwrap_err();
            assert!(
                matches!(error, Error::Source(SourceError::Binding(_))),
                "{error:?}"
            );
            report.foreign_source_refused = true;
            let error = transaction()?
                .with_source_owned_integer_handoff_v29::<(), _>(|_, handoff, budget| {
                    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(500_000_000);
                    let mut foreign = Budget::new(&mut work, 20_000_000);
                    foreign.reserve_storage(budget.storage())?;
                    handoff.output(&foreign)?;
                    panic!("foreign account reached the integer continuation")
                })
                .unwrap_err();
            assert!(
                matches!(
                    error,
                    Error::Source(SourceError::Resource(ResourceError::Accounting))
                ),
                "{error:?}"
            );
            report.foreign_ledger_refused = true;
            let error = transaction()?
                .with_source_owned_integer_handoff_v29::<(), _>(|_, _, _| {
                    Err(Error::Unsupported("integer consumer original refusal"))
                })
                .unwrap_err();
            assert!(matches!(
                error,
                Error::Unsupported("integer consumer original refusal")
            ));
            report.callback_error_preserved = true;
            let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                transaction()
                    .unwrap()
                    .with_source_owned_integer_handoff_v29::<(), _>(|_, _, _| {
                        std::panic::panic_any(0x1781_u32)
                    })
            }));
            let payload = match panicked {
                Err(payload) => payload,
                Ok(_) => panic!("integer consumer lost its original panic"),
            };
            assert_eq!(payload.downcast_ref::<u32>(), Some(&0x1781_u32));
            report.raw_panic_preserved = true;
            let refusal = transaction()?.source_owned_integer_finalizer_refusal_v29();
            assert!(
                matches!(
                    refusal,
                    Error::Unsupported("source-owned integer final admission required")
                ),
                "{refusal:?}"
            );
            report.finalizer_refused = true;
            Ok(report)
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "process helper; requires an exact actual-source request from its parent"]
fn source_owned_integer_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = IntegerCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("integer live-rustc callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("integer result path"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(
        result.is_ok(),
        "integer live-rustc continuation: {result:?}"
    );
}

fn check_integer_sources(body: &str, changed: bool) {
    run_actual_sources::<IntegerResult>(
        &[("integer", body), ("integer", body)],
        &[(0, 0)],
        INTEGER_CHILD,
        "SOURCE_OWNED_INTEGER_V29",
        scalar_source,
        |_, _, label, result, prior| {
            assert_eq!(result.roots, 2);
            assert_eq!(result.changed, changed);
            assert_eq!(result.original != result.output, changed);
            assert_eq!(result.input_binary > 0, changed);
            assert_eq!(result.output_binary, 0);
            assert!(result.foreign_source_refused && result.foreign_ledger_refused);
            assert!(result.callback_error_preserved && result.raw_panic_preserved);
            assert!(result.finalizer_refused);
            if let Some(previous) = prior.get(label) {
                assert_eq!(&result, previous);
            } else {
                prior.insert(label.to_owned(), result);
            }
        },
    );
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn actual_integer_neutral_output_is_owned_under_source_custody_before_final_refusal() {
    check_integer_sources("seed &= u32::MAX;", true);
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn actual_integer_noop_output_remains_unqualified_and_keeps_hostile_controls() {
    check_integer_sources("", false);
}
