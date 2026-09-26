use super::super::storage_native_v18::tests::*;
use super::*;

#[test]
fn pointer_field_uses_flat_word_not_private_holder_width() {
    let rows = vec![
        scalar(),
        Row {
            size: 8,
            alignment: 8,
            kind: Kind::Pointer(StoragePointerV1 {
                pointee: Id(0),
                value_space: Space::Private,
                encoded_space: Space::Generic,
                access: AccessMode::ReadWrite,
                stored_bits: 64,
            }),
        },
    ];
    let module = module(
        rows,
        vec![
            allocate(0, 0, Space::Private, 4),
            allocate(1, 1, Space::Private, 8),
            write(1, 0, Space::Private, 8),
            read(2, 1, address(0, Space::Private), Space::Private, 8),
        ],
        Profile::Gfx942,
    );
    let text = emit(&module, Profile::Gfx942);
    assert!(text.contains("addrspacecast ptr addrspace(5) %v0 to ptr"));
    assert!(text.contains("store ptr addrspace(0)"));
    assert!(text.contains("load ptr addrspace(0), ptr addrspace(5) %v1, align 8"));
    assert!(text.contains(" to ptr addrspace(5)"));
    assert!(!text.contains("ptrtoint"));
    assert!(!text.contains("inttoptr"));
}

#[test]
fn all_pointer_spaces_preserve_identity_or_exact_flat_conversion_in_helpers() {
    for space in [
        Space::Private,
        Space::Workgroup,
        Space::Global,
        Space::Constant,
        Space::Generic,
    ] {
        let access = if space == Space::Constant {
            AccessMode::ReadOnly
        } else {
            AccessMode::ReadWrite
        };
        let pointer = Type::pointer(object(0), space, access);
        let rows = vec![
            scalar(),
            Row {
                size: 8,
                alignment: 8,
                kind: Kind::Pointer(StoragePointerV1 {
                    pointee: Id(0),
                    value_space: space,
                    encoded_space: Space::Generic,
                    access,
                    stored_bits: 64,
                }),
            },
        ];
        let mut module = module(rows, vec![], Profile::Gfx950);
        let mut block = BasicBlock::new(BlockId(0));
        block.operations = vec![
            allocate(1, 1, Space::Private, 8),
            write(1, 0, Space::Private, 8),
            read(2, 1, pointer.clone(), Space::Private, 8),
        ];
        block.terminator = Some(Terminator::Return {
            values: vec![ValueId(2)],
        });
        let mut helper = Function::definition(
            "roundtrip",
            Signature::new(vec![pointer.clone()], vec![pointer]),
            vec![ValueId(0)],
            vec![block],
        );
        helper.role = FunctionRole::InternalHelper;
        module.functions.push(helper);
        let text = emit(&module, Profile::Gfx950);
        assert!(text.contains("store ptr addrspace(0)"));
        assert!(text.contains(&format!("ret ptr addrspace({}) %v2", space_v18(space))));
        assert_eq!(
            text.matches(" = addrspacecast ").count(),
            if space == Space::Generic { 0 } else { 2 }
        );
    }
}

#[test]
fn padded_interleaved_vector_access_never_reads_or_initializes_padding() {
    let vector = fe2o3_kernel_ir::FixedVectorTypeV12::new(
        ScalarType::U32,
        6,
        VectorLayoutV12::Interleaved { factor: 2 },
    );
    let mut module = module(
        vec![Row {
            size: 32,
            alignment: 16,
            kind: Kind::Vector(vector),
        }],
        vec![
            read(1, 0, Type::Vector(vector), Space::Global, 16),
            write(0, 1, Space::Global, 16),
        ],
        Profile::Gfx942,
    );
    module.functions[0]
        .signature
        .parameters
        .push(address(0, Space::Global));
    module.functions[0]
        .body
        .as_mut()
        .unwrap()
        .parameters
        .push(ValueId(0));
    let text = emit(&module, Profile::Gfx942);
    assert_eq!(text.matches(" = load i32,").count(), 6);
    assert_eq!(text.matches("  store i32 ").count(), 6);
    assert!(text.contains(".lane1.address = getelementptr i8, ptr addrspace(1) %arg0, i64 12"));
    assert!(text.contains(".lane2.address = getelementptr i8, ptr addrspace(1) %arg0, i64 4"));
    assert!(!text.contains("%arg0, i64 24"));
    assert!(!text.contains("%arg0, i64 28"));
}

