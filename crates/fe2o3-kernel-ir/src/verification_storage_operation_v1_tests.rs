use super::*;
use crate::{
    BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, FixedVectorTypeV12, Function,
    Module, ScalarType, Signature, StorageCopyOverlapV1, StorageFieldV1, StorageLayoutLimitsV1,
    StoragePointerV1, StorageVariantEncodingV1, StorageVariantV1, Terminator, ValueDef,
    VectorLayoutV12, VerificationDiagnosticCollectorV1, VerificationFunctionStateV1,
    VerificationModuleStateV1, check_module_storage_v1,
};

const FLOOR: usize = 17;

#[test]
fn discriminant_read_requires_original_enum_read_rights_nonvolatile_access_and_u128() {
    let operation = StorageOperationV1::ReadDiscriminant {
        address: ValueId(0),
        access: memory(),
    };
    let mut valid = module(
        vec![private(8)],
        vec![Type::Scalar(ScalarType::U128)],
        operation,
    );
    let StorageLayoutKindV1::Variants { variants, .. } = &mut valid.storage_layouts[8].kind else {
        unreachable!()
    };
    variants[1].discriminant = u128::MAX;
    assert!(inspect(&valid, None).unwrap().is_empty());
    for results in [
        vec![],
        vec![Type::Scalar(ScalarType::U64)],
        vec![Type::Scalar(ScalarType::I128)],
        vec![Type::Scalar(ScalarType::U128); 2],
    ] {
        assert!(
            !inspect(&module(vec![private(8)], results, operation), None)
                .unwrap()
                .is_empty()
        );
    }
    for pointer in [
        private(0),
        address(8, AddressSpace::Private, AccessMode::WriteOnly),
        address(8, AddressSpace::Global, AccessMode::ReadWrite),
    ] {
        assert!(
            !inspect(
                &module(
                    vec![pointer],
                    vec![Type::Scalar(ScalarType::U128)],
                    operation
                ),
                None
            )
            .unwrap()
            .is_empty()
        );
    }
    let aligned = StorageOperationV1::ReadDiscriminant {
        address: ValueId(0),
        access: MemoryAccess::new(AddressSpace::Private, 8),
    };
    assert!(
        inspect(
            &module(
                vec![private(8)],
                vec![Type::Scalar(ScalarType::U128)],
                aligned
            ),
            None
        )
        .unwrap()
        .is_empty()
    );
    for access in [
        MemoryAccess {
            volatile: true,
            ..memory()
        },
        MemoryAccess::new(AddressSpace::Private, 16),
    ] {
        let operation = StorageOperationV1::ReadDiscriminant {
            address: ValueId(0),
            access,
        };
        assert!(
            !inspect(
                &module(
                    vec![private(8)],
                    vec![Type::Scalar(ScalarType::U128)],
                    operation
                ),
                None
            )
            .unwrap()
            .is_empty()
        );
    }
}

const LIMITS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
    rows: 64,
    edges: 256,
    containment_depth: 32,
    object_bytes: 4096,
};

fn field(offset: u64, layout: u32) -> StorageFieldV1 {
    StorageFieldV1 {
        offset,
        layout: StorageLayoutIdV1(layout),
    }
}

fn scalar(ty: ScalarType, size: u64) -> StorageLayoutV1 {
    StorageLayoutV1 {
        size,
        alignment: size as u32,
        kind: StorageLayoutKindV1::Scalar(ty),
    }
}

