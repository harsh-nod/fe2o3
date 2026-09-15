use crate::*;
include!("../subgroup_partition/fixture.rs");

fn conversion() -> ReusableLdsConversionV1 {
    ReusableLdsConversionV1 {
        input: identity(90),
        output: identity(91),
        element: identity(92),
        layout: ExecutionElementLayoutV1 {
            byte_size: 8,
            byte_alignment: 4,
        },
        elements: 256,
        defined_function: [93; 32],
        defined_abi: [94; 32],
        defined_body: [95; 32],
        source_binding: [96; 32],
    }
}
fn converted_module() -> Module {
    let mut m = module();
    let value = conversion();
    let allocation = contract(
        ExecutionCapabilityOperationV1::LdsAllocate {
            workgroup: identity(11),
            lds: value.input,
            element: value.element,
            layout: value.layout,
            elements: value.elements,
        },
        &[11],
        90,
        &[1],
        80,
    );
    let mut converted = contract(
        ExecutionCapabilityOperationV1::ReusableLdsConversion(value),
        &[90],
        91,
        &[90],
        81,
    );
    converted.source.operation = value.defined_function;
    converted.source.occurrence = ExecutionCapabilitySourceOccurrenceV1::from_untrusted_parts(
        [99; 32], [100; 32], [101; 32], 3, 0,
    );
    let input = capability(90, value.input_role());
    let Type::ExecutionCapability(input_type) = &input else {
        unreachable!()
    };
    let output = Type::ExecutionCapability(value.output_type(input_type).unwrap());
    let requirements = allocation.operation.required_capabilities();
    m.required_capabilities.extend(requirements.clone());
    m.functions[0]
        .required_capabilities
        .extend(requirements.clone());
    m.kernels[0].required_capabilities.extend(requirements);
    operations_mut(&mut m).push(Operation::effect_free(
        ValueDef::new(ValueId(90), input),
        OperationKind::ExecutionCapability(allocation),
    ));
    operations_mut(&mut m).push(Operation::effect_free(
        ValueDef::new(ValueId(91), output),
        OperationKind::ExecutionCapability(converted),
    ));
    for (index, operation) in operations_mut(&mut m).iter_mut().enumerate() {
        if let OperationKind::ExecutionCapability(contract) = &mut operation.kind {
            contract.source.occurrence =
                ExecutionCapabilitySourceOccurrenceV1::from_untrusted_parts(
                    [99; 32],
                    [100; 32],
                    [101; 32],
                    index as u32,
                    0,
                );
        }
    }
    m
}

#[test]
fn reusable_lds_keeps_allocation_epoch_and_never_initializes_or_allocates() {
    let m = converted_module();
    verify_module(&m).unwrap();
    let operations = operations(&m);
    let converted = operation_contract(operations.last().unwrap());
    assert!(converted.operation.memory_effects().is_empty());
    assert!(!converted.operation.transitions_epoch());
    assert_eq!(converted.operands, [ValueId(90)]);
    let Type::ExecutionCapability(input) = &operations[operations.len() - 2].results[0].ty else {
        unreachable!()
    };
    let Type::ExecutionCapability(output) = &operations.last().unwrap().results[0].ty else {
        unreachable!()
    };
    assert_eq!(input.epoch, output.epoch);
    assert_eq!(input.workgroup_brand, output.workgroup_brand);
    assert_eq!(input.provenance, output.provenance);
    assert_eq!(conversion().output_type(input), Some(output.clone()));
}

#[test]
fn reusable_lds_revision_six_roundtrips_and_old_revisions_reject_it() {
    let m = converted_module();
    let op = operation_contract(operations(&m).last().unwrap());
    let bytes = encode_execution_capability_contract_v1(op).unwrap();
    assert_eq!(&bytes[..2], &[6, 29]);
    assert_eq!(
        decode_execution_capability_contract_v1(&bytes, op.operands.clone()),
        Some(op.clone())
    );
    for revision in 1..=5 {
        let mut changed = bytes.clone();
        changed[0] = revision;
        assert!(decode_execution_capability_contract_v1(&changed, op.operands.clone()).is_none());
    }
    for end in 0..bytes.len() {
        assert!(
            decode_execution_capability_contract_v1(&bytes[..end], op.operands.clone()).is_none()
        );
    }
    let Type::ExecutionCapability(output) = &operations(&m).last().unwrap().results[0].ty else {
        unreachable!()
    };
    let bytes = encode_execution_capability_type_v1(output).unwrap();
    assert_eq!(bytes[0], 6);
    assert_eq!(
        decode_execution_capability_type_v1(&bytes),
        Some(output.clone())
    );
    assert_eq!(
        decode_module_v13(&encode_module_v13(&m).unwrap()).unwrap(),
        m
    );
}

#[test]
fn reusable_lds_rejects_published_inputs_wrong_epoch_and_empty_operands() {
    for change in 0..5 {
        let mut m = converted_module();
        let last = operations(&m).len() - 1;
        match change {
            0 => contract_mut(&mut m, last).operands.clear(),
            1 => contract_mut(&mut m, last).epoch_before = Some([200; 32]),
            2 => contract_mut(&mut m, last).epoch_after = Some([201; 32]),
            3 => contract_mut(&mut m, last).source.occurrence = None,
            4 => {
                let Type::ExecutionCapability(input) =
                    &mut operations_mut(&mut m)[last - 1].results[0].ty
                else {
                    unreachable!()
                };
                let ExecutionCapabilityRoleV1::Lds { state, .. } = &mut input.role else {
                    unreachable!()
                };
                *state = ExecutionLdsStateV1::Published;
            }
            _ => unreachable!(),
        }
        assert!(
            verify_module(&m)
                .unwrap_err()
                .diagnostics()
                .iter()
                .any(|d| d.code == DiagnosticCode::InvalidExecutionCapability)
        );
    }
}

#[test]
fn reusable_lds_rejects_second_consumption_of_the_same_allocation() {
    let mut m = converted_module();
    let mut extra = operations(&m).last().unwrap().clone();
    extra.results[0].id = ValueId(200);
    let OperationKind::ExecutionCapability(contract) = &mut extra.kind else {
        unreachable!()
    };
    contract.source.occurrence = ExecutionCapabilitySourceOccurrenceV1::from_untrusted_parts(
        [99; 32], [100; 32], [101; 32], 4, 0,
    );
    operations_mut(&mut m).push(extra);
    assert!(
        verify_module(&m)
            .unwrap_err()
            .diagnostics()
            .iter()
            .any(|d| d.code == DiagnosticCode::InvalidExecutionCapability)
    );
}

#[test]
fn reusable_lds_does_not_change_existing_partition_payloads() {
    let before = module();
    let after = converted_module();
    for (left, right) in operations(&before).iter().zip(operations(&after)) {
        if let OperationKind::ExecutionCapability(a) = &left.kind {
            let OperationKind::ExecutionCapability(b) = &right.kind else {
                unreachable!()
            };
            let mut original_occurrence = b.clone();
            original_occurrence.source.occurrence = None;
            assert_eq!(
                encode_execution_capability_contract_v1(a),
                encode_execution_capability_contract_v1(&original_occurrence)
            );
        }
    }
    assert_eq!(
        encode_execution_capability_contract_v1(operation_contract(&operations(&before)[3]))
            .unwrap()[0],
        2
    );
}
