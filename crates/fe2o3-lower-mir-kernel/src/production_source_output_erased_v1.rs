include!("production_source_output_erased_coordinates_v1.rs");
include!("production_source_output_erased_assertions_v1.rs");

#[derive(Clone, Copy, Debug)]
// Production checks dispositions; tests also query each retained coordinate.
#[cfg_attr(not(test), allow(dead_code))]
enum ErasedSourceBlockV1 {
    NotMaterialized,
    DeletedLocalHelper {
        original: ErasedBlockV1,
    },
    Retained {
        original: ErasedBlockV1,
        erased: ErasedBlockV1,
        checked: fe2o3_kernel_analysis::CanonicalKirBlockControlV1,
    },
}

#[derive(Clone, Copy)]
struct ErasedSourceBlockRowV1 {
    site: SemanticKirAssertSiteV1,
    disposition: ErasedSourceBlockV1,
}

#[cfg_attr(not(test), allow(dead_code))]
enum ErasedSourceAssertionV1<'a> {
    Retained(&'a SemanticKirOptimizedAssertBindingV1),
    DeletedLocalHelper {
        original: SemanticKirAssertConditionBindingV1,
    },
}

// This view and every borrow from it are confined to a fresh N/E replay. C is
// the actual Policy3 endpoint, never silently relabeled as final Policy4 O.
struct ErasedSourceOutputOccurrencesV1<'s> {
    coordinates: ErasedSourceCoordinateMapV1<'s>,
    // Retain scoped endpoint and proof custody even when only tests query it.
    #[cfg_attr(not(test), allow(dead_code))]
    bound: &'s fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    #[cfg_attr(not(test), allow(dead_code))]
    checked: Option<&'s fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1>,
    blocks: Vec<ErasedSourceBlockRowV1>,
    #[cfg_attr(not(test), allow(dead_code))]
    assertion_indices: Vec<Option<usize>>,
    #[cfg_attr(not(test), allow(dead_code))]
    assertions: SemanticKirOptimizedAssertOriginOwnerV1,
    #[cfg_attr(not(test), allow(dead_code))]
    catalogs: SourceOutputCatalogsV1,
}

