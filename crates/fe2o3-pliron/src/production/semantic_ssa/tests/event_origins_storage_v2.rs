use super::*;

fn limits_with_storage(storage: usize) -> ProductionSemanticSsaLimitsV1 {
    let broad = SsaPlannerLimitsV1::default();
    ProductionSemanticSsaLimitsV1::new(
        SsaPlannerLimitsV1::try_new(
            broad.max_variables(),
            broad.max_blocks(),
            broad.max_edges(),
            broad.max_events(),
            broad.max_edge_definitions(),
            broad.max_output_items(),
            storage,
            broad.max_work_units(),
        )
        .unwrap(),
    )
}

#[test]
fn retained_source_storage_production_constructor_precharges_exact_capacity() {
    let function = test_function(vec![test_block(
        0,
        vec![
            SemanticStatementV1::new(
                fe2o3_mir_model::semantic_mir_v1::SemanticSourceProvenanceV1::unavailable(),
                SemanticStatementKindV1::Nop,
            ),
            test_assign(2, SemanticOperandV1::Move(test_typed_place(1, 0))),
        ],
        SemanticTerminatorKindV1::Return,
    )]);
    let id = SemanticFunctionIdV1::from_index(0);
    let mut origins = execution::ExecutionEventOriginsV1::for_function(
        id,
        &function,
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    adapter::semantic_function_ssa_input_with_event_origins_v1(
        &function,
        None,
        &[],
        &BTreeSet::new(),
        |block, statement, events| origins.record(block, statement, events),
    );
    let required = origins.resources().unwrap().storage_words;
    let exact = execution::ExecutionEventOriginsV1::for_function(
        id,
        &function,
        limits_with_storage(required),
    );
    assert!(exact.is_ok());
    assert_eq!(
        execution::ExecutionEventOriginsV1::for_function(
            id,
            &function,
            limits_with_storage(required - 1)
        ),
        Err(ProductionSemanticSsaErrorV1::PartialMoveResourceLimit {
            function: id,
            resource: SsaPlannerResourceV1::StorageWords,
            required,
            limit: required - 1,
        })
    );
}

#[test]
fn retained_source_storage_4272_blocks_record_every_adapter_site_without_growth() {
    let function = test_function(
        (0..4272)
            .map(|_| {
                test_block(
                    0,
                    vec![
        SemanticStatementV1::new(
            fe2o3_mir_model::semantic_mir_v1::SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::Nop,
        ),
        test_assign(2, SemanticOperandV1::Move(test_typed_place(1, 0))),
    ],
                    SemanticTerminatorKindV1::Return,
                )
            })
            .collect(),
    );
    let mut origins = execution::ExecutionEventOriginsV1::for_function(
        SemanticFunctionIdV1::from_index(0),
        &function,
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let mut nonempty = 0_usize;
    let (input, _, _) = adapter::semantic_function_ssa_input_with_event_origins_v1(
        &function,
        None,
        &[],
        &BTreeSet::new(),
        |block, statement, events| {
            nonempty += usize::from(!events.is_empty());
            origins.record(block, statement, events);
        },
    );
    let resources = origins.resources().unwrap();
    for block in 0..4272 {
        let ends = origins.block_ends(block).unwrap();
        assert_eq!(ends.len(), 3);
        assert_eq!(
            *ends.last().unwrap() as usize,
            input.blocks()[block as usize].events().len()
        );
        for event in 0..input.blocks()[block as usize].events().len() {
            assert_eq!(origins.statement(block, event as u32), Some(Some(1)));
        }
    }
    let old_words = (6 * nonempty + 8)
        * std::mem::size_of::<(u32, usize, Option<u32>)>().div_ceil(std::mem::size_of::<usize>())
        + 6;
    assert!(old_words - resources.storage_words > 57_156);
    eprintln!(
        "synthetic 4272-block adapter geometry: old={old_words} compact={} sites={} nonempty={nonempty}; not actual ContentSparse measurement",
        resources.storage_words,
        4272 * 3
    );
}
