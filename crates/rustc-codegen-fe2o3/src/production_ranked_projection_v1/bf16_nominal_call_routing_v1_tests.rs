//! Isolated B1-B3 interface/resource controls. These do NOT qualify the genuine
//! nominal owner, sparse facts scope or combined whole-route accounting (B4).
use super::super::canonical_assertion_facts_v1::ProjectedAssertionConditionV1;
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::cell::Cell;
const LIMIT: usize = 1_000_000;
const FLOOR: usize = 23;

#[test]
fn nominal_marker_is_neither_empty_nor_scalar_and_cannot_join_raw_empty() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    with_summary_vector(4, 2, &mut budget, |summary, _| {
        for i in 0..4 {
            let function = SemanticFunctionIdV1::from_index(i);
            assert_eq!(summary.is_nominal_tensor_requires_call(function), i == 2);
            assert!(!summary.is_exact_empty(function));
            assert!(!summary.is_exact_empty_deterministic_scalar(function));
        }
        assert_eq!(
            summary
                .decisions
                .iter()
                .filter(|&&d| d == DefinedCallableEmptyEffectDecisionV1::NominalTensorRequiresCall)
                .count(),
            1
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.storage(), FLOOR);
    let mut decision = DefinedCallableEmptyEffectDecisionV1::NominalTensorRequiresCall;
    assert!(matches!(
        materialized_callable_effect_v1::join_raw_empty_summary_v1(&mut decision),
        Err(ProductionRankedProjectionErrorV1::Unsupported(_))
    ));
    assert_eq!(
        decision,
        DefinedCallableEmptyEffectDecisionV1::NominalTensorRequiresCall
    );
}

#[test]
fn existing_raw_empty_and_local_decisions_are_unchanged() {
    for original in [
        DefinedCallableEmptyEffectDecisionV1::ExactEmptyOnly,
        DefinedCallableEmptyEffectDecisionV1::ExactEmptyDeterministicScalar,
        DefinedCallableEmptyEffectDecisionV1::LocalMemoryRequiresCall,
    ] {
        let mut decision = original;
        materialized_callable_effect_v1::join_raw_empty_summary_v1(&mut decision).unwrap();
        assert_eq!(decision, original);
    }
    let mut rejected = DefinedCallableEmptyEffectDecisionV1::Rejected;
    materialized_callable_effect_v1::join_raw_empty_summary_v1(&mut rejected).unwrap();
    assert_eq!(
        rejected,
        DefinedCallableEmptyEffectDecisionV1::ExactEmptyOnly
    );
}

#[test]
fn summary_exact_and_one_short_storage_and_work_use_original_ledger() {
    let reserved = summary_storage::<()>(3).unwrap();
    for (work_limit, storage_limit, success) in [
        (11, FLOOR + reserved, true),
        (10, FLOOR + reserved, false),
        (11, FLOOR + reserved - 1, false),
    ] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let entered = Cell::new(false);
        let result = with_summary_vector(3, 1, &mut budget, |_, _| {
            entered.set(true);
            Ok(())
        });
        assert_eq!(result.is_ok(), success);
        assert_eq!(entered.get(), success);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.storage(), FLOOR);
        if success {
            assert_eq!(budget.work(), 11);
            assert_eq!(budget.peak_storage(), FLOOR + reserved);
        } else {
            assert!(matches!(
                result,
                Err(QueryError::Resource(
                    Resource::Work(_) | Resource::Storage(_)
                ))
            ));
            assert!(budget.failed_work().is_some() || budget.failed_storage().is_some());
        }
    }
}

#[test]
fn summary_callback_success_error_and_panic_keep_extra_live_charges() {
    for outcome in 0..3 {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = with_summary_vector(3, 1, &mut budget, |_, budget| {
            budget.reserve_storage(17)?;
            budget.charge_work(7)?;
            match outcome {
                0 => Ok(()),
                1 => Err(QueryError::Unavailable("synthetic callback refusal")),
                _ => panic!("synthetic summary callback panic"),
            }
        });
        match outcome {
            0 => assert_eq!(result, Ok(())),
            1 => assert_eq!(
                result,
                Err(QueryError::Unavailable("synthetic callback refusal"))
            ),
            _ => assert_eq!(result, Err(QueryError::CallbackPanicked)),
        }
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.storage(), FLOOR + 17);
        assert_eq!(budget.work(), 18);
    }
}

