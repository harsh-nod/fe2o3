// The new facade authenticates original N. It is not the completed ranked
// result needed to attach reports, optimize a successor or activate default.
include!("production_canonical_ranked_checks_v1.rs");
#[derive(Clone, Copy)]
enum CrQueryFailureV1 {
    Resource(ArgumentResourceV1),
    Invalid(&'static str),
}
impl CrQueryFailureV1 {
    fn error(self) -> ProductionCanonicalRankedSourceErrorV1 {
        match self {
            Self::Resource(e) => ProductionCanonicalRankedSourceErrorV1::Resource(e),
            Self::Invalid(reason) => cr_invalid_v1(reason),
        }
    }
}
struct CrGuardV1 {
    slot: usize,
    ledger: ArgumentLedgerV1,
    floor: usize,
    first: std::cell::Cell<Option<CrQueryFailureV1>>,
}
impl CrGuardV1 {
    fn new(budget: &ArgumentBudgetV1<'_>) -> Self {
        Self {
            slot: std::ptr::from_ref(budget) as usize,
            ledger: budget.work_ledger_identity_v1(),
            floor: budget.storage(),
            first: std::cell::Cell::new(None),
        }
    }
    fn fail(&self, issue: CrQueryFailureV1) -> ProductionCanonicalRankedSourceErrorV1 {
        if self.first.get().is_none() {
            self.first.set(Some(issue));
        }
        self.first
            .get()
            .expect("installed first query error")
            .error()
    }
    fn check(&self, budget: &ArgumentBudgetV1<'_>) -> CrResultV1<()> {
        if self.slot != std::ptr::from_ref(budget) as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.floor
        {
            return Err(self.fail(CrQueryFailureV1::Resource(ArgumentResourceV1::Accounting)));
        }
        if let Some(issue) = self.first.get() {
            return Err(issue.error());
        }
        Ok(())
    }
    fn query(&self, budget: &mut ArgumentBudgetV1<'_>) -> CrResultV1<()> {
        self.check(budget)?;
        budget
            .charge_work(1)
            .map_err(|e| self.fail(CrQueryFailureV1::Resource(e)))
    }
    fn missing(&self, reason: &'static str) -> ProductionCanonicalRankedSourceErrorV1 {
        self.fail(CrQueryFailureV1::Invalid(reason))
    }
}

/// Complete original-source facts attached to the one canonical graph.
/// Construction is owner-only. Facts do not discharge ranked obligations.
/// Every query uses the original Budget slot/ledger and complete live floor;
/// returned slices are borrowed facts, not free metered iteration or authority.
pub struct ProductionCanonicalRankedMetadataV1<'a> {
    owner: &'a ProductionPreRankedKirOwnerV1,
    inventory: &'a CanonicalKirInventoryV1<'a>,
    calls: &'a ProductionCanonicalCallsV1<'a>,
    source: &'a CrSourceRowsV1<'a>,
    arguments: &'a CrArgumentRowsV1<'a>,
    contracts: &'a CrContractsV1<'a>,
    guard: &'a CrGuardV1,
}
impl ProductionCanonicalRankedMetadataV1<'_> {
    /// Original connected N, never a separately editable verification graph.
    pub fn inventory(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CrResultV1<&CanonicalKirInventoryV1<'_>> {
        self.guard.query(budget)?;
        Ok(self.inventory)
    }
    /// Complete source/function associations, including shared helper aliases.
    pub fn function_count(&self, budget: &mut ArgumentBudgetV1<'_>) -> CrResultV1<usize> {
        self.guard.query(budget)?;
        Ok(self.calls.groups.len())
    }
    /// Complete original generated input/output attachment count.
    pub fn generated_value_count(&self, budget: &mut ArgumentBudgetV1<'_>) -> CrResultV1<usize> {
        self.guard.query(budget)?;
        Ok(self.owner.correspondence.generated_terminator_values.len())
    }
    /// Original attachment borrowing source custody, not editable value recipes.
    pub fn generated_values(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CrResultV1<ProductionCanonicalRankedGeneratedValuesV1<'_>> {
        self.guard.query(budget)?;
        self.owner
            .correspondence
            .generated_terminator_values
            .get(ordinal)
            .map(ProductionCanonicalRankedGeneratedValuesV1)
            .ok_or_else(|| self.guard.missing("generated value ordinal"))
    }
    /// Complete original call and return occurrence roster, including zero results.
    pub fn call_transport_count(&self, budget: &mut ArgumentBudgetV1<'_>) -> CrResultV1<usize> {
        self.guard.query(budget)?;
        Ok(self.owner.correspondence.call_returns.len())
    }
    /// Original call/return component transport, checked by the existing call index.
    pub fn call_transport(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CrResultV1<ProductionCanonicalRankedCallTransportV1<'_>> {
        self.guard.query(budget)?;
        let row = self
            .owner
            .correspondence
            .call_returns
            .get(ordinal)
            .ok_or_else(|| self.guard.missing("call transport ordinal"))?;
        Ok(ProductionCanonicalRankedCallTransportV1 {
            row,
            components: &self.owner.correspondence.call_result_components,
        })
    }
    /// Exact source root/function and actual graph function.
    pub fn function(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CrResultV1<&ProductionCanonicalCallFunctionV1<'_>> {
        self.guard.query(budget)?;
        self.calls
            .groups
            .get(ordinal)
            .map(|g| &g.function)
            .ok_or_else(|| self.guard.missing("function ordinal"))
    }
    /// Complete typed source spans, including zero-operation occurrences.
    pub fn spans(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CrResultV1<&[ProductionCanonicalRankedSpanV1<'_>]> {
        self.guard.query(budget)?;
        Ok(&self.source.spans)
    }
    /// Source attribution of one actual operation for one root-qualified alias.
    /// Ordinals enumerate the complete ordered association group, not donor unions.
    pub fn operation_source(
        &self,
        operation: usize,
        alias: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CrResultV1<&ProductionCanonicalRankedSpanV1<'_>> {
        self.guard.query(budget)?;
        let range = self
            .source
            .operation_origins
            .get(operation)
            .ok_or_else(|| self.guard.missing("operation source coordinate"))?;
        let origin = self.source.origins[range.clone()]
            .get(alias)
            .ok_or_else(|| self.guard.missing("operation source alias"))?;
        Ok(&self.source.spans[origin.span])
    }
    /// Complete structural entry arguments. Pointee provenance remains pending.
    pub fn arguments(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CrResultV1<&[ProductionCanonicalRankedArgumentV1]> {
        self.guard.query(budget)?;
        Ok(&self.arguments.rows)
    }
    /// Structural source path and local suffix of the selected argument row.
    pub fn argument_path(
        &self,
        ordinal: usize,
        local: bool,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CrResultV1<&[ProductionArgumentProjectionV1]> {
        self.guard.query(budget)?;
        let row = self
            .arguments
            .rows
            .get(ordinal)
            .ok_or_else(|| self.guard.missing("argument ordinal"))?;
        if local && row.local.is_none() {
            return Err(self.guard.missing("argument has no entry local"));
        }
        let start = row.path.start + if local { row.local_offset } else { 0 };
        Ok(&self.arguments.paths[start..row.path.end])
    }
    /// Original semantic type facts: sizes, alignments, pointer metadata and ADTs.
    /// These are declarations, not a computed dynamic extent or alias proof.
    pub fn types(&self, budget: &mut ArgumentBudgetV1<'_>) -> CrResultV1<&[SemanticTypeDeclV1]> {
        self.guard.query(budget)?;
        Ok(self.owner.semantic_ssa.source_semantic().types())
    }
    /// All actual effects, including private/helper writes and atomics.
    /// Address/value operands stay in N; initialization and provenance are pending.
    pub fn effects(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CrResultV1<&[fe2o3_kernel_analysis::CanonicalKirEffectRefV1<'_>]> {
        self.guard.query(budget)?;
        Ok(self.inventory.effects())
    }
    /// Retained helper allocation/access/control/edge substitutions, joined to
    /// original N. Root associations reject; raw-empty helpers return None,
    /// which is not a return-value, termination or general safety claim.
    pub fn helper_frame(
        &self,
        association: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CrResultV1<Option<&ProductionHelperLocalFrameV1<'_>>> {
        self.guard.query(budget)?;
        let group = self
            .calls
            .groups
            .get(association)
            .ok_or_else(|| self.guard.missing("helper association"))?;
        if group.function.source.role != SemanticKirFunctionRoleV1::InternalHelper {
            return Err(self.guard.missing("root is not a helper frame"));
        }
        Ok(self.arguments.frames[association].as_ref())
    }
    /// Complete original source declarations for intrinsic, tensor, assembly,
    /// pipeline and numerical contracts; no absent external proof is inferred.
    pub fn callables(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CrResultV1<&[SemanticCallableDeclV1]> {
        self.guard.query(budget)?;
        Ok(self.owner.semantic_ssa.source_semantic().callables())
    }
    /// Source-bound ordered launch layouts, not target-bound graph identity.
    pub fn launches(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CrResultV1<&[ProductionCanonicalRankedLaunchV1<'_>]> {
        self.guard.query(budget)?;
        Ok(&self.contracts.launches)
    }
    /// Emitted and source-rule-elided assertions, without assuming predicates.
    pub fn assertions(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CrResultV1<&[ProductionCanonicalRankedAssertionV1]> {
        self.guard.query(budget)?;
        Ok(&self.contracts.assertions)
    }
    /// Genuine complete source pipeline definitions and exact graph allocations.
    pub fn catalog(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CrResultV1<&fe2o3_kernel_ir::InertCanonicalKernelIrContractCatalogV1> {
        self.guard.query(budget)?;
        Ok(&self.contracts.catalog)
    }
    /// The backend's authenticated external reference/runtime evidence is absent
    /// from this owner; refinement is therefore pending, not unnecessary.
    pub const fn external_refinement_is_pending(&self) -> bool {
        true
    }
    /// Source attribution does not provide compiler publication or launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
    /// Full graph-derived ranked verification and retained report attachment remain pending.
    pub const fn ranked_verification_is_complete(&self) -> bool {
        false
    }
}

/// Source-authenticated metadata and an actual independently checked structural
/// view, borrowed together. This is not a completed ranked result or receipt.
pub struct ProductionCanonicalRankedSourceViewV1<'s, 'v, 'i, 'g, 'm> {
    source: &'s ProductionCanonicalRankedMetadataV1<'s>,
    checked: &'s mut fe2o3_kernel_analysis::CheckedCanonicalRankedViewV1<'v, 'i, 'g, 'm>,
}
impl ProductionCanonicalRankedSourceViewV1<'_, '_, '_, '_, '_> {
    /// Immutable source facts under the complete source/candidate/view floor.
    pub fn metadata(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CrResultV1<&ProductionCanonicalRankedMetadataV1<'_>> {
        self.source.guard.query(budget)?;
        Ok(self.source)
    }
    /// Number of genuine checked coverage rows; obligations remain pending.
    pub fn row_count(&mut self, budget: &mut ArgumentBudgetV1<'_>) -> CrResultV1<usize> {
        self.source.guard.query(budget)?;
        Ok(self.checked.row_count(budget)?)
    }
    /// Exact coverage row checked independently against this same N.
    pub fn row(
        &mut self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CrResultV1<&fe2o3_kernel_analysis::CanonicalRankedCoverageRowV1> {
        self.source.guard.query(budget)?;
        Ok(self.checked.row(ordinal, budget)?)
    }
    /// All graph and metadata obligations remain pending.
    pub const fn ranked_verification_is_complete(&self) -> bool {
        false
    }
    /// No native, artifact, load, launch or external proof authority is granted.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

fn cr_protected_v1<'w, T>(
    budget: &mut ArgumentBudgetV1<'w>,
    body: impl FnOnce(&mut ArgumentBudgetV1<'w>) -> CrResultV1<T>,
) -> CrResultV1<T> {
    budget.charge_work(2)?;
    let floor = budget.storage();
    let slot = std::ptr::from_ref(&*budget) as usize;
    let ledger = budget.work_ledger_identity_v1();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(budget)));
    // All body-owned dependencies have dropped before this cleanup. On rejected
    // accounting the returned value/panic payload drops before any refund.
    if slot != std::ptr::from_ref(&*budget) as usize
        || ledger != budget.work_ledger_identity_v1()
        || budget.storage() < floor
    {
        drop(result);
        return Err(ArgumentResourceV1::Accounting.into());
    }
    budget.release_storage(budget.storage() - floor)?;
    match result {
        Ok(value) => value,
        Err(payload) => {
            drop(payload);
            Err(ProductionCanonicalRankedSourceErrorV1::Panicked)
        }
    }
}

fn cr_callback_v1<'w, T>(
    guard: &CrGuardV1,
    budget: &mut ArgumentBudgetV1<'w>,
    run: impl FnOnce(&mut ArgumentBudgetV1<'w>) -> CrResultV1<T>,
) -> CrResultV1<T> {
    let returned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(budget)));
    if let Err(error) = guard.check(budget) {
        // A rejected T may own callback-visible data. Destroy it while source,
        // catalog and candidate backing is still paid. The enclosing protected
        // scope also catches a rejected-value destructor panic before refund.
        if let Err(payload) =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(returned)))
        {
            drop(payload);
        }
        return Err(error);
    }
    match returned {
        Ok(value) => value,
        Err(payload) => {
            drop(payload);
            Err(ProductionCanonicalRankedSourceErrorV1::Panicked)
        }
    }
}

fn cr_with_metadata_v1<'w, T>(
    owner: &ProductionPreRankedKirOwnerV1,
    budget: &mut ArgumentBudgetV1<'w>,
    run: impl for<'s> FnOnce(
        &ProductionCanonicalRankedMetadataV1<'s>,
        &mut ArgumentBudgetV1<'w>,
    ) -> CrResultV1<T>,
) -> CrResultV1<T> {
    cr_protected_v1(budget, |budget| {
        budget.charge_work(2)?;
        if budget.storage() < owner.unit_local_source_storage_floor_v1()? {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        match owner.helper_source_policy_v1() {
            ProductionHelperSourcePolicyV1::RawEmpty => source_output_replay_v1(owner)?,
            ProductionHelperSourcePolicyV1::UnitLocal => {
                source_output_unit_local_replay_v1(owner, budget)?
            }
        }
        fe2o3_pliron::with_canonical_analysis_scope_v1(owner.executable(), budget, |scope| {
            scope.with_inventory_v1(|inventory, budget| {
                cr_protected_v1(budget, |budget| {
                    budget.reserve_storage(argument_sum_v1(&[
                        std::mem::size_of::<CrSourceRowsV1<'_>>(),
                        std::mem::size_of::<CrArgumentRowsV1>(),
                        std::mem::size_of::<CrContractsV1<'_>>()
                            - std::mem::size_of::<
                                fe2o3_kernel_ir::InertCanonicalKernelIrContractCatalogV1,
                            >(),
                        std::mem::size_of::<ProductionCanonicalRankedMetadataV1<'_>>(),
                        std::mem::size_of::<CrGuardV1>(),
                        std::mem::size_of::<std::thread::Result<CrResultV1<T>>>(),
                    ])?)?;
                    let calls = ProductionCanonicalCallsV1::build(owner, inventory, budget)?;
                    canonical_ranked_unit_local_join_v1(owner, inventory, budget)?;
                    let source = cr_build_source_rows_v1(owner, inventory, &calls, budget)?;
                    let arguments = cr_build_arguments_v1(owner, &calls, budget)?;
                    let contracts = cr_build_contracts_v1(owner, inventory, &source, budget)?;
                    cr_check_source_rows_v1(owner, inventory, &calls, &source, budget)?;
                    cr_check_arguments_v1(owner, &calls, &arguments, budget)?;
                    cr_check_contracts_v1(owner, inventory, &source, &contracts, budget)?;
                    let guard = CrGuardV1::new(budget);
                    let metadata = ProductionCanonicalRankedMetadataV1 {
                        owner,
                        inventory,
                        calls: &calls,
                        source: &source,
                        arguments: &arguments,
                        contracts: &contracts,
                        guard: &guard,
                    };
                    cr_callback_v1(&guard, budget, |budget| run(&metadata, budget))
                })
            })
        })
    })
}

struct CrProjectionV1 {
    // Closed schema: Module/Refinement header, ordered Kernel/Launch facts,
    // Function/Memory root-qualified source association triples. Complete typed
    // source facts remain in the source facade; this inert projection is not a
    // substitute for those facts or a claim that other obligations are absent.
    keys: Vec<(CrSubjectV1, CrKindV1, std::ops::Range<usize>)>,
    facts: Vec<CrFactV1>,
}
fn cr_build_projection_v1(
    source: &ProductionCanonicalRankedMetadataV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> CrResultV1<CrProjectionV1> {
    let kernel_count = source.contracts.launches.len();
    let function_count = source.inventory.functions().len();
    let count = argument_sum_v1(&[1, kernel_count, function_count])?;
    let fact_count = argument_sum_v1(&[
        7,
        argument_product_v1(kernel_count, 11)?,
        function_count,
        argument_product_v1(source.calls.groups.len(), 3)?,
    ])?;
    budget.reserve_storage(argument_sum_v1(&[
        std::mem::size_of::<CrProjectionV1>(),
        std::mem::size_of::<Vec<(usize, usize)>>(),
    ])?)?;
    let mut out = CrProjectionV1 {
        keys: cr_vec_v1(count, budget)?,
        facts: cr_vec_v1(fact_count, budget)?,
    };
    for value in [
        1,
        source.source.spans.len(),
        source.arguments.rows.len(),
        source
            .owner
            .semantic_ssa
            .source_semantic()
            .callables()
            .len(),
        source.contracts.catalog.definitions().len(),
        source.contracts.catalog.bindings().len(),
        0,
    ] {
        cr_push_v1(&mut out.facts, CrFactV1::Unsigned(value as u64), budget)?;
    }
    cr_push_v1(
        &mut out.keys,
        (CrSubjectV1::Module, CrKindV1::Refinement, 0..7),
        budget,
    )?;
    for (ordinal, row) in source.contracts.launches.iter().enumerate() {
        let start = out.facts.len();
        let layout = row.source.layout();
        let g = layout.global_extents();
        let w = layout.workgroup_extents();
        for value in [
            u64::from(row.source.selected_root().index()),
            u64::from(row.source.source_rank()),
            layout.grid_identity(),
            g[0],
            g[1],
            g[2],
            w[0],
            w[1],
            w[2],
            layout.subgroup_size(),
            u64::from(layout.full_physical_workgroups()),
        ] {
            cr_push_v1(&mut out.facts, CrFactV1::Unsigned(value), budget)?;
        }
        cr_push_v1(
            &mut out.keys,
            (
                CrSubjectV1::Kernel(ordinal),
                CrKindV1::Launch,
                start..out.facts.len(),
            ),
            budget,
        )?;
    }
    let floor = budget.storage();
    let mut associations = cr_vec_v1(source.calls.groups.len(), budget)?;
    for (i, group) in source.calls.groups.iter().enumerate() {
        cr_push_v1(
            &mut associations,
            (group.function.canonical.coordinate.0 as usize, i),
            budget,
        )?;
    }
    source_catalog_sort_v1(&mut associations, |row| *row, budget)?;
    let mut next = 0;
    for function in 0..function_count {
        let start = out.facts.len();
        let first = next;
        while next < associations.len() && associations[next].0 == function {
            budget.charge_work(1)?;
            next += 1;
        }
        cr_push_v1(
            &mut out.facts,
            CrFactV1::Unsigned((next - first) as u64),
            budget,
        )?;
        for (_, index) in &associations[first..next] {
            let group = &source.calls.groups[*index];
            for value in [
                *index as u64,
                u64::from(group.function.source.correspondence_owner.index()),
                u64::from(group.function.source.semantic_function.index()),
            ] {
                cr_push_v1(&mut out.facts, CrFactV1::Unsigned(value), budget)?;
            }
        }
        cr_push_v1(
            &mut out.keys,
            (
                CrSubjectV1::Function(function),
                CrKindV1::Memory,
                start..out.facts.len(),
            ),
            budget,
        )?;
    }
    if next != associations.len() || out.keys.len() != count || out.facts.len() != fact_count {
        return Err(cr_invalid_v1("projection source roster"));
    }
    drop(associations);
    budget.release_storage(budget.storage() - floor)?;
    Ok(out)
}
fn cr_check_projection_v1(
    source: &ProductionCanonicalRankedMetadataV1<'_>,
    projection: &CrProjectionV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> CrResultV1<()> {
    let kernels = source.contracts.launches.len();
    let functions = source.inventory.functions().len();
    budget.charge_work(4)?;
    if projection.keys.len() != 1 + kernels + functions {
        return Err(cr_invalid_v1("projection row count"));
    }
    let header = [
        1,
        source.source.spans.len(),
        source.arguments.rows.len(),
        source
            .owner
            .semantic_ssa
            .source_semantic()
            .callables()
            .len(),
        source.contracts.catalog.definitions().len(),
        source.contracts.catalog.bindings().len(),
        0,
    ];
    if projection.keys[0] != (CrSubjectV1::Module, CrKindV1::Refinement, 0..7) {
        return Err(cr_invalid_v1("projection pending-refinement header"));
    }
    for (i, value) in header.into_iter().enumerate() {
        budget.charge_work(1)?;
        if projection.facts.get(i) != Some(&CrFactV1::Unsigned(value as u64)) {
            return Err(cr_invalid_v1("projection source header"));
        }
    }
    let mut next = 7;
    for (ordinal, launch) in source.contracts.launches.iter().enumerate() {
        let row = &projection.keys[ordinal + 1];
        if row
            != &(
                CrSubjectV1::Kernel(ordinal),
                CrKindV1::Launch,
                next..next + 11,
            )
        {
            return Err(cr_invalid_v1("projection ordered kernel"));
        }
        let actual = projection
            .facts
            .get(next..next + 11)
            .ok_or_else(|| cr_invalid_v1("projection launch facts"))?;
        let source_row = launch.source;
        let layout = source_row.layout();
        let global = layout.global_extents();
        let workgroup = layout.workgroup_extents();
        let expected = [
            source_row.selected_root().index() as u64,
            source_row.source_rank() as u64,
            layout.grid_identity(),
            global[0],
            global[1],
            global[2],
            workgroup[0],
            workgroup[1],
            workgroup[2],
            layout.subgroup_size(),
            u64::from(layout.full_physical_workgroups()),
        ];
        for (actual, expected) in actual.iter().zip(expected) {
            budget.charge_work(1)?;
            if *actual != CrFactV1::Unsigned(expected) {
                return Err(cr_invalid_v1("projection exact layout"));
            }
        }
        next += 11;
    }
    let floor = budget.storage();
    budget.reserve_storage(std::mem::size_of::<Vec<bool>>())?;
    let mut seen = cr_vec_v1(source.calls.groups.len(), budget)?;
    budget.charge_work(source.calls.groups.len())?;
    seen.resize(source.calls.groups.len(), false);
    for function in 0..functions {
        budget.charge_work(4)?;
        let row = &projection.keys[1 + kernels + function];
        if row.0 != CrSubjectV1::Function(function)
            || row.1 != CrKindV1::Memory
            || row.2.start != next
            || row.2.end < next + 1
        {
            return Err(cr_invalid_v1("projection function row"));
        }
        let facts = projection
            .facts
            .get(row.2.clone())
            .ok_or_else(|| cr_invalid_v1("projection function range"))?;
        let CrFactV1::Unsigned(count) = facts[0] else {
            return Err(cr_invalid_v1("projection association count"));
        };
        let count = usize::try_from(count).map_err(|_| ArgumentResourceV1::Arithmetic)?;
        if facts.len() != argument_sum_v1(&[1, argument_product_v1(count, 3)?])? {
            return Err(cr_invalid_v1("projection association payload"));
        }
        let mut previous = None;
        for triple in facts[1..].chunks_exact(3) {
            budget.charge_work(7)?;
            let CrFactV1::Unsigned(index) = triple[0] else {
                return Err(cr_invalid_v1("projection association index"));
            };
            let index = usize::try_from(index).map_err(|_| ArgumentResourceV1::Arithmetic)?;
            let group = source
                .calls
                .groups
                .get(index)
                .ok_or_else(|| cr_invalid_v1("projection donor association"))?;
            if seen[index]
                || previous.is_some_and(|p| p >= index)
                || group.function.canonical.coordinate.0 as usize != function
                || triple[1]
                    != CrFactV1::Unsigned(group.function.source.correspondence_owner.index() as u64)
                || triple[2]
                    != CrFactV1::Unsigned(group.function.source.semantic_function.index() as u64)
            {
                return Err(cr_invalid_v1("projection source association identity"));
            }
            seen[index] = true;
            previous = Some(index);
        }
        next = row.2.end;
    }
    budget.charge_work(seen.len())?;
    if seen.iter().any(|seen| !seen) || next != projection.facts.len() {
        return Err(cr_invalid_v1("projection completeness"));
    }
    drop(seen);
    budget.release_storage(budget.storage() - floor)?;
    Ok(())
}
fn cr_with_checked_source_v1<'w, T>(
    source: &ProductionCanonicalRankedMetadataV1<'_>,
    budget: &mut ArgumentBudgetV1<'w>,
    run: impl for<'s, 'v, 'i, 'g, 'm> FnOnce(
        &mut ProductionCanonicalRankedSourceViewV1<'s, 'v, 'i, 'g, 'm>,
        &mut ArgumentBudgetV1<'w>,
    ) -> CrResultV1<T>,
) -> CrResultV1<T> {
    cr_protected_v1(budget, |budget| {
        let projection = cr_build_projection_v1(source, budget)?;
        cr_check_projection_v1(source, &projection, budget)?;
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<Vec<CrInertRowV1<'_>>>(),
            std::mem::size_of::<CrInertV1<'_, '_>>(),
            std::mem::size_of::<ProductionCanonicalRankedMetadataV1<'_>>(),
            std::mem::size_of::<CrGuardV1>(),
            std::mem::size_of::<ProductionCanonicalRankedSourceViewV1<'_, '_, '_, '_, '_>>(),
        ])?)?;
        let mut rows = cr_vec_v1(projection.keys.len(), budget)?;
        for (subject, kind, range) in &projection.keys {
            cr_push_v1(
                &mut rows,
                CrInertRowV1 {
                    subject: *subject,
                    kind: *kind,
                    facts: &projection.facts[range.clone()],
                },
                budget,
            )?;
        }
        let inert = CrInertV1::new(source.owner.executable(), &rows);
        let (candidate, storage) = fe2o3_kernel_analysis::build_canonical_ranked_candidate_v1(
            source.inventory,
            &inert,
            budget,
        )?;
        budget.reserve_storage(storage.retained_storage())?;
        fe2o3_kernel_analysis::with_checked_canonical_ranked_view_v1(
            source.inventory,
            &inert,
            &candidate,
            budget,
            |checked, budget| {
                let guard = CrGuardV1::new(budget);
                let metadata = ProductionCanonicalRankedMetadataV1 {
                    owner: source.owner,
                    inventory: source.inventory,
                    calls: source.calls,
                    source: source.source,
                    arguments: source.arguments,
                    contracts: source.contracts,
                    guard: &guard,
                };
                cr_callback_v1(&guard, budget, |budget| {
                    run(
                        &mut ProductionCanonicalRankedSourceViewV1 {
                            source: &metadata,
                            checked,
                        },
                        budget,
                    )
                })
            },
        )
    })
}
