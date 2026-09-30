// This owner is descriptive until a genuine callback-scoped input authenticates
// its exact endpoints and epoch. The fixed pipeline pays before construction.
pub(crate) struct PreparedProgressGraphV2<'a> {
    context: &'a Context,
    function: &'a FuncOp,
    epoch: Option<u64>,
    state: Result<ProgressGraphV2, PlironProgressFindingV1>,
}

struct ProgressGraphV2 {
    inventory: StructuralInventoryV1,
    block_indices: HashMap<Ptr<BasicBlock>, usize>,
    graph: RootGraphV1,
    reachable: Vec<bool>,
    definitely_reachable: Vec<bool>,
    components: Vec<Vec<usize>>,
    reachable_cycle: bool,
}

fn capture_progress_graph_v2(
    observer: ProgressObserverV1<'_, '_, '_>,
    run: impl FnOnce() -> Result<ProgressGraphV2, PlironProgressFindingV1>,
) -> Result<ProgressGraphV2, PlironProgressFindingV1> {
    let state = catch_unwind(AssertUnwindSafe(|| {
        // The observer's unwind guard must run before structural-error
        // conversion catches the panic, including an existing prior denial.
        match observer {
            None => run(),
            Some(observer) => observer.with_projection(&Ok, |_| run()),
        }
    }))
    .unwrap_or_else(|payload| {
        Err(structural_rejection(format!(
            "bounded structural preflight panicked: {}",
            panic_detail(payload)
        )))
    });
    if let (Some(observer), Err(PlironProgressFindingV1::ResourceLimitExceeded { resource, .. })) =
        (observer, &state)
    {
        observer.deny(progress_resource_error_v1(resource));
    }
    state
}

fn build_progress_graph_v2(
    context: &Context,
    inventory: StructuralInventoryV1,
    mut work: ProgressWorkBudgetV1,
) -> Result<ProgressGraphV2, PlironProgressFindingV1> {
    let blocks = &inventory.root_blocks;
    let block_indices = blocks
        .iter()
        .enumerate()
        .map(|(i, block)| (*block, i))
        .collect();
    let graph = build_root_graph(context, blocks, &block_indices)?;
    let edges = graph.edges.iter().map(Vec::len).sum::<usize>();
    work.charge(
        blocks
            .len()
            .checked_add(edges)
            .and_then(|n| n.checked_mul(8))
            .unwrap_or(usize::MAX),
    )?;
    let reachable = reachable_blocks(&graph.edges);
    let definitely_reachable = reachable_blocks(&graph.unconditional_edges);
    let mut components = strongly_connected_components(&graph.edges);
    let mut reachable_cycle = false;
    for component in &mut components {
        component.sort_unstable();
        work.charge(component.len().saturating_mul(4))?;
        reachable_cycle |=
            component.iter().any(|block| reachable[*block]) && is_cycle(component, &graph.edges);
    }
    Ok(ProgressGraphV2 {
        inventory,
        block_indices,
        graph,
        reachable,
        definitely_reachable,
        components,
        reachable_cycle,
    })
}

impl<'a> PreparedProgressGraphV2<'a> {
    pub(crate) fn new(
        context: &'a Context,
        function: &'a FuncOp,
        observer: ProgressObserverV1<'_, '_, '_>,
    ) -> Self {
        let epoch = context
            .ir_mutation_attempt_epoch()
            .ok()
            .map(|epoch| epoch.value());
        let state = capture_progress_graph_v2(observer, || {
            let inventory = bounded_structural_inventory(context, function)?;
            let mut work = ProgressWorkBudgetV1::default();
            work.charge(inventory.structural_work().unwrap_or(usize::MAX))?;
            build_progress_graph_v2(context, inventory, work)
        });
        Self {
            context,
            function,
            epoch,
            state,
        }
    }