#[test]
fn summary_sticky_denial_cannot_be_ignored_by_callback() {
    for storage in [false, true] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let result = with_summary_vector(3, 1, &mut budget, |_, budget| {
            if storage {
                let _ = budget.reserve_storage(LIMIT + 1);
            } else {
                let _ = budget.charge_work(LIMIT + 1);
            }
            Ok(())
        });
        assert_eq!(result, Err(QueryError::Resource(Resource::Accounting)));
        assert_eq!(budget.storage(), FLOOR);
        assert!(budget.failed_work().is_some() || budget.failed_storage().is_some());
    }
}

#[test]
fn summary_callback_floor_debit_is_not_repaired_or_refunded_as_success() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let reserved = summary_storage::<()>(3).unwrap();
    let result = with_summary_vector(3, 1, &mut budget, |_, budget| {
        budget.release_storage(1)?;
        Ok(())
    });
    assert_eq!(result, Err(QueryError::Resource(Resource::Accounting)));
    assert_eq!(budget.storage(), FLOOR + reserved - 1);
}

#[test]
fn summary_shape_and_arithmetic_refuse_before_callback() {
    for (count, helper) in [
        (0, 0),
        (3, 3),
        (MAX_DEFINED_CALLABLE_SUMMARY_FUNCTIONS_V1 + 1, 0),
    ] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        assert!(matches!(
            with_summary_vector(
                count,
                helper,
                &mut budget,
                |_, _| -> Result<(), QueryError> { panic!("invalid summary must not call back") }
            ),
            Err(QueryError::Unavailable(_))
        ));
        assert_eq!(budget.storage(), 0);
        assert_eq!(budget.work(), 0);
    }
    assert_eq!(
        summary_storage::<()>(usize::MAX),
        Err(QueryError::Resource(Resource::Arithmetic))
    );
}

#[test]
fn query_resource_and_nonresource_errors_remain_distinct() {
    for error in [
        Resource::Allocation,
        Resource::Arithmetic,
        Resource::Accounting,
    ] {
        assert!(matches!(query_error(QueryError::Resource(error)),
            ProductionRankedProjectionErrorV1::CanonicalAssertions(
                CanonicalAssertionErrorV1::Resource(actual)) if actual == error));
    }
    for error in [
        QueryError::CallbackPanicked,
        QueryError::Unavailable("exact static reason"),
    ] {
        assert!(matches!(query_error(error),
            ProductionRankedProjectionErrorV1::CanonicalAssertions(
                CanonicalAssertionErrorV1::NominalCall(actual)) if actual == error));
    }
}

#[test]
fn pending_route_consumption_is_always_explicit_n2c_refusal() {
    assert!(
        matches!(require_defined_call_access_ready_v1(DefinedCallAccessRouteV1::NominalPending),
        Err(ProductionRankedProjectionErrorV1::Incomplete(detail)) if detail == NOMINAL_PENDING_V1)
    );
    assert!(require_defined_call_access_ready_v1(DefinedCallAccessRouteV1::ExistingRoute).is_ok());
}

