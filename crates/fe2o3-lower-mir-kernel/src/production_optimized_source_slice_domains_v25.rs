type GlobalStoreFactsV25<'s, 'g> = fe2o3_kernel_ir::CheckedCanonicalGuardedGlobalStoresV24<'s, 'g>;
type GlobalStoreFactV25<'s, 'g> = fe2o3_kernel_ir::CanonicalGuardedGlobalStoreFactV24<'s, 'g>;

fn slice_store_domain_headers_v25() -> Result<usize, ArgumentResourceV1> {
    use fe2o3_kernel_analysis::{
        CanonicalKirBlockRefV1, CanonicalKirEdgeRefV1, CanonicalKirInventoryV18,
        CanonicalKirOperationRefV1,
    };
    type Frame<'a> = (
        &'a PendingGlobalSourceAccessesV18<'a>,
        &'a GlobalStoreFactsV25<'a, 'a>,
        &'a GlobalStoreFactV25<'a, 'a>,
        &'a GlobalSourceAccessPairV18,
        &'a GlobalSourceAccessEndpointV18,
        &'a CanonicalKirInventoryV18<'a>,
        &'a mut ArgumentBudgetV1<'a>,
        fe2o3_kernel_ir::CanonicalGuardedGlobalStoreDomainV24,
        [&'a CanonicalKirDefinitionRefV1<'a>; 7],
        [&'a CanonicalKirOperationRefV1<'a>; 3],
        &'a CanonicalKirBlockRefV1<'a>,
        &'a CanonicalKirEdgeRefV1<'a>,
        fe2o3_kernel_ir::CanonicalGuardedReadIndexOriginV1,
        Option<fe2o3_kernel_ir::CanonicalGuardedPredicateFactV18<'a, 'a>>,
        fe2o3_kernel_ir::CanonicalGuardedPredicateFactV18<'a, 'a>,
        [ValueId; 4],
        [usize; 3],
        u16,
        bool,
        SourceOwnedResultV18<()>,
        fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1,
    );
    argument_sum_v1(&[
        source_global_domain_join_headers_v30()?,
        size_of::<Frame<'_>>(),
        argument_product_v1(2, size_of::<SourceOwnedResultV18<Frame<'_>>>())?,
    ])
}

impl PendingGlobalSourceAccessesV18<'_> {
    // This rejoins a genuine Store-only fact. No Store is converted to a read
    // fact, and the legacy read entrance and its accepted grammar are unchanged.
    fn check_local_store_domain_v25(
        &self,
        facts: &GlobalStoreFactsV25<'_, '_>,
        pair: &GlobalSourceAccessPairV18,
        fact: &GlobalStoreFactV25<'_, '_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        let original = self.original();
        original.global_expression_entry_v23(self.optimized(), budget)?;
        original.retain_query(self.roles.observe_custody(budget))?;
        let inventory = self
            .optimized()
            .pending_global_output_v18(original, budget)?;
        original.retain_query((|| {
            if !std::ptr::eq(fact.owner(), inventory.owner()) {
                return original.source.missing("slice Store fact owner differs");
            }
            Ok(())
        })())?;
        let run = |budget: &mut ArgumentBudgetV1<'_>| -> SourceOwnedResultV18<()> {
            budget.reserve_storage(slice_store_domain_headers_v25()?)?;
            let owner = facts
                .owner(budget)
                .map_err(|error| optimized_source_observed_formal_error_v18(original, &error))?;
            if !std::ptr::eq(owner, inventory.owner())
                || self.access(pair.output.logical.access.operation, budget)? != Some(pair)
            {
                return original
                    .source
                    .missing("slice Store source or formal scope differs");
            }
            check_source_store_endpoint_prepaid_v30(original, inventory, facts, pair, fact, budget)
        };
        let frame = argument_sum_v1(&[std::mem::size_of_val(&run), std::mem::align_of_val(&run)])?;
        original.retain_query(source_scalar_normalization_scratch_v18(
            original.source.cleanup,
            budget,
            frame,
            run,
        ))
    }
}

include!("production_optimized_source_slice_completion_v25.rs");
