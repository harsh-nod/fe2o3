use super::*;

pub(super) fn assert_source_output_refused(source: ProductionPreRankedKirOwnerV1) {
    assert_eq!(
        source.helper_source_policy_v1(),
        ProductionHelperSourcePolicyV1::UnitLocal
    );
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let source_storage = retained(&source);
    budget.reserve_storage(source_storage).unwrap();
    let checked = optimize(source.executable(), &mut budget);
    let (coordinates, coordinate_storage) = check_canonical_kir_coordinate_preservation_v1(
        source.executable(),
        source.executable(),
        &mut budget,
    )
    .unwrap();
    budget
        .reserve_storage(coordinate_storage.retained_storage())
        .unwrap();
    let incoming = budget.storage();
    assert!(matches!(
        derive_source_output_occurrences_v1(&source, &coordinates, &checked, &mut budget),
        Err(ProductionSourceOutputErrorV1::SourceReplay(
            ProductionSemanticKirErrorV1::LocalHelperSourceConsumerUnavailable {
                consumer: "source/output replay",
            }
        ))
    ));
    assert_eq!(budget.storage(), incoming);
    #[allow(
        clippy::drop_non_drop,
        reason = "End witness custody before releasing its reservation"
    )]
    drop(coordinates);
    budget
        .release_storage(coordinate_storage.retained_storage())
        .unwrap();
    let checked_storage = checked.storage().retained_storage();
    drop(checked);
    budget.release_storage(checked_storage).unwrap();
    drop(source);
    budget.release_storage(source_storage).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn source_output_refuses_a_local_helper_reachable_only_under_the_second_root() {
    let source = super::super::local_helper_source_v1_tests::unit_owner(
        super::super::local_helper_source_v1_tests::UnitCase::Initializer,
        &[0, 2],
    );
    assert_eq!(source.empty_effect_helpers().iter().count(), 0);
    assert_eq!(source.helper_memory.unit_source.associations.len(), 1);
    assert_eq!(
        source.helper_memory.unit_source.associations[0]
            .key
            .root
            .index(),
        1
    );
    assert_source_output_refused(source);
}