struct NoOwner;
impl ProjectedAssertionFactsV1 for NoOwner {
    fn charge_private_array_work(
        &mut self,
        _: usize,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        panic!("no-owner routing must not charge unrelated query")
    }
    fn private_array_initializer_count(
        &mut self,
        _: usize,
        _: usize,
    ) -> Result<Option<u64>, ProductionRankedProjectionErrorV1> {
        unreachable!()
    }
    fn is_materialized_block(
        &mut self,
        _: usize,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        unreachable!()
    }
    fn condition(
        &mut self,
        _: usize,
        _: bool,
        _: SemanticBlockIdV1,
    ) -> Result<ProjectedAssertionConditionV1, ProductionRankedProjectionErrorV1> {
        unreachable!()
    }
}
fn source_call() -> SemanticDirectCallV1 {
    SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1::from_index(0),
        vec![],
        None,
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap()
}
#[test]
fn default_and_absent_facts_refuse_before_visiting_a_candidate() {
    let call = source_call();
    let mut visit =
        |_: &CheckedNominalCallProjectionV1<'_>, _: &mut Budget<'_>| -> Result<(), QueryError> {
            panic!("no owner cannot fabricate a candidate")
        };
    let mut none = ProjectedViewsV1::new(0, None);
    assert!(matches!(
        none.with_nominal_call_v1(
            0,
            &call,
            SemanticSourceProvenanceV1::unavailable(),
            &mut visit
        ),
        Err(ProductionRankedProjectionErrorV1::Incomplete(_))
    ));
    let mut facts = NoOwner;
    let mut view = ProjectedViewsV1::new(0, Some(&mut facts));
    assert!(matches!(
        view.with_nominal_call_v1(
            0,
            &call,
            SemanticSourceProvenanceV1::unavailable(),
            &mut visit
        ),
        Err(ProductionRankedProjectionErrorV1::Incomplete(_))
    ));
}

struct RefusingFacts {
    nominal: usize,
    local: usize,
}
impl ProjectedAssertionFactsV1 for RefusingFacts {
    fn with_nominal_call_v1(
        &mut self,
        _: usize,
        _: &SemanticDirectCallV1,
        _: SemanticSourceProvenanceV1,
        _: &mut NominalCallVisitorV1<'_>,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        self.nominal += 1;
        Err(ProductionRankedProjectionErrorV1::Incomplete(
            "synthetic facts refusal",
        ))
    }
    fn require_unit_local_call(
        &mut self,
        _: usize,
        _: &SemanticDirectCallV1,
        _: SemanticSourceProvenanceV1,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        self.local += 1;
        Err(ProductionRankedProjectionErrorV1::Incomplete(
            "synthetic local refusal",
        ))
    }
    fn charge_private_array_work(
        &mut self,
        _: usize,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        unreachable!()
    }
    fn private_array_initializer_count(
        &mut self,
        _: usize,
        _: usize,
    ) -> Result<Option<u64>, ProductionRankedProjectionErrorV1> {
        unreachable!()
    }
    fn is_materialized_block(
        &mut self,
        _: usize,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        unreachable!()
    }
    fn condition(
        &mut self,
        _: usize,
        _: bool,
        _: SemanticBlockIdV1,
    ) -> Result<ProjectedAssertionConditionV1, ProductionRankedProjectionErrorV1> {
        unreachable!()
    }
}

#[test]
fn actual_dispatch_helper_forwards_nominal_once_and_never_falls_back_to_unit_local() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let call = source_call();
    let function = SemanticFunctionIdV1::from_index(0);
    let callables = [SemanticCallableDeclV1::Defined { function }];
    let mut facts = RefusingFacts {
        nominal: 0,
        local: 0,
    };
    with_summary_vector(1, 0, &mut budget, |summary, _| {
        let mut views = ProjectedViewsV1::new(0, Some(&mut facts));
        assert!(matches!(
            resolve_defined_call_access_route_v1(
                &callables,
                summary,
                function,
                0,
                &call,
                SemanticSourceProvenanceV1::unavailable(),
                &mut views
            ),
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "synthetic facts refusal"
            ))
        ));
        Ok(())
    })
    .unwrap();
    assert_eq!(facts.nominal, 1);
    assert_eq!(facts.local, 0);
}

#[test]
fn wrong_declared_callee_refuses_before_any_facts_query() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let call = source_call();
    let callables = [SemanticCallableDeclV1::Defined {
        function: SemanticFunctionIdV1::from_index(0),
    }];
    let mut facts = RefusingFacts {
        nominal: 0,
        local: 0,
    };
    with_summary_vector(2, 1, &mut budget, |summary, _| {
        let mut views = ProjectedViewsV1::new(0, Some(&mut facts));
        assert!(
            resolve_defined_call_access_route_v1(
                &callables,
                summary,
                SemanticFunctionIdV1::from_index(1),
                0,
                &call,
                SemanticSourceProvenanceV1::unavailable(),
                &mut views
            )
            .is_err()
        );
        Ok(())
    })
    .unwrap();
    assert_eq!((facts.nominal, facts.local), (0, 0));
}
