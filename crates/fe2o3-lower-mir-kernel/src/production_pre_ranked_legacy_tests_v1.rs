use super::*;

fn legacy_v9_fixture() -> Module {
    // Same admitted shape as write_only_signature_requires_v9_without_any_store_operation.
    let mut module = Module::new("pre_ranked_legacy_v9");
    module.functions.push(Function::declaration(
        "write_only_signature",
        Signature::new(
            vec![Type::slice(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Global,
                AccessMode::WriteOnly,
            )],
            vec![],
        ),
    ));
    module
}

fn legacy_v11_fixture() -> Module {
    let mut module = legacy_v9_fixture();
    let read_write = Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadWrite);
    let read_only = Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadOnly);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(9), read_only.clone()),
        OperationKind::Cast {
            kind: CastKind::RestrictPointerAccess,
            value: ValueId(8),
            to: read_only,
        },
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    module.functions.push(Function::internal_helper(
        "restrict",
        Signature::new(vec![read_write], vec![]),
        vec![ValueId(8)],
        vec![block],
    ));
    module
}

#[test]
fn pre_ranked_borrowed_legacy_v9_bytes_identity_and_replay_match_the_owned_constructor() {
    let module = legacy_v9_fixture();
    assert!(module_requires_kernel_ir_v9_v1(&module));
    assert!(!module_requires_kernel_ir_v11_v1(&module));
    assert!(fe2o3_kernel_ir::encode_module_v8(&module).is_err());
    let owned = ProductionCanonicalKernelIrV1::from_module(module.clone()).unwrap();
    let borrowed = ProductionCanonicalKernelIrV1::from_module_ref(&module).unwrap();
    let (ProductionCanonicalKernelIrV1::V9(left), ProductionCanonicalKernelIrV1::V9(right)) =
        (&owned, &borrowed)
    else {
        panic!("write-only signature must retain the frozen V9 contract");
    };
    assert_eq!(left.identity(), right.identity());
    assert_eq!(left.canonical_bytes(), right.canonical_bytes());
    assert_eq!(&right.canonical_bytes()[8..10], &[9, 0]);
    owned.revalidate().unwrap();
    borrowed.revalidate().unwrap();
    let inverse = fe2o3_kernel_ir::decode_module_v9(right.canonical_bytes()).unwrap();
    assert_eq!(inverse, module);
    let replay = ProductionCanonicalKernelIrV1::from_module(inverse).unwrap();
    assert_eq!(replay, borrowed);
    assert_eq!(replay.canonical_bytes(), borrowed.canonical_bytes());
}

#[test]
fn pre_ranked_borrowed_legacy_v11_preserves_pointer_narrowing_and_version_precedence() {
    let module = legacy_v11_fixture();
    assert!(module_requires_kernel_ir_v9_v1(&module));
    assert!(module_requires_kernel_ir_v11_v1(&module));
    assert!(fe2o3_kernel_ir::encode_module_v9(&module).is_err());
    let owned = ProductionCanonicalKernelIrV1::from_module(module.clone()).unwrap();
    let borrowed = ProductionCanonicalKernelIrV1::from_module_ref(&module).unwrap();
    let (ProductionCanonicalKernelIrV1::V11(left), ProductionCanonicalKernelIrV1::V11(right)) =
        (&owned, &borrowed)
    else {
        panic!("pointer narrowing must select V11 even when the signature also needs V9");
    };
    assert_eq!(left.identity(), right.identity());
    assert_eq!(left.canonical_bytes(), right.canonical_bytes());
    assert_eq!(&right.canonical_bytes()[8..10], &[11, 0]);
    owned.revalidate().unwrap();
    borrowed.revalidate().unwrap();
    let inverse = fe2o3_kernel_ir::decode_module_v11(right.canonical_bytes()).unwrap();
    assert_eq!(inverse, module);
    assert!(matches!(
        inverse.functions[1].body.as_ref().unwrap().blocks[0].operations[0].kind,
        OperationKind::Cast {
            kind: CastKind::RestrictPointerAccess,
            value: ValueId(8),
            ..
        }
    ));
    let replay = ProductionCanonicalKernelIrV1::from_module(inverse).unwrap();
    assert_eq!(replay, borrowed);
    assert_eq!(replay.canonical_bytes(), borrowed.canonical_bytes());
}
