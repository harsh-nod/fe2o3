use std::collections::BTreeSet;

use fe2o3_kernel_ir::*;

fn u32_type() -> Type {
    Type::Scalar(ScalarType::U32)
}

fn legacy_gfx942_diagnostic_capabilities() -> BTreeSet<TargetCapability> {
    BTreeSet::from([TargetCapability::Extension {
        namespace: AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE.to_owned(),
        name: AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME.to_owned(),
    }])
}

fn legacy_gfx942_diagnostic_declaration(diagnostic: &AmdGpuDiagnosticOperation) -> Function {
    let mut declaration = diagnostic.declaration();
    declaration.required_capabilities = legacy_gfx942_diagnostic_capabilities();
    declaration
}

fn module_with_operation(parameters: Vec<Type>, diagnostic: AmdGpuDiagnosticOperation) -> Module {
    let parameter_ids = (0..parameters.len())
        .map(|index| ValueId(index as u32))
        .collect::<Vec<_>>();
    let result = diagnostic
        .result_type()
        .map(|_| ValueId(parameters.len() as u32));
    let mut block = BasicBlock::new(BlockId(0));
    let terminates = diagnostic.is_terminating();
    block.operations.push(diagnostic.operation(result));
    block.terminator = Some(if terminates {
        Terminator::Unreachable
    } else {
        Terminator::Return {
            values: result.into_iter().collect(),
        }
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
        namespace: AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE.to_owned(),
        name: AMDGPU_DIAGNOSTICS_CAPABILITY_NAME.to_owned(),
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
fn frozen_gfx942_diagnostic_declarations_remain_verifiable() {
    let diagnostic = AmdGpuDiagnosticOperation::Clock32;
    let mut module = module_with_operation(Vec::new(), diagnostic.clone());
    module.functions[1] = legacy_gfx942_diagnostic_declaration(&diagnostic);
    let legacy = legacy_gfx942_diagnostic_capabilities();

    verify_module(&module).unwrap();
    verify_module_with_capabilities(&module, &legacy).unwrap();

    let bytes = encode_module_v2(&module).unwrap();
    let decoded = decode_module_v2(&bytes).unwrap();
    assert_eq!(decoded, module);
    verify_module_with_capabilities(&decoded, &diagnostic.required_capabilities()).unwrap();
}

#[test]
fn legacy_diagnostic_alias_rejects_mixed_or_lookalike_declarations() {
    let diagnostic = AmdGpuDiagnosticOperation::Clock32;
    let mut mixed = module_with_operation(Vec::new(), diagnostic.clone());
    mixed.functions[1] = legacy_gfx942_diagnostic_declaration(&diagnostic);
    mixed.functions[1]
        .required_capabilities
        .extend(diagnostic.required_capabilities());
    assert!(
        verify_module(&mixed)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidAmdGpuDiagnosticOperation)
    );

    let mut lookalike = module_with_operation(Vec::new(), diagnostic.clone());
    lookalike.functions[1] = legacy_gfx942_diagnostic_declaration(&diagnostic);
    lookalike.functions[1].required_capabilities = BTreeSet::from([TargetCapability::Extension {
        namespace: AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE.to_owned(),
        name: "diagnostics.gfx942.v1.lookalike".to_owned(),
    }]);
    assert!(
        verify_module(&lookalike)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidAmdGpuDiagnosticOperation)
    );

    assert!(
        verify_module_with_capabilities(
            &module_with_operation(Vec::new(), diagnostic),
            &lookalike.functions[1].required_capabilities,
        )
        .unwrap_err()
        .contains(DiagnosticCode::UnsupportedCapability)
    );
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
fn terminating_diagnostics_reject_fallthrough_and_nonterminal_placement() {
    for (parameters, diagnostic) in [
        (Vec::new(), AmdGpuDiagnosticOperation::Trap),
        (
            vec![u32_type(); 2],
            AmdGpuDiagnosticOperation::AssertFail {
                site_id: ValueId(0),
                line: ValueId(1),
            },
        ),
    ] {
        assert!(diagnostic.is_terminating());
        let mut fallthrough = module_with_operation(parameters.clone(), diagnostic.clone());
        fallthrough.functions[0].body.as_mut().unwrap().blocks[0].terminator =
            Some(Terminator::Return { values: vec![] });
        assert!(
            verify_module(&fallthrough)
                .unwrap_err()
                .contains(DiagnosticCode::InvalidAmdGpuDiagnosticOperation)
        );
        assert!(VerifiedCanonicalKernelIrV7::from_module(fallthrough).is_err());

        let mut branch = module_with_operation(parameters.clone(), diagnostic.clone());
        branch.functions[0].body.as_mut().unwrap().blocks[0].terminator =
            Some(Terminator::Branch {
                target: BlockId(0),
                arguments: vec![],
            });
        assert!(
            verify_module(&branch)
                .unwrap_err()
                .contains(DiagnosticCode::InvalidAmdGpuDiagnosticOperation)
        );
        assert!(VerifiedCanonicalKernelIrV7::from_module(branch).is_err());

        let mut nonterminal = module_with_operation(parameters, diagnostic);
        nonterminal.functions[0].body.as_mut().unwrap().blocks[0]
            .operations
            .push(Operation::effect_free(
                ValueDef::new(ValueId(8), Type::BOOL),
                OperationKind::Constant(Constant::Bool(false)),
            ));
        assert!(
            verify_module(&nonterminal)
                .unwrap_err()
                .contains(DiagnosticCode::InvalidAmdGpuDiagnosticOperation)
        );
        assert!(VerifiedCanonicalKernelIrV7::from_module(nonterminal).is_err());
    }
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

#[test]
fn frozen_wire_preserves_assert_fail_terminal_contract() {
    let module = module_with_operation(
        vec![u32_type(); 2],
        AmdGpuDiagnosticOperation::AssertFail {
            site_id: ValueId(0),
            line: ValueId(1),
        },
    );
    let bytes = encode_module_v2(&module).unwrap();
    let decoded = decode_module_v2(&bytes).unwrap();
    assert_eq!(decoded, module);
    let block = &decoded.functions[0].body.as_ref().unwrap().blocks[0];
    assert!(matches!(block.terminator, Some(Terminator::Unreachable)));
    let OperationKind::Call { callee, arguments } = &block.operations[0].kind else {
        panic!("assert-fail round trip changed operation kind");
    };
    assert!(
        AmdGpuDiagnosticOperation::from_intrinsic_call(callee, arguments)
            .is_some_and(|diagnostic| diagnostic.is_terminating())
    );
}