    pub(crate) fn continuation_bound(
        &self,
        census: ProductionAnalysisInputCensusV1,
        limits: ProductionAnalysisResourceLimitsV1,
    ) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
        if self.state.as_ref().is_ok_and(|graph| graph.reachable_cycle) {
            // Preserve the original cyclic bound, including its conservative
            // graph workspace. No failed cyclic admission retries as a DAG.
            return preflight_scoped_progress_resource_upper_bound_v1(census, limits);
        }
        let phase = ProductionAnalysisResourcePhaseV1::Progress;
        let diagnostic = MAX_PLIRON_PROGRESS_DIAGNOSTIC_BYTES_V1 + 32;
        let bound = ProductionAnalysisResourceUpperBoundV1::checked_phase(
            phase,
            diagnostic * 2,
            diagnostic,
            diagnostic + 16,
        )?;
        limits.require(phase, bound)
    }

    fn authenticate(
        &self,
        context: &Context,
        function: &FuncOp,
    ) -> Result<(), crate::production_analysis::pliron_pass_contract::PlironPassPreservationErrorV1>
    {
        use crate::production_analysis::pliron_pass_contract::PlironPassPreservationErrorV1 as Error;
        if !std::ptr::eq(self.context, context)
            || self.function.get_operation() != function.get_operation()
            || self.epoch.is_none()
            || self.epoch
                != context
                    .ir_mutation_attempt_epoch()
                    .ok()
                    .map(|epoch| epoch.value())
        {
            return Err(Error::InvalidSessionState {
                detail: "prepared progress graph differs from exact scoped endpoints or epoch",
            });
        }
        Ok(())
    }

    fn run(&self, observer: ProgressObserverV1<'_, '_, '_>) -> PlironProgressReportV1 {
        match &self.state {
            Ok(graph) => run_prepared_progress_graph_v2(self.context, graph, observer),
            Err(finding) => observed_progress_report_v1(finding.clone(), observer),
        }
    }
}

pub(crate) fn preflight_progress_graph_resource_upper_bound_v2(
    census: ProductionAnalysisInputCensusV1,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    preflight_progress_execution_resource_upper_bound_v1(
        census,
        limits,
        ProgressVerifierCostV1::PreparedGraph,
    )
}

fn progress_graph_resources_v2(
    census: ProductionAnalysisInputCensusV1,
    structural: usize,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    let error = "progress prepared graph upper bound";
    let mul = |a, b| checked_progress_product_v1(a, b, error);
    let sum = |items: &[usize]| checked_progress_sum_v1(items, error);
    // Existing analysis field units, not bytes/RSS. Retain original inventory,
    // two Ptr maps, four edge lists, reachability and sorted SCC membership.
    // Include geometric Vec/HashMap spare capacity, row headers and diagnostic.
    let retained = sum(&[
        64,
        MAX_PLIRON_PROGRESS_DIAGNOSTIC_BYTES_V1,
        mul(census.blocks, 48)?,
        mul(census.successors, 12)?,
        mul(census.operations, 8)?,
    ])?;
    // Inventory pending stack, SCC reverse graph/two DFS stacks/order/visited,
    // and bounded error formatting can overlap retained owners.
    let temporary = sum(&[
        64,
        MAX_PLIRON_PROGRESS_DIAGNOSTIC_BYTES_V1,
        mul(census.blocks, 16)?,
        mul(census.successors, 4)?,
        mul(census.operations, 8)?,
        mul(structural, 2)?,
    ])?;
    // Pointer-key hash tables use the pinned hashbrown policy. A table with
    // at most n entries has fewer than 4*n+4 buckets; include the last control
    // group and 32 field/byte visits per probe, plus fixed pointer hashing.
    // Geometric growth reinserts fewer than 2*n keys over all old tables:
    // 3*n complete-cycle lookups cover construction without random-hash or
    // constant-time collision assumptions. Root-edge lookup adds E probes.
    let lookup = |entries| sum(&[64, mul(sum(&[mul(entries, 4)?, 20])?, 32)?]);
    let map_work = sum(&[
        mul(mul(census.operations, 3)?, lookup(census.operations)?)?,
        mul(
            sum(&[mul(census.blocks, 3)?, census.successors])?,
            lookup(census.blocks)?,
        )?,
    ])?;
    // Full structural traversal plus graph/DFS fields; B^2 conservatively
    // covers all component sorting, without reserving dominator convergence.
    let work = sum(&[
        64,
        mul(structural, 16)?,
        mul(sum(&[census.blocks, census.successors])?, 64)?,
        mul(census.blocks, census.blocks)?,
        map_work,
    ])?;
    let phase = ProductionAnalysisResourcePhaseV1::Progress;
    limits.require(
        phase,
        ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, work, retained, temporary)?,
    )
}