impl ErasedSourceOutputOccurrencesV1<'_> {
    fn coordinates(&self) -> &ErasedSourceCoordinateMapV1<'_> {
        &self.coordinates
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn bound(
        &self,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> ErasedOccurrenceResultV1<&fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12> {
        self.coordinates.live(budget)?;
        Ok(self.bound)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn checked_output(
        &self,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> ErasedOccurrenceResultV1<&fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1> {
        self.coordinates.live(budget)?;
        // Decoded actual-pair custody is not a producer execution witness.
        self.checked
            .ok_or(ProductionSourceOutputErrorV1::InputCustody)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn assertions(
        &self,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> ErasedOccurrenceResultV1<&SemanticKirOptimizedAssertOriginOwnerV1> {
        self.coordinates.live(budget)?;
        Ok(&self.assertions)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn assertion(
        &self,
        root: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
        block: SemanticBlockIdV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> ErasedOccurrenceResultV1<ErasedSourceAssertionV1<'_>> {
        self.coordinates.live(budget)?;
        let site = SemanticKirAssertSiteV1::new(root, function, block);
        let original = self.coordinates.deletion.source().assert_origins();
        let ordinal = assert_origin_find_v1(&original.origins.aliases, budget, |row, budget| {
            source_output_site_cmp_v1(row.site, site, budget)
        })
        .map_err(ProductionSourceOutputErrorV1::SourceOrigin)?
        .ok_or_else(|| erased_occurrence_invalid_v1("source assertion site"))?;
        erased_occurrence_charge_v1(budget, 3)?;
        let original_index = original.origins.aliases[ordinal].binding;
        match self
            .assertion_indices
            .get(original_index)
            .ok_or_else(|| erased_occurrence_invalid_v1("total source assertion disposition"))?
        {
            Some(index) => Ok(ErasedSourceAssertionV1::Retained(
                self.assertions
                    .bindings
                    .get(*index)
                    .ok_or_else(|| erased_occurrence_invalid_v1("retained assertion index"))?,
            )),
            None => Ok(ErasedSourceAssertionV1::DeletedLocalHelper {
                original: original
                    .origins
                    .bindings
                    .get(original_index)
                    .copied()
                    .ok_or_else(|| erased_occurrence_invalid_v1("deleted original assertion"))?,
            }),
        }
    }

    fn block(
        &self,
        root: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
        block: SemanticBlockIdV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> ErasedOccurrenceResultV1<ErasedSourceBlockV1> {
        self.coordinates.live(budget)?;
        if !self
            .coordinates
            .deletion
            .source()
            .assert_origins()
            .is_materialized_block(root, function, block, budget)
            .map_err(ProductionSourceOutputErrorV1::SourceOrigin)?
        {
            return Ok(ErasedSourceBlockV1::NotMaterialized);
        }
        let site = SemanticKirAssertSiteV1::new(root, function, block);
        let ordinal = assert_origin_find_v1(&self.blocks, budget, |row, budget| {
            source_output_site_cmp_v1(row.site, site, budget)
        })
        .map_err(ProductionSourceOutputErrorV1::SourceOrigin)?
        .ok_or_else(|| erased_occurrence_invalid_v1("total materialized block disposition"))?;
        erased_occurrence_charge_v1(budget, 1)?;
        Ok(self.blocks[ordinal].disposition)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn input_pipeline_catalog(
        &self,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> ErasedOccurrenceResultV1<&fe2o3_kernel_ir::InertCanonicalKernelIrContractCatalogV1> {
        self.coordinates.live(budget)?;
        Ok(&self.catalogs.source)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn output_pipeline_catalog(
        &self,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> ErasedOccurrenceResultV1<&fe2o3_kernel_ir::InertCanonicalKernelIrContractCatalogV1> {
        self.coordinates.live(budget)?;
        Ok(self.catalogs.transported.catalog())
    }
}

fn erased_source_blocks_v1(
    map: &ErasedSourceCoordinateMapV1<'_>,
    control: &CheckedCanonicalKirControlIndexV1<'_, '_, '_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> ErasedOccurrenceResultV1<Vec<ErasedSourceBlockRowV1>> {
    let source = map.deletion.source();
    let mut blocks = erased_occurrence_vec_v1(source.correspondence.blocks.len(), budget)?;
    for row in &source.correspondence.blocks {
        erased_occurrence_charge_v1(budget, 3)?;
        let site = SemanticKirAssertSiteV1::new(
            row.correspondence_owner,
            row.semantic_function,
            row.semantic_block,
        );
        if !source
            .assert_origins()
            .is_materialized_block(
                row.correspondence_owner,
                row.semantic_function,
                row.semantic_block,
                budget,
            )
            .map_err(ProductionSourceOutputErrorV1::SourceOrigin)?
        {
            return Err(erased_occurrence_invalid_v1(
                "correspondence materialized block",
            ));
        }
        let function =
            map.source_function(row.correspondence_owner, row.semantic_function, budget)?;
        let original = map
            .inventory()
            .block_for_id(function, row.kernel_ir_block, budget)
            .map_err(ProductionSourceOutputErrorV1::Inventory)?
            .ok_or_else(|| erased_occurrence_invalid_v1("original source block"))?
            .coordinate;
        let disposition = match map.block(original, budget)? {
            ErasedSourceOccurrenceV1::Retained(erased) => ErasedSourceBlockV1::Retained {
                original,
                erased,
                checked: control
                    .block(erased, budget)
                    .map_err(ProductionSourceOutputErrorV1::Transition)?,
            },
            ErasedSourceOccurrenceV1::DeletedLocalHelper => {
                ErasedSourceBlockV1::DeletedLocalHelper { original }
            }
            ErasedSourceOccurrenceV1::DeletedUnitCall => {
                return Err(erased_occurrence_invalid_v1("source block is not a call"));
            }
        };
        erased_occurrence_push_v1(
            &mut blocks,
            ErasedSourceBlockRowV1 { site, disposition },
            budget,
        )?;
    }
    assert_origin_sort_v1(&mut blocks, budget, |a, b, budget| {
        source_output_site_cmp_v1(a.site, b.site, budget)
    })
    .map_err(ProductionSourceOutputErrorV1::SourceOrigin)?;
    for pair in blocks.windows(2) {
        erased_occurrence_charge_v1(budget, 3)?;
        if pair[0].site == pair[1].site {
            return Err(erased_occurrence_invalid_v1(
                "unique materialized source block",
            ));
        }
    }
    Ok(blocks)
}

// The caller retains/reserves B, its coordinate/transition/control witnesses and
// both checked owners. New canonical-ledger result/captured payload must be
// prepaid before entry; inherited formal-engine exclusions remain unchanged.
// Callback scratch must be dropped/released to the exact callback floor.
fn with_erased_source_output_occurrences_v1<'w, R>(
    source: &ProductionUnitLocalErasedSourceOwnerV1,
    coordinates: &fe2o3_kernel_analysis::CheckedCanonicalKirCoordinatePreservationV1<'_, '_>,
    checked: &fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1,
    transition: &fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV1<'_, '_, '_, '_>,
    control: &CheckedCanonicalKirControlIndexV1<'_, '_, '_>,
    budget: &mut AssertOriginBudgetV1<'w>,
    next: impl for<'s> FnOnce(
        &ErasedSourceOutputOccurrencesV1<'s>,
        &mut AssertOriginBudgetV1<'w>,
    ) -> ErasedOccurrenceResultV1<R>,
) -> ErasedOccurrenceResultV1<R> {
    erased_occurrence_charge_v1(budget, 20)?;
    let required = source
        .retained_storage_floor_v1()
        .checked_add(checked.storage().retained_storage())
        .ok_or(ProductionSourceOutputErrorV1::Resource(
            AssertOriginResourceV1::Arithmetic,
        ))?;
    if budget.storage() < required {
        return Err(ProductionSourceOutputErrorV1::Resource(
            AssertOriginResourceV1::Accounting,
        ));
    }
    erased_source_output_pair_custody_v1(
        source,
        coordinates,
        checked.owner(),
        checked.occurrences().candidate(),
        checked.native_input_audit_bytes(),
        transition,
        control,
        budget,
    )?;
    source
        .with_checked_erasure_v1(budget, |deletion, budget| {
            Ok(erased_source_output_scope_v1(
                deletion,
                coordinates,
                Some(checked),
                transition,
                control,
                budget,
                next,
            ))
        })
        .map_err(ProductionSourceOutputErrorV1::SourceReplay)?
}

// Numeric prepayment is only a caller storage contract, never semantic or
// execution authority. Graphs, all nine row slices, indexes, audit bytes and
// callback result/captures stay paid on this ledger throughout the callback.
#[allow(clippy::too_many_arguments)]
fn with_erased_source_output_pair_v1<'w, R>(
    source: &ProductionUnitLocalErasedSourceOwnerV1,
    coordinates: &fe2o3_kernel_analysis::CheckedCanonicalKirCoordinatePreservationV1<'_, '_>,
    output: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    submitted: fe2o3_kernel_ir::CanonicalKirTransitionCandidateV1<'_>,
    historical: &[u8],
    required: usize,
    transition: &fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV1<'_, '_, '_, '_>,
    control: &CheckedCanonicalKirControlIndexV1<'_, '_, '_>,
    budget: &mut AssertOriginBudgetV1<'w>,
    next: impl for<'s> FnOnce(
        &ErasedSourceOutputOccurrencesV1<'s>,
        &mut AssertOriginBudgetV1<'w>,
    ) -> ErasedOccurrenceResultV1<R>,
) -> ErasedOccurrenceResultV1<R> {
    erased_occurrence_charge_v1(budget, 20)?;
    if required < source.retained_storage_floor_v1() || budget.storage() < required {
        return Err(ProductionSourceOutputErrorV1::Resource(
            AssertOriginResourceV1::Accounting,
        ));
    }
    erased_source_output_pair_custody_v1(
        source,
        coordinates,
        output,
        submitted,
        historical,
        transition,
        control,
        budget,
    )?;
    source
        .with_checked_erasure_v1(budget, |deletion, budget| {
            Ok(erased_source_output_scope_v1(
                deletion,
                coordinates,
                None,
                transition,
                control,
                budget,
                next,
            ))
        })
        .map_err(ProductionSourceOutputErrorV1::SourceReplay)?
}

#[allow(clippy::too_many_arguments)]
fn erased_source_output_pair_custody_v1(
    source: &ProductionUnitLocalErasedSourceOwnerV1,
    coordinates: &fe2o3_kernel_analysis::CheckedCanonicalKirCoordinatePreservationV1<'_, '_>,
    output: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    submitted: fe2o3_kernel_ir::CanonicalKirTransitionCandidateV1<'_>,
    historical: &[u8],
    transition: &fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV1<'_, '_, '_, '_>,
    control: &CheckedCanonicalKirControlIndexV1<'_, '_, '_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> ErasedOccurrenceResultV1<()> {
    let accepted = transition.rows();
    if !std::ptr::eq(source.erased(), coordinates.input())
        || !std::ptr::eq(coordinates.output(), control.input().owner())
        || !std::ptr::eq(output, control.output().owner())
        || !std::ptr::eq(transition.input(), control.input())
        || !std::ptr::eq(transition.output(), control.output())
        || !std::ptr::eq(accepted.functions, submitted.functions)
        || !std::ptr::eq(accepted.blocks, submitted.blocks)
        || !std::ptr::eq(accepted.segments, submitted.segments)
        || !std::ptr::eq(accepted.operations, submitted.operations)
        || !std::ptr::eq(accepted.definitions, submitted.definitions)
        || !std::ptr::eq(accepted.definition_outputs, submitted.definition_outputs)
        || !std::ptr::eq(accepted.uses, submitted.uses)
        || !std::ptr::eq(accepted.edges, submitted.edges)
        || !std::ptr::eq(accepted.edge_arguments, submitted.edge_arguments)
    {
        return Err(ProductionSourceOutputErrorV1::InputCustody);
    }
    let bound = coordinates.output();
    let bytes = bound.canonical().canonical_bytes();
    erased_occurrence_charge_v1(budget, 1)?;
    let bytes_work = bytes.len().checked_add(historical.len()).ok_or(
        ProductionSourceOutputErrorV1::Resource(AssertOriginResourceV1::Arithmetic),
    )?;
    erased_occurrence_charge_v1(budget, bytes_work)?;
    if bytes != historical {
        return Err(ProductionSourceOutputErrorV1::InputCustody);
    }
    Ok(())
}

fn erased_source_output_scope_v1<'w, R>(
    deletion: &CheckedUnitLocalCallDeletionV1<'_>,
    coordinates: &fe2o3_kernel_analysis::CheckedCanonicalKirCoordinatePreservationV1<'_, '_>,
    checked: Option<&fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1>,
    transition: &fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV1<'_, '_, '_, '_>,
    control: &CheckedCanonicalKirControlIndexV1<'_, '_, '_>,
    budget: &mut AssertOriginBudgetV1<'w>,
    next: impl for<'s> FnOnce(
        &ErasedSourceOutputOccurrencesV1<'s>,
        &mut AssertOriginBudgetV1<'w>,
    ) -> ErasedOccurrenceResultV1<R>,
) -> ErasedOccurrenceResultV1<R> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let slot = budget as *const AssertOriginBudgetV1<'_> as usize;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        erased_occurrence_charge_v1(budget, 9)?;
        if !std::ptr::eq(deletion.output(), coordinates.input()) {
            return Err(ProductionSourceOutputErrorV1::InputCustody);
        }
        let header = std::mem::size_of::<ErasedSourceOutputOccurrencesV1<'_>>()
            .checked_sub(std::mem::size_of::<SemanticKirOptimizedAssertOriginOwnerV1>())
            .and_then(|n| n.checked_sub(std::mem::size_of::<SourceOutputCatalogsV1>()))
            .and_then(|n| n.checked_add(std::mem::size_of::<SealedAssertOriginsV1>()))
            .and_then(|n| n.checked_add(std::mem::size_of::<Vec<ErasedDeletedAssertIndexV1>>()))
            .ok_or(ProductionSourceOutputErrorV1::Resource(
                AssertOriginResourceV1::Arithmetic,
            ))?;
        budget
            .reserve_storage(header)
            .map_err(ProductionSourceOutputErrorV1::Resource)?;
        let mut map = ErasedSourceCoordinateMapV1 {
            deletion,
            floor: budget.storage(),
        };
        let (origins, assertion_indices) = erased_assertion_origins_v1(&map, budget)?;
        let original = deletion.source().assert_origins();
        let erased_origins = SemanticKirAssertOriginsV1 {
            executable: deletion.output(),
            semantic_ssa: original.semantic_ssa,
            origins: &origins,
        };
        let (assertions, storage) =
            source_output_assertion_transport_v1(erased_origins, coordinates, control, budget)
                .map_err(ProductionSourceOutputErrorV1::Assertion)?;
        budget
            .reserve_storage(storage.retained_storage())
            .map_err(ProductionSourceOutputErrorV1::Resource)?;
        let blocks = erased_source_blocks_v1(&map, control, budget)?;
        let catalogs = erased_source_empty_catalogs_v1(&map, transition, budget)?;
        erased_occurrence_charge_v1(budget, 4)?;
        let callback_floor = budget.storage();
        map.floor = callback_floor;
        let view = ErasedSourceOutputOccurrencesV1 {
            coordinates: map,
            bound: coordinates.output(),
            checked,
            blocks,
            assertion_indices,
            assertions,
            catalogs,
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| next(&view, budget)));
        if ledger != budget.work_ledger_identity_v1()
            || slot != budget as *const AssertOriginBudgetV1<'_> as usize
            || budget.storage() != callback_floor
        {
            return Err(ProductionSourceOutputErrorV1::Resource(
                AssertOriginResourceV1::Accounting,
            ));
        }
        result.unwrap_or(Err(ProductionSourceOutputErrorV1::Panicked))
    }));
    if ledger != budget.work_ledger_identity_v1()
        || slot != budget as *const AssertOriginBudgetV1<'_> as usize
        || budget.storage() < floor
    {
        return Err(ProductionSourceOutputErrorV1::Resource(
            AssertOriginResourceV1::Accounting,
        ));
    }
    budget
        .release_storage(budget.storage() - floor)
        .map_err(ProductionSourceOutputErrorV1::Resource)?;
    result.unwrap_or(Err(ProductionSourceOutputErrorV1::Panicked))
}

