//! Bounded observations of the retained original owner, not model admission.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
use fe2o3_mir_model::semantic_mir_v1::{SemanticRvalueKindV1, SemanticStatementKindV1};
use std::fmt::Write as _;

const OUTPUT_LIMIT: usize = 2 * 1024 * 1024;
const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::pending_source_tests::expanded_source_tests::expanded_model_tests::aggregate_diagnostic::expanded_aggregate_diagnostic_child";

struct Output<'a, 'work, 'stdout> {
    budget: &'a mut Budget<'work>,
    stdout: std::io::StdoutLock<'stdout>,
    bytes: usize,
    error: Option<SourceError>,
}

impl std::fmt::Write for Output<'_, '_, '_> {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        let result = (|| {
            let bytes = self
                .bytes
                .checked_add(text.len())
                .ok_or(Resource::Arithmetic)?;
            if bytes > OUTPUT_LIMIT {
                return Err(SourceError::Unsupported(
                    "aggregate diagnostic output bound",
                ));
            }
            self.budget.charge_work(text.len())?;
            std::io::Write::write_all(&mut self.stdout, text.as_bytes())
                .map_err(|_| SourceError::Unsupported("aggregate diagnostic output write"))?;
            self.bytes = bytes;
            Ok(())
        })();
        match result {
            Ok(()) => Ok(()),
            Err(error) => {
                self.error = Some(error);
                Err(std::fmt::Error)
            }
        }
    }
}

struct DiagnosticCallbacks {
    result: Option<Result<usize, String>>,
}

impl Callbacks for DiagnosticCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            let mut work = Work::new(500_000_000);
            let mut budget = Budget::new(&mut work, 20_000_000);
            let floor = budget.storage();
            let observed = transaction.with_original_source_expanded_v259(
                &mut budget,
                |source, original, tile, _, _, pair, budget| {
                    pair.check(source, original, tile, budget)?;
                    let floor = budget.storage();
                    let subject = pair.subject(budget)?;
                    let observe = |budget: &mut Budget<'_>| {
                        let semantic = source.source_semantic(budget)?;
                        let stdout = std::io::stdout();
                        let mut output = Output {
                            budget,
                            stdout: stdout.lock(),
                            bytes: 0,
                            error: None,
                        };
                        macro_rules! line {
                            ($($argument:tt)*) => {
                                writeln!(&mut output, $($argument)*)
                                    .map_err(|_| output.error.take().expect("bounded writer refusal"))?
                            };
                        }
                        line!("AGGREGATE_OWNER_V280 semantic={subject:?}");
                        for (ordinal, declaration) in semantic.types().iter().enumerate() {
                            output.budget.charge_work(1)?;
                            line!("AGGREGATE_TYPE_V280 type={ordinal} declaration={declaration:?}");
                        }
                        let mut count = 0usize;
                        for root in 0..source.root_count(output.budget)? {
                            let root_identity = source.root(root, output.budget)?;
                            line!("AGGREGATE_ROOT_V280 root={root} identity={root_identity:?}");
                            for instance in 0..source.instance_count(root, output.budget)? {
                                let (function, incoming) = source.instance(root, instance, output.budget)?;
                                let active = source.instance_active(root, instance, output.budget)?;
                                let declaration = &semantic.functions()[function.index() as usize];
                                line!("AGGREGATE_INSTANCE_V280 root={root} instance={instance} function={} incoming={incoming:?} active={active} identity={:?}", function.index(), declaration.identity());
                                if !active {
                                    continue;
                                }
                                for (block, row) in declaration.blocks().iter().enumerate() {
                                    output.budget.charge_work(1)?;
                                    for (statement, row) in row.statements().iter().enumerate() {
                                        output.budget.charge_work(1)?;
                                        let SemanticStatementKindV1::Assign(assignment) = row.kind() else {
                                            continue;
                                        };
                                        let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() else {
                                            continue;
                                        };
                                        count = count.checked_add(1).ok_or(Resource::Arithmetic)?;
                                        output.budget.charge_work(aggregate.operands().len())?;
                                        line!("AGGREGATE_SITE_V280 root={root} instance={instance} function={} block={block} statement={statement} result_type={} kind={:?} destination={:?} operands={:?}", function.index(), assignment.value().result_type().index(), aggregate.kind(), assignment.destination(), aggregate.operands());
                                    }
                                }
                            }
                        }
                        pair.check(source, original, tile, output.budget)?;
                        source.check_query_v18(output.budget)?;
                        let prior_bytes = output.bytes;
                        line!("AGGREGATE_COMPLETE_V280 count={count} prior_bytes={prior_bytes}");
                        Ok::<_, SourceError>(count)
                    };
                    let headers = [
                        std::mem::size_of_val(&observe)
                            .checked_mul(2)
                            .ok_or(Resource::Arithmetic)?,
                        std::mem::align_of_val(&observe),
                        std::mem::size_of::<Output<'_, '_, '_>>(),
                        std::mem::size_of::<std::io::Stdout>(),
                        std::mem::size_of::<std::fmt::Arguments<'_>>(),
                        std::mem::size_of_val(&subject),
                        std::mem::size_of::<Result<usize, SourceError>>(),
                        24 * std::mem::size_of::<usize>(),
                    ]
                    .into_iter()
                    .try_fold(0usize, |sum, bytes| sum.checked_add(bytes).ok_or(Resource::Arithmetic))?;
                    let count = budget.with_prepaid_scope(floor, 1, 1, headers, observe)?;
                    assert_eq!(budget.storage(), floor);
                    Ok((count, 0))
                },
            )
            .map_err(|error| format!("actual aggregate diagnostic: {error:?}"))?
            .into_observation();
            assert_eq!(budget.storage(), floor);
            Ok(observed)
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "diagnostic process helper; exact retained source supplied by parent"]
fn expanded_aggregate_diagnostic_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = DiagnosticCallbacks { result: None };
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("actual aggregate diagnostic callback");
    std::fs::write(
        env::var_os(RESULT).unwrap(),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "aggregate diagnostic: {result:?}");
}

#[test]
#[ignore = "diagnostic-only retained original aggregate census; no model execution"]
fn diagnostic_actual_expanded_aggregate_owners_without_model_execution_v280() {
    run_actual_sources::<usize>(
        &[("two", "two"), ("mixed", "mixed"), ("helper", "helper")],
        &[(0, 0), (3, 0)],
        CHILD,
        "ACTUAL_AGGREGATES_V280",
        |case| expanded_roots_tests::root_source_with_grid(case, Some(3)),
        |_, _, _, count, _| assert!(count > 0),
    );
}
