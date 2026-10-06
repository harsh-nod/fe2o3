fn global_native_rebuild_owner_v18(
    base: &ProductionSemanticSsaOwnerV1,
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
) -> ProductionSemanticSsaOwnerV1 {
    let source = base.source_semantic();
    assert_eq!(source.functions().len(), 1);
    let original = &source.functions()[0];
    let root = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        locals,
        original.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        source.types().to_vec(),
        source.allocations().to_vec(),
        source.statics().to_vec(),
        source.vtables().to_vec(),
        vec![root],
        source.callables().to_vec(),
        source.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn global_native_cross_descriptor_copy_owner_v18() -> ProductionSemanticSsaOwnerV1 {
    let base = source_issued_pointer_source_tests_v29::owner_with_shape_uncaptured(2, 0);
    let original = &base.source_semantic().functions()[0];
    assert_eq!(original.blocks().len(), 9);
    assert_eq!(original.locals().len(), 10);
    let scalar = SemanticTypeIdV1::from_index(1);
    let pointer = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(7),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, scalar).unwrap()],
        scalar,
    )
    .unwrap();
    let mut locals = original.locals().to_vec();
    let value = SemanticLocalIdV1::from_index(u32::try_from(locals.len()).unwrap());
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([90; 32]),
        scalar,
        SemanticLocalRoleV1::Temporary,
        SemanticSourceProvenanceV1::unavailable(),
    ));
    let value_place = SemanticPlaceV1::new(value, vec![], scalar).unwrap();
    let mut blocks = original.blocks().to_vec();
    let mut read = blocks[3].statements().to_vec();
    assert_eq!(read.len(), 1, "first issuer payload extraction");
    read.push(assign(
        value_place.clone(),
        SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
            pointer.clone(),
            SemanticVolatilityV1::NonVolatile,
            None,
        )),
    ));
    blocks[3] = SemanticBasicBlockV1::new(
        blocks[3].identity(),
        blocks[3].source(),
        read,
        blocks[3].terminator().clone(),
    )
    .unwrap();
    // A missing first element must exit, not reach a use of its absent value.
    let SemanticTerminatorKindV1::SwitchInt {
        discriminant,
        targets,
    } = blocks[2].terminator().kind()
    else {
        panic!("first issuer guard");
    };
    let guard = SemanticTerminatorV1::new(
        blocks[2].terminator().source(),
        SemanticTerminatorKindV1::SwitchInt {
            discriminant: discriminant.clone(),
            targets: SemanticSwitchTargetsV1::new(
                targets.values().to_vec(),
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::SwitchOtherwise,
                    SemanticBlockIdV1::from_index(8),
                ),
            )
            .unwrap(),
        },
    );
    blocks[2] = SemanticBasicBlockV1::new(
        blocks[2].identity(),
        blocks[2].source(),
        blocks[2].statements().to_vec(),
        guard,
    )
    .unwrap();
    let mut borrow = blocks[4].statements().to_vec();
    assert_eq!(borrow.len(), 1);
    let SemanticStatementKindV1::Assign(assignment) = borrow[0].kind() else {
        panic!("second original receiver borrow");
    };
    let SemanticRvalueKindV1::Borrow {
        kind,
        place: receiver,
    } = assignment.value().kind()
    else {
        panic!("second receiver is borrowed");
    };
    assert_eq!(receiver.local(), SemanticLocalIdV1::from_index(1));
    borrow[0] = assign(
        assignment.destination().clone(),
        SemanticRvalueKindV1::Borrow {
            kind: *kind,
            place: SemanticPlaceV1::new(SemanticLocalIdV1::from_index(2), vec![], receiver.ty())
                .unwrap(),
        },
    );
    blocks[4] = SemanticBasicBlockV1::new(
        blocks[4].identity(),
        blocks[4].source(),
        borrow,
        blocks[4].terminator().clone(),
    )
    .unwrap();
    let mut write = blocks[7].statements().to_vec();
    assert_eq!(write.len(), 1, "second issuer payload extraction");
    write.push(SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            pointer,
            SemanticOperandV1::Copy(value_place),
            SemanticVolatilityV1::NonVolatile,
            None,
        )),
    ));
    blocks[7] = SemanticBasicBlockV1::new(
        blocks[7].identity(),
        blocks[7].source(),
        write,
        blocks[7].terminator().clone(),
    )
    .unwrap();
    global_native_rebuild_owner_v18(&base, locals, blocks)
}

