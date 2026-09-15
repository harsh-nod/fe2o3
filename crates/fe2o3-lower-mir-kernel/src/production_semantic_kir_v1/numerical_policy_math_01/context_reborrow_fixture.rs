include!("context_workgroup_fixture.rs");

// Synthetic component source. The original getter/Bind/root records are reused;
// this is not a source-compiler receipt or AMD qualification fixture.
pub(super) fn context_reborrow_source(
    nested: bool,
    kill_math: bool,
    forged_bind: bool,
) -> AdmittedInertSemanticMirV1 {
    let base = captured_source(true, kill_math, forged_bind, false);
    let mut types = base.types().to_vec();
    assert_eq!(types.len(), 13);
    types.push(
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([222; 32]),
            SemanticLayoutIdentityV1::from_sha256([222; 32]),
            types[2].layout().clone(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    ty(1),
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Mutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        )
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false)
                .with_rustc_layout_is_noundef(true)
                .with_scalar_pointee_info(
                    Some(
                        SemanticAbiPointeeInfoV1::new(
                            SemanticAbiPointeeKindV1::MutableReference { unpin: true },
                            0,
                            1,
                        )
                        .unwrap(),
                    ),
                    None,
                ),
        ),
    );
    let workgroup = append_context_workgroup_fixture(&mut types, &base);
    let root = &base.functions()[0];
    let mut locals = root.locals().to_vec();
    assert_eq!(locals.len(), 15);
    for index in 15..19 {
        locals.push(local(180 + index, 13, SemanticLocalRoleV1::Temporary));
    }
    locals.push(local(199, 17, SemanticLocalRoleV1::Temporary));
    let deref = |local| {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(local),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ty(1)).unwrap()],
            ty(1),
        )
        .unwrap()
    };
    let borrow = |local, reference, kind, source| {
        SemanticStatementV1::new(
            location(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(local, reference),
                SemanticRvalueV1::new(
                    ty(reference),
                    SemanticRvalueKindV1::Borrow {
                        kind,
                        place: source,
                    },
                ),
            )),
        )
    };
    let transfer = |destination, source| {
        SemanticStatementV1::new(
            location(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(destination, 13),
                SemanticRvalueV1::new(
                    ty(13),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(source, 13))),
                ),
            )),
        )
    };
    let mut statements = vec![
        borrow(15, 13, SemanticBorrowKindV1::Mutable, place(1, 1)),
        transfer(16, 15),
    ];
    let receiver = if nested {
        statements.push(borrow(17, 13, SemanticBorrowKindV1::Mutable, deref(16)));
        statements.push(transfer(18, 17));
        18
    } else {
        16
    };
    statements.push(borrow(2, 2, SemanticBorrowKindV1::Shared, deref(receiver)));
    let mut blocks = root.blocks().to_vec();
    blocks[1] = block(181, statements, blocks[1].terminator().kind().clone());
    // All shared Math receiver uses complete before the exclusive Workgroup use.
    blocks[5] = block(185, vec![], call(8,
        vec![SemanticOperandV1::Move(place(receiver, 13))], 19, 17, 7));
    blocks.push(block(187, vec![], SemanticTerminatorKindV1::Return));
    let mut callables = base.callables().to_vec();
    assert_eq!(callables.len(), 8);
    callables.push(workgroup);
    let mut functions = base.functions().to_vec();
    functions[0] = function(135, root.abi().clone(), locals, blocks, true)
        .with_kernel_entry(root.kernel_entry().unwrap().clone());
    InertSemanticMirRequestV1::new_with_callables(
        base.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        base.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v21(SemanticMirLimitsV1::default())
    .unwrap()
}
