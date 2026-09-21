use crate::production_analysis::pliron_pipeline::invocation_receipt_v1::InvocationObserverV1;

type ProgressObserverV1<'o, 'p, 'r> = Option<&'o InvocationObserverV1<'p, 'r>>;

fn observed_progress_report_v1(
    finding: PlironProgressFindingV1,
    observer: ProgressObserverV1<'_, '_, '_>,
) -> PlironProgressReportV1 {
    if let (Some(observer), PlironProgressFindingV1::ResourceLimitExceeded { resource, .. }) =
        (observer, &finding)
    {
        observer.deny(ProductionAnalysisResourceLimitV1 {
            phase: ProductionAnalysisResourcePhaseV1::Progress,
            resource,
        });
    }
    report(finding)
}

pub(crate) struct ScopedProgressReportV1<'scope> {
    pub(crate) context: &'scope Context,
    pub(crate) function: &'scope FuncOp,
    pub(crate) report: PlironProgressReportV1,
}

#[cfg(test)]
pub(crate) fn run_pliron_progress_with_scoped_input_v1<'scope, const BARRIER: bool>(
    input: crate::production_analysis::pliron_pass_contract::ScopedVerifiedProgressInputV1<
        'scope,
        BARRIER,
    >,
) -> Result<
    ScopedProgressReportV1<'scope>,
    crate::production_analysis::pliron_pass_contract::PlironPassPreservationErrorV1,
> {
    run_pliron_progress_with_scoped_observation_v1(input, None)
}

pub(crate) fn run_pliron_progress_with_scoped_observation_v1<'scope, const BARRIER: bool>(
    input: crate::production_analysis::pliron_pass_contract::ScopedVerifiedProgressInputV1<
        'scope,
        BARRIER,
    >,
    observer: ProgressObserverV1<'_, '_, '_>,
) -> Result<
    ScopedProgressReportV1<'scope>,
    crate::production_analysis::pliron_pass_contract::PlironPassPreservationErrorV1,
> {
    let run = || {
        let (context, function) = input.into_endpoints()?;
        let report = run_scoped_progress_body_v1(context, function, observer);
        Ok(ScopedProgressReportV1 {
            context,
            function,
            report,
        })
    };
    match observer {
        None => run(),
        Some(observer) => observer.with_projection(&Ok, |_| run()),
    }
}

fn run_scoped_progress_body_v1(
    context: &Context,
    function: &FuncOp,
    observer: ProgressObserverV1<'_, '_, '_>,
) -> PlironProgressReportV1 {
    catch_progress_panic_v1(observer, || {
        let inventory = match bounded_structural_inventory(context, function) {
            Ok(inventory) => inventory,
            Err(finding) => return observed_progress_report_v1(finding, observer),
        };
        let mut work = ProgressWorkBudgetV1::default();
        if let Err(finding) = work.charge(inventory.structural_work().unwrap_or(usize::MAX)) {
            return observed_progress_report_v1(finding, observer);
        }
        run_pliron_progress_after_verification_with_observation_v1(
            context, inventory, work, observer,
        )
    })
}

fn catch_progress_panic_v1(
    observer: ProgressObserverV1<'_, '_, '_>,
    body: impl FnOnce() -> PlironProgressReportV1,
) -> PlironProgressReportV1 {
    let result = catch_unwind(AssertUnwindSafe(|| {
        // This guard must unwind before the existing catch converts the panic.
        match observer {
            None => body(),
            Some(observer) => observer.with_projection(&Ok, |_| body()),
        }
    }));
    match result {
        Ok(report) => report,
        Err(payload) => report(structural_rejection(format!(
            "bounded structural preflight panicked: {}",
            panic_detail(payload)
        ))),
    }
}

#[cfg(test)]
mod observed_progress_tests {
    use super::*;
    use crate::production_analysis::pliron_ir_identity::LivePlironStructuralIdentityProviderV1;
    use crate::production_analysis::pliron_pass_contract::{
        PRODUCTION_PLIRON_PASS_CONTRACTS_V1, begin_production_pliron_pass_contract_session_v1,
    };
    use crate::production_analysis::pliron_pipeline::invocation_receipt_v1::{
        InvocationReceiptFailureV1 as ReceiptFailure, InvocationReceiptV1 as Receipt,
    };
    use dialect_kernel::ReturnOp;
    use pliron::{builtin::types::FunctionType, dialect::DialectName};

    fn fixture(cycle: bool) -> (Context, FuncOp) {
        let mut context = Context::new();
        dialect_kernel::register_dialect(
            &mut context,
            &DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
        )
        .unwrap();
        let signature = FunctionType::get(&context, vec![], vec![]);
        let function = FuncOp::new(
            &mut context,
            "observed_progress".try_into().unwrap(),
            signature,
        );
        let entry = function.get_entry_block(&context);
        let terminator = if cycle {
            BranchOp::new(&mut context, entry).get_operation()
        } else {
            ReturnOp::new(&mut context).get_operation()
        };
        terminator.insert_at_back(entry, &context);
        (context, function)
    }

