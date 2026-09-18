use super::*;
use crate::rustc_semantic_plan_v1::ProductionSemanticConstructionWorkV1;
use rustc_hir::def_id::{DefId, DefIndex};
use rustc_middle::ty::GenericArgs;

fn work_limits(maximum: u64) -> SemanticMirLimitsV1 {
    SemanticMirLimitsV1::default()
        .with_limit(SemanticMirResourceV1::ValidationWork, maximum)
        .unwrap()
}

fn callable(index: u32) -> ProductionSemanticCallableOwnerEntryV1<'static> {
    // Owner setup only hashes these opaque keys; no rustc body or type query runs.
    let definition = DefId::local(DefIndex::from_u32(index.checked_add(1).unwrap()));
    ProductionSemanticCallableOwnerEntryV1::defined(
        Instance::new_raw(definition, GenericArgs::empty()),
        SemanticCallableIdV1::from_index(index),
    )
}

fn assert_limit<T>(
    result: Result<T, ProductionSemanticBodyErrorV1>,
    expected_resource: SemanticMirResourceV1,
    expected_actual: u64,
    expected_maximum: u64,
) {
    match result.err().expect("expected a construction limit error") {
        ProductionSemanticBodyErrorV1::LimitExceeded {
            resource,
            actual,
            maximum,
        } => {
            assert_eq!(resource, expected_resource);
            assert_eq!(actual, expected_actual);
            assert_eq!(maximum, expected_maximum);
        }
        error => panic!("expected a construction limit error, got {error:?}"),
    }
}

#[test]
fn inherited_work_and_owner_setup_accept_the_exact_limit() {
    let limits = work_limits(7)
        .with_limit(SemanticMirResourceV1::Types, 2)
        .unwrap()
        .with_limit(SemanticMirResourceV1::Callables, 2)
        .unwrap();
    let entries = [callable(0), callable(1)];
    let mut owner = ProductionSemanticBodyRequestOwnerV1::with_preflight_work(
        ProductionSemanticConstructionWorkV1::for_test(limits, 5),
        2,
        &entries,
    )
    .unwrap();
    assert_eq!(owner.limits, limits);
    assert_eq!(owner.callables.len(), 2);
    assert_eq!(
        owner.totals,
        ConstructionTotalsV1 {
            types: 2,
            callables: 2,
            validation_work: 7,
            ..ConstructionTotalsV1::default()
        },
    );
    owner
        .charge(SemanticMirResourceV1::ValidationWork, 0)
        .unwrap();
    assert_limit(
        owner.charge(SemanticMirResourceV1::ValidationWork, 1),
        SemanticMirResourceV1::ValidationWork,
        8,
        7,
    );
}

#[test]
fn inherited_work_and_owner_setup_reject_a_one_short_limit() {
    assert_limit(
        ProductionSemanticBodyRequestOwnerV1::with_preflight_work(
            ProductionSemanticConstructionWorkV1::for_test(work_limits(6), 5),
            2,
            &[callable(0), callable(1)],
        ),
        SemanticMirResourceV1::ValidationWork,
        7,
        6,
    );
}

#[test]
fn exhausted_work_rejects_the_first_callable_setup_charge() {
    let limits = work_limits(3);
    let mut empty = ProductionSemanticBodyRequestOwnerV1::with_preflight_work(
        ProductionSemanticConstructionWorkV1::for_test(limits, 3),
        0,
        &[],
    )
    .unwrap();
    assert_eq!(empty.totals.validation_work, 3);
    empty
        .charge(SemanticMirResourceV1::ValidationWork, 0)
        .unwrap();
    assert_limit(
        ProductionSemanticBodyRequestOwnerV1::with_preflight_work(
            ProductionSemanticConstructionWorkV1::for_test(limits, 3),
            0,
            &[callable(0)],
        ),
        SemanticMirResourceV1::ValidationWork,
        4,
        3,
    );
}

