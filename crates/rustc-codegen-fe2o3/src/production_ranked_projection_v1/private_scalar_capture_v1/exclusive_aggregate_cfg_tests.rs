use fe2o3_mir_model::semantic_mir_v1::*;

#[derive(Clone, Copy, Debug)]
enum Shape {
    LatePredecessor,
    Backedge,
    Cleanup,
}

#[derive(Clone, Copy, Debug)]
enum Mutation {
    None,
    Reassign,
    Escape,
}

fn edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
}

fn goto(target: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, target))
}

fn constant(ty: SemanticTypeIdV1, value: u128, bytes: u8) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        ty,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, bytes).unwrap()),
    ))
}

fn switch(ty: SemanticTypeIdV1, fast: u32, slow: u32) -> SemanticTerminatorKindV1 {
    // The private flow must retain every source edge even for a literal
    // selector. This fixture does not assert path feasibility from that value.
    SemanticTerminatorKindV1::SwitchInt {
        discriminant: constant(ty, 0, 8),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                0,
                edge(SemanticEdgeRoleV1::SwitchValue, fast),
            )],
            edge(SemanticEdgeRoleV1::SwitchOtherwise, slow),
        )
        .unwrap(),
    }
}

fn block(
    index: u32,
    statements: Vec<SemanticStatementV1>,
    term: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    let mut identity = [240; 32];
    identity[28..].copy_from_slice(&index.to_be_bytes());
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256(identity),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), term),
    )
    .unwrap()
}

