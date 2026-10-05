#[test]
fn original_object_completion_gate_admits_only_modeled_tag_and_write_view_shapes() {
    use fe2o3_kernel_ir::StorageCopyOverlapV1;
    let access = MemoryAccess::new(AddressSpace::Private, 1);
    let address = ValueId(10);
    for operation in [
        ScopedObjectOperationV29::ReadValue { address, access },
        ScopedObjectOperationV29::WriteValue {
            address,
            value: ValueId(11),
            access,
        },
        ScopedObjectOperationV29::ReadDiscriminant { address, access },
        ScopedObjectOperationV29::SetDiscriminant {
            address,
            variant: 0,
            access,
        },
        ScopedObjectOperationV29::SetDiscriminant {
            address,
            variant: u32::MAX,
            access,
        },
        ScopedObjectOperationV29::Project {
            base: address,
            step: ScopedObjectProjectionV29::Field(0),
        },
        ScopedObjectOperationV29::Project {
            base: address,
            step: ScopedObjectProjectionV29::ArrayIndex(ValueId(11)),
        },
        ScopedObjectOperationV29::Project {
            base: address,
            step: ScopedObjectProjectionV29::VariantForWrite { index: 0 },
        },
    ] {
        assert!(
            scoped_object_completion_shape_v44(operation),
            "{operation:?}"
        );
    }
    for operation in [
        ScopedObjectOperationV29::Project {
            base: address,
            step: ScopedObjectProjectionV29::Variant { index: 0, access },
        },
        ScopedObjectOperationV29::CopyObject {
            source: address,
            destination: ValueId(11),
            source_access: access,
            destination_access: access,
            overlap: StorageCopyOverlapV1::MayOverlap,
        },
        ScopedObjectOperationV29::CopyObject {
            source: address,
            destination: ValueId(11),
            source_access: access,
            destination_access: access,
            overlap: StorageCopyOverlapV1::NonOverlapping,
        },
    ] {
        assert!(
            !scoped_object_completion_shape_v44(operation),
            "{operation:?}"
        );
    }
}

#[test]
fn original_object_completion_shape_does_not_grant_tag_census_or_lifetime_authority() {
    // The genuine owner must reach final completion, while deleting the tag or
    // substituting its original generation still fails the mandatory census.
    for fault in [0, 1, 4, 10] {
        run_tag_census_case_v43(original_tag_emission_owner, fault);
    }
}
