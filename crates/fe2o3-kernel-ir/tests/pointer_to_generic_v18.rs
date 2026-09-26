use fe2o3_kernel_ir::*;

const LIMITS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
    rows: 16,
    edges: 32,
    containment_depth: 16,
    object_bytes: 1024,
};

fn module(from: Type, to: Type, kind: CastKind) -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(1), to.clone()),
        OperationKind::Cast { kind, value: ValueId(0), to },
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("generic-cast");
    module.functions.push(Function::internal_helper(
        "expose", Signature::new(vec![from], vec![]), vec![ValueId(0)], vec![block],
    ));
    module
}

fn pointer(pointee: Type, space: AddressSpace, access: AccessMode) -> Type {
    Type::pointer(pointee, space, access)
}

fn admit(module: &Module) -> Result<VerifiedCanonicalKernelIrModuleV18, CanonicalKernelIrReplayAdmissionErrorV18> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 100_000_000);
    budget.reserve_storage(13).unwrap();
    let result = VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
        module, LIMITS, &mut budget,
    ).map(|(owner, _)| owner);
    assert_eq!(budget.storage(), 13);
    result
}

#[test]
fn concrete_scalar_and_object_exposure_roundtrip_the_actual_v18_owner() {
    for space in [AddressSpace::Global, AddressSpace::Constant, AddressSpace::Private, AddressSpace::Workgroup] {
        for access in [AccessMode::ReadOnly, AccessMode::ReadWrite, AccessMode::WriteOnly] {
            for object in [false, true] {
                let pointee = if object { Type::StorageObject(StorageLayoutIdV1(0)) } else { Type::F32 };
                let mut candidate = module(
                    pointer(pointee.clone(), space, access),
                    pointer(pointee, AddressSpace::Generic, access),
                    CastKind::PointerToGeneric,
                );
                if object {
                    candidate.storage_layouts.push(StorageLayoutV1 {
                        size: 4, alignment: 4, kind: StorageLayoutKindV1::Scalar(ScalarType::F32),
                    });
                }
                if space == AddressSpace::Constant && access != AccessMode::ReadOnly {
                    assert!(admit(&candidate).is_err());
                    continue;
                }
                let owner = admit(&candidate).unwrap();
                assert_eq!(owner.module(), &candidate);
                assert_eq!(&owner.canonical_bytes()[8..10], &KERNEL_IR_VERSION_V18.to_le_bytes());
                let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
                let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 100_000_000);
                let (replayed, _) = VerifiedCanonicalKernelIrModuleV18::from_canonical_bytes_with_verification_budget_v18(
                    owner.canonical_bytes(), LIMITS, &mut budget,
                ).unwrap();
                assert_eq!(replayed.module(), &candidate);
                assert_eq!(replayed.canonical_bytes(), owner.canonical_bytes());
            }
        }
    }
}

#[test]
fn hostile_direction_pointee_access_and_numeric_disguises_are_rejected() {
    let concrete = pointer(Type::F32, AddressSpace::Private, AccessMode::ReadWrite);
    let generic = pointer(Type::F32, AddressSpace::Generic, AccessMode::ReadWrite);
    let positive = module(concrete.clone(), generic.clone(), CastKind::PointerToGeneric);
    admit(&positive).unwrap();
    for (from, to, kind) in [
        (generic.clone(), concrete.clone(), CastKind::PointerToGeneric),
        (generic.clone(), generic.clone(), CastKind::PointerToGeneric),
        (concrete.clone(), concrete.clone(), CastKind::PointerToGeneric),
        (concrete.clone(), pointer(Type::F32, AddressSpace::Workgroup, AccessMode::ReadWrite), CastKind::PointerToGeneric),
        (concrete.clone(), pointer(Type::Scalar(ScalarType::U32), AddressSpace::Generic, AccessMode::ReadWrite), CastKind::PointerToGeneric),
        (concrete.clone(), pointer(Type::F32, AddressSpace::Generic, AccessMode::ReadOnly), CastKind::PointerToGeneric),
        (concrete.clone(), Type::Scalar(ScalarType::U64), CastKind::PointerToGeneric),
        (Type::Scalar(ScalarType::U64), generic.clone(), CastKind::PointerToGeneric),
        (concrete, generic, CastKind::Bitcast),
    ] {
        assert!(admit(&module(from, to, kind)).is_err());
    }
    let mut bad_result = positive.clone();
    bad_result.functions[0].body.as_mut().unwrap().blocks[0].operations[0].results[0].ty = Type::F32;
    assert!(admit(&bad_result).is_err());
    admit(&positive).unwrap();
}

