use super::*;
use std::error::Error as _;

#[test]
fn mixed_worklist_policy_keeps_typed_refusal_and_real_output_capture_headers() {
    use fe2o3_lower_mir_kernel::ProductionMixedSourceCheckErrorV26 as Check;
    let error = Error::from(ProductionMixedSourceHandoffErrorV26::Check(Check::Source(
        ProductionSourceOwnedViewErrorV18::Resource(Resource::Accounting),
    )));
    assert!(
        error
            .source()
            .unwrap()
            .is::<ProductionMixedSourceHandoffErrorV26>()
    );
    assert!(matches!(
        error,
        Error::ConditionalMixedHandoff(ProductionMixedSourceHandoffErrorV26::Check(Check::Source(
            ProductionSourceOwnedViewErrorV18::Resource(Resource::Accounting)
        )))
    ));
    type Consumer = for<'view, 'source, 'abi, 'work> fn(
        &'view Source<'source>,
        &MixedHandoff<'view, 'source>,
        &[AbiRoot<'abi>],
        TargetProfile,
        &mut Budget<'work>,
    ) -> Result<(), Error>;
    let expected = entry_headers_for_handoff::<(), Consumer, MixedHandoff<'static, 'static>>()
        .unwrap()
        + size_of::<formal_context_v19::PendingConsumerV19<Consumer>>()
        + align_of::<formal_context_v19::PendingConsumerV19<Consumer>>()
        + formal_context_v19::launch_context_headers_v19().unwrap()
        + size_of::<Vec<ExplicitLaunchExtent>>()
        + align_of::<Vec<ExplicitLaunchExtent>>()
        + size_of::<fe2o3_kernel_ir::FormalIndexWidth>()
        + size_of::<ProductionMixedSourceHandoffErrorV26>();
    assert_eq!(
        <ConditionalMixedWorklist as SourceHandoffPolicyV29<(), Consumer>>::entry_headers()
            .unwrap(),
        expected
    );
}

#[test]
fn mixed_worklist_launch_projection_preserves_all_roots_and_refuses_unknown_or_exact_tags() {
    use CanonicalFormalLaunchInputV19 as Launch;
    let first = ExplicitLaunchExtent::Exact {
        rank: 1,
        extents: [192, 1, 1],
    };
    let second = ExplicitLaunchExtent::Exact {
        rank: 2,
        extents: [160, 9, 1],
    };
    let mut work = Work::new(10_000);
    let mut budget = Budget::new(&mut work, 10_000);
    assert_eq!(
        explicit_mixed_launches_v26(
            &[
                Launch::PhysicalEnvelope(first),
                Launch::PhysicalEnvelope(second)
            ],
            &mut budget,
        )
        .unwrap(),
        [first, second]
    );
    for refused in [
        Launch::Exact(first),
        Launch::Exact(ExplicitLaunchExtent::Unknown),
        Launch::PhysicalEnvelope(ExplicitLaunchExtent::Unknown),
    ] {
        assert!(matches!(
            explicit_mixed_launches_v26(&[refused], &mut budget),
            Err(Error::Unsupported(
                "mixed source requires exact physical launch bounds"
            ))
        ));
    }
}

#[test]
fn mixed_worklist_launch_projection_has_exact_and_one_short_cumulative_resources() {
    const FLOOR: usize = 17;
    let input = [CanonicalFormalLaunchInputV19::PhysicalEnvelope(ExplicitLaunchExtent::Exact {
        rank: 1,
        extents: [192, 1, 1],
    }); 3];
    let run = |work_limit, storage_limit| {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        budget.charge_work(5).unwrap();
        let result = explicit_mixed_launches_v26(&input, &mut budget);
        (result, budget.work(), budget.peak_storage())
    };
    let (result, work, storage) = run(10_000, 10_000);
    assert_eq!(result.unwrap().len(), input.len());
    assert!(work > 5 && storage > FLOOR);
    let (result, exact_work, exact_storage) = run(work, storage);
    assert_eq!(result.unwrap().len(), input.len());
    assert_eq!((exact_work, exact_storage), (work, storage));
    assert!(matches!(
        run(work - 1, storage).0,
        Err(Error::Resource(Resource::Work(_)))
    ));
    assert!(matches!(
        run(work, storage - 1).0,
        Err(Error::Resource(Resource::Storage(_)))
    ));
    let mut denied_work = Work::new(0);
    let mut denied = Budget::new(&mut denied_work, 10_000);
    let first = denied.charge_work(1).unwrap_err();
    assert!(matches!(
        explicit_mixed_launches_v26(&[], &mut denied),
        Err(Error::Resource(error)) if error == first
    ));
    assert_eq!(denied.storage(), 0);
}
