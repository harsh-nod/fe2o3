include!("production_local_helper_relations_v1.rs");
include!("production_local_helper_source_v1.rs");
include!("production_local_helper_ranked_v1.rs");
include!("production_local_helper_values_v1.rs");
include!("production_local_helper_statements_v1.rs");
include!("production_local_helper_memory_v1.rs");
include!("production_local_helper_control_v1.rs");
include!("production_local_helper_calls_v1.rs");

use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as HelperMemoryResourceV1,
    LocalFrameAccessV1 as RetainedLocalAccessV1,
    LocalFrameAllocationV1 as RetainedLocalAllocationV1,
    LocalFrameControlV1 as RetainedLocalControlV1,
    LocalFrameEdgeBindingV1 as RetainedLocalEdgeBindingV1,
};

/// Logical retained helper payload. This receipt has no semantic authority by
/// itself and is reserved alongside the same owner's graph and origin receipts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionHelperMemoryStorageV1(usize);

impl ProductionHelperMemoryStorageV1 {
    /// Header and actual vector-capacity bytes, excluding the executable graph.
    /// The nominal BF16 profile additionally retains its conservative prepaid
    /// construction envelope until owner teardown; this is not RSS.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RetainedHelperKindV1 {
    NotHelper,
    Pending,
    RawEmpty,
    Bf16Nominal,
    Local {
        allocations: (usize, usize),
        accesses: (usize, usize),
        control: (usize, usize),
        edge_bindings: (usize, usize),
    },
}

#[derive(Clone, Copy, Debug)]
struct RetainedHelperAssociationV1 {
    physical: usize,
}

#[derive(Clone, Copy, Debug)]
enum HelperOccurrenceCaptureV1 {
    Absent,
    Preexisting(fe2o3_pliron::ProductionSemanticSsaOccurrenceStorageV1),
    Transferred(fe2o3_pliron::ProductionSemanticSsaOccurrenceStorageV1),
}

impl HelperOccurrenceCaptureV1 {
    fn preexisting_storage(self) -> usize {
        match self {
            Self::Preexisting(receipt) => receipt.retained_storage(),
            Self::Absent | Self::Transferred(_) => 0,
        }
    }

    fn transferred_storage(self) -> usize {
        match self {
            Self::Transferred(receipt) => receipt.retained_storage(),
            Self::Absent | Self::Preexisting(_) => 0,
        }
    }
}

#[derive(Debug)]
struct SealedHelperMemoryV1 {
    functions: Vec<RetainedHelperKindV1>,
    associations: Vec<RetainedHelperAssociationV1>,
    allocations: Vec<RetainedLocalAllocationV1>,
    accesses: Vec<RetainedLocalAccessV1>,
    control: Vec<RetainedLocalControlV1>,
    edge_bindings: Vec<RetainedLocalEdgeBindingV1>,
    unit_source: SealedUnitLocalSourceV1,
    bf16_nominal: Option<Box<SealedBf16CallRelationV1>>,
    capture: HelperOccurrenceCaptureV1,
    storage: ProductionHelperMemoryStorageV1,
    analysis_storage: usize,
}

fn helper_memory_live_storage_v1(
    graph: usize,
    origins: usize,
    helpers: usize,
) -> Result<usize, HelperMemoryResourceV1> {
    graph
        .checked_add(origins)
        .and_then(|n| n.checked_add(helpers))
        .ok_or(HelperMemoryResourceV1::Arithmetic)
}

fn helper_memory_vec_v1<T>(
    count: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<T>, ProductionPreRankedKirErrorV1> {
    budget.charge_work(3)?;
    let bytes = count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or(HelperMemoryResourceV1::Arithmetic)?;
    budget.reserve_storage(bytes)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| HelperMemoryResourceV1::Allocation)?;
    let extra = rows
        .capacity()
        .checked_sub(count)
        .and_then(|n| n.checked_mul(std::mem::size_of::<T>()))
        .ok_or(HelperMemoryResourceV1::Arithmetic)?;
    budget.reserve_storage(extra)?;
    Ok(rows)
}