#[test]
fn same_sized_object_rows_cannot_be_substituted_by_the_cast() {
    let mut candidate = module(
        pointer(Type::StorageObject(StorageLayoutIdV1(0)), AddressSpace::Private, AccessMode::ReadWrite),
        pointer(Type::StorageObject(StorageLayoutIdV1(0)), AddressSpace::Generic, AccessMode::ReadWrite),
        CastKind::PointerToGeneric,
    );
    candidate.storage_layouts = vec![
        StorageLayoutV1 { size: 4, alignment: 4, kind: StorageLayoutKindV1::Scalar(ScalarType::U32) },
        StorageLayoutV1 { size: 4, alignment: 4, kind: StorageLayoutKindV1::Scalar(ScalarType::F32) },
    ];
    admit(&candidate).unwrap();
    let replacement = pointer(Type::StorageObject(StorageLayoutIdV1(1)), AddressSpace::Generic, AccessMode::ReadWrite);
    let cast = &mut candidate.functions[0].body.as_mut().unwrap().blocks[0].operations[0];
    cast.results[0].ty = replacement.clone();
    let OperationKind::Cast { to, .. } = &mut cast.kind else { unreachable!() };
    *to = replacement;
    assert!(admit(&candidate).is_err());
}

type Encoder = fn(&Module) -> Result<Vec<u8>, KernelIrEncodeError>;
type Decoder = fn(&[u8]) -> Result<Module, KernelIrDecodeError>;

#[test]
fn constant_write_permissions_cannot_be_hidden_by_generic_exposure() {
    let from = pointer(Type::F32, AddressSpace::Constant, AccessMode::ReadWrite);
    let read_only = pointer(Type::F32, AddressSpace::Constant, AccessMode::ReadOnly);
    let generic = pointer(Type::F32, AddressSpace::Generic, AccessMode::ReadOnly);
    let mut candidate = module(from, read_only, CastKind::RestrictPointerAccess);
    let operations = &mut candidate.functions[0].body.as_mut().unwrap().blocks[0].operations;
    operations.push(Operation::effect_free(ValueDef::new(ValueId(2), generic.clone()), OperationKind::Cast {
        kind: CastKind::PointerToGeneric, value: ValueId(1), to: generic,
    }));
    admit(&candidate).unwrap();
    let operations = &mut candidate.functions[0].body.as_mut().unwrap().blocks[0].operations;
    operations.push(Operation::effect_free(ValueDef::new(ValueId(3), Type::F32),
        OperationKind::Constant(Constant::F32Bits(0))));
    operations.push(Operation::new(vec![], OperationKind::Store {
        pointer: ValueId(2), value: ValueId(3), access: MemoryAccess::new(AddressSpace::Generic, 4),
    }));
    assert!(admit(&candidate).is_err());
}

#[test]
fn every_frozen_wire_profile_rejects_tag_ten_without_changing_old_cast_bytes() {
    let exposed = module(
        pointer(Type::F32, AddressSpace::Global, AccessMode::ReadWrite),
        pointer(Type::F32, AddressSpace::Generic, AccessMode::ReadWrite),
        CastKind::PointerToGeneric,
    );
    let scalar = module(Type::Scalar(ScalarType::U32), Type::Scalar(ScalarType::U64), CastKind::ZeroExtend);
    let profiles: &[(u16, Encoder, Decoder)] = &[
        (1, encode_module_v1, decode_module_v1), (2, encode_module_v2, decode_module_v2),
        (3, encode_module_v3, decode_module_v3), (4, encode_module_v4, decode_module_v4),
        (5, encode_module_v5, decode_module_v5), (6, encode_module_v6, decode_module_v6),
        (7, encode_module_v7, decode_module_v7), (8, encode_module_v8, decode_module_v8),
        (9, encode_module_v9, decode_module_v9), (10, encode_module_v10, decode_module_v10),
        (11, encode_module_v11, decode_module_v11), (12, encode_module_v12, decode_module_v12),
        (15, encode_module_v15, decode_module_v15), (16, encode_module_v16, decode_module_v16),
        (17, encode_module_v17, decode_module_v17),
    ];
    for &(version, encode, decode) in profiles {
        assert_eq!(encode(&exposed), Err(KernelIrEncodeError::UnsupportedInVersion {
            version, feature: "pointer to generic cast",
        }));
        let mut bytes = encode(&scalar).unwrap();
        assert_eq!(decode(&bytes).unwrap(), scalar);
        let offsets: Vec<_> = bytes.windows(6).enumerate()
            .filter_map(|(offset, row)| (row == [6, 2, 0, 0, 0, 0]).then_some(offset))
            .collect();
        assert_eq!(offsets.len(), 1);
        bytes[offsets[0] + 1] = 10;
        assert!(matches!(decode(&bytes), Err(KernelIrDecodeError::UnknownTag {
            kind: "cast kind", tag: 10,
        })));
    }
}
