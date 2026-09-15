mod receipt_endpoint_tests {
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKirTransitionReceiptStorageV1 as ReceiptStorage,
        InertCanonicalKirTransitionReceiptV1 as Receipt,
    };
    use fe2o3_lower_mir_kernel::derive_source_output_occurrences_from_receipt_v1;

    fn sparse_source() -> ProductionPreRankedKirOwnerV1 {
        assertion_materialized(assertion_root_with_access(
            vec![
                (A_UNIT, SemanticLocalRoleV1::Return),
                (A_ARRAY, SemanticLocalRoleV1::Temporary),
                (A_U32, SemanticLocalRoleV1::Temporary),
            ],
            vec![],
            vec![
                block(
                    201,
                    vec![],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 2)),
                ),
                block(202, vec![], SemanticTerminatorKindV1::Return),
                block(
                    203,
                    assertion_ranked_write_statements(1, 2),
                    SemanticTerminatorKindV1::Return,
                ),
            ],
            false,
        ))
    }

    fn receipt(
        bound: &Owner,
        checked: &CheckedNeutralKernelIrOwnerV1,
        swap: bool,
        budget: &mut Budget<'_>,
    ) -> (Receipt, ReceiptStorage) {
        let (input, output) = if swap {
            (
                checked.owner().canonical().identity(),
                bound.canonical().identity(),
            )
        } else {
            (
                bound.canonical().identity(),
                checked.owner().canonical().identity(),
            )
        };
        let (encoded, encoded_storage) = Receipt::from_candidate_with_budget(
            input,
            output,
            checked.occurrences().candidate(),
            budget,
        )
        .unwrap();
        budget
            .reserve_storage(encoded_storage.retained_storage())
            .unwrap();
        let (receipt, storage) =
            Receipt::decode_with_budget(encoded.canonical_bytes(), budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        drop(encoded);
        budget
            .release_storage(encoded_storage.retained_storage())
            .unwrap();
        (receipt, storage)
    }

    #[test]
    fn producer_and_receipt_consumers_share_source_placement_and_actual_output_catalogs() {
        for (source_kind, source) in [
            sparse_source(),
            pipeline_source(false),
            pipeline_source(true),
        ]
        .into_iter()
        .enumerate()
        {
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                with_actual(&source, profile, |bound, checked, budget| {
                    let floor = budget.storage();
                    if source_kind == 0 {
                        let operations = |owner: &Owner| {
                            owner
                                .module()
                                .functions
                                .iter()
                                .filter_map(|function| function.body.as_ref())
                                .flat_map(|body| &body.blocks)
                                .map(|block| block.operations.len())
                                .sum::<usize>()
                        };
                        assert_eq!(operations(bound), 7);
                        assert_eq!(operations(checked.owner()), 6);
                    }
                    let (coordinates, coordinate_storage) =
                        dialect_amdgcn::check_production_target_coordinate_preservation_v1(
                            source.executable(),
                            bound,
                            profile,
                            budget,
                        )
                        .unwrap();
                    budget
                        .reserve_storage(coordinate_storage.retained_storage())
                        .unwrap();
                    let (wire, wire_storage) = receipt(bound, checked, false, budget);
                    // This is the independent consumer's one admission of supplied
                    // O bytes. The producer keeps its original move-only O owner.
                    let (output, output_storage) =
                        Owner::from_canonical_bytes_with_verification_budget_v12(
                            checked.owner().canonical().canonical_bytes(),
                            budget,
                        )
                        .unwrap();
                    budget
                        .reserve_storage(output_storage.retained_storage())
                        .unwrap();
                    assert!(!std::ptr::eq(&output, checked.owner()));
                    assert_eq!(
                        output.canonical().identity(),
                        checked.owner().canonical().identity()
                    );
                    let (producer, producer_storage) =
                        derive_source_output_occurrences_v1(&source, &coordinates, checked, budget)
                            .unwrap();
                    budget
                        .reserve_storage(producer_storage.retained_storage())
                        .unwrap();
                    let (consumer, consumer_storage) =
                        derive_source_output_occurrences_from_receipt_v1(
                            &source,
                            &coordinates,
                            &output,
                            output_storage,
                            &wire,
                            wire_storage,
                            budget,
                        )
                        .unwrap();
                    budget
                        .reserve_storage(consumer_storage.retained_storage())
                        .unwrap();
                    assert!(std::ptr::eq(producer.output(), checked.owner()));
                    assert!(std::ptr::eq(consumer.output(), &output));
                    assert!(!consumer.grants_authority());
                    assert_eq!(
                        producer
                            .input_pipeline_catalog(budget)
                            .unwrap()
                            .canonical_bytes(),
                        consumer
                            .input_pipeline_catalog(budget)
                            .unwrap()
                            .canonical_bytes(),
                    );
                    assert_eq!(
                        producer
                            .output_pipeline_catalog(budget)
                            .unwrap()
                            .canonical_bytes(),
                        consumer
                            .output_pipeline_catalog(budget)
                            .unwrap()
                            .canonical_bytes(),
                    );
                    assert_eq!(
                        producer.pipeline_allocation_placements(budget).unwrap(),
                        consumer.pipeline_allocation_placements(budget).unwrap()
                    );
                    assert_eq!(
                        producer.pipeline_marker_placements(budget).unwrap(),
                        consumer.pipeline_marker_placements(budget).unwrap()
                    );
                    let semantic = source.semantic_ssa().source_semantic();
                    let mut covered = 0;
                    for (function_index, function) in semantic.functions().iter().enumerate() {
                        let function_id = SemanticFunctionIdV1::from_index(
                            u32::try_from(function_index).unwrap(),
                        );
                        for block_index in 0..function.blocks().len() {
                            let block_id =
                                SemanticBlockIdV1::from_index(u32::try_from(block_index).unwrap());
                            let p = producer.block_coverage(ROOT, function_id, block_id, budget);
                            let c = consumer.block_coverage(ROOT, function_id, block_id, budget);
                            match (p, c) {
                                (Ok(p), Ok(c)) => {
                                    covered += 1;
                                    assert_eq!(p.disposition(), c.disposition());
                                    assert_eq!(p.source_statements(), c.source_statements());
                                    assert_eq!(p.original_operations(), c.original_operations());
                                }
                                (Err(p), Err(c)) => assert_eq!(format!("{p:?}"), format!("{c:?}")),
                                _ => panic!("producer/consumer block coverage differs"),
                            }
                        }
                    }
                    assert!(covered > 0);
                    drop(consumer);
                    budget
                        .release_storage(consumer_storage.retained_storage())
                        .unwrap();
                    drop(producer);
                    budget
                        .release_storage(producer_storage.retained_storage())
                        .unwrap();
                    drop(output);
                    budget
                        .release_storage(output_storage.retained_storage())
                        .unwrap();
                    drop(wire);
                    budget
                        .release_storage(wire_storage.retained_storage())
                        .unwrap();
                    drop(coordinates);
                    budget
                        .release_storage(coordinate_storage.retained_storage())
                        .unwrap();
                    assert_eq!(budget.storage(), floor);
                });
            }
        }
    }

    #[test]
    fn receipt_source_consumer_rejects_swapped_endpoints_without_replacing_actual_output() {
        let source = sparse_source();
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_actual(&source, profile, |bound, checked, budget| {
                assert_ne!(
                    bound.canonical().identity(),
                    checked.owner().canonical().identity()
                );
                let (coordinates, coordinate_storage) =
                    dialect_amdgcn::check_production_target_coordinate_preservation_v1(
                        source.executable(),
                        bound,
                        profile,
                        budget,
                    )
                    .unwrap();
                budget
                    .reserve_storage(coordinate_storage.retained_storage())
                    .unwrap();
                let (wire, wire_storage) = receipt(bound, checked, true, budget);
                let (output, output_storage) =
                    Owner::from_canonical_bytes_with_verification_budget_v12(
                        checked.owner().canonical().canonical_bytes(),
                        budget,
                    )
                    .unwrap();
                budget
                    .reserve_storage(output_storage.retained_storage())
                    .unwrap();
                let floor = budget.storage();
                let result = derive_source_output_occurrences_from_receipt_v1(
                    &source,
                    &coordinates,
                    &output,
                    output_storage,
                    &wire,
                    wire_storage,
                    budget,
                );
                assert!(matches!(
                    result,
                    Err(ProductionSourceOutputErrorV1::Transition(_))
                ));
                assert_eq!(budget.storage(), floor);
                assert_eq!(
                    output.canonical().identity(),
                    checked.owner().canonical().identity()
                );
                drop(output);
                budget
                    .release_storage(output_storage.retained_storage())
                    .unwrap();
                drop(wire);
                budget
                    .release_storage(wire_storage.retained_storage())
                    .unwrap();
                drop(coordinates);
                budget
                    .release_storage(coordinate_storage.retained_storage())
                    .unwrap();
            });
        }
    }

    #[test]
    fn receipt_source_consumer_preserves_live_prefix_on_work_and_storage_denial() {
        // Receipt transfer addition = 1; common source/floor checks = 5;
        // endpoint/history dispatch = 1. These are independently counted
        // adapter prefixes, not measured whole-query success ceilings.
        for remaining_work in [None, Some(0), Some(1), Some(6), Some(7)] {
            let source = sparse_source();
            with_actual(&source, Profile::Gfx942, |bound, checked, budget| {
                let (coordinates, coordinate_storage) =
                    dialect_amdgcn::check_production_target_coordinate_preservation_v1(
                        source.executable(),
                        bound,
                        Profile::Gfx942,
                        budget,
                    )
                    .unwrap();
                budget
                    .reserve_storage(coordinate_storage.retained_storage())
                    .unwrap();
                let (wire, wire_storage) = receipt(bound, checked, false, budget);
                let (output, output_storage) =
                    Owner::from_canonical_bytes_with_verification_budget_v12(
                        checked.owner().canonical().canonical_bytes(),
                        budget,
                    )
                    .unwrap();
                budget
                    .reserve_storage(output_storage.retained_storage())
                    .unwrap();
                let prefix = budget.storage();
                let extra = if remaining_work.is_some() {
                    0
                } else {
                    budget.storage_limit() - prefix
                };
                budget.reserve_storage(extra).unwrap();
                if let Some(remaining) = remaining_work {
                    budget
                        .charge_work(LIMIT - budget.work() - remaining)
                        .unwrap();
                }
                let floor = budget.storage();
                let before = budget.work();
                let result = derive_source_output_occurrences_from_receipt_v1(
                    &source,
                    &coordinates,
                    &output,
                    output_storage,
                    &wire,
                    wire_storage,
                    budget,
                );
                assert!(result.is_err());
                drop(result);
                assert_eq!(budget.storage(), floor);
                if let Some(remaining) = remaining_work {
                    assert_eq!(budget.work(), before + remaining);
                    assert_eq!(budget.failed_storage(), None);
                } else {
                    assert!(budget.work() > before);
                    assert!(budget.failed_storage().unwrap() > floor);
                }
                budget.release_storage(extra).unwrap();
                assert_eq!(budget.storage(), prefix);
                drop(output);
                budget
                    .release_storage(output_storage.retained_storage())
                    .unwrap();
                drop(wire);
                budget
                    .release_storage(wire_storage.retained_storage())
                    .unwrap();
                drop(coordinates);
                budget
                    .release_storage(coordinate_storage.retained_storage())
                    .unwrap();
            });
        }
    }
}
