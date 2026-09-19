fn indexed_read_source_v1(
    block: usize,
    statement: Option<usize>,
    operation: usize,
) -> ProjectedAccessSourceV1 {
    ProjectedAccessSourceV1 {
        block: 0,
        operation,
        access: AccessKindAttr::Read,
        memory_space: MemorySpaceAttr::Global,
        source: SemanticSourceProvenanceV1::unavailable(),
        output_extent: None,
        semantic_site: Some(ProjectedSemanticAccessSiteV1 { block, statement }),
    }
}

// The former production scan, including actual pointer-position identity.
fn reference_read_source_scan_v1(
    sources: &[ProjectedAccessSourceV1],
    source: usize,
) -> Option<(usize, usize)> {
    let source = sources.get(source)?;
    if source.access != AccessKindAttr::Read || source.memory_space != MemorySpaceAttr::Global {
        return None;
    }
    source.semantic_site?.statement?;
    let matching = sources
        .iter()
        .filter(|candidate| {
            candidate.access == AccessKindAttr::Read
                && candidate.memory_space == MemorySpaceAttr::Global
                && candidate.semantic_site == source.semantic_site
        })
        .collect::<Vec<_>>();
    let ordinal = matching
        .iter()
        .position(|candidate| std::ptr::eq(*candidate, source))?;
    Some((ordinal, matching.len()))
}

fn assert_read_source_index_matches_scan_v1(sources: &[ProjectedAccessSourceV1]) {
    let index = RankedReadSourceIndexV1::new(sources).unwrap();
    for source in 0..sources.len() {
        assert_eq!(
            index.get(source),
            reference_read_source_scan_v1(sources, source)
        );
    }
    assert_eq!(index.get(sources.len()), None);
    assert_eq!(index.get(usize::MAX), None);
}

#[test]
fn read_source_index_preserves_interleaved_sites_and_duplicate_pointer_ordinals() {
    let duplicate = indexed_read_source_v1(8, Some(3), 11);
    let sources = [
        duplicate,
        indexed_read_source_v1(1, Some(7), 12),
        duplicate,
        indexed_read_source_v1(8, Some(2), 13),
        indexed_read_source_v1(1, Some(7), 14),
        duplicate,
    ];
    assert_read_source_index_matches_scan_v1(&sources);
    let index = RankedReadSourceIndexV1::new(&sources).unwrap();
    assert_eq!(index.get(0), Some((0, 3)));
    assert_eq!(index.get(2), Some((1, 3)));
    assert_eq!(index.get(5), Some((2, 3)));
    assert_eq!(index.get(1), Some((0, 2)));
    assert_eq!(index.get(4), Some((1, 2)));
    assert_eq!(index.get(3), Some((0, 1)));
}

#[test]
fn read_source_index_filters_exact_kind_space_and_statement_presence() {
    let base = indexed_read_source_v1(2, Some(4), 0);
    let mut sources = vec![base];
    for access in [
        AccessKindAttr::Write,
        AccessKindAttr::AtomicRead,
        AccessKindAttr::AtomicWrite,
        AccessKindAttr::AtomicReadModifyWrite,
    ] {
        sources.push(ProjectedAccessSourceV1 { access, ..base });
    }
    for memory_space in [MemorySpaceAttr::Private, MemorySpaceAttr::Workgroup] {
        sources.push(ProjectedAccessSourceV1 {
            memory_space,
            ..base
        });
    }
    sources.push(ProjectedAccessSourceV1 {
        output_extent: None,
        semantic_site: None,
        ..base
    });
    sources.push(indexed_read_source_v1(2, None, 0));
    sources.push(base);
    assert_read_source_index_matches_scan_v1(&sources);
    let index = RankedReadSourceIndexV1::new(&sources).unwrap();
    assert_eq!(index.rows.len(), 2);
    assert_eq!(index.get(0), Some((0, 2)));
    assert_eq!(index.get(sources.len() - 1), Some((1, 2)));
}

#[test]
fn read_source_index_keeps_cardinality_mismatch_and_sparse_invalid_sites_inert() {
    let sources = [
        indexed_read_source_v1(usize::MAX, Some(usize::MAX), 4),
        indexed_read_source_v1(0, Some(1), 5),
        indexed_read_source_v1(usize::MAX, Some(usize::MAX), 4),
        indexed_read_source_v1(0, Some(1), 6),
    ];
    assert_read_source_index_matches_scan_v1(&sources);
    let index = RankedReadSourceIndexV1::new(&sources).unwrap();
    assert_eq!(index.rows.len(), 4);
    for source in 0..sources.len() {
        for read_place_count in 0..=4 {
            let indexed = index
                .get(source)
                .and_then(|(ordinal, count)| (count == read_place_count).then_some(ordinal));
            let reference = reference_read_source_scan_v1(&sources, source)
                .and_then(|(ordinal, count)| (count == read_place_count).then_some(ordinal));
            assert_eq!(indexed, reference);
            assert_eq!(indexed.is_some(), read_place_count == 2);
        }
    }
}

