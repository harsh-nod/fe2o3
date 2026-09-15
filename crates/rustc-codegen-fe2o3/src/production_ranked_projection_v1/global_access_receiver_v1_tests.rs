fn receiver_reuse_fixture_v1() -> (
    Vec<SemanticTypeDeclV1>,
    Vec<SemanticCallableDeclV1>,
    SemanticFunctionDeclV1,
) {
    let (types, callables, function) = blocked_global_projection_fixture_v1();
    let mut blocks = function.blocks().to_vec();
    // A failed checked-index conversion still exits before either store.
    blocks[5] = block(
        165,
        blocks[5].statements().to_vec(),
        zero_switch(23, U64_TYPE, 10, 6),
    );
    let original = blocked_global_fixture_store(&function);
    let mut arguments = original.arguments().to_vec();
    // Reproduce repeated nonescaping Copy uses of one borrowed helper parameter.
    arguments[0] = typed_operand(20, function.locals()[20].ty());
    blocks[8] = block(
        197,
        blocks[8].statements().to_vec(),
        blocked_global_fixture_call(8, arguments.clone(), 21, BOOL_TYPE, 9),
    );
    arguments[2] = typed_constant(U64_TYPE, 3, 8);
    arguments[3] = typed_constant(U64_TYPE, 888, 8);
    blocks[9] = block(
        198,
        vec![],
        blocked_global_fixture_call(8, arguments, 21, BOOL_TYPE, 10),
    );
    blocks.push(block(199, vec![], SemanticTerminatorKindV1::Return));
    let function = typed_global_fixture_with_body_v1(&function, function.locals().to_vec(), blocks);
    (types, callables, function)
}

fn receiver_reuse_call_v1(
    function: &SemanticFunctionDeclV1,
    block: usize,
) -> &SemanticDirectCallV1 {
    let SemanticTerminatorKindV1::Call(call) = function.blocks()[block].terminator().kind() else {
        panic!("source store call");
    };
    call
}

fn receiver_reuse_error_v1(
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    function: &SemanticFunctionDeclV1,
    block: usize,
    reason: &'static str,
) {
    let error = project_capability_index_fixture(types, callables, function)
        .err()
        .expect("mutated receiver must be rejected");
    assert!(
        matches!(error, ProductionRankedProjectionErrorV1::GlobalAccess {
        block: actual, receiver: Some(20), reason: detail, ..
    } if actual == block && detail == reason),
        "{error:?}"
    );
}

#[test]
fn global_receiver_reuse_retains_exact_source_effects_indices_rhs_and_extent() {
    let (types, callables, function) = receiver_reuse_fixture_v1();
    let (projection, blocks, sources) =
        typed_global_ranked_source_fixture_v1(&types, &callables, &function);
    let first = projection.direct_write_effects[8].as_ref().unwrap();
    let second = projection.direct_write_effects[9].as_ref().unwrap();
    assert_eq!(first.view, second.view);
    assert_eq!(first.comparisons[0].1, second.comparisons[0].1);
    assert_ne!(first.indices, second.indices);
    let writes = projected_reference_gpu_writes_v2(
        &types,
        &callables,
        &function,
        &projection,
        &blocks,
        &sources,
    )
    .unwrap();
    assert_eq!(writes.len(), 2);
    for (write, expected) in writes.iter().zip([777, 888]) {
        assert_eq!(write.allocation_origin, 2);
        assert!(matches!(&write.value,
            Ok(ProductionSemanticExpressionV2::Constant { bits, .. }) if *bits == expected));
    }
    let sites: Vec<_> = sources
        .iter()
        .filter(|source| source.access == AccessKindAttr::Write)
        .map(|source| source.semantic_site.unwrap())
        .collect();
    assert_eq!(
        sites,
        [
            ProjectedSemanticAccessSiteV1 {
                block: 8,
                statement: None
            },
            ProjectedSemanticAccessSiteV1 {
                block: 9,
                statement: None
            },
        ]
    );
}