fn rows() -> Vec<StorageLayoutV1> {
    vec![
        scalar(ScalarType::U32, 4),
        scalar(ScalarType::U64, 8),
        StorageLayoutV1 {
            size: 8,
            alignment: 8,
            kind: StorageLayoutKindV1::Pointer(StoragePointerV1 {
                pointee: StorageLayoutIdV1(0),
                value_space: AddressSpace::Global,
                encoded_space: AddressSpace::Generic,
                access: AccessMode::ReadOnly,
                stored_bits: 64,
            }),
        },
        scalar(ScalarType::Index, 8),
        StorageLayoutV1 {
            size: 16,
            alignment: 8,
            kind: StorageLayoutKindV1::Slice {
                element: StorageLayoutIdV1(0),
                value_space: AddressSpace::Global,
                access: AccessMode::ReadOnly,
                data: field(0, 2),
                length: field(8, 3),
            },
        },
        StorageLayoutV1 {
            size: 24,
            alignment: 8,
            kind: StorageLayoutKindV1::Record(
                vec![field(0, 0), field(8, 2), field(16, 3)].into_boxed_slice(),
            ),
        },
        StorageLayoutV1 {
            size: 8,
            alignment: 8,
            kind: StorageLayoutKindV1::Union(vec![field(0, 0), field(0, 1)].into_boxed_slice()),
        },
        StorageLayoutV1 {
            size: 12,
            alignment: 4,
            kind: StorageLayoutKindV1::Array {
                element: StorageLayoutIdV1(0),
                length: 3,
                stride: 4,
            },
        },
        StorageLayoutV1 {
            size: 24,
            alignment: 8,
            kind: StorageLayoutKindV1::Variants {
                encoding: StorageVariantEncodingV1::Direct { tag: field(0, 0) },
                variants: vec![
                    StorageVariantV1 {
                        discriminant: 0,
                        direct_tag_bits: Some(0),
                        uninhabited: false,
                        layout: StorageLayoutIdV1(5),
                    },
                    StorageVariantV1 {
                        discriminant: 1,
                        direct_tag_bits: Some(1),
                        uninhabited: false,
                        layout: StorageLayoutIdV1(6),
                    },
                ]
                .into_boxed_slice(),
            },
        },
        StorageLayoutV1 {
            size: 0,
            alignment: 4,
            kind: StorageLayoutKindV1::Array {
                element: StorageLayoutIdV1(0),
                length: 0,
                stride: 4,
            },
        },
    ]
}

fn address(layout: u32, space: AddressSpace, rights: AccessMode) -> Type {
    Type::pointer(
        Type::StorageObject(StorageLayoutIdV1(layout)),
        space,
        rights,
    )
}

fn private(layout: u32) -> Type {
    address(layout, AddressSpace::Private, AccessMode::ReadWrite)
}

fn memory() -> MemoryAccess {
    MemoryAccess::new(AddressSpace::Private, 1)
}

fn read() -> StorageOperationV1 {
    StorageOperationV1::ReadValue {
        address: ValueId(0),
        access: memory(),
    }
}

fn module(parameters: Vec<Type>, result_types: Vec<Type>, operation: StorageOperationV1) -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::new(
        result_types
            .into_iter()
            .enumerate()
            .map(|(i, ty)| ValueDef::new(ValueId(100 + i as u32), ty))
            .collect(),
        OperationKind::Storage(operation),
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let parameter_values = (0..parameters.len()).map(|i| ValueId(i as u32)).collect();
    let mut module = Module::new("storage");
    module.storage_layouts = rows();
    module.functions.push(Function::definition(
        "f",
        Signature::new(parameters, vec![]),
        parameter_values,
        vec![block],
    ));
    module
}

fn inspect(
    module: &Module,
    foreign: Option<&Module>,
) -> Result<Vec<DiagnosticCode>, ResourceError> {
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    budget.reserve_storage(FLOOR).unwrap();
    let storage = check_module_storage_v1(foreign.unwrap_or(module), LIMITS, &mut budget).unwrap();
    let function = &module.functions[0];
    let module_state = VerificationModuleStateV1::build(module, &mut budget).unwrap();
    let state = VerificationFunctionStateV1::build(function, &mut budget)
        .unwrap()
        .unwrap();
    let operation = &function.body.as_ref().unwrap().blocks[0].operations[0];
    let location = VerificationDiagnosticLocationV1::function(module, function)
        .at_block(BlockId(0))
        .at_operation(0);
    let invoke = |diagnostics: &mut VerificationDiagnosticCollectorV1, budget: &mut Budget<'_>| {
        let mut pass = VerificationFunctionPassV1 {
            module,
            function,
            module_state: &module_state,
            function_state: &state,
            supported_capabilities: None,
            diagnostics,
            budget,
            control_flow: None,
            dynamic_workgroup_memory_declarations: 0,
            gfx950_lds_transpose_current_formats: 0,
        };
        pass.verify_storage_operation_v1(&storage, operation, &location)
    };
    let mut counter = VerificationDiagnosticCollectorV1::count();
    let result = invoke(&mut counter, &mut budget);
    let output = if result.is_ok() {
        let mut collected =
            VerificationDiagnosticCollectorV1::materialize(counter.counted().unwrap(), &mut budget)
                .unwrap();
        invoke(&mut collected, &mut budget).unwrap();
        let diagnostics = collected.finish_materialized(&mut budget).unwrap();
        Ok(diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.code)
            .collect())
    } else {
        result.map(|_| Vec::new())
    };
    state.release(&mut budget).unwrap();
    module_state.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    output
}