fn source_mir(shape: Shape, mutation: Mutation) -> ProductionSemanticMirOwnerV1 {
    let base = exclusive_aggregate_owner(ExclusiveAggregateChange::None);
    let semantic = base.source_semantic();
    let original = &semantic.functions()[0];
    assert_eq!(original.blocks().len(), 2);
    let statements = original.blocks()[0].statements();
    let first_read = statements.iter().position(|statement| {
        matches!(statement.kind(), SemanticStatementKindV1::Assign(assignment)
            if matches!(assignment.value().kind(), SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place))
                if matches!(place.projections(), [d, f]
                    if d.kind() == SemanticProjectionKindV1::Dereference
                        && f.kind() == SemanticProjectionKindV1::Field(0))))
    }).unwrap();
    assert_eq!(statements.len(), first_read + 2);
    let SemanticStatementKindV1::Assign(construction) = statements[first_read - 2].kind() else {
        panic!()
    };
    assert!(matches!(
        construction.value().kind(),
        SemanticRvalueKindV1::Aggregate(_)
    ));
    let SemanticStatementKindV1::Assign(borrow) = statements[first_read - 1].kind() else {
        panic!()
    };
    assert!(
        matches!(borrow.value().kind(), SemanticRvalueKindV1::Borrow {
        kind: SemanticBorrowKindV1::Mutable, place
    } if place == construction.destination())
    );
    let SemanticStatementKindV1::Assign(read) = statements[first_read].kind() else {
        panic!()
    };
    let integer = read.value().result_type();
    let mut types = semantic.types().to_vec();
    let prefix_term = match shape {
        Shape::LatePredecessor => switch(integer, 3, 4),
        Shape::Backedge => goto(2),
        Shape::Cleanup => {
            let boolean = SemanticTypeIdV1::from_index(types.len() as u32);
            types.push(SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([234; 32]),
                SemanticLayoutIdentityV1::from_sha256([234; 32]),
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(1),
                    1,
                    SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::integer(false, 8, 1),
                        SemanticScalarValidityRangeV1::new(0, 1),
                    )),
                    false,
                )
                .unwrap(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
            ));
            SemanticTerminatorKindV1::Assert {
                condition: constant(boolean, 1, 1),
                expected: true,
                message: SemanticAssertMessageV1::BoundsCheck {
                    length: constant(integer, 1, 8),
                    index: constant(integer, 0, 8),
                },
                target: edge(SemanticEdgeRoleV1::AssertSuccess, 3),
                unwind: SemanticUnwindActionV1::Cleanup(edge(SemanticEdgeRoleV1::AssertUnwind, 4)),
            }
        }
    };
    let late = match mutation {
        Mutation::None => vec![],
        Mutation::Reassign => vec![statements[first_read - 2].clone()],
        Mutation::Escape => {
            let raw = original.locals().iter().enumerate().find(|(_, local)| {
                matches!(types[local.ty().index() as usize].shape(), SemanticTypeShapeV1::Pointer(p)
                    if p.kind() == SemanticPointerKindV1::Raw
                        && p.pointee() == construction.destination().ty())
            }).unwrap();
            vec![SemanticStatementV1::new(
                original.source(),
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    SemanticPlaceV1::new(
                        SemanticLocalIdV1::from_index(raw.0 as u32),
                        vec![],
                        raw.1.ty(),
                    )
                    .unwrap(),
                    SemanticRvalueV1::new(
                        raw.1.ty(),
                        SemanticRvalueKindV1::Cast {
                            kind: SemanticCastKindV1::Pointer,
                            operand: SemanticOperandV1::Copy(borrow.destination().clone()),
                        },
                    ),
                )),
            )]
        }
    };
    let mut blocks = original.blocks().to_vec();
    blocks[0] = SemanticBasicBlockV1::new(
        blocks[0].identity(),
        blocks[0].source(),
        statements[..first_read].to_vec(),
        SemanticTerminatorV1::new(original.source(), prefix_term),
    )
    .unwrap();
    blocks.push(block(
        2,
        statements[first_read..].to_vec(),
        if matches!(shape, Shape::Backedge) {
            switch(integer, 7, 4)
        } else {
            original.blocks()[0].terminator().kind().clone()
        },
    ));
    blocks.push(block(3, vec![], goto(2)));
    blocks.push(block(4, vec![], goto(5)));
    blocks.push(block(5, vec![], goto(6)));
    blocks.push(block(6, late, goto(2)));
    if matches!(shape, Shape::Backedge) {
        // The original helper call (and its independent shared scalar read)
        // remains after the loop exit, not a moved argument on every iteration.
        blocks.push(block(
            7,
            vec![],
            original.blocks()[0].terminator().kind().clone(),
        ));
    }
    let body = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        original.locals().to_vec(),
        original.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let mut functions = semantic.functions().to_vec();
    functions[0] = body;
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        types,
        semantic.allocations().to_vec(),
        semantic.statics().to_vec(),
        semantic.vtables().to_vec(),
        functions,
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticMirOwnerV1::try_new(
        admitted,
        fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap()
}

fn owner(shape: Shape, mutation: Mutation) -> ProductionSemanticSsaOwnerV1 {
    ProductionSemanticSsaOwnerV1::try_new(
        source_mir(shape, mutation),
        fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn expanded_block(view: &SemanticExpandedRootV1, source: u32) -> usize {
    let matches = view
        .block_origins()
        .iter()
        .enumerate()
        .filter(|(_, origin)| {
            origin.instance().index() == 0
                && origin.function().index() == 0
                && origin.block().index() == source
        })
        .map(|(block, _)| block)
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1);
    matches[0]
}

fn source_reads(view: &SemanticExpandedRootV1) -> Vec<&SemanticStatementV1> {
    let read = expanded_block(view, 2);
    let origin = &view.block_origins()[read];
    let statements = view.body().blocks()[read].statements();
    (0..2).map(|wanted| {
        let index = origin.statements().iter().position(|statement|
            matches!(statement, SemanticExpandedStatementOriginV1::Source { statement } if *statement == wanted)).unwrap();
        &statements[index]
    }).collect()
}

fn early_fields<'a>(analysis: &mut Analysis<'a>) -> (&'a SemanticPlaceV1, Reference, Flow) {
    let entry = expanded_block(analysis.view, 0);
    let mut flow = Flow::default();
    analysis.statements(entry, &mut flow, None).unwrap();
    let reads = source_reads(analysis.view);
    let place = analysis.aggregate_candidate(reads[0]).unwrap();
    let reference = analysis
        .aggregate_read_reference(place, &flow)
        .unwrap()
        .unwrap();
    assert!(reference.mutable);
    assert!(matches!(flow.values[&reference.target], Value::Fields(_)));
    assert_eq!(reference.borrow.block, entry);
    assert!(
        analysis
            .original_mutable_aggregate_borrow(reference, place.projections()[0].result_type())
            .unwrap()
    );
    (place, reference, flow)
}

