// Source-qualified occurrence connection for actual V12 N -> B -> checked O.
// Included in the production lowerer so source custody and origin fields stay private.

/// Actual original materialization and checked output control remain distinct.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSourceOutputBlockV1 {
    /// The valid source block was not materialized in the original N executable.
    NotMaterialized,
    /// Original materialization plus independent physical and execution placement.
    Materialized {
        /// Exact original N block coordinate authenticated by source custody.
        original: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1,
        /// Actual O placement, including physically retained unreachable blocks.
        placement: Option<fe2o3_kernel_analysis::CanonicalKirBlockPlacementV1>,
        /// Whether checked execution can reach this original block.
        executable: bool,
    },
}

#[derive(Clone, Copy, Debug)]
struct SourceOutputBlockRowV1 {
    source: SemanticKirAssertSiteV1,
    original: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1,
    control: fe2o3_kernel_analysis::CanonicalKirBlockControlV1,
}

/// Borrowed endpoints plus checked, owned numeric source/control/assertion rows.
/// Source-qualified catalogs describe checked placement, not lifecycle safety.
/// Direct retained-array writes and eligible ordinary Global accesses retain
/// checked operand placement. Explicit refusal states and all final proof
/// discharges remain unsupported. This view grants no authority.
pub struct ProductionSourceOutputOccurrencesV1<'source, 'output> {
    source: &'source ProductionPreRankedKirOwnerV1,
    bound: &'source fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    checked_output: SourceOutputCheckedEndpointV1<'output>,
    blocks: Vec<SourceOutputBlockRowV1>,
    private_arrays: Vec<SourceOutputArrayRowV1>,
    global_spans: Vec<SourceOutputGlobalSpanV1>,
    global_accesses: Vec<SourceOutputGlobalRowV1>,
    ordinary_calls: SourceOutputOrdinaryCallIndexV1,
    store_values: SourceOutputStoreValueIndexV1,
    checked_control_rows: SourceOutputCheckedControlRowsV1,
    storage: ProductionSourceOutputStorageV1,
    assertions: SemanticKirOptimizedAssertOriginOwnerV1,
    catalogs: SourceOutputCatalogsV1,
}

impl fmt::Debug for ProductionSourceOutputOccurrencesV1<'_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionSourceOutputOccurrencesV1")
            .field("source", self.source.executable().canonical().identity())
            .field("bound", self.bound.canonical().identity())
            .field("output", self.checked_output.owner().canonical().identity())
            .field("blocks", &self.blocks.len())
            .field("private_arrays", &self.private_arrays.len())
            .field("global_spans", &self.global_spans.len())
            .field("global_accesses", &self.global_accesses.len())
            .field("storage", &self.storage)
            .finish_non_exhaustive()
    }
}