#[test]
fn read_source_index_actual_resolver_still_skips_unknown_or_invalid_statement_sites() {
    let types = projection_types();
    let function = projection_function(vec![block(0, vec![], SemanticTerminatorKindV1::Return)]);
    let sources = [
        indexed_read_source_v1(usize::MAX, Some(0), usize::MAX),
        indexed_read_source_v1(0, Some(usize::MAX), usize::MAX),
        indexed_read_source_v1(0, Some(0), usize::MAX),
        indexed_read_source_v1(0, None, usize::MAX),
        ProjectedAccessSourceV1 {
            output_extent: None,
            semantic_site: None,
            ..indexed_read_source_v1(0, Some(0), usize::MAX)
        },
    ];
    let resolver =
        GpuSemanticExpressionResolverV2::with_ranked_reads(&types, &function, &[], &[], &sources)
            .unwrap();
    assert!(resolver.loads.is_empty());
    assert!(resolver.place_loads.is_empty());
}

fn indexed_two_read_function_v1() -> SemanticFunctionDeclV1 {
    aggregate_function_v2(vec![aggregate_assignment_v2(
        2,
        SCALAR_TYPE,
        SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::Add,
            left: SemanticOperandV1::Copy(dereferenced_place()),
            right: SemanticOperandV1::Copy(dereferenced_place()),
        },
    )])
}

fn indexed_two_read_blocks_v1() -> Vec<ProductionRankedBlockV1> {
    let view = ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0));
    let index = ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(1));
    vec![ProductionRankedBlockV1::new(
        vec![
            ProductionRankedOperationV1::ViewInSpace {
                result: ProductionRankedValueIdV1::new(0),
                element_width: 32,
                writable: false,
                shape: vec![1],
                dynamic_extents: vec![],
                memory_space: MemorySpaceAttr::Global,
                allocation_origin: 1,
                noalias_class: 1,
            },
            ProductionRankedOperationV1::IndexConstant {
                result: ProductionRankedValueIdV1::new(1),
                value: 0,
            },
            ProductionRankedOperationV1::Access {
                kind: AccessKindAttr::Read,
                view,
                indices: vec![index],
            },
            ProductionRankedOperationV1::Access {
                kind: AccessKindAttr::Read,
                view,
                indices: vec![index],
            },
        ],
        ProductionRankedTerminatorV1::Return,
    )]
}

#[test]
fn read_source_index_actual_resolver_keeps_same_statement_operand_order() {
    let types = aggregate_types_v2();
    let function = indexed_two_read_function_v1();
    let blocks = indexed_two_read_blocks_v1();
    let SemanticStatementKindV1::Assign(assignment) = function.blocks()[0].statements()[0].kind()
    else {
        panic!("two-read assignment");
    };
    let mut places = Vec::new();
    semantic_rvalue_read_places_v2(assignment.value(), &mut places);
    assert_eq!(places.len(), 2);
    for operations in [[2, 3], [3, 2], [2, 2]] {
        let sources = operations.map(|operation| indexed_read_source_v1(0, Some(0), operation));
        let resolver = GpuSemanticExpressionResolverV2::with_ranked_reads(
            &types,
            &function,
            &[],
            &blocks,
            &sources,
        )
        .unwrap();
        assert_eq!(resolver.place_loads.len(), 2);
        for (ordinal, place) in places.iter().enumerate() {
            let load = &resolver.place_loads[&(*place as *const SemanticPlaceV1)];
            assert_eq!(load.operation as usize, operations[ordinal]);
        }
    }
}

#[test]
fn read_source_index_actual_resolver_preserves_cardinality_before_location_error() {
    let types = aggregate_types_v2();
    let function = indexed_two_read_function_v1();
    let blocks = indexed_two_read_blocks_v1();
    let invalid = indexed_read_source_v1(0, Some(0), usize::MAX);
    let resolver = GpuSemanticExpressionResolverV2::with_ranked_reads(
        &types,
        &function,
        &[],
        &blocks,
        &[invalid],
    )
    .unwrap();
    assert!(resolver.place_loads.is_empty());
    assert!(matches!(
        GpuSemanticExpressionResolverV2::with_ranked_reads(
            &types,
            &function,
            &[],
            &blocks,
            &[invalid, indexed_read_source_v1(0, Some(0), 2)],
        ),
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "a projected load correspondence does not identify one ranked read"
        ))
    ));
}

