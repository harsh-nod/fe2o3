// Existing source-contract fixtures, replayed through the production producer.
// These component tests do not claim actual rustc collection or proof receipts.

#[test]
fn reference_roster_retention_does_not_change_existing_write_expressions() {
    let (types, callables, function) = typed_global_source_fixture_v1();
    let (projection, blocks, sources) =
        typed_global_ranked_source_fixture_v1(&types, &callables, &function);
    let (before, disabled) = projected_reference_gpu_effects_v2(
        &types,
        &callables,
        &function,
        &projection,
        &blocks,
        &sources,
        false,
        None,
    )
    .unwrap();
    let (after, retained) = projected_reference_gpu_effects_v2(
        &types,
        &callables,
        &function,
        &projection,
        &blocks,
        &sources,
        true,
        None,
    )
    .unwrap();
    assert_eq!(before, after);
    assert_eq!(disabled.loads().count(), 0);
    assert_eq!(retained.loads().count(), 1);
}

#[test]
fn reference_roster_retains_typed_read_without_any_payload_use() {
    let (types, callables, function) = typed_global_source_fixture_v1();
    let mut blocks = function.blocks().to_vec();
    let original = &blocks[11];
    let SemanticTerminatorKindV1::Call(call) = original.terminator().kind() else {
        panic!("store")
    };
    let mut arguments = call.arguments().to_vec();
    arguments[2] = typed_constant(U64_TYPE, 1, 8);
    blocks[11] = SemanticBasicBlockV1::new(
        original.identity(),
        original.source(),
        original.statements()[2..].to_vec(),
        SemanticTerminatorV1::new(
            original.terminator().source(),
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    call.callee(),
                    arguments,
                    call.destination().cloned(),
                    call.unwind(),
                )
                .unwrap(),
            ),
        ),
    )
    .unwrap();
    let function = typed_global_fixture_with_body_v1(&function, function.locals().to_vec(), blocks);
    let (projection, blocks, sources) =
        typed_global_ranked_source_fixture_v1(&types, &callables, &function);
    assert!(projection.global_uses.load_operands.is_empty());
    let (writes, reads) = projected_reference_gpu_effects_v2(
        &types,
        &callables,
        &function,
        &projection,
        &blocks,
        &sources,
        true,
        None,
    )
    .unwrap();
    assert!(matches!(
        writes[0].value,
        Ok(ProductionSemanticExpressionV2::Constant { bits: 1, .. })
    ));
    let loads = reads.loads().collect::<Vec<_>>();
    assert_eq!(loads.len(), 1);
    let source = sources
        .iter()
        .find(|source| source.access == AccessKindAttr::Read)
        .unwrap();
    assert_eq!(
        (loads[0].block as usize, loads[0].operation as usize),
        (source.block, source.operation)
    );
    assert_eq!(
        loads[0].read_mode,
        fe2o3_pliron::ProductionSemanticReadModeV2::UnorderedVolatile
    );
    let accesses = production_access_sources(&blocks, &sources).unwrap();
    let kernel =
        ProductionRankedKernelV1::new("read_roster", projection.extent_argument_count, blocks)
            .unwrap();
    ProjectedReferenceInputV2::new(
        kernel,
        function.identity(),
        writes,
        reads,
        &sources,
        accesses,
        Vec::new(),
    )
    .unwrap_or_else(|error| panic!("{error:?}"));
}

#[test]
fn reference_roster_does_not_promote_mutable_reads_to_initial_memory() {
    let (types, callables, function) = typed_global_exclusive_source_fixture_v1(true);
    let (projection, blocks, sources) =
        typed_global_ranked_source_fixture_v1(&types, &callables, &function);
    let (writes, reads) = projected_reference_gpu_effects_v2(
        &types,
        &callables,
        &function,
        &projection,
        &blocks,
        &sources,
        true,
        None,
    )
    .unwrap();
    assert!(writes.iter().all(|write| write.value.is_ok()));
    assert_eq!(reads.loads().count(), 0);
    let mut resolver = GpuSemanticExpressionResolverV2::with_ranked_reads(
        &types,
        &callables,
        &function,
        &projection,
        &blocks,
        &sources,
    )
    .unwrap();
    resolver.memory_versions.clear();
    let read = sources
        .iter()
        .find(|source| source.access == AccessKindAttr::Read)
        .unwrap();
    let allocation = blocks
        .iter()
        .flat_map(|block| block.operations())
        .filter_map(|operation| match operation {
            ProductionRankedOperationV1::View {
                result,
                allocation_origin,
                ..
            }
            | ProductionRankedOperationV1::ViewInSpace {
                result,
                allocation_origin,
                ..
            } => Some((ProductionRankedValueV1::Local(*result), *allocation_origin)),
            _ => None,
        })
        .collect();
    assert!(matches!(
        resolver.bind_typed_global_read_v2(
            &callables,
            &projection,
            &blocks,
            &sources,
            read,
            &allocation
        ),
        Err(ProductionRankedProjectionErrorV1::Incomplete(
            "typed global mutable load lacks an authenticated reaching memory version"
        ))
    ));
}