impl<'source, 'output> ProductionSourceOutputOccurrencesV1<'source, 'output> {
    /// The actual sealed source owner replayed by the constructor.
    pub const fn source(&self) -> &'source ProductionPreRankedKirOwnerV1 {
        self.source
    }
    /// The complete admitted B endpoint checked against N and optimizer history.
    pub const fn bound(&self) -> &'source fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12 {
        self.bound
    }
    /// The actual independently checked O endpoint.
    pub fn output(&self) -> &'output fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12 {
        self.checked_output.owner()
    }
    /// Source/output occurrence records alone grant no proof or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
    /// Borrows the source-derived catalog bound to the exact admitted B graph.
    /// Traversals of its payload remain the caller's canonical-budget obligation.
    pub fn input_pipeline_catalog(
        &self,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<
        &fe2o3_kernel_ir::InertCanonicalKernelIrContractCatalogV1,
        ProductionSourceOutputErrorV1,
    > {
        budget
            .charge_work(1)
            .map_err(ProductionSourceOutputErrorV1::Resource)?;
        Ok(&self.catalogs.source)
    }

    /// Borrows the fresh catalog independently transported and checked on O.
    pub fn output_pipeline_catalog(
        &self,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<
        &fe2o3_kernel_ir::InertCanonicalKernelIrContractCatalogV1,
        ProductionSourceOutputErrorV1,
    > {
        budget
            .charge_work(1)
            .map_err(ProductionSourceOutputErrorV1::Resource)?;
        Ok(self.catalogs.transported.catalog())
    }

    /// Borrows exact allocation placements, not source lifecycle or proof authority.
    /// Each caller traversal must remain on the shared canonical ledger.
    pub fn pipeline_allocation_placements(
        &self,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<
        &[fe2o3_kernel_analysis::KernelIrPipelineAllocationTransportV1],
        ProductionSourceOutputErrorV1,
    > {
        budget
            .charge_work(1)
            .map_err(ProductionSourceOutputErrorV1::Resource)?;
        Ok(self.catalogs.transported.allocations())
    }

    /// Borrows exact ordered-marker placements; omission is not lifecycle safety.
    /// Each caller traversal must remain on the shared canonical ledger.
    pub fn pipeline_marker_placements(
        &self,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<
        &[fe2o3_kernel_analysis::KernelIrPipelineMarkerTransportV1],
        ProductionSourceOutputErrorV1,
    > {
        budget
            .charge_work(1)
            .map_err(ProductionSourceOutputErrorV1::Resource)?;
        Ok(self.catalogs.transported.markers())
    }

    /// Looks up a source/root-qualified assertion's checked output placement.
    pub fn assertion(
        &self,
        owner: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
        block: SemanticBlockIdV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<&SemanticKirOptimizedAssertBindingV1, ProductionSourceOutputErrorV1> {
        self.assertions
            .assert_condition(owner, function, block, budget)
            .map_err(ProductionSourceOutputErrorV1::Assertion)
    }
    /// Looks up edge arguments only for the exact borrowed binding from this view.
    pub fn assertion_arguments(
        &self,
        binding: &SemanticKirOptimizedAssertBindingV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<&[SemanticKirOptimizedAssertArgumentV1], ProductionSourceOutputErrorV1> {
        self.assertions
            .arguments_for(binding, budget)
            .map_err(ProductionSourceOutputErrorV1::Assertion)
    }
    /// Resolves an authenticated source block without conflating omission and death.
    pub fn block(
        &self,
        owner: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
        block: SemanticBlockIdV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<ProductionSourceOutputBlockV1, ProductionSourceOutputErrorV1> {
        if !self
            .source
            .assert_origins()
            .is_materialized_block(owner, function, block, budget)
            .map_err(ProductionSourceOutputErrorV1::SourceOrigin)?
        {
            return Ok(ProductionSourceOutputBlockV1::NotMaterialized);
        }
        let source = SemanticKirAssertSiteV1::new(owner, function, block);
        let ordinal = assert_origin_find_v1(&self.blocks, budget, |row, budget| {
            source_output_site_cmp_v1(row.source, source, budget)
        })
        .map_err(ProductionSourceOutputErrorV1::SourceOrigin)?
        .ok_or(ProductionSourceOutputErrorV1::Invalid(
            "missing materialized source block",
        ))?;
        budget
            .charge_work(1)
            .map_err(ProductionSourceOutputErrorV1::Resource)?;
        let row = self.blocks[ordinal];
        Ok(ProductionSourceOutputBlockV1::Materialized {
            original: row.original,
            placement: row.control.placement,
            executable: row.control.reachable,
        })
    }
}

/// Retained view/header, source rows, transported assertions and pipeline catalogs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSourceOutputStorageV1(usize);
impl ProductionSourceOutputStorageV1 {
    /// Logical bytes to reserve while retaining the source/output view.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Failure to connect actual source custody and checked output occurrences.
#[derive(Debug)]
pub enum ProductionSourceOutputErrorV1 {
    /// The shared ledger refused work, storage, allocation or arithmetic.
    Resource(AssertOriginResourceV1),
    /// The original source SSA or executable replay failed.
    SourceReplay(ProductionSemanticKirErrorV1),
    /// An original source/root binding could not be resolved.
    SourceOrigin(SemanticKirAssertOriginErrorV1),
    /// Independent occurrence inventory construction failed.
    Inventory(fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1),
    /// The independent actual B/O transition or control check failed.
    Transition(fe2o3_kernel_analysis::CanonicalKirTransitionErrorV1),
    /// Transporting or resolving an exact assertion occurrence failed.
    Assertion(SemanticKirOptimizedAssertOriginErrorV1),
    /// Source-derived catalog construction or checked output placement failed.
    Catalog(ProductionSourceOutputCatalogErrorV1),
    /// Original array provenance or a borrowed physical array check failed.
    PrivateArray(SemanticKirPrivateArrayQueryErrorV1),
    /// The source N pointer or complete historical B endpoint did not match.
    InputCustody,
    /// A source replay or connection operation unwound; temporary storage was released.
    Panicked,
    /// A required source/output coordinate or retained-row invariant failed.
    Invalid(&'static str),
}
impl fmt::Display for ProductionSourceOutputErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(formatter),
            Self::SourceReplay(error) => error.fmt(formatter),
            Self::SourceOrigin(error) => error.fmt(formatter),
            Self::Inventory(error) => error.fmt(formatter),
            Self::Transition(error) => error.fmt(formatter),
            Self::Assertion(error) => error.fmt(formatter),
            Self::PrivateArray(error) => error.fmt(formatter),
            Self::Catalog(error) => error.fmt(formatter),
            Self::InputCustody => formatter.write_str("source/output endpoint custody mismatch"),
            Self::Panicked => formatter.write_str("source/output occurrence connection panicked"),
            Self::Invalid(rule) => write!(
                formatter,
                "source/output occurrence connection rejected: {rule}"
            ),
        }
    }
}
impl std::error::Error for ProductionSourceOutputErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::SourceReplay(error) => Some(error),
            Self::SourceOrigin(error) => Some(error),
            Self::Inventory(error) => Some(error),
            Self::Transition(error) => Some(error),
            Self::Assertion(error) => Some(error),
            Self::PrivateArray(error) => Some(error),
            Self::Catalog(error) => Some(error),
            Self::InputCustody | Self::Panicked | Self::Invalid(_) => None,
        }
    }
}

/// Connects the actual sealed source to the actual checked output. The neutral
/// coordinate witness does not itself identify an AMD target; the production
/// orchestrator must obtain it from the exact target metadata checker.
///
/// Source SSA/re-lowering replay uses the existing source engine and its stored
/// limits, outside the canonical analysis budget. Everything newly retained or
/// traversed for B/O indexes and assertion transport uses the caller ledger.
/// N G+A, B, checked O/history and the borrowed coordinate view must already be
/// reserved by their owner. This API never creates a replacement phase budget.
pub fn derive_source_output_occurrences_v1<'source, 'output>(
    source: &'source ProductionPreRankedKirOwnerV1,
    coordinates: &fe2o3_kernel_analysis::CheckedCanonicalKirCoordinatePreservationV1<
        'source,
        'source,
    >,
    checked_output: &'output fe2o3_pliron::CheckedNeutralKernelIrOwnerV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<
    (
        ProductionSourceOutputOccurrencesV1<'source, 'output>,
        ProductionSourceOutputStorageV1,
    ),
    ProductionSourceOutputErrorV1,
> {
    derive_source_output_occurrences_with_endpoint_v1(
        source,
        coordinates,
        SourceOutputCheckedEndpointV1::Optimizer(checked_output),
        budget,
    )
}

fn derive_source_output_occurrences_with_endpoint_v1<'source, 'output>(
    source: &'source ProductionPreRankedKirOwnerV1,
    coordinates: &fe2o3_kernel_analysis::CheckedCanonicalKirCoordinatePreservationV1<
        'source,
        'source,
    >,
    checked_output: SourceOutputCheckedEndpointV1<'output>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<
    (
        ProductionSourceOutputOccurrencesV1<'source, 'output>,
        ProductionSourceOutputStorageV1,
    ),
    ProductionSourceOutputErrorV1,
> {
    use ProductionSourceOutputErrorV1 as Error;
    let floor = budget.storage();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // Entry, source-pointer comparison, two live sums and floor comparison.
        budget.charge_work(5).map_err(Error::Resource)?;
        if !std::ptr::eq(source.executable(), coordinates.input()) {
            return Err(Error::InputCustody);
        }
        let live = source
            .executable_storage()
            .retained_storage()
            .checked_add(source.assert_origin_storage().payload_storage())
            .and_then(|n| n.checked_add(checked_output.storage().retained_storage()))
            .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
        if floor < live {
            return Err(Error::Resource(AssertOriginResourceV1::Accounting));
        }
        let bound = coordinates.output();
        budget.charge_work(1).map_err(Error::Resource)?;
        if let Some(historical) = checked_output.native_input_audit_bytes() {
            let input_bytes = bound.canonical().canonical_bytes();
            budget.charge_work(2).map_err(Error::Resource)?;
            let bytes = input_bytes
                .len()
                .checked_add(historical.len())
                .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
            budget.charge_work(bytes).map_err(Error::Resource)?;
            if input_bytes != historical {
                return Err(Error::InputCustody);
            }
        }
        source_output_replay_v1(source).map_err(Error::SourceReplay)?;

        let (input, input_storage) =
            fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(bound, budget)
                .map_err(Error::Inventory)?;
        budget
            .reserve_storage(input_storage.retained_storage())
            .map_err(Error::Resource)?;
        let (output, output_storage) =
            fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(checked_output.owner(), budget)
                .map_err(Error::Inventory)?;
        budget
            .reserve_storage(output_storage.retained_storage())
            .map_err(Error::Resource)?;
        budget.charge_work(1).map_err(Error::Resource)?;
        let (transition, transition_storage) = match checked_output {
            SourceOutputCheckedEndpointV1::Optimizer(_)
            | SourceOutputCheckedEndpointV1::OptimizerPolicy3(_) => {
                fe2o3_kernel_analysis::check_canonical_kir_transition_v1(
                    &input,
                    &output,
                    checked_output.candidate(),
                    budget,
                )
            }
            SourceOutputCheckedEndpointV1::Receipt { receipt, .. } => {
                fe2o3_kernel_analysis::check_canonical_kir_transition_receipt_v1(
                    &input, &output, receipt, budget,
                )
            }
        }
        .map_err(Error::Transition)?;
        budget
            .reserve_storage(transition_storage.retained_storage())
            .map_err(Error::Resource)?;
        let (control, control_storage) =
            fe2o3_kernel_analysis::CheckedCanonicalKirControlIndexV1::derive(&transition, budget)
                .map_err(Error::Transition)?;
        budget
            .reserve_storage(control_storage.retained_storage())
            .map_err(Error::Resource)?;
        let (assertions, assertion_storage) = source_output_assertion_transport_v1(
            source.assert_origins(),
            coordinates,
            &control,
            budget,
        )
        .map_err(Error::Assertion)?;
        budget
            .reserve_storage(assertion_storage.retained_storage())
            .map_err(Error::Resource)?;

        let (private_arrays, array_payload, array_header) =
            source_output_array_rows_with_header_v1(source, &control, budget)?;

        let (catalogs, catalog_storage) =
            source_output_catalogs_after_replay_v1(source, coordinates, &transition, budget)
                .map_err(Error::Catalog)?;
        budget
            .reserve_storage(catalog_storage)
            .map_err(Error::Resource)?;

        let (global_spans, global_accesses, global_payload, global_header) =
            source_output_global_rows_v1(source, &transition, &control, budget)?;
        let (ordinary_calls, ordinary_payload, ordinary_header) =
            source_output_ordinary_calls_v1(source, &transition, budget)?;
        let (store_values, store_payload, store_header) =
            source_output_store_values_v1(source, coordinates, &transition, &control, budget)?;
        let (checked_control_rows, checked_control_storage) =
            source_output_checked_control_rows_v1(
                source,
                coordinates,
                &transition,
                &control,
                budget,
            )?;
        budget
            .reserve_storage(checked_control_storage.retained_storage())
            .map_err(Error::Resource)?;
        budget.charge_work(1).map_err(Error::Resource)?;
        let checked_control_header = std::mem::size_of::<SourceOutputCheckedControlRowsV1>();
        let checked_control_payload = checked_control_storage
            .retained_storage()
            .checked_sub(checked_control_header)
            .ok_or(Error::Resource(AssertOriginResourceV1::Accounting))?;

        budget.charge_work(4).map_err(Error::Resource)?;
        let count = source.correspondence.blocks.len();
        let bytes = count
            .checked_mul(std::mem::size_of::<SourceOutputBlockRowV1>())
            .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
        let header = std::mem::size_of::<ProductionSourceOutputOccurrencesV1<'_, '_>>()
            .checked_sub(std::mem::size_of::<SemanticKirOptimizedAssertOriginOwnerV1>())
            .and_then(|n| n.checked_sub(std::mem::size_of::<SourceOutputCatalogsV1>()))
            .ok_or(Error::Resource(AssertOriginResourceV1::Accounting))?;
        let own = header
            .checked_add(bytes)
            .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
        budget.charge_work(5).map_err(Error::Resource)?;
        let remaining_own = own
            .checked_sub(array_header)
            .and_then(|n| n.checked_sub(global_header))
            .and_then(|n| n.checked_sub(ordinary_header))
            .and_then(|n| n.checked_sub(store_header))
            .and_then(|n| n.checked_sub(checked_control_header))
            .ok_or(Error::Resource(AssertOriginResourceV1::Accounting))?;
        budget
            .reserve_storage(remaining_own)
            .map_err(Error::Resource)?;
        let mut blocks = Vec::new();
        budget.charge_work(1).map_err(Error::Resource)?;
        blocks
            .try_reserve_exact(count)
            .map_err(|_| Error::Resource(AssertOriginResourceV1::Allocation))?;
        let origins = source.assert_origins();
        for row in source.correspondence.blocks.iter() {
            budget.charge_work(1).map_err(Error::Resource)?;
            let site = SemanticKirAssertSiteV1::new(
                row.correspondence_owner,
                row.semantic_function,
                row.semantic_block,
            );
            if !origins
                .is_materialized_block(
                    row.correspondence_owner,
                    row.semantic_function,
                    row.semantic_block,
                    budget,
                )
                .map_err(Error::SourceOrigin)?
            {
                return Err(Error::Invalid(
                    "correspondence names an unmaterialized block",
                ));
            }
            let function =
                assert_origin_find_v1(&origins.origins.functions, budget, |entry, budget| {
                    budget.charge_work(2)?;
                    Ok((entry.owner, entry.function)
                        .cmp(&(row.correspondence_owner, row.semantic_function)))
                })
                .map_err(Error::SourceOrigin)?
                .ok_or(Error::Invalid("missing source function alias"))?;
            budget.charge_work(1).map_err(Error::Resource)?;
            let physical = input
                .block_for_id(
                    origins.origins.functions[function].canonical,
                    row.kernel_ir_block,
                    budget,
                )
                .map_err(Error::Inventory)?
                .ok_or(Error::Invalid("missing exact original block"))?;
            let state = control
                .block(physical.coordinate, budget)
                .map_err(Error::Transition)?;
            budget.charge_work(1).map_err(Error::Resource)?;
            blocks.push(SourceOutputBlockRowV1 {
                source: site,
                original: physical.coordinate,
                control: state,
            });
        }
        assert_origin_sort_v1(&mut blocks, budget, |a, b, budget| {
            source_output_site_cmp_v1(a.source, b.source, budget)
        })
        .map_err(Error::SourceOrigin)?;
        for pair in blocks.windows(2) {
            budget.charge_work(3).map_err(Error::Resource)?;
            if pair[0].source == pair[1].source {
                return Err(Error::Invalid("duplicate source block alias"));
            }
        }
        budget.charge_work(7).map_err(Error::Resource)?;
        let retained = own
            .checked_add(assertion_storage.retained_storage())
            .and_then(|n| n.checked_add(catalog_storage))
            .and_then(|bytes| bytes.checked_add(array_payload))
            .and_then(|bytes| bytes.checked_add(global_payload))
            .and_then(|bytes| bytes.checked_add(ordinary_payload))
            .and_then(|bytes| bytes.checked_add(store_payload))
            .and_then(|bytes| bytes.checked_add(checked_control_payload))
            .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
        drop(control);
        budget
            .release_storage(control_storage.retained_storage())
            .map_err(Error::Resource)?;
        #[allow(
            clippy::drop_non_drop,
            reason = "End the borrowed witness lifetime before releasing its ledger reservation"
        )]
        drop(transition);
        budget
            .release_storage(transition_storage.retained_storage())
            .map_err(Error::Resource)?;
        drop(output);
        budget
            .release_storage(output_storage.retained_storage())
            .map_err(Error::Resource)?;
        drop(input);
        budget
            .release_storage(input_storage.retained_storage())
            .map_err(Error::Resource)?;
        Ok((
            ProductionSourceOutputOccurrencesV1 {
                source,
                bound,
                checked_output,
                blocks,
                private_arrays,
                global_spans,
                global_accesses,
                ordinary_calls,
                store_values,
                checked_control_rows,
                storage: ProductionSourceOutputStorageV1(retained),
                assertions,
                catalogs,
            },
            ProductionSourceOutputStorageV1(retained),
        ))
    }))
    .unwrap_or(Err(Error::Panicked));
    let release = budget
        .storage()
        .checked_sub(floor)
        .ok_or(Error::Resource(AssertOriginResourceV1::Accounting))?;
    budget.release_storage(release).map_err(Error::Resource)?;
    result
}

