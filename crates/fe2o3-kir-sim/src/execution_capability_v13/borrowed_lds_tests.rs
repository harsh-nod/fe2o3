use super::*;
mod fixture {
    use fe2o3_kernel_ir::*;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fe2o3-kernel-ir/src/execution_capability_v1/borrowed_lds/fixture.rs"
    ));
}

#[test]
fn borrowed_lds_sim_dispatch_preserves_single_allocation_and_uninitialized_transfer() {
    let module = fixture::module(true, true);
    fe2o3_kernel_ir::verify_module(&module).unwrap();
    let types = fixture::operations(&module)
        .iter()
        .flat_map(|op| op.results.iter())
        .map(|r| (r.id, r.ty.clone()))
        .collect();
    let scalars = BTreeMap::from([(fixture::id(92), ScalarType::F32)]);
    let mut aliases = Vec::new();
    let mut promoted = BTreeMap::new();
    let mut output = Vec::new();
    for index in 1..=3 {
        let op = &fixture::operations(&module)[index];
        output.extend(
            project_operation(
                op.results.clone(),
                fixture::contract_at(&module, index).clone(),
                &types,
                &[],
                &scalars,
                &mut aliases,
                &mut promoted,
                &mut BTreeMap::new(),
                &mut BTreeMap::new(),
                &mut subgroup_partition::Bindings::default(),
                &mut 100,
                Some(fe2o3_kernel_ir::WorkgroupSize::new(64, 1, 1)),
            )
            .unwrap(),
        );
    }
    assert_eq!(
        SimulationExecutionCapabilityFamilyV13::of(&fixture::contract_at(&module, 2).operation),
        SimulationExecutionCapabilityFamilyV13::LdsAllocateBorrowed
    );
    let [allocation] = output.as_slice() else {
        panic!("one physical allocation");
    };
    assert!(
        matches!(&allocation.kind, OperationKind::WorkgroupMemory(memory)
        if memory.element == Type::Scalar(ScalarType::F32)
            && memory.extent == WorkgroupMemoryExtent::Static(64) && memory.alignment == 4)
    );
    assert_eq!(allocation.results[0].id, ValueId(90));
    assert_eq!(aliases, [(ValueId(91), ValueId(90))]);
    assert_eq!(promoted.get(&ValueId(90)), promoted.get(&ValueId(91)));
    assert!(promoted.get(&ValueId(90)).is_some());
}

#[test]
fn borrowed_lds_sim_keeps_missing_scalar_witness_fail_closed() {
    let module = fixture::module(false, false);
    let op = &fixture::operations(&module)[2];
    let error = project_operation(
        op.results.clone(),
        fixture::contract_at(&module, 2).clone(),
        &BTreeMap::new(),
        &[],
        &BTreeMap::new(),
        &mut Vec::new(),
        &mut BTreeMap::new(),
        &mut BTreeMap::new(),
        &mut BTreeMap::new(),
        &mut subgroup_partition::Bindings::default(),
        &mut 100,
        Some(fe2o3_kernel_ir::WorkgroupSize::new(64, 1, 1)),
    );
    assert!(matches!(
        error,
        Err(ExecutionCapabilityProjectionErrorV13::Incomplete(
            IncompleteExecutionCapabilityOperationV13::LdsAllocate
        ))
    ));
}
