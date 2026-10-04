use super::*;

impl ProductionOwnedPrivateCellPromotionContinuationV1 {
    // Tests the private origin-transport leaf with an actual checked pair from
    // an earlier genuine source graph. This does not admit that graph as P8.
    pub(crate) fn exercise_private_cell_load_source_transport_v1(
        input: &StoreOwner,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) {
        let floor = budget.storage();
        promotion_scoped(floor, budget, |budget, binding| {
            let tail = prepare_promotion(input, budget).map_err(PError::Continuation)?;
            binding.check(budget)?;
            budget.reserve_storage(tail.retained_storage())?;
            assert!(!tail.selected_allocations().is_empty());
            let (relation, storage) = tail
                .replay_against(input, budget)
                .map_err(PError::Continuation)?;
            budget.reserve_storage(storage.retained_storage())?;
            let (input, storage) =
                CanonicalKirInventoryV1::derive(input, budget).map_err(inventory_error)?;
            budget.reserve_storage(storage.retained_storage())?;
            let (output, storage) =
                CanonicalKirInventoryV1::derive(tail.output(), budget).map_err(inventory_error)?;
            budget.reserve_storage(storage.retained_storage())?;
            let mapping = PromotionMapping {
                input: &input,
                output: &output,
                relation: &relation,
            };
            let mut sites = scratch::<Site>(input.operations().len(), budget)?;
            for ordinal in 0..input.operations().len() {
                sites.push(Some((
                    SemanticFunctionIdV1::from_index(0),
                    SemanticBlockIdV1::from_index(0),
                    u32::try_from(ordinal).unwrap(),
                )));
            }
            let (mapped, traps) = promoted_sites_and_traps(&mapping, &sites, budget)?;
            assert_eq!(mapped.len(), output.operations().len());
            assert_eq!(traps.len(), mapped.len());
            let mut copies = 0;
            for (ordinal, row) in relation.origins().iter().enumerate() {
                let old = operation_ordinal(&input, row.input)?;
                assert_eq!(mapped[ordinal], sites[old]);
                if let PromotionOriginKind::LoadCopy { previous_store, .. } = row.kind {
                    copies += 1;
                    let stored = operation_ordinal(&input, previous_store)?;
                    assert_ne!(old, stored);
                    assert_ne!(mapped[ordinal], sites[stored]);
                    assert!(!traps[ordinal]);
                }
            }
            assert!(copies > 0, "must exercise an actual checked LoadCopy");
            assert!(matches!(
                promoted_sites_and_traps(&mapping, &sites[..sites.len() - 1], budget),
                Err(PError::Resource(AssertOriginResourceV1::Accounting))
            ));
            let foreign_mapping = PromotionMapping {
                input: &input,
                output: &input,
                relation: &relation,
            };
            assert!(matches!(
                promoted_sites_and_traps(&foreign_mapping, &sites, budget),
                Err(PError::Resource(AssertOriginResourceV1::Accounting))
            ));
            binding.check(budget)?;
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), floor);
    }
}
