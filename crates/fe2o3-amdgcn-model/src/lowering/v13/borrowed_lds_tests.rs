use super::*;
mod fixture {
    use fe2o3_kernel_ir::*;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fe2o3-kernel-ir/src/execution_capability_v1/borrowed_lds/fixture.rs"
    ));
}

#[test]
fn borrowed_lds_backend_dispatch_allocates_once_and_conversion_keeps_same_pointer() {
    let module = fixture::module(true, true);
    fe2o3_kernel_ir::verify_module(&module).unwrap();
    let original = value_types(&module.functions[0]);
    // Internal physical adapter test with an exact structural witness. This is
    // not an authority/receipt to bypass production infer_element_types.
    let elements = BTreeMap::from([(fixture::id(92), Type::Scalar(ScalarType::F32))]);
    let mut lowered = original.clone();
    let mut aliases = BTreeMap::new();
    let mut output = Vec::new();
    let mut helpers = Vec::new();
    let mut next = 100;
    let mut wave = None;
    for index in 1..=3 {
        let operation = &fixture::operations(&module)[index];
        let contract = fixture::contract_at(&module, index);
        lower_operation(
            &module,
            operation,
            contract,
            &original,
            &BTreeSet::new(),
            None,
            &elements,
            &mut lowered,
            &mut aliases,
            &mut next,
            &mut wave,
            &mut output,
            &mut helpers,
            0,
            fe2o3_kernel_ir::BlockId(0),
            index,
        )
        .unwrap();
    }
    assert!(helpers.is_empty());
    let [allocation] = output.as_slice() else {
        panic!("exactly one physical allocation");
    };
    assert_eq!(allocation.results[0].id, ValueId(90));
    assert!(
        matches!(&allocation.kind, OperationKind::WorkgroupMemory(memory)
        if memory.element == Type::Scalar(ScalarType::F32)
            && memory.extent == WorkgroupMemoryExtent::Static(64) && memory.alignment == 4)
    );
    assert!(!aliases.contains_key(&ValueId(90)));
    assert!(
        matches!(aliases.get(&ValueId(91)), Some(ExecutionAliasV1::Physical(p))
        if p.value == ValueId(90) && p.extent == Some(PhysicalExtentV1::Static(64)))
    );
}

#[test]
fn borrowed_lds_backend_retains_missing_structural_type_and_extent_limits() {
    for wide in [false, true] {
        let mut module = fixture::module(false, false);
        if wide {
            let ExecutionCapabilityOperationV1::LdsAllocateBorrowed { elements, .. } =
                &mut fixture::contract_mut(&mut module, 2).operation
            else {
                unreachable!()
            };
            *elements = u64::from(u32::MAX) + 1;
        }
        let original = value_types(&module.functions[0]);
        let elements = if wide {
            BTreeMap::from([(fixture::id(92), Type::Scalar(ScalarType::F32))])
        } else {
            BTreeMap::new()
        };
        let mut lowered = original.clone();
        let mut aliases = BTreeMap::new();
        let mut output = Vec::new();
        let error = lower_operation(
            &module,
            &fixture::operations(&module)[2],
            fixture::contract_at(&module, 2),
            &original,
            &BTreeSet::new(),
            None,
            &elements,
            &mut lowered,
            &mut aliases,
            &mut 100,
            &mut None,
            &mut output,
            &mut Vec::new(),
            0,
            fe2o3_kernel_ir::BlockId(0),
            2,
        );
        assert!(error.is_err());
        assert!(output.is_empty());
        assert!(!aliases.contains_key(&ValueId(90)));
    }
}
