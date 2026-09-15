use super::*;
mod fixture {
    use fe2o3_kernel_ir::*;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fe2o3-kernel-ir/src/execution_capability_v1/borrowed_lds/fixture.rs"
    ));
}

#[test]
fn borrowed_lds_resource_counts_one_allocation_not_reusable_conversion() {
    let module = fixture::module(true, true);
    fe2o3_kernel_ir::verify_module(&module).unwrap();
    let mut shared = 0;
    let mut private = 0;
    for operation in fixture::operations(&module) {
        accumulate_static_resource_footprint(&operation.kind, &mut shared, &mut private).unwrap();
    }
    assert_eq!((shared, private), (256, 0));
    accumulate_static_resource_footprint(
        &fixture::operations(&module)[2].kind,
        &mut shared,
        &mut private,
    )
    .unwrap();
    assert_eq!((shared, private), (512, 0));
}

#[test]
fn borrowed_lds_target_requirements_match_owned_layout_without_barrier() {
    let module = fixture::module(false, false);
    let borrowed = &fixture::contract_at(&module, 2).operation;
    let ExecutionCapabilityOperationV1::LdsAllocateBorrowed {
        workgroup,
        lds,
        element,
        layout,
        elements,
        ..
    } = borrowed
    else {
        unreachable!()
    };
    let owned = ExecutionCapabilityOperationV1::LdsAllocate {
        workgroup: *workgroup,
        lds: *lds,
        element: *element,
        layout: *layout,
        elements: *elements,
    };
    let actual = target_requirements_for_execution_operation_v1(borrowed).unwrap();
    assert_eq!(
        actual,
        target_requirements_for_execution_operation_v1(&owned).unwrap()
    );
    assert!(actual.contains(&TargetCapabilityRequirementV1::Resource(
        TargetResourceRequirementV1::StaticSharedMemoryBytesAtMost(256)
    )));
    assert_eq!(
        borrowed.required_capabilities(),
        owned.required_capabilities()
    );
    assert_eq!(borrowed.memory_effects(), owned.memory_effects());
    assert!(!borrowed.transitions_epoch());
}
