// Mirrors successful nested Option-load predecessors, as used by typed vecadd.
// These inputs test the actual join, not source authentication or proof execution.
fn cross_block_read_placement_fixture() -> (
    ProductionRankedKernelV1,
    AuthenticatedReferenceEffectBindingsV1,
    RankedGpuWriteV2,
    Vec<ProductionRankedValueIdV1>,
) {
    let (kernel, bindings, mut write, reserved) = read_placement_fixture();
    let mut blocks = kernel.blocks().to_vec();
    let original = blocks[3].operations().to_vec();
    for (block, operation) in [(1, 0), (2, 1)] {
        blocks[block] = ProductionRankedBlockV1::new(
            vec![original[operation].clone()],
            blocks[block].terminator().clone(),
        );
    }
    blocks[3] =
        ProductionRankedBlockV1::new(vec![original[2].clone()], blocks[3].terminator().clone());
    write.operation = 0;
    let Ok(ProductionSemanticExpressionV2::Binary { lhs, rhs, .. }) = &mut write.value else {
        panic!("fixture GPU add")
    };
    for (operand, block) in [(lhs, 1), (rhs, 2)] {
        let ProductionSemanticExpressionV2::Load(load) = operand.as_mut() else {
            panic!("fixture source read")
        };
        load.block = block;
        load.operation = 0;
    }
    let kernel =
        ProductionRankedKernelV1::new(kernel.function_name(), kernel.argument_count(), blocks)
            .unwrap();
    (kernel, bindings, write, reserved)
}

fn cross_block_prepared_output(
    bindings: &AuthenticatedReferenceEffectBindingsV1,
    write: RankedGpuWriteV2,
    reserved_values: Vec<ProductionRankedValueIdV1>,
) -> PreparedReferenceOutputV2 {
    let gpu_expression = write.value.clone().unwrap();
    PreparedReferenceOutputV2 {
        numerical_contract: ProductionNumericalContractV2::exact_for_expression(&gpu_expression),
        reference_expression: ProductionSemanticExpressionV2::Constant {
            scalar: gpu_expression.scalar(),
            bits: 17,
        },
        gpu_expression,
        write,
        reference_write: bindings.as_slice()[0].observable_output_writes[0].clone(),
        output_argument: 2,
        reserved_values,
    }
}

#[test]
fn read_placement_cross_block_predecessors_keep_memory_events_and_exact_rosters() {
    use fe2o3_lower_mir_kernel::ProductionRankedAccessSourceV1 as Source;
    let (kernel, bindings, write, reserved) = cross_block_read_placement_fixture();
    let original = kernel.clone();
    let mut request = prepare_reference_effect_request_v2(kernel, &bindings, &[write], reserved)
        .expect("both retained reads dominate the write despite separate source blocks");
    let placed = request.kernel.blocks();
    for block in 1..=2 {
        assert_eq!(&placed[block], &original.blocks()[block]);
    }
    for (before, after) in original.blocks().iter().zip(placed) {
        assert_eq!(before.terminator(), after.terminator());
    }
    let operations = placed[3].operations();
    let ProductionRankedOperationV1::SemanticExpression {
        result: gpu,
        expression: ProductionSemanticExpressionV2::Binary { lhs, rhs, .. },
        ..
    } = &operations[0]
    else {
        panic!("GPU root immediately before output")
    };
    for (operand, block) in [(lhs, 1), (rhs, 2)] {
        let ProductionSemanticExpressionV2::Load(load) = operand.as_ref() else {
            panic!("retained read")
        };
        assert_eq!((load.block, load.operation), (block, 0));
    }
    assert!(matches!(
        operations[1],
        ProductionRankedOperationV1::SemanticExpression { .. }
    ));
    assert!(
        matches!(operations[2], ProductionRankedOperationV1::ValueAccess { value, .. }
        if value == ProductionRankedValueV1::Local(*gpu))
    );
    let ProductionRankedOperationV1::RequestEffectRefinement { contract, .. } = &operations[3]
    else {
        panic!("exact output contract")
    };
    assert_eq!(
        contract.gpu_write_site(),
        ProductionGpuWriteSiteV2::new(3, 2)
    );
    assert_eq!(placed[0].operations().iter().filter(|op| matches!(op,
        ProductionRankedOperationV1::OwnershipContract { view, coverage: OwnershipCoverageAttr::TotalView, partition: OwnershipPartitionAttr::ExactSets }
        if *view == contract.view())).count(), 1);
    assert!(
        placed
            .iter()
            .skip(1)
            .flat_map(|block| block.operations())
            .all(|op| !matches!(op, ProductionRankedOperationV1::OwnershipContract { .. }))
    );
    let mut sources = [
        Source::new(21, Some(7), 0, 1, 0),
        Source::new(29, Some(2), 0, 2, 0),
        Source::new(35, Some(5), 1, 3, 0),
    ];
    request
        .remap_source_sites_v2(&mut sources, &mut [])
        .unwrap();
    assert_eq!(
        sources,
        [
            Source::new(21, Some(7), 0, 1, 0),
            Source::new(29, Some(2), 0, 2, 0),
            Source::new(35, Some(5), 1, 3, 2),
        ]
    );
    assert!(request.source_site_remap.is_none());
}