#[test]
fn read_source_index_differential_permutations_preserve_original_order() {
    let mut sources = (0..64)
        .map(|source| {
            let mut row = indexed_read_source_v1(source % 5, Some(source % 3), source % 7);
            if source % 11 == 0 {
                row.access = AccessKindAttr::AtomicRead;
            }
            if source % 13 == 0 {
                row.memory_space = MemorySpaceAttr::Private;
            }
            if source % 17 == 0 {
                row.semantic_site = None;
            }
            row
        })
        .collect::<Vec<_>>();
    for _ in 0..sources.len() {
        assert_read_source_index_matches_scan_v1(&sources);
        sources.rotate_left(1);
    }
    sources.reverse();
    assert_read_source_index_matches_scan_v1(&sources);
}

#[test]
fn read_source_index_large_roster_has_one_sparse_row_per_eligible_read() {
    let count = MAX_PROJECTED_OPERATIONS_V1;
    let sources = (0..count)
        .map(|source| indexed_read_source_v1(usize::MAX, Some(source % 257), source))
        .collect::<Vec<_>>();
    let index = RankedReadSourceIndexV1::new(&sources).unwrap();
    assert_eq!(index.rows.len(), count);
    for source in 0..count {
        let site = source % 257;
        let site_count = (count - 1 - site) / 257 + 1;
        assert_eq!(index.get(source), Some((source / 257, site_count)));
    }
    // The old scan visits this entire roster once for every eligible read.
    assert_eq!(count.checked_mul(count), Some(4_294_836_225));
}

#[test]
fn read_source_index_empty_or_filtered_roster_needs_no_sparse_payload() {
    for sources in [vec![], vec![indexed_read_source_v1(0, None, 0); 128]] {
        let index = RankedReadSourceIndexV1::new(&sources).unwrap();
        assert!(index.rows.is_empty());
        assert_eq!(index.rows.capacity(), 0);
        assert_eq!(index.get(0), None);
    }
}

#[test]
fn read_source_index_actual_resolver_still_skips_non_assignment_statements() {
    let types = projection_types();
    let function = projection_function(vec![block(
        0,
        vec![statement(SemanticStatementKindV1::Nop)],
        SemanticTerminatorKindV1::Return,
    )]);
    let sources = [indexed_read_source_v1(0, Some(0), usize::MAX)];
    let resolver =
        GpuSemanticExpressionResolverV2::with_ranked_reads(&types, &function, &[], &[], &sources)
            .unwrap();
    assert!(resolver.loads.is_empty());
    assert!(resolver.place_loads.is_empty());
}

#[test]
fn read_source_index_actual_resolver_keeps_first_refusal_under_source_reversal() {
    let types = aggregate_types_v2();
    let function = indexed_two_read_function_v1();
    let view = ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0));
    let index = ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(1));
    for (first, expected) in [
        (
            ProductionRankedOperationV1::Access {
                kind: AccessKindAttr::Write,
                view,
                indices: vec![index],
            },
            "a projected scalar load changed ranked access kind",
        ),
        (
            ProductionRankedOperationV1::Access {
                kind: AccessKindAttr::Read,
                view: ProductionRankedValueV1::Argument(99),
                indices: vec![index],
            },
            "a projected load view has no exact allocation origin",
        ),
    ] {
        let mut operations = indexed_two_read_blocks_v1()[0].operations().to_vec();
        operations[2] = first;
        let blocks = [ProductionRankedBlockV1::new(
            operations,
            ProductionRankedTerminatorV1::Return,
        )];
        let mut sources = [
            indexed_read_source_v1(0, Some(0), 2),
            indexed_read_source_v1(0, Some(0), usize::MAX),
        ];
        for detail in [
            expected,
            "a projected load correspondence does not identify one ranked read",
        ] {
            let result = GpuSemanticExpressionResolverV2::with_ranked_reads(
                &types,
                &function,
                &[],
                &blocks,
                &sources,
            );
            match result {
                Err(ProductionRankedProjectionErrorV1::Unsupported(actual)) => {
                    assert_eq!(actual, detail);
                }
                _ => panic!("expected the first read's exact refusal"),
            }
            sources.reverse();
        }
    }
}