fn accepts(module: &Module) {
    assert_eq!(inspect(module, None).unwrap(), []);
}

fn rejects(module: &Module, code: DiagnosticCode) {
    let errors = inspect(module, None).unwrap();
    assert!(errors.contains(&code), "{errors:?}");
}

#[test]
fn storage_operation_verifier_reads_and_writes_scalar_pointer_and_slice_values() {
    for (layout, ty) in [
        (0, Type::Scalar(ScalarType::U32)),
        (1, Type::Scalar(ScalarType::U64)),
        (
            2,
            Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Global,
                AccessMode::ReadOnly,
            ),
        ),
        (
            4,
            Type::slice(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Global,
                AccessMode::ReadOnly,
            ),
        ),
    ] {
        accepts(&module(vec![private(layout)], vec![ty.clone()], read()));
        accepts(&module(
            vec![private(layout), ty],
            vec![],
            StorageOperationV1::WriteValue {
                address: ValueId(0),
                value: ValueId(1),
                access: memory(),
            },
        ));
    }
}

#[test]
fn storage_operation_verifier_preserves_holder_and_payload_address_spaces() {
    let correct = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    accepts(&module(vec![private(2)], vec![correct], read()));
    for wrong in [
        Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Private,
            AccessMode::ReadOnly,
        ),
        Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadWrite,
        ),
        Type::pointer(
            Type::Scalar(ScalarType::U64),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        ),
    ] {
        rejects(
            &module(vec![private(2)], vec![wrong.clone()], read()),
            DiagnosticCode::TypeMismatch,
        );
        rejects(
            &module(
                vec![private(2), wrong],
                vec![],
                StorageOperationV1::WriteValue {
                    address: ValueId(0),
                    value: ValueId(1),
                    access: memory(),
                },
            ),
            DiagnosticCode::TypeMismatch,
        );
    }
}

#[test]
fn storage_operation_verifier_vectors_preserve_logical_lanes_and_layout() {
    let vector = FixedVectorTypeV12::new(ScalarType::U32, 4, VectorLayoutV12::Contiguous);
    let mut m = module(vec![private(10)], vec![Type::Vector(vector)], read());
    m.storage_layouts.push(StorageLayoutV1 {
        size: 16,
        alignment: 16,
        kind: StorageLayoutKindV1::Vector(vector),
    });
    accepts(&m);
    for wrong in [
        FixedVectorTypeV12::new(ScalarType::U32, 3, VectorLayoutV12::Contiguous),
        FixedVectorTypeV12::new(
            ScalarType::U32,
            4,
            VectorLayoutV12::Interleaved { factor: 2 },
        ),
    ] {
        m.functions[0].body.as_mut().unwrap().blocks[0].operations[0].results[0].ty =
            Type::Vector(wrong);
        rejects(&m, DiagnosticCode::TypeMismatch);
    }
}

#[test]
fn storage_operation_verifier_rejects_unrepresented_values_bad_ids_and_arity() {
    for layout in [5, 6, 7, 8] {
        for ty in [
            Type::Scalar(ScalarType::U32),
            Type::StorageObject(StorageLayoutIdV1(layout)),
        ] {
            rejects(
                &module(vec![private(layout)], vec![ty], read()),
                DiagnosticCode::TypeMismatch,
            );
        }
    }
    rejects(
        &module(vec![private(99)], vec![Type::INDEX], read()),
        DiagnosticCode::InvalidMemoryAccess,
    );
    rejects(
        &module(vec![Type::INDEX], vec![Type::INDEX], read()),
        DiagnosticCode::InvalidOperandType,
    );
    rejects(
        &module(
            vec![Type::pointer(
                Type::INDEX,
                AddressSpace::Private,
                AccessMode::ReadWrite,
            )],
            vec![Type::INDEX],
            read(),
        ),
        DiagnosticCode::InvalidOperandType,
    );
    for results in [vec![], vec![Type::INDEX, Type::INDEX]] {
        rejects(
            &module(vec![private(3)], results, read()),
            DiagnosticCode::ResultArity,
        );
    }
    rejects(
        &module(
            vec![private(0), Type::Scalar(ScalarType::U32)],
            vec![Type::INDEX],
            StorageOperationV1::WriteValue {
                address: ValueId(0),
                value: ValueId(1),
                access: memory(),
            },
        ),
        DiagnosticCode::ResultArity,
    );
}