fn helper_memory_effect_error_v1(
    error: fe2o3_kernel_analysis::CanonicalKirCallEffectErrorV1,
) -> ProductionPreRankedKirErrorV1 {
    match error {
        fe2o3_kernel_analysis::CanonicalKirCallEffectErrorV1::Resource(error) => error.into(),
        _ => ProductionSemanticKirErrorV1::CorrespondenceMismatch.into(),
    }
}

impl SealedHelperMemoryV1 {
    fn is_raw_empty(&self, association: usize) -> bool {
        self.associations
            .get(association)
            .and_then(|row| self.functions.get(row.physical))
            == Some(&RetainedHelperKindV1::RawEmpty)
    }

    fn derive(
        subject: CanonicalCallSubjectV1<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionPreRankedKirErrorV1> {
        Self::derive_with_origins_v1(subject, None, budget)
    }

    fn derive_with_origins_v1(
        subject: CanonicalCallSubjectV1<'_>,
        origins: Option<&SealedAssertOriginsV1>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionPreRankedKirErrorV1> {
        let floor = budget.storage();
        let work_ledger = budget.work_ledger_identity_v1();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Self::build(subject, origins, budget)
        }));
        if work_ledger != budget.work_ledger_identity_v1() {
            return Err(HelperMemoryResourceV1::Accounting.into());
        }
        let release = budget
            .storage()
            .checked_sub(floor)
            .ok_or(HelperMemoryResourceV1::Accounting)?;
        budget.release_storage(release)?;
        match result {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    fn build(
        subject: CanonicalCallSubjectV1<'_>,
        origins: Option<&SealedAssertOriginsV1>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionPreRankedKirErrorV1> {
        use fe2o3_kernel_analysis::{CanonicalKirCallEffectsV1, CanonicalKirInventoryV1};
        let mismatch = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
        budget.charge_work(4)?;
        let floor = budget.storage();
        budget.reserve_storage(std::mem::size_of::<Self>())?;
        budget.charge_work(argument_product_v1(
            subject.correspondence.lowered_functions.len(),
            2,
        )?)?;
        let source_helpers = subject
            .correspondence
            .lowered_functions
            .iter()
            .filter(|row| row.role == SemanticKirFunctionRoleV1::InternalHelper)
            .count();
        budget.charge_work(argument_product_v1(
            subject.executable.module().functions.len(),
            2,
        )?)?;
        let physical_helpers = subject
            .executable
            .module()
            .functions
            .iter()
            .filter(|function| function.role == fe2o3_kernel_ir::FunctionRole::InternalHelper)
            .count();
        budget.charge_work(2)?;
        if source_helpers == 0 && physical_helpers == 0 {
            return Ok(Self {
                functions: Vec::new(),
                associations: Vec::new(),
                allocations: Vec::new(),
                accesses: Vec::new(),
                control: Vec::new(),
                edge_bindings: Vec::new(),
                unit_source: SealedUnitLocalSourceV1::empty(),
                bf16_nominal: None,
                capture: HelperOccurrenceCaptureV1::Absent,
                storage: ProductionHelperMemoryStorageV1(std::mem::size_of::<Self>()),
                analysis_storage: 0,
            });
        }
        if source_helpers == 0 || physical_helpers == 0 {
            return Err(mismatch().into());
        }
        let mut functions =
            helper_memory_vec_v1(subject.executable.module().functions.len(), budget)?;
        let mut associations =
            helper_memory_vec_v1(subject.correspondence.lowered_functions.len(), budget)?;
        budget.charge_work(subject.executable.module().functions.len())?;
        functions.resize(
            subject.executable.module().functions.len(),
            RetainedHelperKindV1::NotHelper,
        );
        let (inventory, inventory_storage) =
            CanonicalKirInventoryV1::derive(subject.executable, budget)
                .map_err(canonical_call_inventory_error_v1)?;
        budget.reserve_storage(inventory_storage.retained_storage())?;
        let (effects, effect_storage) = CanonicalKirCallEffectsV1::derive(&inventory, budget)
            .map_err(helper_memory_effect_error_v1)?;
        budget.reserve_storage(effect_storage.retained_storage())?;

        budget.charge_work(2)?;
        let call_floor = budget.storage();
        let call_ledger = budget.work_ledger_identity_v1();
        let (groups, calls) = build_canonical_call_index_v1(subject, &inventory, budget)?;
        let call_live = budget.storage();
        let call_storage = call_live
            .checked_sub(call_floor)
            .ok_or(HelperMemoryResourceV1::Accounting)?;
        budget.charge_work(argument_product_v1(groups.len(), 7)?)?;
        for (index, group) in groups.iter().enumerate() {
            let source = subject
                .correspondence
                .lowered_functions
                .get(index)
                .ok_or_else(mismatch)?;
            let physical = group.function.canonical.coordinate.0 as usize;
            if !std::ptr::eq(source, group.function.source)
                || !std::ptr::eq(
                    subject
                        .executable
                        .module()
                        .functions
                        .get(physical)
                        .ok_or_else(mismatch)?,
                    group.function.canonical.function,
                )
                || associations.len() >= subject.correspondence.lowered_functions.len()
                || associations.len() == associations.capacity()
            {
                return Err(mismatch().into());
            }
            if source.role == SemanticKirFunctionRoleV1::InternalHelper {
                let state = functions.get_mut(physical).ok_or_else(mismatch)?;
                if *state == RetainedHelperKindV1::NotHelper {
                    *state = RetainedHelperKindV1::Pending;
                }
            }
            associations.push(RetainedHelperAssociationV1 { physical });
        }
        if associations.len() != subject.correspondence.lowered_functions.len() {
            return Err(mismatch().into());
        }
        let mut prepared_calls = Some((groups, calls));
        if origins.is_none() {
            if call_ledger != budget.work_ledger_identity_v1() || budget.storage() != call_live {
                return Err(HelperMemoryResourceV1::Accounting.into());
            }
            drop(prepared_calls.take());
            budget.release_storage(call_storage)?;
        }

        let (allocations, accesses, control, edge_bindings) =
            derive_retained_helper_physical_rows_v1(
                subject.executable,
                &inventory,
                &effects,
                &mut functions,
                budget,
            )?;
        let unit_source = if let Some(origins) = origins {
            let (groups, calls) = prepared_calls.as_ref().ok_or_else(mismatch)?;
            let source = check_unit_local_source_v1(
                subject,
                &inventory,
                origins,
                UnitLocalPhysicalRowsV1 {
                    functions: &functions,
                    allocations: &allocations,
                    accesses: &accesses,
                    control: &control,
                    edge_bindings: &edge_bindings,
                },
                groups,
                calls,
                budget,
            )?;
            if call_ledger != budget.work_ledger_identity_v1() || budget.storage() < call_live {
                return Err(HelperMemoryResourceV1::Accounting.into());
            }
            drop(prepared_calls.take());
            budget.release_storage(call_storage)?;
            source
        } else {
            SealedUnitLocalSourceV1::empty()
        };
        drop(effects);
        budget.release_storage(effect_storage.retained_storage())?;
        drop(inventory);
        budget.release_storage(inventory_storage.retained_storage())?;
        let storage = budget
            .storage()
            .checked_sub(floor)
            .ok_or(HelperMemoryResourceV1::Accounting)?;
        Ok(Self {
            functions,
            associations,
            allocations,
            accesses,
            control,
            edge_bindings,
            unit_source,
            bf16_nominal: None,
            capture: HelperOccurrenceCaptureV1::Absent,
            storage: ProductionHelperMemoryStorageV1(storage),
            analysis_storage: 0,
        })
    }
}

fn derive_retained_helper_physical_rows_v1(
    executable: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    inventory: &CanonicalKirInventoryV1<'_>,
    effects: &fe2o3_kernel_analysis::CanonicalKirCallEffectsV1<'_, '_>,
    functions: &mut [RetainedHelperKindV1],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<
    (
        Vec<RetainedLocalAllocationV1>,
        Vec<RetainedLocalAccessV1>,
        Vec<RetainedLocalControlV1>,
        Vec<RetainedLocalEdgeBindingV1>,
    ),
    ProductionPreRankedKirErrorV1,
> {
    use fe2o3_kernel_analysis::CanonicalKirCallEffectDecisionV1 as Decision;
    let mismatch = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
    let mut allocation_count = 0usize;
    let mut access_count = 0usize;
    let mut control_count = 0usize;
    let mut binding_count = 0usize;
    for (physical, state) in functions.iter_mut().enumerate() {
        budget.charge_work(4)?;
        if (executable.module().functions[physical].role
            == fe2o3_kernel_ir::FunctionRole::InternalHelper)
            != (*state == RetainedHelperKindV1::Pending)
        {
            return Err(mismatch().into());
        }
        if *state != RetainedHelperKindV1::Pending {
            continue;
        }
        let coordinate = inventory
            .functions()
            .get(physical)
            .ok_or_else(mismatch)?
            .coordinate;
        match effects
            .decision(coordinate, budget)
            .map_err(helper_memory_effect_error_v1)?
        {
            Decision::CompleteEmpty => *state = RetainedHelperKindV1::RawEmpty,
            Decision::Incomplete => return Err(mismatch().into()),
            Decision::CompleteNonempty => {
                budget.charge_work(6)?;
                let body = executable.module().functions[physical]
                    .body
                    .as_ref()
                    .ok_or_else(mismatch)?;
                let first_allocation = allocation_count;
                let first_access = access_count;
                let first_control = control_count;
                let first_binding = binding_count;
                for block in &body.blocks {
                    budget.charge_work(3)?;
                    control_count = control_count
                        .checked_add(1)
                        .ok_or(HelperMemoryResourceV1::Arithmetic)?;
                    binding_count = binding_count
                        .checked_add(block.parameters.len())
                        .ok_or(HelperMemoryResourceV1::Arithmetic)?;
                    for operation in &block.operations {
                        budget.charge_work(3)?;
                        match operation.kind {
                            OperationKind::Alloca { .. } => {
                                allocation_count = allocation_count
                                    .checked_add(1)
                                    .ok_or(HelperMemoryResourceV1::Arithmetic)?
                            }
                            OperationKind::Load { .. } | OperationKind::Store { .. } => {
                                access_count = access_count
                                    .checked_add(1)
                                    .ok_or(HelperMemoryResourceV1::Arithmetic)?
                            }
                            _ => {}
                        }
                    }
                }
                *state = RetainedHelperKindV1::Local {
                    allocations: (first_allocation, allocation_count),
                    accesses: (first_access, access_count),
                    control: (first_control, control_count),
                    edge_bindings: (first_binding, binding_count),
                };
            }
        }
    }
    // These output capacities remain live in every producer callback's floor.
    let mut allocations = helper_memory_vec_v1(allocation_count, budget)?;
    let mut accesses = helper_memory_vec_v1(access_count, budget)?;
    let mut control = helper_memory_vec_v1(control_count, budget)?;
    let mut edge_bindings = helper_memory_vec_v1(binding_count, budget)?;
    for (physical, state) in functions.iter().enumerate() {
        budget.charge_work(1)?;
        let RetainedHelperKindV1::Local {
            allocations: ar,
            accesses: mr,
            control: cr,
            edge_bindings: er,
        } = *state
        else {
            continue;
        };
        fe2o3_kernel_ir::with_checked_local_frame_chain_function_v1(
            executable.verified_module_ref_v1(),
            physical,
            budget,
            |checked, budget| {
                copy_retained_helper_rows_v1(
                    executable,
                    physical,
                    [ar, mr, cr, er],
                    &checked,
                    (
                        &mut allocations,
                        &mut accesses,
                        &mut control,
                        &mut edge_bindings,
                    ),
                    budget,
                )
            },
        )
        .map_err(ProductionPreRankedKirErrorV1::LocalFrame)?;
    }
    budget.charge_work(5)?;
    if allocations.len() != allocation_count
        || accesses.len() != access_count
        || control.len() != control_count
        || edge_bindings.len() != binding_count
    {
        return Err(mismatch().into());
    }
    Ok((allocations, accesses, control, edge_bindings))
}

fn copy_retained_helper_rows_v1(
    executable: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    physical: usize,
    [ar, mr, cr, er]: [(usize, usize); 4],
    checked: &fe2o3_kernel_ir::CheckedLocalFrameChainV1<'_, '_>,
    (allocations, accesses, control, edge_bindings): (
        &mut Vec<RetainedLocalAllocationV1>,
        &mut Vec<RetainedLocalAccessV1>,
        &mut Vec<RetainedLocalControlV1>,
        &mut Vec<RetainedLocalEdgeBindingV1>,
    ),
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), fe2o3_kernel_ir::LocalFrameErrorV1> {
    budget.charge_work(12)?;
    if !std::ptr::eq(checked.module(), executable.module())
        || checked.function_ordinal() != physical
        || !std::ptr::eq(checked.function(), &executable.module().functions[physical])
        || allocations.len() != ar.0
        || accesses.len() != mr.0
        || control.len() != cr.0
        || edge_bindings.len() != er.0
    {
        return Err(HelperMemoryResourceV1::Accounting.into());
    }
    let source_allocations = checked.allocations(budget)?;
    let source_accesses = checked.accesses(budget)?;
    let source_control = checked.control(budget)?;
    let source_bindings = checked.edge_bindings(budget)?;
    if Some(source_allocations.len()) != ar.1.checked_sub(ar.0)
        || Some(source_accesses.len()) != mr.1.checked_sub(mr.0)
        || Some(source_control.len()) != cr.1.checked_sub(cr.0)
        || Some(source_bindings.len()) != er.1.checked_sub(er.0)
    {
        return Err(HelperMemoryResourceV1::Accounting.into());
    }
    for row in source_allocations {
        budget.charge_work(3)?;
        if row.location().function_ordinal() != physical
            || allocations.len() == allocations.capacity()
        {
            return Err(HelperMemoryResourceV1::Accounting.into());
        }
        allocations.push(*row);
    }
    for row in source_accesses {
        budget.charge_work(4)?;
        if row.location().function_ordinal() != physical
            || row.allocation() >= source_allocations.len()
            || accesses.len() == accesses.capacity()
        {
            return Err(HelperMemoryResourceV1::Accounting.into());
        }
        accesses.push(*row);
    }
    for row in source_control {
        budget.charge_work(3)?;
        if row.function_ordinal() != physical || control.len() == control.capacity() {
            return Err(HelperMemoryResourceV1::Accounting.into());
        }
        control.push(*row);
    }
    for row in source_bindings {
        budget.charge_work(3)?;
        if row.function_ordinal() != physical || edge_bindings.len() == edge_bindings.capacity() {
            return Err(HelperMemoryResourceV1::Accounting.into());
        }
        edge_bindings.push(*row);
    }
    Ok(())
}

/// Borrowed exact-source local obligations, not purity, return-value semantics,
/// source equivalence, frame feasibility or artifact/launch authority.
///
/// The control and substitution slices cannot escape the same scoped view.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1;
/// use fe2o3_kernel_analysis::CanonicalKirInventoryV1;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn escape(owner: &ProductionPreRankedKirOwnerV1,
///     inventory: &CanonicalKirInventoryV1<'_>, budget: &mut Budget<'_>) {
///     let mut saved = None;
///     owner.with_checked_helper_memory_v1(inventory, budget, |view, budget| {
///         if let Some(local) = view.local_frame(1, budget)? {
///             saved = Some((local.control(), local.edge_bindings()));
///         }
///         Ok(())
///     }).unwrap();
///     drop(saved);
/// }
/// ```
pub struct ProductionHelperLocalFrameV1<'s> {
    source: &'s SemanticKirFunctionCorrespondenceV1,
    function: &'s Function,
    allocations: &'s [RetainedLocalAllocationV1],
    accesses: &'s [RetainedLocalAccessV1],
    control: &'s [RetainedLocalControlV1],
    edge_bindings: &'s [RetainedLocalEdgeBindingV1],
}

impl ProductionHelperLocalFrameV1<'_> {
    /// Exact root-qualified source association, not just a physical function ID.
    pub const fn source(&self) -> &SemanticKirFunctionCorrespondenceV1 {
        self.source
    }
    /// Actual function in the same retained verified executable.
    pub const fn function(&self) -> &Function {
        self.function
    }
    /// Full physical allocation census, prepaid when this scoped view is issued.
    pub const fn allocations(&self) -> &[RetainedLocalAllocationV1] {
        self.allocations
    }
    /// Full physical access census, prepaid when this scoped view is issued.
    pub const fn accesses(&self) -> &[RetainedLocalAccessV1] {
        self.accesses
    }
    /// Complete checked physical control census, including the inactive trap.
    /// These rows are not source control or optimized-output transport proofs.
    pub const fn control(&self) -> &[RetainedLocalControlV1] {
        self.control
    }
    /// Exact simultaneous substitutions of selected scalar edges, prepaid with
    /// this view. Keep these obligations with the control and memory rows.
    pub const fn edge_bindings(&self) -> &[RetainedLocalEdgeBindingV1] {
        self.edge_bindings
    }
}

/// Same-owner, same-ledger scoped access to retained helper obligations.
pub struct ProductionHelperMemoryV1<'s> {
    owner: &'s ProductionPreRankedKirOwnerV1,
    ledger: usize,
    work_ledger: ArgumentLedgerV1,
    floor: usize,
}