#[test]
fn global_receiver_reuse_move_still_consumes_the_borrow() {
    let (types, callables, function) = receiver_reuse_fixture_v1();
    let mut arguments = receiver_reuse_call_v1(&function, 8).arguments().to_vec();
    arguments[0] = SemanticOperandV1::Move(typed_place(20, function.locals()[20].ty()));
    let changed = blocked_global_changed_store(&function, arguments);
    assert!(!global_access_retains_receiver_v1(
        &types,
        &callables[8],
        receiver_reuse_call_v1(&changed, 8)
    ));
    receiver_reuse_error_v1(
        &types,
        &callables,
        &changed,
        9,
        "receiver binding is absent",
    );
}

#[test]
fn global_receiver_reuse_storage_death_and_deinitialization_stay_invalid() {
    let (types, callables, function) = receiver_reuse_fixture_v1();
    for (kind, reason) in [
        (
            SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(20)),
            "receiver binding is absent",
        ),
        (
            SemanticStatementKindV1::Deinitialize(typed_place(20, function.locals()[20].ty())),
            "receiver binding was invalidated",
        ),
    ] {
        let mut blocks = function.blocks().to_vec();
        blocks[9] = block(
            198,
            vec![SemanticStatementV1::new(
                SemanticSourceProvenanceV1::unavailable(),
                kind,
            )],
            blocks[9].terminator().kind().clone(),
        );
        let changed =
            typed_global_fixture_with_body_v1(&function, function.locals().to_vec(), blocks);
        receiver_reuse_error_v1(&types, &callables, &changed, 9, reason);
    }
}

#[test]
fn global_receiver_reuse_does_not_preserve_generic_mutable_copy() {
    let (types, callables, function) = receiver_reuse_fixture_v1();
    let ty = function.locals()[20].ty();
    let mut locals = function.locals().to_vec();
    let copied = locals.len() as u32;
    locals.push(local(239, ty, SemanticLocalRoleV1::Temporary));
    let mut blocks = function.blocks().to_vec();
    blocks[9] = block(
        198,
        vec![typed_assignment(
            copied,
            ty,
            SemanticRvalueKindV1::Use(typed_operand(20, ty)),
        )],
        blocks[9].terminator().kind().clone(),
    );
    let changed = typed_global_fixture_with_body_v1(&function, locals, blocks);
    receiver_reuse_error_v1(
        &types,
        &callables,
        &changed,
        9,
        "receiver binding was invalidated",
    );
}

#[test]
fn global_receiver_reuse_rejects_foreign_root_contract_and_source_identity() {
    let (types, callables, function) = receiver_reuse_fixture_v1();
    for mutation in 0..3 {
        let mut changed_callables = callables.clone();
        let mut second = callables[8].clone();
        let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = &mut second else {
            unreachable!()
        };
        let SemanticCompilerIntrinsicOperationV1::CapabilityGlobalStoreBlock {
            provenance,
            contract,
            source_identity,
            ..
        } = operation
        else {
            unreachable!()
        };
        let reason = match mutation {
            0 => {
                *provenance = capability_index_provenance(Some(0));
                "root provenance differs"
            }
            1 => {
                *contract = SemanticCapabilityMemoryContractV1::global_disjoint_write(
                    CAP_INDEX_CONTEXT,
                    SemanticDisjointIndexSpaceV1::Index1d,
                );
                "memory contract differs"
            }
            _ => {
                *source_identity = SemanticFunctionIdentityV1::from_sha256(bytes(249));
                "intrinsic source identity differs"
            }
        };
        let callee = changed_callables.len() as u32;
        changed_callables.push(second);
        let mut blocks = function.blocks().to_vec();
        blocks[9] = block(
            198,
            vec![],
            blocked_global_fixture_call(
                callee,
                receiver_reuse_call_v1(&function, 9).arguments().to_vec(),
                21,
                BOOL_TYPE,
                10,
            ),
        );
        let changed =
            typed_global_fixture_with_body_v1(&function, function.locals().to_vec(), blocks);
        receiver_reuse_error_v1(&types, &changed_callables, &changed, 9, reason);
    }
}