fn source_output_replay_v1(
    source: &ProductionPreRankedKirOwnerV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    source
        .semantic_ssa
        .verify_replay()
        .map_err(ProductionSemanticKirErrorV1::SemanticSsa)?;
    let (module, correspondence) = lower_module(
        &source.semantic_ssa,
        source.limits,
        Some(source.launch_roots.as_ref()),
    )?;
    if source.executable().module() != &module || source.correspondence != correspondence {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    Ok(())
}

fn source_output_site_cmp_v1(
    a: SemanticKirAssertSiteV1,
    b: SemanticKirAssertSiteV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> AssertOriginResultV1<std::cmp::Ordering> {
    budget.charge_work(3)?;
    Ok((
        a.correspondence_owner,
        a.semantic_function,
        a.semantic_block,
    )
        .cmp(&(
            b.correspondence_owner,
            b.semantic_function,
            b.semantic_block,
        )))
}

fn source_output_assertion_transport_v1(
    original: SemanticKirAssertOriginsV1<'_>,
    coordinates: &fe2o3_kernel_analysis::CheckedCanonicalKirCoordinatePreservationV1<'_, '_>,
    control: &fe2o3_kernel_analysis::CheckedCanonicalKirControlIndexV1<'_, '_, '_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<
    (
        SemanticKirOptimizedAssertOriginOwnerV1,
        SemanticKirOptimizedAssertOriginStorageV1,
    ),
    SemanticKirOptimizedAssertOriginErrorV1,
> {
    budget.charge_work(2)?;
    if !std::ptr::eq(original.executable(), coordinates.input())
        || !std::ptr::eq(coordinates.output(), control.input().owner())
    {
        return Err(SemanticKirOptimizedAssertOriginErrorV1::InputOwner);
    }
    // This private rebase exists only after complete coordinate identity. The
    // old public N-only transport's exact executable-pointer check stays intact.
    let rebound = SemanticKirAssertOriginsV1 {
        executable: coordinates.output(),
        semantic_ssa: original.semantic_ssa,
        origins: original.origins,
    };
    transport_semantic_kir_assert_origins_v1(rebound, control, budget)
}

include!("production_source_output_selected_successor_v1.rs");

include!("production_source_output_block_coverage_v1.rs");

include!("production_source_output_checked_endpoint_v1.rs");

include!("production_source_output_effect_census_v1.rs");

include!("production_source_output_ordinary_calls_v1.rs");

include!("production_source_output_memory_operands_v1.rs");
include!("production_source_output_store_values_v1.rs");
include!("production_source_output_control_rows_v1.rs");
include!("production_source_output_control_coverage_v1.rs");
include!("production_source_output_canonical_store_analysis_v1.rs");
include!("production_source_output_formal_complete_v1.rs");
