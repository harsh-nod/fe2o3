// Component inputs for the real compiler join. No receipt execution or source
// authentication is claimed by these synthetic reference bindings.
fn read_placement_fixture() -> (
    ProductionRankedKernelV1,
    AuthenticatedReferenceEffectBindingsV1,
    RankedGpuWriteV2,
    Vec<ProductionRankedValueIdV1>,
) {
    let (kernel, write) = input_guard_kernel(InputGuardShape::And, [64, 64]);
    let mut blocks = kernel.blocks().to_vec();
    let mut entry = blocks[0].operations().to_vec();
    for id in 7..10 {
        entry.push(ProductionRankedOperationV1::SemanticConstant {
            result: ProductionRankedValueIdV1::new(id),
            value: 0,
        });
    }
    entry.push(ProductionRankedOperationV1::SemanticSymbol {
        result: ProductionRankedValueIdV1::new(10),
        symbol: 0,
    });
    blocks[0] = ProductionRankedBlockV1::new(entry, blocks[0].terminator().clone());
    let kernel =
        ProductionRankedKernelV1::new(kernel.function_name(), kernel.argument_count(), blocks)
            .unwrap();
    (
        kernel,
        AuthenticatedReferenceEffectBindingsV1::new(vec![input_guard_reference(
            InputGuardShape::And,
        )]),
        write,
        (7..11).map(ProductionRankedValueIdV1::new).collect(),
    )
}

fn prepared_read_placement() -> CompilerOwnedReferenceEffectRequestV2 {
    let (kernel, bindings, write, reserved) = read_placement_fixture();
    prepare_reference_effect_request_v2(kernel, &bindings, &[write], reserved)
        .unwrap_or_else(|error| panic!("read-root placement preparation failed: {error:?}"))
}

fn read_placement_sources() -> Vec<fe2o3_lower_mir_kernel::ProductionRankedAccessSourceV1> {
    (0..3)
        .map(|ordinal| {
            fe2o3_lower_mir_kernel::ProductionRankedAccessSourceV1::new(
                11,
                Some(2),
                ordinal,
                3,
                ordinal,
            )
        })
        .collect()
}

#[test]
fn actual_effect_join_places_read_roots_after_their_producers_before_the_write() {
    let request = prepared_read_placement();
    assert!(request.source_site_remap.is_some());
    let blocks = request.kernel.blocks();
    assert_eq!(blocks.len(), 5);
    let operations = blocks[3].operations();
    assert!(matches!(
        operations[0],
        ProductionRankedOperationV1::Access {
            kind: AccessKindAttr::Read,
            ..
        }
    ));
    assert!(matches!(
        operations[1],
        ProductionRankedOperationV1::Access {
            kind: AccessKindAttr::Read,
            ..
        }
    ));
    let ProductionRankedOperationV1::SemanticExpression {
        result: gpu,
        expression,
        ..
    } = &operations[2]
    else {
        panic!("actual GPU root must follow both original reads")
    };
    let ProductionSemanticExpressionV2::Binary { lhs, rhs, .. } = expression else {
        panic!("GPU add")
    };
    for (operand, site) in [(lhs.as_ref(), 0), (rhs.as_ref(), 1)] {
        let ProductionSemanticExpressionV2::Load(load) = operand else {
            panic!("actual source read")
        };
        assert_eq!((load.block, load.operation), (3, site));
        assert_eq!(
            load.read_mode,
            fe2o3_pliron::ProductionSemanticReadModeV2::UnorderedNonVolatile
        );
    }
    let ProductionRankedOperationV1::SemanticExpression {
        result: cpu,
        expression,
        ..
    } = &operations[3]
    else {
        panic!("independent CPU root")
    };
    assert!(matches!(
        expression,
        ProductionSemanticExpressionV2::Constant { bits: 17, .. }
    ));
    assert_ne!(gpu, cpu);
    assert!(
        matches!(operations[4], ProductionRankedOperationV1::ValueAccess { value, .. }
        if value == ProductionRankedValueV1::Local(*gpu))
    );
    let ProductionRankedOperationV1::RequestEffectRefinement { contract, .. } = &operations[5]
    else {
        panic!("exact appended output contract")
    };
    assert_eq!(
        contract.gpu_write_site(),
        ProductionGpuWriteSiteV2::new(3, 4)
    );
    assert_eq!(
        contract.reference_value(),
        ProductionRankedValueV1::Local(*cpu)
    );
    assert_eq!(request.requests[0].operation, 5);
    assert_eq!(blocks[0].operations().len(), 10);
    assert_eq!(
        blocks[0]
            .operations()
            .iter()
            .filter(|op| matches!(op,
                ProductionRankedOperationV1::OwnershipContract { view,
                    coverage: OwnershipCoverageAttr::TotalView,
                    partition: OwnershipPartitionAttr::ExactSets } if *view == contract.view()
            ))
            .count(),
        1
    );
    assert!(
        blocks[1..]
            .iter()
            .flat_map(|block| block.operations())
            .all(|op| !matches!(op, ProductionRankedOperationV1::OwnershipContract { .. }))
    );
}

