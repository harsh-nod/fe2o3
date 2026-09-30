use super::*;
use crate::{
    FormalPathExclusionDecisionV19 as Exclusion, FormalPathExclusionErrorV19 as ExclusionError,
    FormalPathExclusionsV19, PresburgerQueryErrorV2, PresburgerQueryLimitsV2,
    PresburgerQueryResourceV2, with_presburger_queries_v2,
};
use std::cell::Cell;

fn fresh<'owner>(
    owner: &'owner VerifiedCanonicalKernelIrModuleV18,
    extent: ExplicitLaunchExtent,
    width: FormalIndexWidth,
) -> fe2o3_kernel_ir::CanonicalOwnerFormalAnalysisV18<'owner> {
    CanonicalOwnerFormalScopeV18::new(owner, Default::default())
        .unwrap()
        .derive(&KernelId::new("root"), extent, width)
        .unwrap()
}

fn exclude<'owner>(
    owner: &'owner VerifiedCanonicalKernelIrModuleV18,
    extent: ExplicitLaunchExtent,
) -> std::result::Result<FormalPathExclusionsV19<'owner>, ExclusionError> {
    let formal = fresh(owner, extent, FormalIndexWidth::Bits64);
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 100_000);
    budget.reserve_storage(37).unwrap();
    let result = with_presburger_queries_v2(Default::default(), &mut budget, |queries, budget| {
        Ok(FormalPathExclusionsV19::from_formal(
            formal,
            Default::default(),
            queries,
            budget,
        ))
    })
    .unwrap();
    assert_eq!(budget.storage(), 37);
    result
}

fn predicate(module: &mut Module, comparison: ComparePredicate) {
    let OperationKind::Compare { predicate, .. } =
        &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[2].kind
    else {
        panic!("fixture comparison");
    };
    *predicate = comparison;
}

#[test]
fn full_coordinate_singleton_uses_actual_owner_and_retains_the_entire_report() {
    with_owner(&fixture(1), |owner| {
        for extent in [2, 4, 64] {
            let result = exclude(owner, launch(extent)).unwrap();
            assert!(result.belongs_to(owner));
            assert!(std::ptr::eq(result.owner(), owner));
            assert_eq!(result.formal().launch(), launch(extent));
            assert_eq!(
                result.decisions(),
                &[Exclusion::ExcludedForAllU64Coordinates]
            );
            assert_eq!(result.queries(), 2);
            assert!(result.construction_steps() > 0);
            assert_eq!(
                result.formal().analysis(),
                fresh(owner, launch(extent), FormalIndexWidth::Bits64).analysis()
            );
        }
    });
}

#[test]
fn full_coordinate_proof_does_not_promote_an_empty_two_invocation_witness() {
    let mut module = fixture(2);
    predicate(&mut module, ComparePredicate::GreaterThanOrEqual);
    with_owner(&module, |owner| {
        let bounded_two = derive(owner, launch(2), FormalIndexWidth::Bits64);
        assert_eq!(bounded_two.decisions(), &[Decision::Disjoint]);
        let bounded_four = derive(owner, launch(4), FormalIndexWidth::Bits64);
        assert!(matches!(
            bounded_four.decisions(),
            [Decision::PossibleOverlap { .. }]
        ));
        for extent in [2, 4] {
            let result = exclude(owner, launch(extent)).unwrap();
            assert_eq!(result.decisions(), &[Exclusion::NotProved]);
            assert_eq!(result.queries(), 2);
        }
    });
}

#[test]
fn full_coordinate_domain_includes_u64_max_without_an_exclusive_endpoint_loss() {
    for (bound, expected) in [
        (u64::MAX, Exclusion::ExcludedForAllU64Coordinates),
        (u64::MAX - 1, Exclusion::NotProved),
    ] {
        let mut module = fixture(bound);
        predicate(&mut module, ComparePredicate::GreaterThanOrEqual);
        with_owner(&module, |owner| {
            let result = exclude(owner, launch(2)).unwrap();
            assert_eq!(result.decisions(), &[expected]);
            assert_eq!(result.queries(), 2);
        });
    }
}

#[test]
fn full_coordinate_x_singleton_does_not_assert_multidimensional_uniqueness() {
    for (domain, rank, extents) in [
        (
            LaunchDomain::D2 {
                x: LaunchExtent::Dynamic,
                y: LaunchExtent::Dynamic,
            },
            2,
            [4, 2, 1],
        ),
        (
            LaunchDomain::D3 {
                x: LaunchExtent::Dynamic,
                y: LaunchExtent::Dynamic,
                z: LaunchExtent::Dynamic,
            },
            3,
            [4, 1, 2],
        ),
    ] {
        let mut module = fixture(1);
        module.kernels[0].domain = domain;
        with_owner(&module, |owner| {
            assert!(matches!(
                exclude(owner, ExplicitLaunchExtent::Exact { rank, extents }),
                Err(ExclusionError::UnsupportedCoordinateDomain)
            ));
        });
    }
}

