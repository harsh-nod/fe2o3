use fe2o3_kernel_ir::*;

fn restriction_module() -> Module {
    let read_write = Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadWrite);
    let read_only = Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadOnly);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(1), read_only.clone()),
        OperationKind::Cast {
            kind: CastKind::RestrictPointerAccess,
            value: ValueId(0),
            to: read_only,
        },
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("pointer-access-cast-v11");
    module.functions.push(Function::internal_helper(
        "restrict",
        Signature::new(vec![read_write], vec![]),
        vec![ValueId(0)],
        vec![block],
    ));
    module
}

#[test]
fn pointer_access_cast_requires_v11_and_round_trips_canonically() {
    let module = restriction_module();
    for (version, encoded) in [
        (KERNEL_IR_VERSION_V1, encode_module_v1(&module)),
        (KERNEL_IR_VERSION_V2, encode_module_v2(&module)),
        (KERNEL_IR_VERSION_V3, encode_module_v3(&module)),
        (KERNEL_IR_VERSION_V4, encode_module_v4(&module)),
        (KERNEL_IR_VERSION_V5, encode_module_v5(&module)),
        (KERNEL_IR_VERSION_V6, encode_module_v6(&module)),
        (KERNEL_IR_VERSION_V7, encode_module_v7(&module)),
        (KERNEL_IR_VERSION_V8, encode_module_v8(&module)),
        (KERNEL_IR_VERSION_V9, encode_module_v9(&module)),
        (KERNEL_IR_VERSION_V10, encode_module_v10(&module)),
    ] {
        assert_eq!(
            encoded,
            Err(KernelIrEncodeError::UnsupportedInVersion {
                version,
                feature: "pointer access restriction cast",
            })
        );
    }

    let encoded = encode_module_v11(&module).unwrap();
    assert_eq!(&encoded[8..10], &KERNEL_IR_VERSION_V11.to_le_bytes());
    assert_eq!(decode_module_v11(&encoded).unwrap(), module);
    assert_eq!(
        encode_module_v11(&decode_module_v11(&encoded).unwrap()).unwrap(),
        encoded
    );
    VerifiedCanonicalKernelIrV11::from_module(module).unwrap();
}

#[test]
fn mixed_pointer_restriction_shapes_preserve_invalid_cast_diagnostics() {
    let pointer = Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadOnly);
    for (from, to) in [(Type::INDEX, pointer.clone()), (pointer, Type::INDEX)] {
        let mut module = restriction_module();
        let function = &mut module.functions[0];
        function.signature.parameters = vec![from.clone()];
        let operation = &mut function.body.as_mut().unwrap().blocks[0].operations[0];
        operation.results[0].ty = to.clone();
        operation.kind = OperationKind::Cast {
            kind: CastKind::RestrictPointerAccess,
            value: ValueId(0),
            to: to.clone(),
        };
        let expected = [Diagnostic {
            location: DiagnosticLocation {
                module: module.id.clone(),
                function: Some(module.functions[0].id.clone()),
                kernel: None,
                block: Some(BlockId(0)),
                operation: Some(0),
            },
            code: DiagnosticCode::InvalidCast,
            message: format!("invalid RestrictPointerAccess cast from {from:?} to {to:?}"),
        }];
        for result in [
            verify_module(&module),
            verify_module_ref(&module).map(|_| ()),
            verify_module_with_capabilities(&module, &Default::default()),
        ] {
            assert_eq!(result.unwrap_err().diagnostics(), &expected);
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, usize::MAX);
        let Err(BorrowedKernelIrVerificationErrorV1::Verification(errors)) =
            verify_module_ref_with_budget_v1(&module, None, &mut budget)
        else {
            panic!("expected the same invalid-cast diagnostic");
        };
        assert_eq!(errors.diagnostics(), &expected);
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn frozen_v10_reader_rejects_a_forged_pointer_access_cast_tag() {
    let mut encoded = encode_module_v11(&restriction_module()).unwrap();
    encoded[8..10].copy_from_slice(&KERNEL_IR_VERSION_V10.to_le_bytes());
    assert!(matches!(
        decode_module_v10(&encoded),
        Err(KernelIrDecodeError::UnknownTag {
            kind: "cast kind",
            tag: 9,
        })
    ));
}