#[test]
fn bool_values_use_bytes_without_using_bool_ssa_for_tag_patterns() {
    let module = module(
        vec![Row {
            size: 1,
            alignment: 1,
            kind: Kind::Scalar(ScalarType::Bool),
        }],
        vec![
            allocate(0, 0, Space::Private, 1),
            value(
                1,
                Type::Scalar(ScalarType::Bool),
                OperationKind::Constant(Constant::Bool(true)),
            ),
            write(0, 1, Space::Private, 1),
            read(2, 0, Type::Scalar(ScalarType::Bool), Space::Private, 1),
        ],
        Profile::Gfx942,
    );
    let text = emit(&module, Profile::Gfx942);
    assert!(text.contains("zext i1 true to i8"));
    assert!(text.contains("store i8 "));
    assert!(text.contains("load i8, ptr addrspace(5) %v0, align 1"));
    assert!(text.contains("%v2 = trunc i8 "));
}

#[test]
fn wrong_pointer_word_width_is_rejected_even_when_unused() {
    let rows = vec![
        scalar(),
        Row {
            size: 4,
            alignment: 4,
            kind: Kind::Pointer(StoragePointerV1 {
                pointee: Id(0),
                value_space: Space::Global,
                encoded_space: Space::Generic,
                access: AccessMode::ReadOnly,
                stored_bits: 32,
            }),
        },
    ];
    let module = module(rows, vec![], Profile::Gfx942);
    with_owner(&module, |owner, budget| {
        assert!(lower_canonical_storage_module_v18(owner, Profile::Gfx942, budget).is_err())
    });
}

#[test]
fn slice_components_use_source_offsets_and_helper_return_abi() {
    let slice = Type::slice(object(0), Space::Global, AccessMode::ReadOnly);
    let rows = vec![
        scalar(),
        Row {
            size: 8,
            alignment: 8,
            kind: Kind::Pointer(StoragePointerV1 {
                pointee: Id(0),
                value_space: Space::Global,
                encoded_space: Space::Global,
                access: AccessMode::ReadOnly,
                stored_bits: 64,
            }),
        },
        Row {
            size: 8,
            alignment: 8,
            kind: Kind::Scalar(ScalarType::Index),
        },
        Row {
            size: 24,
            alignment: 8,
            kind: Kind::Slice {
                element: Id(0),
                value_space: Space::Global,
                access: AccessMode::ReadOnly,
                data: Field {
                    offset: 8,
                    layout: Id(1),
                },
                length: Field {
                    offset: 0,
                    layout: Id(2),
                },
            },
        },
    ];
    let mut module = module(rows, vec![], Profile::Gfx942);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        allocate(1, 3, Space::Private, 8),
        write(1, 0, Space::Private, 8),
        read(2, 1, slice.clone(), Space::Private, 8),
    ];
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(2)],
    });
    let mut helper = Function::definition(
        "slice_roundtrip",
        Signature::new(vec![slice.clone()], vec![slice]),
        vec![ValueId(0)],
        vec![block],
    );
    helper.role = FunctionRole::InternalHelper;
    module.functions.push(helper);
    let text = emit(&module, Profile::Gfx942);
    assert!(text.contains("define internal { ptr addrspace(1), i64 } @slice_roundtrip(ptr addrspace(1) %arg0.data, i64 %arg0.len)"));
    assert!(text.contains(".data.address = getelementptr i8, ptr addrspace(5) %v1, i64 8"));
    assert!(text.contains(".length.address = getelementptr i8, ptr addrspace(5) %v1, i64 0"));
    assert!(text.contains("ret { ptr addrspace(1), i64 } %storage.return.0.slice"));
}

#[test]
fn slice_helper_call_roundtrip_uses_the_real_aggregate_result() {
    let slice = Type::slice(object(0), Space::Global, AccessMode::ReadOnly);
    let mut module = module(
        vec![scalar()],
        vec![value(
            1,
            slice.clone(),
            OperationKind::Call {
                callee: FunctionId::new("identity"),
                arguments: vec![ValueId(0)],
            },
        )],
        Profile::Gfx942,
    );
    module.functions[0].signature.parameters.push(slice.clone());
    module.functions[0]
        .body
        .as_mut()
        .unwrap()
        .parameters
        .push(ValueId(0));
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(0)],
    });
    module.functions.push(Function::definition(
        "identity",
        Signature::new(vec![slice.clone()], vec![slice]),
        vec![ValueId(0)],
        vec![block],
    ));
    let text = emit(&module, Profile::Gfx942);
    assert!(text.contains(
        "call { ptr addrspace(1), i64 } @identity(ptr addrspace(1) %arg0.data, i64 %arg0.len)"
    ));
    assert!(
        text.contains("%v1.data = extractvalue { ptr addrspace(1), i64 } %storage.bb0.op0.call, 0")
    );
    assert!(
        text.contains("%v1.len = extractvalue { ptr addrspace(1), i64 } %storage.bb0.op0.call, 1")
    );
}
