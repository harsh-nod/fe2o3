use super::inline_v29::{Shape, capture, fixture_input, fixture_owner};
use super::*;

#[test]
fn inline_region_query_has_independent_exact_and_one_short_work_without_allocation() {
    for shape in [
        Shape::Record,
        Shape::Array,
        Shape::Pair,
        Shape::Overaligned,
        Shape::Ignored,
        Shape::DirectEnum,
        Shape::NicheEnum,
    ] {
        for available in [44, 43, 31] {
            let owner = fixture_owner(shape, 1);
            let input = fixture_input(&owner);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
            budget.reserve_storage(FLOOR).unwrap();
            let profile = capture(&owner, &input, &mut budget);
            let floor = budget.storage();
            let peak = budget.peak_storage();
            // Discard setup headroom on this same ledger. The query oracle is
            // independently 32 indexed checks plus 12 packed-interval units.
            budget
                .charge_work(LIMIT - budget.work() - available)
                .unwrap();
            let before = budget.work();
            let result = profile.by_value_argument_v29(&owner, 0, 2, &mut budget);
            if available == 44 {
                assert!(result.unwrap().is_some());
                assert_eq!(budget.work() - before, 44);
            } else {
                assert!(matches!(
                    result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Work(_)
                        )
                    )
                ));
                assert_eq!(budget.work() - before, if available < 32 { 0 } else { 32 });
            }
            assert_eq!((budget.storage(), budget.peak_storage()), (floor, peak));
            let credit = profile.retained_storage();
            drop(profile);
            budget.release_storage(credit).unwrap();
            assert_eq!(budget.storage(), FLOOR);
        }
    }
}

#[test]
fn invalid_inline_query_stops_at_its_independent_checked_prefix() {
    for available in [32, 31] {
        let owner = fixture_owner(Shape::Record, 1);
        let input = fixture_input(&owner);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let profile = capture(&owner, &input, &mut budget);
        let floor = budget.storage();
        budget
            .charge_work(LIMIT - budget.work() - available)
            .unwrap();
        let before = budget.work();
        let result = profile.by_value_argument_v29(&owner, usize::MAX, 2, &mut budget);
        assert!(result.is_err());
        assert_eq!(budget.work() - before, if available == 32 { 32 } else { 0 });
        assert_eq!(budget.storage(), floor);
        let credit = profile.retained_storage();
        drop(profile);
        budget.release_storage(credit).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

pub(super) fn with_inline_view(
    shape: Shape,
    consume: impl FnOnce(
        &ProductionSourceOwnedViewV18<'_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSourceOwnedViewErrorV18>,
) -> std::thread::Result<Result<(), ProductionSourceOwnedViewErrorV18>> {
    with_inline_view_options(shape, true, false, consume)
}

fn with_inline_view_options(
    shape: Shape,
    profiled: bool,
    denied: bool,
    consume: impl FnOnce(
        &ProductionSourceOwnedViewV18<'_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSourceOwnedViewErrorV18>,
) -> std::thread::Result<Result<(), ProductionSourceOwnedViewErrorV18>> {
    let owner = fixture_owner(shape, 1);
    let input = fixture_input(&owner);
    let source_hash = *owner.source_semantic_sha256();
    let launch = crate::ProductionSourceLaunchRosterV1::try_new(
        owner.source_semantic(),
        &[crate::ProductionSourceLaunchRootInputV1::new(
            "inline",
            input.bindings[0],
            crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
        )],
    )
    .unwrap();
    let classes = [crate::ProductionScopeCallableCandidateV29::Ordinary];
    let source = crate::ProductionExecutionSourceInputV29 {
        semantic_sha256: &source_hash,
        roots: &[],
        classes: &classes,
        events: &[],
    };
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 100_000_000);
    budget.reserve_storage(FLOOR).unwrap();
    let prepared = if profiled {
        ProductionPendingScopedSourceOwnerV29::prepare_source_with_kernel_abi_budget_v18(
            owner,
            launch,
            source,
            ProductionKernelArgumentAbiInputV18 {
                roots: &input.roots(),
            },
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )
    } else {
        ProductionPendingScopedSourceOwnerV29::prepare_source_with_budget_v18(
            owner,
            launch,
            source,
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )
    }
    .unwrap();
    assert!(prepared.adopted_storage() > 0);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        prepared.with_checked_source_v18(&mut budget, consume)
    }));
    if denied {
        assert!(
            budget.storage() > FLOOR,
            "lost custody forbids containing refunds"
        );
        // The consuming scope and every fixture-owned payload have now dropped.
        // Test-owned ledger recovery is not a successful production refund.
        budget.release_storage(budget.storage() - FLOOR).unwrap();
    } else {
        assert_eq!(
            budget.storage(),
            FLOOR,
            "original source/profile/scratch credits settle once"
        );
    }
    result
}