#[test]
fn storage_operation_verifier_local_ids_never_rebind_to_a_foreign_module() {
    let local = module(
        vec![private(0)],
        vec![Type::Scalar(ScalarType::U32)],
        read(),
    );
    let mut foreign = local.clone();
    foreign.storage_layouts[0] = scalar(ScalarType::U64, 8);
    // Remove dependent layouts so the foreign structural graph remains valid.
    foreign.storage_layouts.truncate(1);
    assert_eq!(
        inspect(&local, Some(&foreign)),
        Err(ResourceError::Accounting)
    );
    accepts(&local);
    let mut changed = local.clone();
    changed.storage_layouts[0] = scalar(ScalarType::U64, 8);
    changed.storage_layouts.truncate(1);
    rejects(&changed, DiagnosticCode::TypeMismatch);
}

#[test]
fn storage_operation_verifier_checks_holder_rights_space_and_alignment() {
    let scalar = Type::Scalar(ScalarType::U32);
    for rights in [
        AccessMode::ReadOnly,
        AccessMode::WriteOnly,
        AccessMode::ReadWrite,
    ] {
        let m = module(
            vec![address(0, AddressSpace::Private, rights)],
            vec![scalar.clone()],
            read(),
        );
        if rights == AccessMode::WriteOnly {
            rejects(&m, DiagnosticCode::InvalidMemoryAccess);
        } else {
            accepts(&m);
        }
        let m = module(
            vec![address(0, AddressSpace::Private, rights), scalar.clone()],
            vec![],
            StorageOperationV1::WriteValue {
                address: ValueId(0),
                value: ValueId(1),
                access: memory(),
            },
        );
        if rights == AccessMode::ReadOnly {
            rejects(&m, DiagnosticCode::InvalidMemoryAccess);
        } else {
            accepts(&m);
        }
    }
    for (access, expected) in [
        (
            MemoryAccess::new(AddressSpace::Global, 4),
            DiagnosticCode::InvalidMemoryAccess,
        ),
        (
            MemoryAccess::new(AddressSpace::Private, 0),
            DiagnosticCode::InvalidAlignment,
        ),
        (
            MemoryAccess::new(AddressSpace::Private, 3),
            DiagnosticCode::InvalidAlignment,
        ),
    ] {
        rejects(
            &module(
                vec![private(0)],
                vec![scalar.clone()],
                StorageOperationV1::ReadValue {
                    address: ValueId(0),
                    access,
                },
            ),
            expected,
        );
    }
    rejects(
        &module(
            vec![
                address(0, AddressSpace::Constant, AccessMode::ReadWrite),
                scalar,
            ],
            vec![],
            StorageOperationV1::WriteValue {
                address: ValueId(0),
                value: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Constant, 4),
            },
        ),
        DiagnosticCode::InvalidMemoryAccess,
    );
}

#[test]
fn storage_operation_verifier_loaded_constant_pointer_cannot_grant_write_rights() {
    for rights in [
        AccessMode::ReadOnly,
        AccessMode::WriteOnly,
        AccessMode::ReadWrite,
    ] {
        let mut m = module(
            vec![private(2)],
            vec![Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Constant,
                rights,
            )],
            read(),
        );
        let StorageLayoutKindV1::Pointer(pointer) = &mut m.storage_layouts[2].kind else {
            unreachable!()
        };
        pointer.value_space = AddressSpace::Constant;
        pointer.access = rights;
        // The unused slice row's duplicated component facts must still agree.
        let StorageLayoutKindV1::Slice {
            value_space,
            access,
            ..
        } = &mut m.storage_layouts[4].kind
        else {
            unreachable!()
        };
        *value_space = AddressSpace::Constant;
        *access = rights;
        if rights == AccessMode::ReadOnly {
            accepts(&m);
        } else {
            rejects(&m, DiagnosticCode::TypeMismatch);
        }
    }
}