fn assert_early_queue_order(view: &SemanticExpandedRootV1) {
    let read = expanded_block(view, 2);
    let late = expanded_block(view, 6);
    let mut seen = BTreeSet::new();
    let mut pending = VecDeque::from([view.body().entry().index() as usize]);
    let mut order = Vec::new();
    while let Some(block) = pending.pop_front() {
        if !seen.insert(block) {
            continue;
        }
        order.push(block);
        let mut edges = BTreeSet::new();
        view.body().blocks()[block]
            .terminator()
            .kind()
            .try_for_each_edge(|edge| {
                edges.insert(edge.target().index() as usize);
                Ok::<(), ()>(())
            })
            .unwrap();
        pending.extend(edges);
    }
    assert!(
        order.iter().position(|&block| block == read).unwrap()
            < order.iter().position(|&block| block == late).unwrap()
    );
}

fn assert_ranked_read(
    owner: &ProductionSemanticSsaOwnerV1,
    reads: &PrivateScalarReads<'_>,
    statement: &SemanticStatementV1,
    accepted: bool,
) {
    let view = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let function = view.body();
    let types = owner.source_semantic().types();
    let contracts = ProjectionLocalContractsV1 {
        checked_references: CheckedReferencesV1 {
            origins: vec![None; function.locals().len()],
            option_dominance: SemanticOptionDominanceV1::analyze(function, &[]).unwrap(),
            enum_payload_dominance: SemanticEnumPayloadDominanceV1::analyze(function, types)
                .unwrap(),
        },
        allocations: vec![None; function.locals().len()],
        allocation_provenance: vec![None; function.locals().len()],
    };
    let mut operations = Vec::new();
    let mut sources = Vec::new();
    let result = project_statement_accesses(
        types,
        function,
        expanded_block(view, 2),
        &[],
        statement,
        &ProjectedGlobalSemanticUsesV1::default(),
        Some(reads),
        &vec![None; function.locals().len()],
        &contracts,
        &[],
        &mut Vec::new(),
        &mut vec![None; function.locals().len()],
        &mut operations,
        &mut sources,
        &mut 0,
        &mut String::new(),
    );
    assert!(operations.is_empty() && sources.is_empty());
    if accepted {
        result.unwrap();
    } else {
        assert!(matches!(
            result,
            Err(ProductionRankedProjectionErrorV1::UnrankedDereference(_))
        ));
    }
}

fn check(shape: Shape, mutation: Mutation) {
    assert!(!matches!(shape, Shape::Cleanup));
    let owner = owner(shape, mutation);
    owner.verify_replay().unwrap();
    let root = SemanticFunctionIdV1::from_index(0);
    let view = owner.execution_view_for_root(root).unwrap();
    let source = owner.source_semantic().canonical_encoding().to_vec();
    let body = view.body().clone();
    assert_early_queue_order(view);
    let mut early = Analysis::new(owner.source_semantic().types(), view);
    let (place, original, mut flow) = early_fields(&mut early);
    let late = expanded_block(view, 6);
    early.statements(late, &mut flow, None).unwrap();
    // A same-value rewrite leaves the exact aggregate initialized; rejection
    // must come from lost borrow custody, not an Opaque aggregate allowance.
    assert!(matches!(flow.values[&original.target], Value::Fields(_)));
    let unchanged = matches!(mutation, Mutation::None);
    assert_eq!(
        early
            .aggregate_read_reference(place, &flow)
            .unwrap()
            .is_some(),
        unchanged
    );
    if !unchanged {
        assert!(flow.escaped.contains(&original.target));
    }
    let reads = PrivateScalarReads::for_root(&owner, root).unwrap();
    assert!(
        reads.observation.completed_without_exhaustion(),
        "{:?}",
        reads.observation
    );
    assert_eq!(reads.reads.len(), if unchanged { 3 } else { 1 });
    for statement in source_reads(view) {
        assert_eq!(
            reads.contains(view.body(), statement),
            unchanged,
            "{shape:?} {mutation:?}"
        );
        assert_ranked_read(&owner, &reads, statement, unchanged);
        let copied = (*statement).clone();
        assert!(!reads.contains(view.body(), &copied));
        assert_ranked_read(&owner, &reads, &copied, false);
    }
    assert_eq!(view.body(), &body);
    assert_eq!(owner.source_semantic().canonical_encoding(), source);
}

