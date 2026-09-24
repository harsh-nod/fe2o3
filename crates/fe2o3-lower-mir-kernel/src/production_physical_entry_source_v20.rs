// Included in production_semantic_kir_v1, the sole semantic importer.
// MIR37 source transport and allocated KIR20 executable profile.
// Normal source owners are consumed; no pending Function/Module is an input.
include!("production_physical_entry_context_v20.rs");
#[path = "production_physical_entry_parameter_v20.rs"]
mod physical_entry_parameter_v20;
include!("production_physical_entry_transport_v20.rs");
include!("production_physical_entry_correspondence_v20.rs");

/// Source reconstruction, budget or exact KIR20 admission failure.
#[derive(Debug)]
pub enum ProductionPhysicalEntrySourceErrorV20 {
    /// The retained source, ABI, launch or exact reconstruction was refused.
    Lowering(ProductionSemanticKirErrorV1),
    /// Exact KIR20 inverse/general/profile admission was refused.
    Canonical(fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV20),
}
impl fmt::Display for ProductionPhysicalEntrySourceErrorV20 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lowering(e) => e.fmt(f),
            Self::Canonical(e) => e.fmt(f),
        }
    }
}
impl Error for ProductionPhysicalEntrySourceErrorV20 {}
impl From<ProductionSemanticKirErrorV1> for ProductionPhysicalEntrySourceErrorV20 {
    fn from(e: ProductionSemanticKirErrorV1) -> Self {
        Self::Lowering(e)
    }
}
impl From<ArgumentResourceV1> for ProductionPhysicalEntrySourceErrorV20 {
    fn from(e: ArgumentResourceV1) -> Self {
        Self::Lowering(e.into())
    }
}