#[test]
fn storage_operation_verifier_projects_real_record_union_and_slice_children() {
    for (parent, ordinal, child) in [
        (5, 0, 0),
        (5, 1, 2),
        (6, 0, 0),
        (6, 1, 1),
        (4, 0, 2),
        (4, 1, 3),
    ] {
        accepts(&module(
            vec![private(parent)],
            vec![private(child)],
            StorageOperationV1::Project {
                base: ValueId(0),
                step: StorageProjectionV1::Field(ordinal),
            },
        ));
    }
    let mut packed = module(
        vec![private(5)],
        vec![private(1)],
        StorageOperationV1::Project {
            base: ValueId(0),
            step: StorageProjectionV1::Field(1),
        },
    );
    packed.storage_layouts[5] = StorageLayoutV1 {
        size: 9,
        alignment: 1,
        kind: StorageLayoutKindV1::Record(vec![field(0, 0), field(1, 1)].into_boxed_slice()),
    };
    // Replace first child with a one-byte scalar to keep packed fields disjoint.
    packed.storage_layouts[0] = scalar(ScalarType::U8, 1);
    // All dependent row sizes must remain structurally valid.
    packed.storage_layouts[7] = StorageLayoutV1 {
        size: 3,
        alignment: 1,
        kind: StorageLayoutKindV1::Array {
            element: StorageLayoutIdV1(0),
            length: 3,
            stride: 1,
        },
    };
    packed.storage_layouts[9] = StorageLayoutV1 {
        size: 0,
        alignment: 1,
        kind: StorageLayoutKindV1::Array {
            element: StorageLayoutIdV1(0),
            length: 0,
            stride: 1,
        },
    };
    accepts(&packed);
}

#[test]
fn storage_operation_verifier_projection_cannot_change_child_space_or_strengthen_rights() {
    let operation = StorageOperationV1::Project {
        base: ValueId(0),
        step: StorageProjectionV1::Field(0),
    };
    for (from, to, accepted) in [
        (AccessMode::ReadWrite, AccessMode::ReadOnly, true),
        (AccessMode::ReadWrite, AccessMode::WriteOnly, true),
        (AccessMode::ReadOnly, AccessMode::ReadOnly, true),
        (AccessMode::ReadOnly, AccessMode::ReadWrite, false),
        (AccessMode::WriteOnly, AccessMode::ReadOnly, false),
    ] {
        let m = module(
            vec![address(5, AddressSpace::Private, from)],
            vec![address(0, AddressSpace::Private, to)],
            operation,
        );
        if accepted {
            accepts(&m);
        } else {
            rejects(&m, DiagnosticCode::TypeMismatch);
        }
    }
    for wrong in [
        private(1),
        address(0, AddressSpace::Global, AccessMode::ReadWrite),
        Type::INDEX,
    ] {
        rejects(
            &module(vec![private(5)], vec![wrong], operation),
            DiagnosticCode::TypeMismatch,
        );
    }
    rejects(
        &module(vec![private(5)], vec![], operation),
        DiagnosticCode::ResultArity,
    );
    rejects(
        &module(vec![private(5)], vec![private(0), private(0)], operation),
        DiagnosticCode::ResultArity,
    );
    rejects(
        &module(
            vec![private(5)],
            vec![private(0)],
            StorageOperationV1::Project {
                base: ValueId(0),
                step: StorageProjectionV1::Field(99),
            },
        ),
        DiagnosticCode::InvalidMemoryAccess,
    );
}