#[test]
fn full_coordinate_widening_does_not_reuse_small_witness_nonoverflow_facts() {
    let mut module = fixture(2);
    let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
    operations.insert(
        2,
        value(8, Type::INDEX, OperationKind::Constant(Constant::Index(1))),
    );
    operations.insert(
        3,
        value(
            9,
            Type::INDEX,
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(2),
                rhs: ValueId(8),
            },
        ),
    );
    let OperationKind::Compare { lhs, .. } = &mut operations[4].kind else {
        unreachable!()
    };
    *lhs = ValueId(9);
    with_owner(&module, |owner| {
        // The bounded domain sees x+1<2 only at x=0. Over U64, wrapping at
        // MAX makes two coordinates satisfy it; V19 must not reuse that fit.
        assert_eq!(
            derive(owner, launch(2), FormalIndexWidth::Bits64).decisions(),
            &[Decision::Disjoint]
        );
        assert_eq!(
            exclude(owner, launch(2)).unwrap().decisions(),
            &[Exclusion::NotProved]
        );
    });
}

#[test]
fn full_coordinate_both_invocation_orders_and_all_conflict_ordinals_are_checked() {
    let mut module = fixture(1);
    let body = module.functions[0].body.as_mut().unwrap();
    let store = body.blocks[1].operations[0].clone();
    body.blocks[2].operations.push(store);
    with_owner(&module, |owner| {
        let result = exclude(owner, launch(4)).unwrap();
        let report = result.formal().analysis();
        assert_eq!(report.obligations().inter_invocation_conflicts().len(), 3);
        assert_eq!(result.decisions().len(), 3);
        assert_eq!(result.queries(), 6);
        assert_eq!(
            result
                .decisions()
                .iter()
                .filter(|row| **row == Exclusion::ExcludedForAllU64Coordinates)
                .count(),
            1
        );
        assert_eq!(
            result
                .decisions()
                .iter()
                .filter(|row| **row == Exclusion::NotProved)
                .count(),
            2
        );
        assert_eq!(
            report,
            fresh(owner, launch(4), FormalIndexWidth::Bits64).analysis()
        );
    });
}

#[test]
fn full_coordinate_per_conflict_exclusion_does_not_complete_an_incomplete_report() {
    let mut module = fixture(1);
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .insert(
            0,
            Operation::new(
                vec![],
                OperationKind::Call {
                    callee: "external".into(),
                    arguments: vec![],
                },
            ),
        );
    module.functions.push(Function::external_import(
        "external",
        Signature::new(vec![], vec![]),
    ));
    with_owner(&module, |owner| {
        let result = exclude(owner, launch(64)).unwrap();
        assert!(!result.formal().analysis().is_complete());
        assert_eq!(result.formal().analysis().incomplete_reasons().len(), 1);
        assert_eq!(
            result.decisions(),
            &[Exclusion::ExcludedForAllU64Coordinates]
        );
        assert_eq!(
            result.formal().analysis(),
            fresh(owner, launch(64), FormalIndexWidth::Bits64).analysis()
        );
    });
}

#[test]
fn full_coordinate_equal_bytes_and_changed_source_do_not_transfer_owner_identity() {
    with_owner(&fixture(1), |owner| {
        let result = exclude(owner, launch(4)).unwrap();
        with_owner(&fixture(1), |equal| {
            assert_eq!(owner.module(), equal.module());
            assert!(!result.belongs_to(equal));
            assert!(!exclude(equal, launch(4)).unwrap().belongs_to(owner));
        });
        with_owner(&fixture(2), |changed| {
            assert!(!result.belongs_to(changed));
            assert_eq!(
                exclude(changed, launch(4)).unwrap().decisions(),
                &[Exclusion::NotProved]
            );
        });
    });
}

#[test]
fn full_coordinate_unknown_or_32_bit_index_never_claims_a_u64_domain() {
    with_owner(&fixture(1), |owner| {
        for width in [FormalIndexWidth::Unknown, FormalIndexWidth::Bits32] {
            let formal = fresh(owner, launch(4), width);
            let mut work = Work::new(1_000_000);
            let mut budget = Budget::new(&mut work, 100_000);
            let result =
                with_presburger_queries_v2(Default::default(), &mut budget, |queries, budget| {
                    Ok(FormalPathExclusionsV19::from_formal(
                        formal,
                        Default::default(),
                        queries,
                        budget,
                    ))
                })
                .unwrap();
            assert!(matches!(
                result,
                Err(ExclusionError::UnsupportedCoordinateDomain)
            ));
            assert_eq!(budget.work(), 4);
            assert_eq!(budget.storage(), 0);
        }
    });
}

// Independent equation for this fixture: session4; each order has entry4,
// validation4+3*3, rank2 census4, two bounds copies12, sweep1, two narrowing
// rows at 2+4+2*8+2*12, then contradiction at 2+4+2*8. Total 4+2*148.
const SINGLETON_WORK: usize = 300;