impl ProductionHelperMemoryV1<'_> {
    /// Complete source-association count, including root rows.
    pub fn association_count(&self) -> usize {
        self.owner.correspondence.lowered_functions.len()
    }

    /// Selects one exact association. Raw-empty helpers return None; root rows
    /// and invalid associations reject. None makes no value/termination claim.
    pub fn local_frame(
        &self,
        association: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<ProductionHelperLocalFrameV1<'_>>, ProductionSemanticKirErrorV1> {
        budget.charge_work(5)?;
        if self.ledger != budget as *const ArgumentBudgetV1<'_> as usize
            || self.work_ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.floor
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        self.owner.helper_memory_view_v1(association, budget)
    }
}

impl ProductionPreRankedKirOwnerV1 {
    fn helper_memory_view_v1(
        &self,
        association: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<ProductionHelperLocalFrameV1<'_>>, ProductionSemanticKirErrorV1> {
        let mismatch = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
        budget.charge_work(9)?;
        let rows = &self.helper_memory;
        let source = self
            .correspondence
            .lowered_functions
            .get(association)
            .ok_or_else(mismatch)?;
        if source.role != SemanticKirFunctionRoleV1::InternalHelper {
            return Err(mismatch());
        }
        let row = rows.associations.get(association).ok_or_else(mismatch)?;
        let function = self
            .executable
            .module()
            .functions
            .get(row.physical)
            .ok_or_else(mismatch)?;
        budget.charge_work(argument_sum_v1(&[
            function.id.as_str().len(),
            source.kernel_ir_function.as_str().len(),
        ])?)?;
        if function.id != source.kernel_ir_function {
            return Err(mismatch());
        }
        match rows.functions.get(row.physical).ok_or_else(mismatch)? {
            RetainedHelperKindV1::RawEmpty => Ok(None),
            RetainedHelperKindV1::Local {
                allocations,
                accesses,
                control,
                edge_bindings,
            } => {
                budget.charge_work(6)?;
                let allocations = rows
                    .allocations
                    .get(allocations.0..allocations.1)
                    .ok_or_else(mismatch)?;
                let accesses = rows
                    .accesses
                    .get(accesses.0..accesses.1)
                    .ok_or_else(mismatch)?;
                let control = rows
                    .control
                    .get(control.0..control.1)
                    .ok_or_else(mismatch)?;
                let edge_bindings = rows
                    .edge_bindings
                    .get(edge_bindings.0..edge_bindings.1)
                    .ok_or_else(mismatch)?;
                budget.charge_work(argument_sum_v1(&[
                    allocations.len(),
                    accesses.len(),
                    control.len(),
                    edge_bindings.len(),
                ])?)?;
                Ok(Some(ProductionHelperLocalFrameV1 {
                    source,
                    function,
                    allocations,
                    accesses,
                    control,
                    edge_bindings,
                }))
            }
            RetainedHelperKindV1::Bf16Nominal => Err(bf16_emission_refusal_v1(
                "BF16 generic call-local-frame consumer unavailable",
            )),
            RetainedHelperKindV1::NotHelper | RetainedHelperKindV1::Pending => Err(mismatch()),
        }
    }