#[test]
fn read_placement_rejects_nonentry_output_view_without_moving_its_definition() {
    let (kernel, bindings, mut write, reserved) = read_placement_fixture();
    let mut blocks = kernel.blocks().to_vec();
    let mut view = blocks[0].operations()[6].clone();
    let ProductionRankedOperationV1::ViewInSpace { result, .. } = &mut view else {
        panic!("output view")
    };
    *result = ProductionRankedValueIdV1::new(11);
    write.view = ProductionRankedValueV1::Local(*result);
    blocks[1] = ProductionRankedBlockV1::new(vec![view], blocks[1].terminator().clone());
    let mut operations = blocks[3].operations().to_vec();
    let ProductionRankedOperationV1::Access { view, .. } = &mut operations[2] else {
        panic!("output write")
    };
    *view = write.view;
    blocks[3] = ProductionRankedBlockV1::new(operations, blocks[3].terminator().clone());
    let kernel =
        ProductionRankedKernelV1::new(kernel.function_name(), kernel.argument_count(), blocks)
            .unwrap();
    assert!(matches!(
        prepare_reference_effect_request_v2(kernel, &bindings, &[write], reserved),
        Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
            "output ownership metadata requires an entry-defined view"
        ))
    ));
}

#[test]
fn read_placement_refuses_proof_execution_until_source_rosters_are_remapped() {
    let error = prepared_read_placement()
        .prove_and_compile()
        .err()
        .expect("pending remap must reject before opening the runtime");
    assert!(matches!(
        error,
        ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
            "read-root placement requires its exact source-site roster remap before compilation"
        )
    ));
}

#[test]
fn read_placement_remaps_ranked_sites_without_changing_original_source_custody() {
    let mut request = prepared_read_placement();
    let mut sources = read_placement_sources();
    let before = sources.clone();
    request
        .remap_source_sites_v2(&mut sources, &mut [])
        .unwrap();
    assert!(request.source_site_remap.is_none());
    for (index, (old, new)) in before.iter().zip(&sources).enumerate() {
        assert_eq!(old.semantic_block(), new.semantic_block());
        assert_eq!(old.semantic_statement(), new.semantic_statement());
        assert_eq!(old.semantic_access_ordinal(), new.semantic_access_ordinal());
        assert_eq!(new.ranked_block(), 3);
        assert_eq!(new.ranked_operation(), [0, 1, 4][index]);
    }
}

#[test]
fn read_placement_duplicate_missing_and_stale_source_sites_reject_atomically() {
    for change in 0..3 {
        let mut request = prepared_read_placement();
        let mut sources = read_placement_sources();
        match change {
            0 => {
                sources[1] = sources[0];
            }
            1 => {
                sources.pop();
            }
            _ => {
                sources[2] = fe2o3_lower_mir_kernel::ProductionRankedAccessSourceV1::new(
                    11,
                    Some(2),
                    2,
                    3,
                    4,
                )
            }
        }
        let before = sources.clone();
        assert!(
            matches!(request.remap_source_sites_v2(&mut sources, &mut []),
            Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(reason))
                if reason == match change {
                    0 => "read-root placement source effect is duplicated",
                    1 => "read-root placement requires the complete exact source effect roster",
                    _ => "read-root placement source effect is absent",
                })
        );
        assert_eq!(sources, before);
        assert!(request.source_site_remap.is_some());
    }
}

#[test]
fn read_placement_rejects_a_load_from_the_write_site_or_a_non_read_site() {
    for cross_block in [false, true] {
        let (kernel, bindings, mut write, reserved) = read_placement_fixture();
        let Ok(ProductionSemanticExpressionV2::Binary { lhs, .. }) = &mut write.value else {
            panic!("GPU add")
        };
        let ProductionSemanticExpressionV2::Load(load) = lhs.as_mut() else {
            panic!("load")
        };
        if cross_block {
            load.block = 0;
        } else {
            load.operation = 2;
        }
        assert!(matches!(
            prepare_reference_effect_request_v2(kernel, &bindings, &[write], reserved),
            Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(reason))
                if reason == if cross_block {
                    "read-root placement lost the exact original read access"
                } else {
                    "read-root placement requires the original read before the write"
                }
        ));
    }
}