#[test]
fn over_limit_and_overflow_seeds_fail_before_structural_setup() {
    let limits = work_limits(3)
        .with_limit(SemanticMirResourceV1::Types, 0)
        .unwrap()
        .with_limit(SemanticMirResourceV1::Callables, 0)
        .unwrap();
    for used in [4, u64::MAX] {
        for type_count in [0, 1] {
            assert_limit(
                ProductionSemanticBodyRequestOwnerV1::with_preflight_work(
                    ProductionSemanticConstructionWorkV1::for_test(limits, used),
                    type_count,
                    &[callable(0)],
                ),
                SemanticMirResourceV1::ValidationWork,
                used,
                3,
            );
        }
    }
    assert_limit(
        ProductionSemanticBodyRequestOwnerV1::with_preflight_work(
            ProductionSemanticConstructionWorkV1::for_test(limits, 0),
            1,
            &[callable(0)],
        ),
        SemanticMirResourceV1::Types,
        1,
        0,
    );
    assert_limit(
        ProductionSemanticBodyRequestOwnerV1::with_preflight_work(
            ProductionSemanticConstructionWorkV1::for_test(limits, 0),
            0,
            &[callable(0)],
        ),
        SemanticMirResourceV1::Callables,
        1,
        0,
    );
}

#[test]
fn continuation_seeds_no_structural_counters_or_duplicate_charges() {
    let structural = [
        (SemanticMirResourceV1::Functions, 2),
        (SemanticMirResourceV1::Locals, 4),
        (SemanticMirResourceV1::Blocks, 3),
        (SemanticMirResourceV1::Statements, 6),
        (SemanticMirResourceV1::Projections, 1),
        (SemanticMirResourceV1::Operands, 5),
        (SemanticMirResourceV1::CallArguments, 3),
        (SemanticMirResourceV1::SwitchTargets, 1),
        (SemanticMirResourceV1::ConstantBytes, 8),
    ];
    let mut limits = work_limits(13)
        .with_limit(SemanticMirResourceV1::Types, 2)
        .unwrap()
        .with_limit(SemanticMirResourceV1::Callables, 2)
        .unwrap();
    for (resource, amount) in structural {
        limits = limits.with_limit(resource, amount).unwrap();
    }
    let mut owner = ProductionSemanticBodyRequestOwnerV1::with_preflight_work(
        ProductionSemanticConstructionWorkV1::for_test(limits, 11),
        2,
        &[callable(0), callable(1)],
    )
    .unwrap();
    assert_eq!(
        owner.totals,
        ConstructionTotalsV1 {
            types: 2,
            callables: 2,
            validation_work: 13,
            ..ConstructionTotalsV1::default()
        },
    );
    for (resource, amount) in structural {
        owner
            .charge(resource, usize::try_from(amount).unwrap())
            .unwrap();
    }
    assert_eq!(
        owner.totals,
        ConstructionTotalsV1 {
            types: 2,
            functions: 2,
            callables: 2,
            locals: 4,
            blocks: 3,
            statements: 6,
            projections: 1,
            operands: 5,
            call_arguments: 3,
            switch_targets: 1,
            constant_bytes: 8,
            validation_work: 13,
        },
    );
    assert_limit(
        owner.charge(SemanticMirResourceV1::Locals, 1),
        SemanticMirResourceV1::Locals,
        5,
        4,
    );
}

#[test]
fn continuation_work_accumulates_across_body_charge_batches() {
    let limits = work_limits(10)
        .with_limit(SemanticMirResourceV1::Functions, 2)
        .unwrap()
        .with_limit(SemanticMirResourceV1::Locals, 4)
        .unwrap();
    let mut owner = ProductionSemanticBodyRequestOwnerV1::with_preflight_work(
        ProductionSemanticConstructionWorkV1::for_test(limits, 3),
        0,
        &[callable(0), callable(1)],
    )
    .unwrap();
    assert_eq!(owner.totals.validation_work, 5);

    // Exercise the shared ledger across body-sized batches without constructing MIR.
    for (work, expected_total) in [(2, 7), (3, 10)] {
        owner.charge(SemanticMirResourceV1::Functions, 1).unwrap();
        owner.charge(SemanticMirResourceV1::Locals, 2).unwrap();
        owner
            .charge(SemanticMirResourceV1::ValidationWork, work)
            .unwrap();
        assert_eq!(owner.totals.validation_work, expected_total);
    }
    assert_eq!(owner.totals.functions, 2);
    assert_eq!(owner.totals.locals, 4);
    assert_limit(
        owner.charge(SemanticMirResourceV1::ValidationWork, 1),
        SemanticMirResourceV1::ValidationWork,
        11,
        10,
    );
}