#[test]
fn storage_operation_verifier_array_index_uses_exact_index_type_and_nonempty_extent() {
    let operation = StorageOperationV1::Project {
        base: ValueId(0),
        step: StorageProjectionV1::ArrayIndex(ValueId(1)),
    };
    accepts(&module(
        vec![private(7), Type::INDEX],
        vec![private(0)],
        operation,
    ));
    rejects(
        &module(
            vec![private(7), Type::Scalar(ScalarType::I32)],
            vec![private(0)],
            operation,
        ),
        DiagnosticCode::TypeMismatch,
    );
    rejects(
        &module(vec![private(9), Type::INDEX], vec![private(0)], operation),
        DiagnosticCode::InvalidMemoryAccess,
    );
    rejects(
        &module(vec![private(5), Type::INDEX], vec![private(0)], operation),
        DiagnosticCode::InvalidMemoryAccess,
    );
}

#[test]
fn storage_operation_verifier_variant_tag_is_a_real_read_not_an_authority_flag() {
    let project = |index, access| StorageOperationV1::Project {
        base: ValueId(0),
        step: StorageProjectionV1::Variant { index, access },
    };
    accepts(&module(
        vec![private(8)],
        vec![private(5)],
        project(0, memory()),
    ));
    let mut niche = module(vec![private(8)], vec![private(5)], project(0, memory()));
    let StorageLayoutKindV1::Variants { encoding, variants } = &mut niche.storage_layouts[8].kind
    else {
        unreachable!()
    };
    *encoding = StorageVariantEncodingV1::Niche {
        tag: field(0, 0),
        untagged_variant: 0,
        first_niche_variant: 1,
        last_niche_variant: 1,
        niche_start: 2,
    };
    for variant in variants {
        variant.direct_tag_bits = None;
    }
    accepts(&niche);
    for (index, access) in [
        (9, memory()),
        (0, MemoryAccess::new(AddressSpace::Global, 1)),
        (0, MemoryAccess::new(AddressSpace::Private, 16)),
    ] {
        rejects(
            &module(vec![private(8)], vec![private(5)], project(index, access)),
            DiagnosticCode::InvalidMemoryAccess,
        );
    }
    rejects(
        &module(
            vec![address(8, AddressSpace::Private, AccessMode::WriteOnly)],
            vec![address(5, AddressSpace::Private, AccessMode::WriteOnly)],
            project(0, memory()),
        ),
        DiagnosticCode::InvalidMemoryAccess,
    );
    let mut m = module(vec![private(8)], vec![private(5)], project(0, memory()));
    let StorageLayoutKindV1::Variants { variants, .. } = &mut m.storage_layouts[8].kind else {
        unreachable!()
    };
    variants[0].uninhabited = true;
    rejects(&m, DiagnosticCode::InvalidMemoryAccess);
}

#[test]
fn storage_operation_verifier_copy_requires_same_local_layout_and_two_valid_accesses() {
    let operation = |overlap| StorageOperationV1::CopyObject {
        source: ValueId(0),
        destination: ValueId(1),
        source_access: MemoryAccess::new(AddressSpace::Global, 1),
        destination_access: memory(),
        overlap,
    };
    for overlap in [
        StorageCopyOverlapV1::MayOverlap,
        StorageCopyOverlapV1::NonOverlapping,
    ] {
        let mut coincident = module(
            vec![
                address(0, AddressSpace::Global, AccessMode::ReadOnly),
                private(10),
            ],
            vec![],
            operation(overlap),
        );
        coincident.storage_layouts.push(scalar(ScalarType::U32, 4));
        rejects(&coincident, DiagnosticCode::TypeMismatch);
        for layout in [0, 2, 4, 5, 6, 7, 8, 9] {
            accepts(&module(
                vec![
                    address(layout, AddressSpace::Global, AccessMode::ReadOnly),
                    private(layout),
                ],
                vec![],
                operation(overlap),
            ));
        }
        rejects(
            &module(
                vec![
                    address(5, AddressSpace::Global, AccessMode::ReadOnly),
                    private(6),
                ],
                vec![],
                operation(overlap),
            ),
            DiagnosticCode::TypeMismatch,
        );
        rejects(
            &module(
                vec![
                    address(5, AddressSpace::Global, AccessMode::WriteOnly),
                    private(5),
                ],
                vec![],
                operation(overlap),
            ),
            DiagnosticCode::InvalidMemoryAccess,
        );
        rejects(
            &module(
                vec![
                    address(5, AddressSpace::Global, AccessMode::ReadOnly),
                    address(5, AddressSpace::Private, AccessMode::ReadOnly),
                ],
                vec![],
                operation(overlap),
            ),
            DiagnosticCode::InvalidMemoryAccess,
        );
        rejects(
            &module(
                vec![
                    address(5, AddressSpace::Global, AccessMode::ReadOnly),
                    private(5),
                ],
                vec![Type::INDEX],
                operation(overlap),
            ),
            DiagnosticCode::ResultArity,
        );
    }
}

