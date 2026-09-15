use super::*;
mod fixture {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fe2o3-kernel-ir/src/execution_capability_v1/reusable_lds/physical_fixture.rs"
    ));
}

#[test]
fn reusable_lds_sim_retains_physical_definition_without_memory_events() {
    let (conversion, contract, input, result, pointer) = fixture::fixture();
    let types = BTreeMap::from([(ValueId(1), input)]);
    let mut promoted = BTreeMap::from([(ValueId(1), pointer.clone())]);
    let mut aliases = Vec::new();
    let operations = project(
        vec![result.clone()],
        &contract,
        conversion,
        &types,
        &mut aliases,
        &mut promoted,
    )
    .unwrap();
    assert!(operations.is_empty());
    assert_eq!(aliases, vec![(result.id, ValueId(1))]);
    assert_eq!(promoted.get(&ValueId(1)), Some(&pointer));
    assert_eq!(promoted.get(&result.id), Some(&pointer));
    assert!(
        project(
            vec![result],
            &contract,
            conversion,
            &types,
            &mut aliases,
            &mut promoted
        )
        .is_err()
    );
}

#[test]
fn reusable_lds_sim_does_not_invent_allocation_or_publish() {
    for change in 0..3 {
        let (conversion, mut contract, input, mut result, pointer) = fixture::fixture();
        if change == 1 {
            contract.epoch_before = Some([99; 32]);
        }
        if change == 2 {
            let Type::ExecutionCapability(capability) = &mut result.ty else {
                unreachable!()
            };
            capability.role = fe2o3_kernel_ir::ExecutionCapabilityRoleV1::Lds {
                element: conversion.element,
                layout: conversion.layout,
                elements: conversion.elements,
                state: fe2o3_kernel_ir::ExecutionLdsStateV1::Published,
            };
        }
        let types = BTreeMap::from([(ValueId(1), input)]);
        let mut promoted = BTreeMap::new();
        if change != 0 {
            promoted.insert(ValueId(1), pointer);
        }
        let mut aliases = Vec::new();
        assert!(matches!(
            project(
                vec![result],
                &contract,
                conversion,
                &types,
                &mut aliases,
                &mut promoted
            ),
            Err(ExecutionCapabilityProjectionErrorV13::Invalid(
                "reusable LDS lost its exact allocated handle or epoch"
            ))
        ));
        assert!(aliases.is_empty());
    }
}
