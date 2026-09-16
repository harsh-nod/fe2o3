use fe2o3_kernel_analysis::{CanonicalKirFunctionRefV1, CanonicalKirInventoryV1};

/// One root-qualified source association bound to its actual canonical function.
/// This is occurrence identity, not a purity, value, provenance or authority proof.
pub struct ProductionCanonicalCallFunctionV1<'a> {
    source: &'a SemanticKirFunctionCorrespondenceV1,
    canonical: &'a CanonicalKirFunctionRefV1<'a>,
}

impl ProductionCanonicalCallFunctionV1<'_> {
    /// Original association, including its source root and body identities.
    pub const fn source(&self) -> &SemanticKirFunctionCorrespondenceV1 {
        self.source
    }
    /// Exact physical function in the bound inventory.
    pub const fn canonical(&self) -> &CanonicalKirFunctionRefV1<'_> {
        self.canonical
    }
}

struct CanonicalCallGroupV1<'a> {
    function: ProductionCanonicalCallFunctionV1<'a>,
    calls: &'a [SemanticKirCallReturnV1],
    spans: &'a [SemanticKirTerminatorOperationSpanV1],
    direct: &'a [SemanticKirParameterBindingV1],
    components: &'a [SemanticKirParameterComponentBindingV1],
    ignored: &'a [SemanticKirIgnoredParameterBindingV1],
}

impl CanonicalCallGroupV1<'_> {
    fn parameters(&self) -> ArgumentTraceV1<'_> {
        ArgumentTraceV1 {
            direct: self.direct,
            components: self.components,
            ignored: self.ignored,
        }
    }
}

struct CanonicalCallBindingV1<'a> {
    call: usize,
    callee: usize,
    site: CheckedCallSiteV1<'a>,
}

impl CanonicalCallBindingV1<'_> {
    fn key(&self) -> (u32, usize) {
        (self.site.caller.correspondence_owner.index(), self.call)
    }
}

/// Scoped source bindings for one exact canonical inventory. Shared physical
/// calls retain separate root associations; invocation paths are not duplicated.
/// Preparation scans correspondence and validates each association once. Queries
/// do no whole-module or whole-correspondence scans; selected ABI/result work and
/// callback work remain separately metered. No executable graph is copied.
pub struct ProductionCanonicalCallsV1<'a> {
    owner: &'a ProductionPreRankedKirOwnerV1,
    inventory: &'a CanonicalKirInventoryV1<'a>,
    groups: Vec<CanonicalCallGroupV1<'a>>,
    calls: Vec<CanonicalCallBindingV1<'a>>,
}

impl ProductionCanonicalCallsV1<'_> {
    /// Locators and references apply only to this exact inventory instance.
    pub fn belongs_to(&self, inventory: &CanonicalKirInventoryV1<'_>) -> bool {
        std::ptr::eq(self.inventory, inventory)
    }

    /// Number of root-qualified associations, including kernel entries.
    pub fn function_count(&self) -> usize {
        self.groups.len()
    }

    /// Caller charges the function count before scanning these borrowed rows.
    pub fn functions(&self) -> impl Iterator<Item = &ProductionCanonicalCallFunctionV1<'_>> {
        self.groups.iter().map(|group| &group.function)
    }

    /// Ordinary calls across all associations, excluding registered builtins.
    pub fn call_count(&self) -> usize {
        self.calls.len()
    }

    /// Ordered root and inventory-call locators, not transferable evidence.
    /// Caller charges the call count before scanning this roster.
    pub fn sites(&self) -> impl Iterator<Item = (SemanticFunctionIdV1, usize)> + '_ {
        self.calls
            .iter()
            .map(|call| (call.site.caller.correspondence_owner, call.call))
    }

    /// Reuses the checked call view, including physical slots, RustCall packing,
    /// ignored components and result transport. Scratch drops before restoring
    /// the incoming floor on success, error or unwind; work/peak history remains.
    pub fn with_call<'w, R>(
        &self,
        root: SemanticFunctionIdV1,
        call: usize,
        budget: &mut ArgumentBudgetV1<'w>,
        use_view: impl for<'s> FnOnce(
            &mut ProductionCallViewV1<'s, 'w>,
        ) -> Result<R, ProductionSemanticKirErrorV1>,
    ) -> Result<R, ProductionSemanticKirErrorV1> {
        with_canonical_call_scratch_v1(budget, |budget| {
            let index = assert_origin_find_v1(&self.calls, budget, |row, budget| {
                budget.charge_work(1)?;
                Ok(row.key().cmp(&(root.index(), call)))
            })
            .map_err(call_index_error_v1)?
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            let binding = &self.calls[index];
            with_checked_call_site_v1(
                &self.owner.semantic_ssa,
                &self.owner.correspondence.call_result_components,
                binding.site,
                self.groups[binding.callee].parameters(),
                budget,
                use_view,
            )
        })
    }
}

impl ProductionPreRankedKirOwnerV1 {
    /// Binds the complete ordinary-call roster before invoking the consumer.
    /// Registered builtins use their existing closed effect classification.
    /// Foreign owners, missing/extra sites and resource exhaustion reject before
    /// callbacks. Caller retains graph, inventory and source reservations.
    /// Callback allocations are scratch-only or separately pre-reserved outputs;
    /// this logical ledger is not an allocator/RSS bound or a security capability.
    ///
    /// ```compile_fail
    /// use fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1;
    /// use fe2o3_kernel_analysis::CanonicalKirInventoryV1;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// fn escape(owner: &ProductionPreRankedKirOwnerV1, inventory: &CanonicalKirInventoryV1<'_>, budget: &mut Budget<'_>) {
    ///     let mut saved = None;
    ///     owner.with_checked_canonical_calls_v1(inventory, budget, |calls, _| {
    ///         saved = calls.functions().next();
    ///         Ok(())
    ///     }).unwrap();
    ///     drop(saved);
    /// }
    /// ```
    pub fn with_checked_canonical_calls_v1<'w, R>(
        &self,
        inventory: &CanonicalKirInventoryV1<'_>,
        budget: &mut ArgumentBudgetV1<'w>,
        use_calls: impl for<'s> FnOnce(
            &ProductionCanonicalCallsV1<'s>,
            &mut ArgumentBudgetV1<'w>,
        ) -> Result<R, ProductionSemanticKirErrorV1>,
    ) -> Result<R, ProductionSemanticKirErrorV1> {
        with_canonical_call_scratch_v1(budget, |budget| {
            budget.charge_work(1)?;
            if !inventory.belongs_to(self.executable()) {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            let calls = ProductionCanonicalCallsV1::build(self, inventory, budget)?;
            let callback_floor = budget.storage();
            let result = use_calls(&calls, budget);
            if budget.storage() != callback_floor {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            result
        })
    }
}

fn with_canonical_call_scratch_v1<'w, R>(
    budget: &mut ArgumentBudgetV1<'w>,
    body: impl FnOnce(&mut ArgumentBudgetV1<'w>) -> Result<R, ProductionSemanticKirErrorV1>,
) -> Result<R, ProductionSemanticKirErrorV1> {
    let floor = budget.storage();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(budget)));
    budget.release_storage(
        budget
            .storage()
            .checked_sub(floor)
            .ok_or(ArgumentResourceV1::Accounting)?,
    )?;
    match result {
        Ok(result) => result,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

fn canonical_call_inventory_error_v1(
    error: fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1,
) -> ProductionSemanticKirErrorV1 {
    match error {
        fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::Resource(error) => error.into(),
        _ => ProductionSemanticKirErrorV1::CorrespondenceMismatch,
    }
}

include!("production_canonical_call_index_v1.rs");