/// Exact source/pre-ranked custody. Immutable source SSA and source launch stay
/// with the one freshly inverse-decoded executable graph and its correspondence.
/// There is no Clone, deserialize, mutable escape, V12 conversion or artifact admission.
/// This is explicitly pre-ranked: mandatory ranked/formal/descriptor continuation
/// is not supplied by this owner. Diagnostic emission cannot certify those stages.
/// Source/provider authentication belongs to the retained compiler preparation.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionPhysicalEntryPreRankedKirOwnerV20;
/// fn copy<T: Clone>() {}
/// copy::<ProductionPhysicalEntryPreRankedKirOwnerV20>();
/// ```
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionPhysicalEntryPreRankedKirOwnerV20;
/// fn mutate(x: &mut ProductionPhysicalEntryPreRankedKirOwnerV20) {
///     x.executable().module().functions.clear();
/// }
/// ```
#[must_use]
pub struct ProductionPhysicalEntryPreRankedKirOwnerV20 {
    semantic_ssa: ProductionSemanticSsaOwnerV1,
    source_launch: crate::ProductionSourceLaunchRosterV1,
    executable: fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV20,
    correspondence: PhysicalEntrySourceCorrespondenceV20,
    limits: ProductionSemanticKirLimitsV1,
    retained_storage: usize,
}
impl ProductionPhysicalEntryPreRankedKirOwnerV20 {
    /// Existing source/SSA/root-planner limits remain separately bounded; the
    /// source must declare exactly max_grid [2, 1, 1]; a dynamic or different
    /// source envelope is not replaced by a synthetic concrete launch. The
    /// receipt covers added owner/correspondence/canonical requested payload,
    /// not borrowed compiler state, old planner allocation excess or RSS.
    /// The same ledger's incoming storage floor survives Result and unwind.
    pub fn try_materialize_with_budget(
        semantic_ssa: ProductionSemanticSsaOwnerV1,
        source_launch: crate::ProductionSourceLaunchRosterV1,
        limits: ProductionSemanticKirLimitsV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionPhysicalEntrySourceErrorV20> {
        semantic_ssa
            .verify_replay()
            .map_err(ProductionSemanticKirErrorV1::SemanticSsa)?;
        let mut scope = PhysicalEntryLedgerScopeV20::new(budget);
        scope.reserve(std::mem::size_of::<Self>())?;
        let context =
            physical_entry_context_v20(&semantic_ssa, &source_launch, limits, scope.budget)?;
        let (module, correspondence, temporary, rows) = lower_physical_entry_normal_root_v20(
            &semantic_ssa,
            &source_launch,
            &context,
            limits,
            scope.budget,
        )?;
        let canonical_floor = scope.budget.storage();
        let (executable, storage) = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV20::
            from_module_ref_with_verification_budget_v20(&module, scope.budget)
            .map_err(ProductionPhysicalEntrySourceErrorV20::Canonical)?;
        if scope.budget.storage() != canonical_floor {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        // The inverse checks exact entire-Module equality and general SSA/type/
        // effect + new ordinal/profile rules before this graph can be retained.
        // Drop the transient source graph; keep no second executable body.
        drop(module);
        scope.budget.release_storage(temporary)?;
        scope.reserve(storage.retained_storage())?;
        let retained_storage = argument_sum_v1(&[
            std::mem::size_of::<Self>(),
            rows,
            storage.retained_storage(),
        ])?;
        if scope.budget.storage() != argument_sum_v1(&[scope.floor, retained_storage])? {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let result = Self {
            semantic_ssa,
            source_launch,
            executable,
            correspondence,
            limits,
            retained_storage,
        };
        result.verify_equivalence(scope.budget)?;
        Ok(result)
    }
    /// Actual retained semantic SSA owner.
    pub fn semantic_ssa(&self) -> &ProductionSemanticSsaOwnerV1 {
        &self.semantic_ssa
    }
    /// Actual retained source launch roster.
    pub fn source_launch(&self) -> &crate::ProductionSourceLaunchRosterV1 {
        &self.source_launch
    }
    /// One inverse-decoded immutable KIR20 executable owner.
    pub fn executable(&self) -> &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV20 {
        &self.executable
    }
    /// Exact source expansion/elimination relation.
    pub fn correspondence(&self) -> &PhysicalEntrySourceCorrespondenceV20 {
        &self.correspondence
    }
    /// Unreserved logical added-owner storage receipt, excluding borrowed compiler state.
    pub const fn retained_storage(&self) -> usize {
        self.retained_storage
    }
    /// Source preparation alone grants no artifact or launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }

    /// Replays from the actual retained semantic/launch owners. Identity hashes
    /// are not a substitute for the exact reconstructed function/module/rows.
    pub fn verify_equivalence(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionPhysicalEntrySourceErrorV20> {
        if budget.storage() < self.retained_storage {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        self.semantic_ssa
            .verify_replay()
            .map_err(ProductionSemanticKirErrorV1::SemanticSsa)?;
        let scope = PhysicalEntryLedgerScopeV20::new(budget);
        let context = physical_entry_context_v20(
            &self.semantic_ssa,
            &self.source_launch,
            self.limits,
            scope.budget,
        )?;
        let (module, correspondence, _, _) = lower_physical_entry_normal_root_v20(
            &self.semantic_ssa,
            &self.source_launch,
            &context,
            self.limits,
            scope.budget,
        )?;
        // Both reassembled graphs contain only the <=4 authored blocks/64
        // native primitives; no compiler-owned tail. Source rows are separately bounded.
        scope.budget.charge_work(argument_sum_v1(&[
            32768,
            argument_product_v1(context.function.blocks().len(), 64)?,
        ])?)?;
        if module != *self.executable.module() || correspondence != self.correspondence {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch.into());
        }
        Ok(())
    }
}

/// The source-specific body algorithm is inside the sole importer, while the
/// ordinary root ABI and kernel launch/metadata planners remain authoritative.
/// This function is private and accepts the same actual owners as its caller;
/// there is no public Module-wrapping or pending-plan admission endpoint.
fn lower_physical_entry_normal_root_v20(
    owner: &ProductionSemanticSsaOwnerV1,
    launch: &crate::ProductionSourceLaunchRosterV1,
    context: &PhysicalEntryContextV20<'_>,
    limits: ProductionSemanticKirLimitsV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<
    (Module, PhysicalEntrySourceCorrespondenceV20, usize, usize),
    ProductionSemanticKirErrorV1,
> {
    let mut closure_budget = ReachableClosureBudgetV1::new(limits.max_blocks);
    let plan = kernel_entry_plan_v1(
        owner.source_semantic(),
        context.root,
        context.root,
        FunctionId::new(context.symbol),
        limits.max_operations,
        &mut closure_budget,
    )?;
    let input = context.input();
    let pending = crate::physical_entry_materialization_v20::materialize(&input, budget)
        .map_err(physical_entry_materialization_error_v20)?;
    let body_storage = pending.retained_storage();
    budget.reserve_storage(body_storage)?;
    if plan.parameter_types != pending.function().signature.parameters
        || plan.result_types != pending.function().signature.results
        || plan.kernel_ir_function != pending.function().id
    {
        return Err(physical_entry_refusal(
            "physical-entry ordinary root ABI plan differs",
        ));
    }
    let body = pending
        .function()
        .body
        .as_ref()
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    if body.blocks.len() > limits.max_blocks {
        return Err(physical_entry_refusal(
            "physical-entry canonical block limit",
        ));
    }
    let operation_count = body.blocks.iter().try_fold(0usize, |count, block| {
        count
            .checked_add(block.operations.len())
            .ok_or(ArgumentResourceV1::Arithmetic)
    })?;
    if operation_count > limits.max_operations {
        return Err(physical_entry_refusal(
            "physical-entry canonical operation limit",
        ));
    }
    let (correspondence, rows) = PhysicalEntrySourceCorrespondenceV20::capture(
        context,
        *owner.source_semantic().semantic_sha256().as_bytes(),
        &plan,
        budget,
    )?;
    // Reuse the exact authenticated launch conversion used by normal lowering.
    let root_bytes = argument_product_v1(std::mem::size_of::<RetainedRankedLaunchRootV1>(), 2)?;
    budget.reserve_storage(root_bytes)?;
    let roots = materialization_launch_roots_v1(owner, launch)?;
    let [root] = roots.as_ref() else {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    };
    let actual_launch = *root;
    // No fixed synthetic launch/function is supplied by the caller. This is
    // exactly the ordinary semantic module identity and shared kernel metadata.
    // Logical set entries and their exact strings at function/kernel/module
    // scope. Allocator BTree node excess is excluded, as for existing payload
    // accounting; these are not RSS or host-OOM recovery bounds.
    let capability_payload = physical_entry_capability_payload_v20()?;
    let shell = argument_sum_v1(&[
        capability_payload,
        std::mem::size_of::<Module>(),
        std::mem::size_of::<Function>(),
        std::mem::size_of::<Kernel>(),
        "fe2o3::semantic::".len() + 64,
        argument_product_v1(context.symbol.len(), 2)?,
    ])?;
    budget.reserve_storage(shell)?;
    let name = format!(
        "fe2o3::semantic::{}",
        hex_identity(owner.source_semantic().semantic_sha256().as_bytes())
    );
    let mut functions = Vec::new();
    functions
        .try_reserve_exact(1)
        .map_err(|_| ArgumentResourceV1::Allocation)?;
    functions.push(pending.into_function());
    let mut module = Module::new(name);
    module.functions = functions;
    module.functions[0].required_capabilities = physical_entry_capabilities_v20();
    module.required_capabilities = module.functions[0].required_capabilities.clone();
    finish_semantic_root_module_v1(
        &mut module,
        context.symbol,
        Some([64, 1, 1]),
        1,
        Some(actual_launch),
    )?;
    drop(roots);
    budget.release_storage(root_bytes)?;
    // No helper or unsupported intrinsic declaration, no duplicate executable
    // body, and no alternate native artifact path is introduced.
    Ok((
        module,
        correspondence,
        argument_sum_v1(&[body_storage, shell])?,
        rows,
    ))
}

fn physical_entry_capabilities_v20() -> std::collections::BTreeSet<fe2o3_kernel_ir::TargetCapability>
{
    use fe2o3_kernel_ir::*;
    [
        TargetCapability::Extension {
            namespace: AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE.into(),
            name: AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME.into(),
        },
        TargetCapability::WaveWidth(WaveWidth::Wave64),
        TargetCapability::Extension {
            namespace: AMDGPU_GFX942_PHYSICAL_ENTRY_CAPABILITY_NAMESPACE_V20.into(),
            name: AMDGPU_GFX942_PHYSICAL_ENTRY_CAPABILITY_NAME_V20.into(),
        },
    ]
    .into_iter()
    .collect()
}

fn physical_entry_capability_payload_v20() -> Result<usize, ArgumentResourceV1> {
    use fe2o3_kernel_ir::{
        AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE, AMDGPU_GFX942_PHYSICAL_ENTRY_CAPABILITY_NAME_V20,
        AMDGPU_GFX942_PHYSICAL_ENTRY_CAPABILITY_NAMESPACE_V20,
        AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME, TargetCapability,
    };
    let strings = argument_sum_v1(&[
        AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE.len(),
        AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME.len(),
        AMDGPU_GFX942_PHYSICAL_ENTRY_CAPABILITY_NAMESPACE_V20.len(),
        AMDGPU_GFX942_PHYSICAL_ENTRY_CAPABILITY_NAME_V20.len(),
    ])?;
    argument_product_v1(
        3,
        argument_sum_v1(&[
            argument_product_v1(3, std::mem::size_of::<TargetCapability>())?,
            strings,
        ])?,
    )
}

fn physical_entry_materialization_error_v20(
    error: crate::physical_entry_materialization_v20::Error,
) -> ProductionSemanticKirErrorV1 {
    use crate::physical_entry_materialization_v20::Error as E;
    match error {
        E::Resource(error) => error.into(),
        E::Origin => physical_entry_refusal("physical-entry canonical origin is incomplete"),
        E::Symbol => physical_entry_refusal("physical-entry canonical symbol differs"),
        E::Occurrence => {
            physical_entry_refusal("physical-entry canonical occurrence order differs")
        }
        E::Shape => physical_entry_refusal("physical-entry canonical control-flow grammar differs"),
        E::Instruction => {
            physical_entry_refusal("physical-entry canonical instruction shape differs")
        }
        E::UndefinedRegister => {
            physical_entry_refusal("physical-entry reads an undefined physical register")
        }
        E::Count => physical_entry_refusal("physical-entry canonical count exceeds exact bounds"),
        E::Accounting => ArgumentResourceV1::Accounting.into(),
    }
}

struct PhysicalEntryLedgerScopeV20<'a, 'work> {
    budget: &'a mut ArgumentBudgetV1<'work>,
    floor: usize,
}
impl<'a, 'work> PhysicalEntryLedgerScopeV20<'a, 'work> {
    fn new(budget: &'a mut ArgumentBudgetV1<'work>) -> Self {
        let floor = budget.storage();
        Self { budget, floor }
    }
    fn reserve(&mut self, bytes: usize) -> Result<(), ArgumentResourceV1> {
        self.budget.reserve_storage(bytes)
    }
}
impl Drop for PhysicalEntryLedgerScopeV20<'_, '_> {
    fn drop(&mut self) {
        // Exclusive ledger borrow; all callees obey the same entry-floor rule.
        if let Some(added) = self.budget.storage().checked_sub(self.floor) {
            let _ = self.budget.release_storage(added);
        }
    }
}