fn global_native_two_index_shared_owner_v18() -> ProductionSemanticSsaOwnerV1 {
    let base = descriptor_source_owner(DescriptorCase::READ);
    let original = &base.source_semantic().functions()[0];
    assert_eq!(original.blocks().len(), 2);
    assert_eq!(original.locals().len(), 7);
    let word = original.locals()[3].ty();
    let boolean = original.locals()[5].ty();
    let mut locals = original.locals().to_vec();
    locals.push(local(209, word, SemanticLocalRoleV1::Temporary));
    let zero = SemanticOperandV1::Constant(SemanticConstantV1::new(
        word,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(0, 8).unwrap()),
    ));
    let mut blocks = original.blocks().to_vec();
    blocks[1] = SemanticBasicBlockV1::new(
        blocks[1].identity(),
        blocks[1].source(),
        blocks[1].statements().to_vec(),
        SemanticTerminatorV1::new(
            blocks[1].terminator().source(),
            SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::Goto,
                SemanticBlockIdV1::from_index(2),
            )),
        ),
    )
    .unwrap();
    blocks.push(block(
        213,
        vec![
            assign(place(7, word), SemanticRvalueKindV1::Use(zero)),
            assign(
                place(5, boolean),
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::LessThan,
                    left: SemanticOperandV1::Copy(place(7, word)),
                    right: SemanticOperandV1::Copy(place(4, word)),
                },
            ),
        ],
        SemanticTerminatorKindV1::Assert {
            condition: SemanticOperandV1::Copy(place(5, boolean)),
            expected: true,
            message: SemanticAssertMessageV1::BoundsCheck {
                length: SemanticOperandV1::Copy(place(4, word)),
                index: SemanticOperandV1::Copy(place(7, word)),
            },
            target: SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::AssertSuccess,
                SemanticBlockIdV1::from_index(3),
            ),
            unwind: SemanticUnwindActionV1::Unreachable,
        },
    ));
    let SemanticStatementKindV1::Assign(first_read) = original.blocks()[1].statements()[0].kind()
    else {
        panic!("original Shared read");
    };
    let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(first_place)) = first_read.value().kind()
    else {
        panic!("ordinary original read operand");
    };
    assert_eq!(first_place.projections().len(), 2);
    let mut projection = first_place.projections().to_vec();
    projection[1] = SemanticProjectionV1::new(
        SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(7)),
        first_place.ty(),
    )
    .unwrap();
    let second = SemanticPlaceV1::new(first_place.local(), projection, first_place.ty()).unwrap();
    blocks.push(block(
        214,
        vec![assign(
            first_read.destination().clone(),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(second)),
        )],
        SemanticTerminatorKindV1::Return,
    ));
    global_native_rebuild_owner_v18(&base, locals, blocks)
}

fn run_global_native_join_source_v18(
    entrance: DescriptorRoleEntranceV18,
    mode: DescriptorRoleSourceV18,
    consume: impl for<'scope, 'work> FnOnce(
        &ProductionSourceCorrespondenceV18<'scope>,
        &ProductionOptimizedSourceCorrespondenceV18<'scope>,
        &mut ArgumentBudgetV1<'work>,
    ) -> SourceOwnedResultV18<()>,
) -> (SourceOwnedResultV18<()>, usize, usize) {
    if matches!(entrance, DescriptorRoleEntranceV18::IssuedDisjointSlice)
        && matches!(mode, DescriptorRoleSourceV18::ReadValue)
    {
        let owner = global_native_cross_descriptor_copy_owner_v18();
        let abi = issued_descriptor_role_abi_v18(&owner);
        run_descriptor_role_owner_with_abi_v18(
            owner,
            abi,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            consume,
        )
    } else {
        run_descriptor_roles_v18(
            entrance,
            mode,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            consume,
        )
    }
}
