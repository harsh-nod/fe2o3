fn check_array_rows(row: &PendingInstanceSidecarsV29, function: SemanticFunctionIdV1) {
    let arrays = &row.private_arrays;
    assert!(arrays.active);
    let placement = row.lifecycle_events.as_ref().unwrap().placement;
    assert_ne!(placement, SemanticEmissionPlacementV1::default());
    assert_eq!(arrays.placement, placement);
    assert_eq!(arrays.slots.len(), 1);
    assert_eq!(arrays.effects.len(), 2);
    let payload = size_of::<PrivateArraySlotV1>() + 2 * size_of::<PrivateArrayEffectV1>();
    assert_eq!(arrays.payload.occupied, payload);
    assert_eq!(arrays.payload.capacity, payload);
    let local = if function == HELPER { 6 } else { 3 };
    let location = |operation| PrivateArrayPhysicalLocationV1 {
        block_ordinal: 0,
        block: placement.block(0).unwrap(),
        operation,
    };
    let slot = arrays.slots[0];
    assert_eq!(
        (slot.owner, slot.function, slot.local),
        (ROOT, function, local)
    );
    assert_eq!(slot.count_location, location(0));
    assert_eq!(slot.alloca_location, location(1));
    assert_eq!(slot.length, 1);
    assert_eq!(slot.element_type, U32);
    for (index, effect) in arrays.effects.iter().enumerate() {
        assert_eq!(
            (effect.owner, effect.function, effect.local),
            (ROOT, function, local)
        );
        assert_eq!(effect.semantic_block, 0);
        assert_eq!(effect.semantic_statement, index as u32 + 1);
        assert_eq!(
            (effect.source_first_operation, effect.source_end_operation),
            if index == 0 { (3, 7) } else { (7, 11) }
        );
        assert_eq!(effect.access, PrivateArrayAccessV1::Write);
        assert_eq!(effect.offset_location, Some(location(4 + 4 * index)));
        assert_eq!(effect.gep_location, location(5 + 4 * index));
        assert_eq!(effect.memory_location, location(6 + 4 * index));
    }
    assert!(matches!(arrays.effects[0].original_index,
        PrivateArrayIndexV1::InitializerElement { component: 0, value: PrivateArrayInitializerValueV1::LiteralScalar { definition, .. } } if definition == location(3)));
    assert!(matches!(arrays.effects[1].original_index,
        PrivateArrayIndexV1::Local { local: index, direct_definition: Some(definition), .. } if index == local + 1 && definition == location(2)));
}

fn placed_array_recorder(
    first_block: u32,
    limit: usize,
) -> PrivateArrayFunctionRecorderV1<'static> {
    let mut work = PrivateArrayLazyBudgetV1::new(1, limit);
    work.activate().unwrap();
    PrivateArrayFunctionRecorderV1::new(
        PrivateArrayRecorderWorkV1::Owned(work),
        true,
        16,
        PrivateArrayPayloadV1::default(),
        SemanticEmissionPlacementV1 {
            first_block,
            first_value: 0,
        },
    )
}

#[test]
fn scoped_array_recorder_rejects_wrong_blocks_without_state_changes() {
    let mut recorder = placed_array_recorder(17, 16);
    let source = SemanticBlockIdV1::from_index(2);
    assert!(matches!(
        recorder.begin_block(source, BlockId(18)),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
    assert!(recorder.block.is_none());
    assert!(recorder.frame.is_none());
    assert!(recorder.pending.is_none());
    recorder.begin_block(source, BlockId(19)).unwrap();
    let expected = PrivateArrayPhysicalLocationV1 {
        block_ordinal: 0,
        block: BlockId(19),
        operation: 5,
    };
    assert_eq!(recorder.location(5).unwrap(), expected);
    assert!(recorder.begin_block(source, BlockId(20)).is_err());
    assert_eq!(recorder.location(5).unwrap(), expected);
}

#[test]
fn scoped_array_recorder_checks_overflow_and_exact_work() {
    let mut recorder = placed_array_recorder(u32::MAX, 16);
    assert!(matches!(
        recorder.begin_block(SemanticBlockIdV1::from_index(1), BlockId(0)),
        Err(ProductionSemanticKirErrorV1::Unsupported {
            detail: "expanded block identity overflow",
            ..
        })
    ));
    assert!(recorder.block.is_none());
    for limit in [2, 3] {
        let mut recorder = placed_array_recorder(17, limit);
        let result = recorder.begin_block(SemanticBlockIdV1::from_index(2), BlockId(19));
        if limit == 3 {
            result.unwrap();
        } else {
            assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::ResourceLimit {
                    resource: ProductionSemanticKirResourceV1::AnalysisWork,
                    actual: 3,
                    limit: 2,
                })
            ));
            assert!(recorder.block.is_none());
        }
    }
}