#[test]
fn full_coordinate_queries_consume_exact_cumulative_work_and_refuse_one_short() {
    with_owner(&fixture(1), |owner| {
        for limit in [SINGLETON_WORK + 11, SINGLETON_WORK + 10] {
            let formal = fresh(owner, launch(2), FormalIndexWidth::Bits64);
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, 100_000);
            budget.charge_work(11).unwrap();
            budget.reserve_storage(37).unwrap();
            let result =
                with_presburger_queries_v2(Default::default(), &mut budget, |queries, budget| {
                    Ok(FormalPathExclusionsV19::from_formal(
                        formal,
                        Default::default(),
                        queries,
                        budget,
                    ))
                });
            if limit == SINGLETON_WORK + 11 {
                assert_eq!(
                    result.unwrap().unwrap().decisions(),
                    &[Exclusion::ExcludedForAllU64Coordinates]
                );
                assert_eq!(budget.work(), limit);
            } else {
                assert!(matches!(result, Err(PresburgerQueryErrorV2::Resource(_))));
                assert_eq!(budget.failed_work(), Some(SINGLETON_WORK + 11));
            }
            assert_eq!(budget.storage(), 37);
        }
    });
}

#[test]
fn full_coordinate_second_order_query_refusal_is_sticky_and_cannot_be_swallowed() {
    with_owner(&fixture(1), |owner| {
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 100_000);
        budget.reserve_storage(37).unwrap();
        let completed = Cell::new(false);
        let result = with_presburger_queries_v2(
            PresburgerQueryLimitsV2 {
                queries: 1,
                ..Default::default()
            },
            &mut budget,
            |queries, budget| {
                let first = FormalPathExclusionsV19::from_formal(
                    fresh(owner, launch(2), FormalIndexWidth::Bits64),
                    Default::default(),
                    queries,
                    budget,
                )
                .unwrap_err();
                let ExclusionError::Query(first) = first else {
                    panic!("query limit must win")
                };
                assert!(matches!(
                    first,
                    PresburgerQueryErrorV2::Limit {
                        resource: PresburgerQueryResourceV2::Queries,
                        actual: 2,
                        limit: 1
                    }
                ));
                let before = budget.work();
                let retry = FormalPathExclusionsV19::from_formal(
                    fresh(owner, launch(2), FormalIndexWidth::Bits64),
                    Default::default(),
                    queries,
                    budget,
                )
                .unwrap_err();
                assert!(matches!(retry, ExclusionError::Query(ref error) if error == &first));
                assert_eq!(budget.work(), before);
                completed.set(true);
                Ok(())
            },
        );
        assert!(completed.get());
        assert!(matches!(
            result,
            Err(PresburgerQueryErrorV2::Limit {
                resource: PresburgerQueryResourceV2::Queries,
                actual: 2,
                limit: 1
            })
        ));
        assert_eq!(budget.storage(), 37);
    });
}

#[test]
fn full_coordinate_fixed_path_caps_still_refuse_before_any_query() {
    with_owner(&fixture(1), |owner| {
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 100_000);
        let result =
            with_presburger_queries_v2(Default::default(), &mut budget, |queries, budget| {
                Ok(FormalPathExclusionsV19::from_formal(
                    fresh(owner, launch(2), FormalIndexWidth::Bits64),
                    Limits {
                        conflicts: 0,
                        ..Default::default()
                    },
                    queries,
                    budget,
                ))
            })
            .unwrap();
        assert!(matches!(
            result,
            Err(ExclusionError::Path(Error::Limit {
                resource: Resource::Conflicts,
                actual: 1,
                limit: 0
            }))
        ));
        assert_eq!(budget.work(), 4);
        assert_eq!(budget.storage(), 0);
    });
}

#[test]
fn full_coordinate_foreign_query_ledger_refuses_without_touching_its_credit() {
    with_owner(&fixture(1), |owner| {
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 100_000);
        budget.reserve_storage(37).unwrap();
        let completed = Cell::new(false);
        let result = with_presburger_queries_v2(Default::default(), &mut budget, |queries, _| {
            let mut other_work = Work::new(1_000_000);
            let mut other = Budget::new(&mut other_work, 100_000);
            other.charge_work(19).unwrap();
            other.reserve_storage(41).unwrap();
            let result = FormalPathExclusionsV19::from_formal(
                fresh(owner, launch(2), FormalIndexWidth::Bits64),
                Default::default(),
                queries,
                &mut other,
            );
            assert!(matches!(
                result,
                Err(ExclusionError::Query(PresburgerQueryErrorV2::Resource(
                    fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting
                )))
            ));
            assert_eq!((other.work(), other.storage()), (19, 41));
            completed.set(true);
            Ok(())
        });
        assert!(completed.get());
        assert!(matches!(
            result,
            Err(PresburgerQueryErrorV2::Resource(
                fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting
            ))
        ));
        assert_eq!(budget.work(), 4);
        assert_eq!(budget.storage(), 37);
    });
}