#[test]
fn load_free_effect_join_keeps_existing_entry_roots_and_source_sites() {
    let (kernel, bindings, mut write, reserved) = read_placement_fixture();
    write.value = Ok(ProductionSemanticExpressionV2::Constant {
        scalar: ProductionSemanticScalarTypeV2::Integer {
            signed: false,
            bits: 32,
        },
        bits: 17,
    });
    let request =
        prepare_reference_effect_request_v2(kernel, &bindings, &[write], reserved).unwrap();
    assert!(request.source_site_remap.is_none());
    assert!(matches!(
        request.kernel.blocks()[3].operations()[2],
        ProductionRankedOperationV1::ValueAccess { .. }
    ));
    assert_eq!(request.kernel.blocks()[0].operations().len(), 12);
}

#[test]
fn read_placement_rejects_conflicting_modes_for_the_same_original_read() {
    let (kernel, bindings, mut write, reserved) = read_placement_fixture();
    let Ok(ProductionSemanticExpressionV2::Binary { lhs, rhs, .. }) = &mut write.value else {
        panic!("GPU add")
    };
    *rhs = lhs.clone();
    let ProductionSemanticExpressionV2::Load(load) = rhs.as_mut() else {
        panic!("read")
    };
    load.read_mode = fe2o3_pliron::ProductionSemanticReadModeV2::UnorderedVolatile;
    // The independent read lookup now rejects this same occurrence conflict
    // before read-root placement starts mutating the recipe.
    assert!(matches!(
        prepare_reference_effect_request_v2(kernel, &bindings, &[write], reserved),
        Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
            "one reference read occurrence has conflicting source metadata"
        ))
    ));
}

#[test]
fn read_placement_rejects_changed_target_access_before_changing_source_rosters() {
    let mut request = prepared_read_placement();
    let mut blocks = request.kernel.blocks().to_vec();
    let mut operations = blocks[3].operations().to_vec();
    let changed_index = ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(1));
    let ProductionRankedOperationV1::Access { indices, .. } = &mut operations[0] else {
        panic!("read")
    };
    indices[0] = changed_index;
    let ProductionRankedOperationV1::SemanticExpression {
        expression: ProductionSemanticExpressionV2::Binary { lhs, .. },
        ..
    } = &mut operations[2]
    else {
        panic!("GPU root")
    };
    let ProductionSemanticExpressionV2::Load(load) = lhs.as_mut() else {
        panic!("read")
    };
    load.indices = vec![changed_index].into_boxed_slice();
    blocks[3] = ProductionRankedBlockV1::new(operations, blocks[3].terminator().clone());
    request.kernel = ProductionRankedKernelV1::new(
        request.kernel.function_name(),
        request.kernel.argument_count(),
        blocks,
    )
    .unwrap();
    let mut sources = read_placement_sources();
    let before = sources.clone();
    assert!(matches!(
        request.remap_source_sites_v2(&mut sources, &mut []),
        Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
            "read-root placement changed its retained target effect"
        ))
    ));
    assert_eq!(sources, before);
    assert!(request.source_site_remap.is_some());
}

#[test]
fn read_placement_preserves_generated_effect_origin_and_recipe_identity() {
    use fe2o3_lower_mir_kernel::{
        ProductionRankedExecutableEffectOriginV1, ProductionRankedExecutableEffectSourceV1,
    };
    let mut request = prepared_read_placement();
    let mut sources = read_placement_sources();
    sources.pop();
    let original = ProductionRankedExecutableEffectSourceV1::new(
        19,
        3,
        3,
        2,
        ProductionRankedExecutableEffectOriginV1::GeneratedFromSemanticTerminator,
        [37; 32],
    );
    let mut generated = [original];
    request
        .remap_source_sites_v2(&mut sources, &mut generated)
        .unwrap();
    assert_eq!(generated[0].semantic_block(), original.semantic_block());
    assert_eq!(
        generated[0].semantic_effect_ordinal(),
        original.semantic_effect_ordinal()
    );
    assert_eq!(generated[0].origin(), original.origin());
    assert_eq!(generated[0].recipe_identity(), original.recipe_identity());
    assert_eq!(
        (generated[0].ranked_block(), generated[0].ranked_operation()),
        (3, 4)
    );
}

include!("read_root_placement_v1/cross_block_tests.rs");