#[test]
fn global_receiver_reuse_shared_reborrow_cannot_write() {
    let (types, callables, function) = receiver_reuse_fixture_v1();
    let ty = function.locals()[20].ty();
    let mut blocks = function.blocks().to_vec();
    let pointee = function.locals()[14].ty();
    let dereferenced = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(20),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, pointee).unwrap()],
        pointee,
    )
    .unwrap();
    blocks[9] = block(
        198,
        vec![typed_assignment(
            20,
            ty,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: dereferenced,
            },
        )],
        blocks[9].terminator().kind().clone(),
    );
    let changed = typed_global_fixture_with_body_v1(&function, function.locals().to_vec(), blocks);
    receiver_reuse_error_v1(
        &types,
        &callables,
        &changed,
        9,
        "receiver borrow kind differs",
    );
}

#[test]
fn global_receiver_reuse_meet_does_not_restore_dead_path_fact() {
    let (types, callables, function) = receiver_reuse_fixture_v1();
    let mut blocks = function.blocks()[..9].to_vec();
    blocks[5] = block(
        165,
        blocks[5].statements().to_vec(),
        zero_switch(23, U64_TYPE, 13, 6),
    );
    blocks.push(block(198, vec![], zero_switch(21, BOOL_TYPE, 10, 11)));
    blocks.push(block(
        199,
        vec![SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(20)),
        )],
        SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 12)),
    ));
    blocks.push(block(
        200,
        vec![],
        SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 12)),
    ));
    blocks.push(block(
        201,
        vec![],
        blocked_global_fixture_call(
            8,
            receiver_reuse_call_v1(&function, 9).arguments().to_vec(),
            21,
            BOOL_TYPE,
            13,
        ),
    ));
    blocks.push(block(202, vec![], SemanticTerminatorKindV1::Return));
    let changed = typed_global_fixture_with_body_v1(&function, function.locals().to_vec(), blocks);
    receiver_reuse_error_v1(
        &types,
        &callables,
        &changed,
        12,
        "receiver binding is absent",
    );
}

#[test]
fn global_receiver_reuse_requires_closed_abi_and_nonescaping_result() {
    let (types, callables, function) = receiver_reuse_fixture_v1();
    let call = receiver_reuse_call_v1(&function, 8);
    let SemanticCallableDeclV1::CompilerIntrinsic {
        binding, operation, ..
    } = &callables[8]
    else {
        unreachable!()
    };
    assert!(global_access_retains_receiver_v1(
        &types,
        &callables[8],
        call
    ));
    let inputs: Vec<_> = binding
        .abi()
        .source_input_types()
        .iter()
        .copied()
        .zip(binding.abi().source_argument_ownership().iter().copied())
        .collect();
    for mutation in 0..4 {
        let mut changed = inputs.clone();
        let mut output = BOOL_TYPE;
        match mutation {
            0 => changed[0].1 = SemanticSourceArgumentOwnershipV1::ByValue,
            1 => changed[1].1 = SemanticSourceArgumentOwnershipV1::UniqueBorrow,
            2 => changed[0].0 = function.locals()[14].ty(),
            _ => output = inputs[0].0,
        }
        let callable = capability_index_callable(operation.clone(), &changed, output);
        assert!(
            !global_access_retains_receiver_v1(&types, &callable, call),
            "mutation {mutation}"
        );
    }
    let read = receiver_reuse_call_v1(&function, 7);
    assert!(!global_access_retains_receiver_v1(
        &types,
        &callables[read.callee().index() as usize],
        read
    ));
}
