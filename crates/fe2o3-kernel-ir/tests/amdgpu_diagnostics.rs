use std::collections::BTreeSet;

use fe2o3_kernel_ir::*;

fn u32_type() -> Type {
    Type::Scalar(ScalarType::U32)
}

fn module_with_operation(parameters: Vec<Type>, diagnostic: AmdGpuDiagnosticOperation) -> Module {
    let parameter_ids = (0..parameters.len())
        .map(|index| ValueId(index as u32))
        .collect::<Vec<_>>();
    let result = diagnostic
        .result_type()
        .map(|_| ValueId(parameters.len() as u32));
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(diagnostic.operation(result));
    block.terminator = Some(Terminator::Return {
        values: result.into_iter().collect(),
    });

    let declaration = diagnostic.declaration();
    let function = Function::internal_helper(
        "diagnostic_helper",
        Signature::new(parameters, diagnostic.result_type().into_iter().collect()),
        parameter_ids,
        vec![block],
    );
    let mut module = Module::new("tests::amdgpu-diagnostics");
    module.functions.push(function);
    module.functions.push(declaration);
    module
}

fn operations() -> Vec<(Vec<Type>, AmdGpuDiagnosticOperation)> {
    vec![
        (Vec::new(), AmdGpuDiagnosticOperation::Clock32),
        (Vec::new(), AmdGpuDiagnosticOperation::Trap),
        (Vec::new(), AmdGpuDiagnosticOperation::DebugTrap),
        (
            vec![u32_type()],
            AmdGpuDiagnosticOperation::ProfilingMarker { marker: ValueId(0) },
        ),
        (
            vec![u32_type()],
            AmdGpuDiagnosticOperation::Print {
                format_id: ValueId(0),
                arguments: Vec::new(),
            },
        ),
        (
            vec![u32_type(); 2],
            AmdGpuDiagnosticOperation::Print {
                format_id: ValueId(0),
                arguments: vec![ValueId(1)],
            },
        ),
        (
            vec![u32_type(); 3],
            AmdGpuDiagnosticOperation::Print {
                format_id: ValueId(0),
                arguments: vec![ValueId(1), ValueId(2)],
            },
        ),
        (
            vec![u32_type(); 2],
            AmdGpuDiagnosticOperation::AssertFail {
                site_id: ValueId(0),
                line: ValueId(1),
            },
        ),
    ]
}

#[test]
fn exact_diagnostic_contracts_verify_and_derive_capability() {
    let expected = BTreeSet::from([TargetCapability::Extension {
        namespace: AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE.to_owned(),
        name: AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME.to_owned(),
    }]);

    for (parameters, diagnostic) in operations() {
        assert_eq!(diagnostic.required_capabilities(), expected);
        let module = module_with_operation(parameters, diagnostic);
        verify_module(&module).unwrap();
        let operation = &module.functions[0].body.as_ref().unwrap().blocks[0].operations[0];
        assert_eq!(operation.required_capabilities(), expected);
        assert!(operation.memory_effects().is_empty());
    }
}

#[test]
fn reserved_diagnostic_calls_reject_arity_type_and_declaration_mutation() {
    let invalid_arity = AmdGpuDiagnosticOperation::Print {
        format_id: ValueId(0),
        arguments: vec![ValueId(1), ValueId(2), ValueId(3)],
    };
    assert!(
        verify_module(&module_with_operation(vec![u32_type(); 4], invalid_arity))
            .unwrap_err()
            .contains(DiagnosticCode::InvalidAmdGpuDiagnosticOperation)
    );

    let mut wrong_call_arity = module_with_operation(
        vec![u32_type(); 2],
        AmdGpuDiagnosticOperation::Print {
            format_id: ValueId(0),
            arguments: vec![ValueId(1)],
        },
    );
    let OperationKind::Call { arguments, .. } =
        &mut wrong_call_arity.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    arguments.pop();
    assert!(
        verify_module(&wrong_call_arity)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidAmdGpuDiagnosticOperation)
    );

    let wrong_type = module_with_operation(
        vec![Type::Scalar(ScalarType::U16)],
        AmdGpuDiagnosticOperation::ProfilingMarker { marker: ValueId(0) },
    );
    assert!(
        verify_module(&wrong_type)
            .unwrap_err()
            .contains(DiagnosticCode::TypeMismatch)
    );

    let mut mutated_declaration =
        module_with_operation(Vec::new(), AmdGpuDiagnosticOperation::Clock32);
    mutated_declaration.functions[1].signature.results.clear();
    assert!(
        verify_module(&mutated_declaration)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidAmdGpuDiagnosticOperation)
    );
}

#[test]
fn frozen_wire_round_trips_canonical_diagnostic_calls() {
    let module = module_with_operation(
        vec![u32_type(); 3],
        AmdGpuDiagnosticOperation::Print {
            format_id: ValueId(0),
            arguments: vec![ValueId(1), ValueId(2)],
        },
    );
    let bytes = encode_module_v2(&module).unwrap();
    assert_eq!(&bytes[8..10], &KERNEL_IR_VERSION_V2.to_le_bytes());
    assert_eq!(decode_module_v2(&bytes).unwrap(), module);
    assert_eq!(
        encode_module_v2(&decode_module_v2(&bytes).unwrap()).unwrap(),
        bytes
    );
    for length in 0..bytes.len() {
        assert!(decode_module_v2(&bytes[..length]).is_err());
    }
}