#[test]
fn storage_operation_verifier_pointer_recursion_terminates_at_the_exact_object_id() {
    let ty = Type::pointer(
        Type::StorageObject(StorageLayoutIdV1(0)),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    let mut m = module(vec![private(0)], vec![ty], read());
    m.storage_layouts = vec![StorageLayoutV1 {
        size: 8,
        alignment: 8,
        kind: StorageLayoutKindV1::Pointer(StoragePointerV1 {
            pointee: StorageLayoutIdV1(0),
            value_space: AddressSpace::Global,
            encoded_space: AddressSpace::Generic,
            access: AccessMode::ReadOnly,
            stored_bits: 64,
        }),
    }];
    accepts(&m);
    m.functions[0].body.as_mut().unwrap().blocks[0].operations[0].results[0].ty = Type::pointer(
        Type::StorageObject(StorageLayoutIdV1(1)),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    rejects(&m, DiagnosticCode::TypeMismatch);
}

#[test]
fn storage_operation_verifier_value_match_work_cutoffs_are_source_derived() {
    let pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    let m = module(
        vec![private(2), pointer],
        vec![],
        StorageOperationV1::WriteValue {
            address: ValueId(0),
            value: ValueId(1),
            access: memory(),
        },
    );
    let mut setup_work = Work::new(1_000_000);
    let mut setup = Budget::new(&mut setup_work, 1_000_000);
    let storage = check_module_storage_v1(&m, LIMITS, &mut setup).unwrap();
    assert_eq!(setup.storage(), 0);
    let ty = &storage.module().functions[0].signature.parameters[1];
    // Two finite Type/layout nodes, each prepaid by one indivisible debit of 2.
    for limit in 0..=4 {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, FLOOR);
        budget.reserve_storage(FLOOR).unwrap();
        let result = storage_value_type_matches_v1(&storage, StorageLayoutIdV1(2), ty, &mut budget);
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.peak_storage(), FLOOR);
        match limit {
            0 | 1 => {
                assert!(matches!(result, Err(ResourceError::Work(error)) if error.actual() == 2));
                assert_eq!(budget.work(), 0);
            }
            2 | 3 => {
                assert!(matches!(result, Err(ResourceError::Work(error)) if error.actual() == 4));
                assert_eq!(budget.work(), 2);
            }
            _ => {
                assert_eq!(result, Ok(true));
                assert_eq!(budget.work(), 4);
            }
        }
    }
}

#[test]
fn storage_operation_verifier_real_module_dispatch_and_legacy_load_refusal() {
    let m = module(
        vec![private(0)],
        vec![Type::Scalar(ScalarType::U32)],
        read(),
    );
    assert!(crate::verify_module_ref(&m).is_err());
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    budget.reserve_storage(FLOOR).unwrap();
    let checked = check_module_storage_v1(&m, LIMITS, &mut budget).unwrap();
    let verified =
        crate::verify_storage_module_ref_with_budget_v1(checked, None, &mut budget).unwrap();
    assert!(std::ptr::eq(verified.module(), &m));
    assert_eq!(budget.storage(), FLOOR);
    let mut legacy = m.clone();
    legacy.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind = OperationKind::Load {
        pointer: ValueId(0),
        access: memory(),
    };
    assert!(crate::verify_module_ref(&legacy).is_err());
    let checked = check_module_storage_v1(&legacy, LIMITS, &mut budget).unwrap();
    assert!(crate::verify_storage_module_ref_with_budget_v1(checked, None, &mut budget).is_err());
    assert_eq!(budget.storage(), FLOOR);
    let mut raw = m.clone();
    raw.storage_layouts.clear();
    assert!(crate::verify_module_ref(&raw).is_err());
}

#[path = "verification_storage_initialization_v1_tests.rs"]
mod initialization;
