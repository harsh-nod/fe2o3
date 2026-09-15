use super::*;
mod fixture {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fe2o3-kernel-ir/src/execution_capability_v1/reusable_lds/physical_fixture.rs"
    ));
}

#[test]
fn reusable_lds_backend_transfers_existing_pointer_extent_once() {
    let (conversion, contract, input, result, pointer) = fixture::fixture();
    let operation = Operation::new(
        vec![result.clone()],
        OperationKind::ExecutionCapability(contract.clone()),
    );
    let physical = PhysicalValueV1 {
        value: ValueId(20),
        extent: Some(PhysicalExtentV1::Static(64)),
    };
    let mut aliases = BTreeMap::from([(ValueId(1), ExecutionAliasV1::Physical(physical))]);
    let mut lowered = BTreeMap::from([(physical.value, pointer.clone())]);
    let original = BTreeMap::from([(ValueId(1), input)]);
    let module = Module::new("reusable");
    lower(
        &module,
        &operation,
        &contract,
        conversion,
        &original,
        &mut lowered,
        &mut aliases,
    )
    .unwrap();
    assert!(!aliases.contains_key(&ValueId(1)));
    assert!(
        matches!(aliases.get(&result.id), Some(ExecutionAliasV1::Physical(p)) if p.value == physical.value && p.extent == physical.extent)
    );
    assert_eq!(lowered.get(&result.id), Some(&pointer));
    assert!(
        lower(
            &module,
            &operation,
            &contract,
            conversion,
            &original,
            &mut lowered,
            &mut aliases
        )
        .is_err()
    );
}

#[test]
fn reusable_lds_backend_rejects_absent_extent_epoch_and_erasure() {
    for change in 0..4 {
        let (conversion, mut contract, input, result, pointer) = fixture::fixture();
        if change == 2 {
            contract.epoch_before = Some([99; 32]);
        }
        let operation = Operation::new(
            vec![result.clone()],
            OperationKind::ExecutionCapability(contract.clone()),
        );
        let physical = PhysicalValueV1 {
            value: ValueId(20),
            extent: Some(PhysicalExtentV1::Static(if change == 1 { 63 } else { 64 })),
        };
        let mut aliases = BTreeMap::new();
        if change != 0 {
            aliases.insert(ValueId(1), ExecutionAliasV1::Physical(physical));
        }
        let mut lowered = BTreeMap::from([(physical.value, pointer)]);
        let original = BTreeMap::from([(ValueId(1), input)]);
        let module = Module::new("reusable");
        if change == 3 {
            assert!(erase_capability_results(&module, &operation, &mut aliases).is_err());
            assert!(alias_capability_results(&module, &operation, physical, &mut aliases).is_err());
        } else {
            assert!(
                lower(
                    &module,
                    &operation,
                    &contract,
                    conversion,
                    &original,
                    &mut lowered,
                    &mut aliases
                )
                .is_err()
            );
            assert!(!aliases.contains_key(&result.id));
        }
    }
}