#[test]
fn private_exclusive_aggregate_cfg_unchanged_late_predecessor_and_backedge_preserve_reads() {
    for shape in [Shape::LatePredecessor, Shape::Backedge] {
        check(shape, Mutation::None);
    }
}

#[test]
fn private_exclusive_aggregate_cfg_late_predecessor_invalidates_early_fields_read() {
    for mutation in [Mutation::Reassign, Mutation::Escape] {
        check(Shape::LatePredecessor, mutation);
    }
}

#[test]
fn private_exclusive_aggregate_cfg_late_backedge_invalidates_early_fields_read() {
    for mutation in [Mutation::Reassign, Mutation::Escape] {
        check(Shape::Backedge, mutation);
    }
}

#[test]
fn private_exclusive_aggregate_cfg_cleanup_is_rejected_before_private_flow() {
    for mutation in [Mutation::None, Mutation::Reassign, Mutation::Escape] {
        let mir = source_mir(Shape::Cleanup, mutation);
        let original = &mir.semantic().functions()[0];
        assert!(matches!(original.blocks()[0].terminator().kind(),
            SemanticTerminatorKindV1::Assert {
                target, unwind: SemanticUnwindActionV1::Cleanup(cleanup), ..
            } if target.role() == SemanticEdgeRoleV1::AssertSuccess
                && target.target().index() == 3
                && cleanup.role() == SemanticEdgeRoleV1::AssertUnwind
                && cleanup.target().index() == 4));
        let error = match ProductionSemanticSsaOwnerV1::try_new(
            mir,
            fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
        ) {
            Err(error) => error,
            Ok(_) => panic!("unsupported source cleanup acquired an execution owner"),
        };
        assert!(
            matches!(error,
            fe2o3_pliron::ProductionSemanticSsaErrorV1::CallExpansion(
                fe2o3_mir_model::SemanticCallExpansionErrorV1::Unsupported {
                    function, block: Some(block), reason: "assert unwind is not unreachable",
                }
            ) if function.index() == 0 && block.index() == 0),
            "{error:?}"
        );
    }
}

#[test]
fn private_exclusive_aggregate_cfg_exact_work_boundary_never_publishes_early_read() {
    for mutation in [Mutation::None, Mutation::Reassign] {
        let owner = owner(Shape::Backedge, mutation);
        let view = owner
            .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
            .unwrap();
        let types = owner.source_semantic().types();
        let mut complete = Analysis::new(types, view);
        let expected = complete.run().unwrap();
        assert_eq!(
            expected.len(),
            if matches!(mutation, Mutation::None) {
                3
            } else {
                1
            }
        );
        let used = MAX_WORK - complete.budget.remaining;
        for available in [0, used / 2, used - 1, used] {
            let mut analysis = Analysis::new(types, view);
            analysis.budget = Budget::new(available);
            let result = analysis.run();
            if available == used {
                assert_eq!(result.unwrap(), expected);
            } else {
                assert!(result.is_err());
                assert!(analysis.observation.work_exhausted);
                assert_eq!(analysis.observation.admitted_reads, 0);
            }
            assert!(
                analysis.budget.reserve(MAX_STORAGE).is_ok(),
                "all retained state releases on exit"
            );
        }
    }
}