// Initial erased execution catalogs remain empty-only. The old constructor is
// reused with original semantic custody, then each actual graph is independently
// checked; missing helper functions never authorize skipping a nonempty catalog.
fn erased_source_empty_catalogs_v1(
    map: &ErasedSourceCoordinateMapV1<'_>,
    transition: &fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV1<'_, '_, '_, '_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> ErasedOccurrenceResultV1<SourceOutputCatalogsV1> {
    use ProductionSourceOutputCatalogErrorV1 as CatalogError;
    use ProductionSourceOutputErrorV1 as Error;
    map.live(budget)?;
    erased_occurrence_charge_v1(budget, 3)?;
    let wrapper = std::mem::size_of::<SourceOutputCatalogsV1>()
        .checked_sub(std::mem::size_of::<
            fe2o3_kernel_ir::InertCanonicalKernelIrContractCatalogV1,
        >())
        .and_then(|n| {
            n.checked_sub(std::mem::size_of::<
                fe2o3_kernel_analysis::TransportedKernelIrContractCatalogV1,
            >())
        })
        .ok_or(Error::Resource(AssertOriginResourceV1::Accounting))?;
    budget.reserve_storage(wrapper).map_err(Error::Resource)?;
    let source = map.deletion.source();
    let semantic = source.semantic_ssa.source_semantic();
    let (original_catalog, storage) = source_catalog_from_live_v1(
        semantic,
        SourceCatalogCorrespondenceV1(&source.correspondence),
        map.inventory(),
        budget,
    )
    .map_err(Error::Catalog)?;
    budget
        .reserve_storage(storage.retained_storage())
        .map_err(Error::Resource)?;
    erased_occurrence_charge_v1(budget, 2)?;
    if !original_catalog.definitions().is_empty() || !original_catalog.bindings().is_empty() {
        return Err(erased_occurrence_invalid_v1(
            "erased source requires an empty execution catalog",
        ));
    }
    let (_original_binding, storage) = fe2o3_kernel_analysis::check_kernel_ir_contract_catalog_v1(
        map.inventory(),
        &original_catalog,
        budget,
    )
    .map_err(|error| Error::Catalog(CatalogError::Binding(error)))?;
    budget
        .reserve_storage(storage.retained_storage())
        .map_err(Error::Resource)?;
    let (erased_inventory, storage) =
        CanonicalKirInventoryV1::derive(map.deletion.output(), budget).map_err(Error::Inventory)?;
    budget
        .reserve_storage(storage.retained_storage())
        .map_err(Error::Resource)?;
    let (catalog, storage) = source_catalog_from_live_v1(
        semantic,
        SourceCatalogCorrespondenceV1(&source.correspondence),
        &erased_inventory,
        budget,
    )
    .map_err(Error::Catalog)?;
    budget
        .reserve_storage(storage.retained_storage())
        .map_err(Error::Resource)?;
    erased_occurrence_charge_v1(budget, 3)?;
    let bytes = original_catalog
        .canonical_bytes()
        .len()
        .checked_add(catalog.canonical_bytes().len())
        .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
    erased_occurrence_charge_v1(budget, bytes)?;
    if !catalog.definitions().is_empty()
        || !catalog.bindings().is_empty()
        || original_catalog.canonical_bytes() != catalog.canonical_bytes()
    {
        return Err(erased_occurrence_invalid_v1(
            "same empty semantic catalog on N and E",
        ));
    }
    {
        let (_erased_binding, storage) =
            fe2o3_kernel_analysis::check_kernel_ir_contract_catalog_v1(
                &erased_inventory,
                &catalog,
                budget,
            )
            .map_err(|error| Error::Catalog(CatalogError::Binding(error)))?;
        budget
            .reserve_storage(storage.retained_storage())
            .map_err(Error::Resource)?;
    }
    let transported = {
        let (bound_catalog, storage) = fe2o3_kernel_analysis::check_kernel_ir_contract_catalog_v1(
            transition.input(),
            &catalog,
            budget,
        )
        .map_err(|error| Error::Catalog(CatalogError::Binding(error)))?;
        budget
            .reserve_storage(storage.retained_storage())
            .map_err(Error::Resource)?;
        let (transported, storage) =
            fe2o3_kernel_analysis::transport_kernel_ir_contract_catalog_v1(
                transition,
                &bound_catalog,
                budget,
            )
            .map_err(|error| Error::Catalog(CatalogError::Transport(error)))?;
        budget
            .reserve_storage(storage.retained_storage())
            .map_err(Error::Resource)?;
        transported
    };
    Ok(SourceOutputCatalogsV1 {
        source: catalog,
        transported,
    })
}