#[test]
fn read_placement_cross_block_rejects_a_reachable_bypass_of_either_read() {
    for bypassed in [1, 2] {
        let (kernel, bindings, write, reserved) = cross_block_read_placement_fixture();
        let mut blocks = kernel.blocks().to_vec();
        let predecessor = bypassed - 1;
        blocks[predecessor] = ProductionRankedBlockV1::new(
            blocks[predecessor].operations().to_vec(),
            ProductionRankedTerminatorV1::IndexLessThan {
                lhs: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0)),
                rhs: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(1)),
                true_block: bypassed as u32,
                false_block: bypassed as u32 + 1,
            },
        );
        let kernel =
            ProductionRankedKernelV1::new(kernel.function_name(), kernel.argument_count(), blocks)
                .unwrap();
        let mut outputs = [cross_block_prepared_output(&bindings, write, reserved)];
        assert!(
            matches!(
                read_root_placement_v1::place(
                    &kernel,
                    &mut outputs,
                    &bindings.as_slice()[0].effect_ir,
                    &mut ReferenceSymbolicWorkBudgetV2::default()
                ),
                Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
                    "read-root placement requires each original read to dominate the write"
                ))
            ),
            "read block {bypassed} may be skipped on a path to the write"
        );
    }
}

#[test]
fn read_placement_cross_block_rejects_an_unreachable_retained_read() {
    let (kernel, bindings, write, reserved) = cross_block_read_placement_fixture();
    let mut blocks = kernel.blocks().to_vec();
    blocks[0] = ProductionRankedBlockV1::new(
        blocks[0].operations().to_vec(),
        ProductionRankedTerminatorV1::Branch { target: 2 },
    );
    let kernel =
        ProductionRankedKernelV1::new(kernel.function_name(), kernel.argument_count(), blocks)
            .unwrap();
    let mut outputs = [cross_block_prepared_output(&bindings, write, reserved)];
    assert!(matches!(
        read_root_placement_v1::place(
            &kernel,
            &mut outputs,
            &bindings.as_slice()[0].effect_ir,
            &mut ReferenceSymbolicWorkBudgetV2::default()
        ),
        Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
            "read-root placement requires reachable source and write sites"
        ))
    ));
}

#[test]
fn read_placement_cross_block_rejects_changed_read_metadata() {
    let (kernel, bindings, mut write, reserved) = cross_block_read_placement_fixture();
    let Ok(ProductionSemanticExpressionV2::Binary { lhs, .. }) = &mut write.value else {
        panic!()
    };
    let ProductionSemanticExpressionV2::Load(load) = lhs.as_mut() else {
        panic!()
    };
    load.view = ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(5));
    let mut outputs = [cross_block_prepared_output(&bindings, write, reserved)];
    assert!(matches!(
        read_root_placement_v1::place(
            &kernel,
            &mut outputs,
            &bindings.as_slice()[0].effect_ir,
            &mut ReferenceSymbolicWorkBudgetV2::default()
        ),
        Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
            "read-root placement lost the exact original read access"
        ))
    ));
}