    /// Borrows the retained roster without a new owner or ledger. Output must
    /// be pre-reserved; callback allocations are scratch and must be released.
    /// The scoped borrow cannot escape. Accounting overrides a body error or
    /// panic when the original Work ledger or live callback floor is not preserved.
    ///
    /// ```compile_fail
    /// use fe2o3_lower_mir_kernel::ProductionHelperMemoryV1;
    /// let forged = ProductionHelperMemoryV1 {};
    /// ```
    ///
    /// ```compile_fail
    /// use fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1;
    /// use fe2o3_kernel_analysis::CanonicalKirInventoryV1;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// fn escape(owner: &ProductionPreRankedKirOwnerV1,
    ///     inventory: &CanonicalKirInventoryV1<'_>, budget: &mut Budget<'_>) {
    ///     let mut saved = None;
    ///     owner.with_checked_helper_memory_v1(inventory, budget, |view, budget| {
    ///         saved = view.local_frame(1, budget)?;
    ///         Ok(())
    ///     }).unwrap();
    ///     drop(saved);
    /// }
    /// ```
    pub fn with_checked_helper_memory_v1<'w, R>(
        &self,
        inventory: &CanonicalKirInventoryV1<'_>,
        budget: &mut ArgumentBudgetV1<'w>,
        next: impl for<'s> FnOnce(
            &ProductionHelperMemoryV1<'s>,
            &mut ArgumentBudgetV1<'w>,
        ) -> Result<R, ProductionSemanticKirErrorV1>,
    ) -> Result<R, ProductionSemanticKirErrorV1> {
        if self.helper_source_policy_v1() == ProductionHelperSourcePolicyV1::Bf16Nominal {
            return Err(bf16_emission_refusal_v1(
                "BF16 generic helper-memory consumer unavailable",
            ));
        }
        with_canonical_call_scratch_v1(budget, |budget| {
            budget.charge_work(7)?;
            if !inventory.belongs_to(self.executable()) {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            if budget.storage() < self.retained_analysis_storage_v1() {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            let floor = budget.storage();
            let view = ProductionHelperMemoryV1 {
                owner: self,
                ledger: budget as *const ArgumentBudgetV1<'_> as usize,
                work_ledger: budget.work_ledger_identity_v1(),
                floor,
            };
            let result =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| next(&view, budget)));
            if view.work_ledger != budget.work_ledger_identity_v1() || budget.storage() != floor {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            match result {
                Ok(result) => result,
                Err(payload) => std::panic::resume_unwind(payload),
            }
        })
    }
}

#[cfg(test)]
#[path = "production_pre_ranked_local_frames_v1_tests.rs"]
mod retained_helper_memory_tests;