#[test]
fn reference_roster_rejects_changed_source_occurrence_and_duplicate_read() {
    for duplicate in [false, true] {
        let (types, callables, function) = typed_global_source_fixture_v1();
        let (projection, blocks, mut sources) =
            typed_global_ranked_source_fixture_v1(&types, &callables, &function);
        let (writes, reads) = projected_reference_gpu_effects_v2(
            &types,
            &callables,
            &function,
            &projection,
            &blocks,
            &sources,
            true,
            None,
        )
        .unwrap();
        assert!(reads.loads().count() > 0);
        assert!(writes.iter().all(|write| write.value.is_ok()));
        let read = sources
            .iter()
            .position(|source| source.access == AccessKindAttr::Read)
            .unwrap();
        if duplicate {
            sources.push(sources[read]);
        } else {
            sources[read].semantic_site.as_mut().unwrap().block = 11;
        }
        let result = projected_reference_gpu_effects_v2(
            &types,
            &callables,
            &function,
            &projection,
            &blocks,
            &sources,
            true,
            None,
        );
        if duplicate {
            assert!(matches!(
                result,
                Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "typed global load allocation or source custody changed"
                ))
            ));
        } else {
            // Reject the substituted call site before returning a read roster.
            assert!(matches!(
                result,
                Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "typed global effect lacks its authenticated call-site projection"
                ))
            ));
        }
    }
}

#[test]
fn reference_roster_checks_original_source_ordinal_and_complete_access_roster() {
    for missing in [false, true] {
        let (types, callables, function) = typed_global_source_fixture_v1();
        let (projection, blocks, sources) =
            typed_global_ranked_source_fixture_v1(&types, &callables, &function);
        let (writes, reads) = projected_reference_gpu_effects_v2(
            &types,
            &callables,
            &function,
            &projection,
            &blocks,
            &sources,
            true,
            None,
        )
        .unwrap();
        let mut accesses = production_access_sources(&blocks, &sources).unwrap();
        if missing {
            accesses.pop();
        } else {
            let old = accesses[0];
            accesses[0] = ProductionRankedAccessSourceV1::new(
                old.semantic_block(),
                old.semantic_statement(),
                1,
                old.ranked_block(),
                old.ranked_operation(),
            );
        }
        let kernel =
            ProductionRankedKernelV1::new("read_roster", projection.extent_argument_count, blocks)
                .unwrap();
        assert!(matches!(
            ProjectedReferenceInputV2::new(
                kernel,
                function.identity(),
                writes,
                reads,
                &sources,
                accesses,
                Vec::new()
            ),
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "reference source roster differs from the original access projection"
            ))
        ));
    }
}

#[test]
fn reference_roster_work_limit_is_inclusive_and_never_resets() {
    let mut reserved = ReferenceReadRosterV1::default();
    reserved.reserve(1).unwrap();
    assert!(matches!(
        reserved.reserve(2),
        Err(ProductionRankedProjectionErrorV1::Incomplete(
            "reference read source reservation cannot be replaced"
        ))
    ));
    let roster = ReferenceReadRosterV1::default();
    roster
        .charge(crate::reference_effect_v1::MAX_REFERENCE_STATEMENTS_V1 - 1)
        .unwrap();
    roster.charge(1).unwrap();
    for _ in 0..2 {
        assert_eq!(
            roster.charge(1),
            Err("reference read roster exceeds the existing reference work limit")
        );
    }
}