    #[test]
    fn authenticated_scoped_progress_keeps_clean_and_nonterminating_reports() {
        let hard = ProductionAnalysisResourceLimitsV1::production_hard_ceiling();
        for cycle in [false, true] {
            let (context, function) = fixture(cycle);
            let ordinary = run_pliron_progress_check_v1(&context, &function);
            let mut session = begin_production_pliron_pass_contract_session_v1(
                LivePlironStructuralIdentityProviderV1::new(&context, &function),
            )
            .unwrap();
            for contract in &PRODUCTION_PLIRON_PASS_CONTRACTS_V1[..8] {
                session
                    .run_contiguous_pass(contract.pass(), || Ok::<_, ()>(()))
                    .unwrap()
                    .unwrap();
            }
            let mut receipt = Receipt::new(Default::default(), hard).unwrap();
            let phase = receipt
                .phase(ProductionAnalysisResourcePhaseV1::Progress, 0)
                .unwrap();
            let observed = session
                .run_scoped_semantic_refinement_with_resource_limits_v1(hard, |input| {
                    let scoped = run_pliron_progress_with_scoped_observation_v1(
                        input,
                        Some(&phase.observer(&Ok)),
                    )?;
                    assert!(std::ptr::eq(scoped.context, &context));
                    assert_eq!(scoped.function.get_operation(), function.get_operation());
                    Ok(Ok::<_, ()>(scoped.report))
                })
                .unwrap()
                .unwrap();
            drop(phase);
            assert_eq!(observed, ordinary);
            assert_eq!(observed.is_clean(), !cycle);
            assert_eq!(receipt.complete(), Ok(Default::default()));
        }
    }

    #[test]
    fn real_inventory_denial_survives_success_and_caught_graph_borrow_panic() {
        let (mut large_context, large_function) = fixture(false);
        let entry = large_function.get_entry_block(&large_context);
        for _ in 0..MAX_PLIRON_PROGRESS_OPERATIONS_V1 {
            IndexConstantOp::new(&mut large_context, 0)
                .get_operation()
                .insert_at_front(entry, &large_context);
        }
        let ordinary = run_scoped_progress_body_v1(&large_context, &large_function, None);
        assert!(matches!(ordinary.findings(),
            [PlironProgressFindingV1::ResourceLimitExceeded { resource: "operations", actual, limit }]
            if *actual == MAX_PLIRON_PROGRESS_OPERATIONS_V1 + 1 && *limit == MAX_PLIRON_PROGRESS_OPERATIONS_V1));
        let (context, function) = fixture(false);
        for seed_denial in [false, true] {
            let mut receipt = Receipt::new(
                Default::default(),
                ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
            )
            .unwrap();
            let phase = receipt
                .phase(ProductionAnalysisResourcePhaseV1::Progress, 0)
                .unwrap();
            let observer = phase.observer(&Ok);
            if seed_denial {
                assert_eq!(
                    run_scoped_progress_body_v1(&large_context, &large_function, Some(&observer)),
                    ordinary,
                );
            }
            assert!(run_scoped_progress_body_v1(&context, &function, Some(&observer)).is_clean());
            let inventory = bounded_structural_inventory(&context, &function).unwrap();
            let mut work = ProgressWorkBudgetV1::default();
            work.charge(inventory.structural_work().unwrap()).unwrap();
            let borrowed = function.get_entry_block(&context).deref_mut(&context);
            let early = run_scoped_progress_body_v1(&context, &function, Some(&observer));
            assert!(matches!(early.findings(),
                [PlironProgressFindingV1::StructuralPrerequisiteRejected { reason }]
                if !reason.starts_with("bounded structural preflight panicked: ")));
            drop(borrowed);
            drop(phase);
            let error = ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::Progress,
                resource: "operations",
            };
            assert!(!receipt.snapshot().caught_panic);
            assert_eq!(
                receipt.snapshot().first_denial,
                seed_denial.then_some(error)
            );
            let phase = receipt
                .phase(ProductionAnalysisResourcePhaseV1::Progress, 0)
                .unwrap();
            let observer = phase.observer(&Ok);
            let borrowed = function.get_entry_block(&context).deref_mut(&context);
            // The real graph access follows a successful inventory, using the
            // same guard/catch boundary as the authenticated scoped body.
            let caught = catch_progress_panic_v1(Some(&observer), || {
                run_pliron_progress_after_verification_with_observation_v1(
                    &context,
                    inventory,
                    work,
                    Some(&observer),
                )
            });
            drop(borrowed);
            assert!(matches!(caught.findings(),
                [PlironProgressFindingV1::StructuralPrerequisiteRejected { reason }]
                if reason.starts_with("bounded structural preflight panicked: ")));
            drop(phase);
            assert_eq!(
                receipt.snapshot().first_denial,
                seed_denial.then_some(error)
            );
            assert!(receipt.snapshot().caught_panic);
            // Shared-body failure classification, not a forged scoped input or admission.
            assert_eq!(receipt.snapshot().committed, Default::default());
            assert_eq!(
                receipt.complete(),
                Err(if seed_denial {
                    ReceiptFailure::Denied(error)
                } else {
                    ReceiptFailure::CaughtPanic
                })
            );
        }
    }
}