#[test]
fn actual_source_owned_inline_query_preserves_scope_floor_and_repeated_identity() {
    for shape in [Shape::Record, Shape::Array, Shape::Pair] {
        with_inline_view(shape, |view, budget| {
            let floor = budget.storage();
            for _ in 0..3 {
                let before = budget.work();
                let region = view
                    .kernel_argument_by_value_abi_v29(0, 2, budget)?
                    .unwrap();
                assert_eq!(budget.work() - before, 45);
                assert_eq!(region.argument(), 2);
                assert!(matches!(region, ProductionKernelByValueAbiV29::Inline(_)));
                assert_eq!(budget.storage(), floor);
            }
            assert!(
                view.kernel_argument_by_value_abi_v29(0, 0, budget)?
                    .is_none()
            );
            Ok(())
        })
        .unwrap()
        .unwrap();
    }
}

#[test]
fn inline_query_original_error_is_sticky_and_callback_panic_releases_only_owned_credit() {
    let result = with_inline_view(Shape::Record, |view, budget| {
        assert!(matches!(
            view.kernel_argument_by_value_abi_v29(0, usize::MAX, budget),
            Err(ProductionSourceOwnedViewErrorV18::Binding(_))
        ));
        assert!(matches!(
            view.kernel_argument_by_value_abi_v29(0, 2, budget),
            Err(ProductionSourceOwnedViewErrorV18::Binding(_))
        ));
        Err(ProductionSourceOwnedViewErrorV18::Binding(
            "later callback sentinel",
        ))
    })
    .unwrap();
    assert!(
        matches!(result, Err(ProductionSourceOwnedViewErrorV18::Binding(message))
        if message != "later callback sentinel")
    );
    let result = with_inline_view(Shape::Record, |view, budget| {
        assert!(
            view.kernel_argument_by_value_abi_v29(0, 2, budget)?
                .is_some()
        );
        std::panic::panic_any(0x524930u64);
    });
    assert_eq!(
        result.unwrap_err().downcast_ref::<u64>(),
        Some(&0x524930u64)
    );
}

#[test]
fn missing_profile_never_becomes_an_empty_inline_region() {
    let result = with_inline_view_options(Shape::Record, false, false, |view, budget| {
        assert_eq!(view.kernel_argument_abi_count(0, budget)?, None);
        assert!(matches!(
            view.kernel_argument_by_value_abi_v29(0, 2, budget),
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "original source has no kernel argument ABI profile"
            ))
        ));
        Ok(())
    })
    .unwrap();
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Binding(
            "original source has no kernel argument ABI profile"
        ))
    ));
}

#[test]
fn inline_query_rejects_foreign_ledger_even_with_equal_live_storage() {
    let result = with_inline_view_options(Shape::Record, true, true, |view, budget| {
        let floor = budget.storage();
        let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, floor);
        foreign.reserve_storage(floor).unwrap();
        assert!(matches!(
            view.kernel_argument_by_value_abi_v29(0, 2, &mut foreign),
            Err(ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Accounting
            ))
        ));
        assert_eq!(foreign.storage(), floor);
        assert_eq!(budget.storage(), floor);
        assert!(view.kernel_argument_by_value_abi_v29(0, 2, budget).is_err());
        Err(ProductionSourceOwnedViewErrorV18::Binding(
            "later foreign-ledger sentinel",
        ))
    })
    .unwrap();
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Accounting
        ))
    ));
}
