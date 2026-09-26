#[test]
fn final_descriptor_census_rejects_gep_data_extent_guard_and_receipt_substitution() {
    for fault in [
        DescriptorFault::Gep,
        DescriptorFault::Data,
        DescriptorFault::Extent,
        DescriptorFault::Guard,
        DescriptorFault::Missing,
    ] {
        let result =
            run_descriptor_module(DescriptorCase::READ, fault, MODULE_LIMIT, MODULE_LIMIT).0;
        assert!(
            DESCRIPTOR_TAMPERED.get() > 0,
            "{fault:?} must reach actual producer: {result:?}"
        );
        assert!(result.is_err(), "{fault:?}");
    }
}

#[test]
fn source_other_descriptor_stale_index_and_no_success_guard_are_not_bounds_proofs() {
    for case in [
        DescriptorCase {
            foreign_extent: true,
            ..DescriptorCase::READ
        },
        DescriptorCase {
            changed_index: true,
            ..DescriptorCase::READ
        },
        DescriptorCase {
            bypass: true,
            ..DescriptorCase::READ
        },
    ] {
        let result =
            run_descriptor_module(case, DescriptorFault::None, MODULE_LIMIT, MODULE_LIMIT).0;
        assert!(
            DESCRIPTOR_EMITTED.get() > 0,
            "{case:?} must reach actual emission: {result:?}"
        );
        assert!(result.is_err(), "{case:?}");
    }
}
#[test]
fn original_descriptor_rows_reject_same_shaped_source_and_ssa_substitutions() {
    run_descriptor_module(
        DescriptorCase::READ,
        DescriptorFault::SourceAudit,
        MODULE_LIMIT,
        MODULE_LIMIT,
    )
    .0
    .unwrap();
    assert!(DESCRIPTOR_EMITTED.get() > 0);
}
