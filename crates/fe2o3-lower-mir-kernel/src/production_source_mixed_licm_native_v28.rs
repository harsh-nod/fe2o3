//! Fresh final-owner native policies joined to the genuine source prefix.
use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirPrivateMemoryErrorV1 as PhysicalError,
    CanonicalKirPrivateMemoryLimitsV1 as PhysicalLimits, CanonicalRankedMetadataV18 as Metadata,
    CanonicalRankedViewErrorV1 as RankedError, CheckedCanonicalKirLicmV18 as Pair,
    CheckedCanonicalKirPrivateMemoryV18 as Physical, build_canonical_ranked_candidate_v18,
    check_canonical_kir_private_memory_v18, with_checked_canonical_ranked_view_v18,
};
use fe2o3_kernel_ir::{
    CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirOperationCoordinateV1 as Operation,
    CheckedCanonicalConditionalSliceDomainsV26 as Globals,
};
use fe2o3_pliron::{
    CanonicalRankedPolicyChecksErrorV1 as NativeChecksError,
    CanonicalRankedPolicyFailureV1 as NativeError, CanonicalRankedPolicyHistoryV1 as History,
    CanonicalRankedSourceRequirementV18 as Requirement,
    PendingCanonicalMixedMemoryPoliciesV26 as Native,
    with_pending_canonical_ranked_source_roles_v18,
};

/// Refusal while joining fresh final-native checks to a source-bound LICM tail.
#[derive(Debug)]
pub enum ProductionMixedLicmCompletionErrorV28 {
    /// Original source, relocation or retained custody replay was refused.
    Relocation(ProductionMixedLicmRelocationErrorV28),
    /// The final ranked candidate did not validate against its exact owner.
    Ranked(RankedError),
    /// Private memory classification or reaching-store analysis was refused.
    Physical(PhysicalError),
    /// A native or conditional-global query was refused.
    Native(NativeError),
    /// Native stage execution failed, retaining accepted resource/history data.
    NativeChecks(NativeChecksError),
}
type Error = ProductionMixedLicmCompletionErrorV28;
type Result<T> = std::result::Result<T, Error>;
impl From<ProductionMixedLicmRelocationErrorV28> for Error {
    fn from(error: ProductionMixedLicmRelocationErrorV28) -> Self {
        Self::Relocation(error)
    }
}
impl From<ProductionSourceOwnedViewErrorV18> for Error {
    fn from(error: ProductionSourceOwnedViewErrorV18) -> Self {
        Self::Relocation(error.into())
    }
}
impl From<ArgumentResourceV1> for Error {
    fn from(error: ArgumentResourceV1) -> Self {
        Self::Relocation(error.into())
    }
}
impl From<InventoryError> for Error {
    fn from(error: InventoryError) -> Self {
        Self::Relocation(error.into())
    }
}
impl From<MotionError> for Error {
    fn from(error: MotionError) -> Self {
        Self::Relocation(error.into())
    }
}
impl From<coordinates::Error> for Error {
    fn from(error: coordinates::Error) -> Self {
        Self::Relocation(error.into())
    }
}
impl From<RankedError> for Error {
    fn from(error: RankedError) -> Self {
        Self::Ranked(error)
    }
}
impl From<PhysicalError> for Error {
    fn from(error: PhysicalError) -> Self {
        Self::Physical(error)
    }
}
impl From<NativeError> for Error {
    fn from(error: NativeError) -> Self {
        Self::Native(error)
    }
}
impl From<NativeChecksError> for Error {
    fn from(error: NativeChecksError) -> Self {
        Self::NativeChecks(error)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "mixed LICM final native completion: {self:?}")
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Relocation(error) => Some(error),
            Self::Ranked(error) => Some(error),
            Self::Physical(error) => Some(error),
            Self::Native(error) => Some(error),
            Self::NativeChecks(error) => Some(error),
        }
    }
}
fn mismatch(message: &'static str) -> Error {
    ProductionMixedLicmRelocationErrorV28::Binding(message).into()
}

// The enclosing source attempt owns both the prepaid payload and any allocator
// excess until these vectors are dropped or moved into the retained handoff.
fn vector<T>(
    count: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> std::result::Result<Vec<T>, ArgumentResourceV1> {
    budget.charge_work(1)?;
    budget.reserve_storage(argument_product_v1(count, size_of::<T>())?)?;
    let rows = argument_vec_v1::<T>(count)?;
    budget.reserve_storage(argument_product_v1(
        rows.capacity()
            .checked_sub(count)
            .ok_or(ArgumentResourceV1::Accounting)?,
        size_of::<T>(),
    )?)?;
    Ok(rows)
}

/// Exact original occurrence plus independently checked final coordinates.
/// Copies are inert: source and conditional native authority stay in the owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionMixedLicmRuntimeOccurrenceV28 {
    prefix: ProductionMixedRuntimeOccurrenceV26,
    operation: Operation,
    formation: Operation,
    address_index: Definition,
    condition: Definition,
}
impl ProductionMixedLicmRuntimeOccurrenceV28 {
    /// Index into the retained complete original slice-premise roster.
    pub const fn premise_index(&self) -> usize {
        self.prefix.premise_index()
    }
    /// Original semantic call-instance ordinal owning the memory occurrence.
    pub const fn original_instance(&self) -> usize {
        self.prefix.original_instance()
    }
    /// Memory-access coordinate before the checked prefix and LICM tail.
    pub const fn original_operation(&self) -> Operation {
        self.prefix.original_operation()
    }
    /// Independently relocated memory-access coordinate in the final graph.
    pub const fn output_operation(&self) -> Operation {
        self.operation
    }
    /// Address-formation coordinate in the original source-bound graph.
    pub const fn original_address_formation(&self) -> Operation {
        self.prefix.original_address_formation()
    }
    /// Independently relocated address-formation coordinate in the final graph.
    pub const fn output_address_formation(&self) -> Operation {
        self.formation
    }
    /// Final definition supplying this occurrence's address index.
    pub const fn output_address_index(&self) -> Definition {
        self.address_index
    }
    /// Final definition supplying the retained access guard's condition.
    pub const fn output_guard_condition(&self) -> Definition {
        self.condition
    }
    /// Access guard edge, preserved because this LICM tail does not change CFG.
    pub const fn output_guard_edge(&self) -> fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1 {
        self.prefix.output_guard_edge()
    }
    /// Conditional index domain rechecked on the actual final occurrence.
    pub const fn domain(&self) -> fe2o3_kernel_ir::CanonicalConditionalSliceDomainV26 {
        self.prefix.domain()
    }
    /// Retained invocation projection used by the conditional access domain.
    pub const fn invocation_projection(&self) -> Option<(Axis, ValueId)> {
        self.prefix.invocation_projection()
    }
    /// Exact memory-access representation preserved across both transformations.
    pub const fn memory_access(&self) -> MemoryAccess {
        self.prefix.memory_access()
    }
    /// Always true: address formation needs its own domain, not just the access guard.
    pub const fn requires_address_formation_domain(&self) -> bool {
        true
    }
    /// Always false: copied occurrence coordinates do not confer authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

/// Complete conditional source/native conjunction for the actual LICM output.
/// Concrete runtime bindings and executed refinement remain separate gates.
#[must_use = "discard this native owner's exact credit before its borrowed relocation"]
pub struct ProductionConditionalMixedLicmOutputHandoffV28<'native, 'prefix, 'view, 'source,
    P: ProductionMixedPrefixOwnerV29<'view, 'source> = ProductionConditionalMixedPureCseOutputHandoffV26<'view, 'source>> {
    relocation: &'native ProductionMixedLicmRelocationV28<'prefix, 'view, 'source, P>,
    occurrences: Vec<ProductionMixedLicmRuntimeOccurrenceV28>,
    histories: Vec<Option<History>>,
    retained: usize,
    required: usize,
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
}
/// Final native completion retaining the exact Policy11 prefix and LICM owner.
pub type ProductionConditionalMixedFixedpointLicmOutputHandoffV29<
    'native,
    'prefix,
    'view,
    'source,
> = ProductionConditionalMixedLicmOutputHandoffV28<
    'native,
    'prefix,
    'view,
    'source,
    ProductionConditionalMixedFixedpointOutputHandoffV29<'view, 'source>,
>;

impl<'native, 'prefix, 'view, 'source, P: ProductionMixedPrefixOwnerV29<'view, 'source>>
    ProductionConditionalMixedLicmOutputHandoffV28<'native, 'prefix, 'view, 'source, P>
{
    fn custody(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        let source = self.relocation.prefix.source_owned_v29();
        if self.slot != std::ptr::from_ref(budget) as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.required
        {
            source.cleanup.deny_refund();
            return source.retain_query(Err(ArgumentResourceV1::Accounting.into()));
        }
        self.relocation.custody(budget)
    }
    fn check(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        let custody = self.custody(budget);
        self.relocation.check(budget).and(custody)
    }
    /// Borrows the genuine source-prefix and LICM owner retained by this handoff.
    pub fn relocation(
        &self,
        budget: &ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&'native ProductionMixedLicmRelocationV28<'prefix, 'view, 'source, P>>
    {
        self.check(budget)?;
        Ok(self.relocation)
    }
    /// Borrows the exact final graph on which native completion was performed.
    pub fn output(
        &self,
        budget: &ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18> {
        self.check(budget)?;
        Ok(self.relocation.tail.output())
    }
    /// Rejoins this handoff to the exact original semantic SSA owner.
    pub fn check_original_source(
        &self,
        source: &ProductionSemanticSsaOwnerV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        self.check(budget)?;
        self.relocation.check_original_source(source, budget)
    }
    /// Checks the complete proposed ABI roster against the original source ABI.
    pub fn check_original_argument_abi_v26(
        &self,
        abi: ProductionKernelArgumentAbiInputV18<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        self.check(budget)?;
        self.relocation
            .prefix
            .check_original_argument_abi_v26(abi, budget)
    }
    /// Borrows all original slice premises, including parameters with no access.
    pub fn runtime_premises(
        &self,
        budget: &ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&[ProductionMixedSliceRuntimePremiseV26]> {
        self.check(budget)?;
        self.relocation.prefix.runtime_premises(budget)
    }
    /// Borrows complete source occurrences with separately checked final coordinates.
    pub fn runtime_occurrences(
        &self,
        budget: &ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&[ProductionMixedLicmRuntimeOccurrenceV28]> {
        self.check(budget)?;
        Ok(&self.occurrences)
    }
    /// Borrows the exact launch extents and index width used for conditional checks.
    pub fn launch_context(
        &self,
        budget: &ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<(
        &[fe2o3_kernel_ir::ExplicitLaunchExtent],
        fe2o3_kernel_ir::FormalIndexWidth,
    )> {
        self.check(budget)?;
        self.relocation.prefix.launch_context(budget)
    }
    /// Returns nine-stage final-native histories; declaration-only functions have none.
    pub fn native_histories(
        &self,
        budget: &ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&[Option<History>]> {
        self.check(budget)?;
        Ok(&self.histories)
    }
    /// Returns this handoff's credit, excluding its borrowed source/relocation owners.
    pub fn retained_storage(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<usize> {
        self.check(budget)?;
        Ok(self.retained)
    }
    /// Checks same-ledger custody and the larger of the supplied and retained floors.
    /// Any observed floor loss permanently denies source-linked refunds.
    pub fn observe_retained_storage_v28(
        &self,
        required: usize,
        budget: &ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        let source = self.relocation.prefix.source_owned_v29();
        if budget.storage() < required.max(self.required) {
            source.cleanup.deny_refund();
            return source.retain_query(Err(ArgumentResourceV1::Accounting.into()));
        }
        self.custody(budget)
    }
    /// Drops this handoff's payload before refunding only its intact retained credit.
    /// The borrowed relocation and source prefix remain owned by their caller.
    pub fn discard(self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        let selected = self.check(budget);
        let custody = self.custody(budget);
        let Self {
            relocation,
            occurrences,
            histories,
            retained,
            ..
        } = self;
        drop((occurrences, histories));
        let settled = custody.and_then(|()| {
            relocation
                .prefix
                .source_owned_v29()
                .retain_query(budget.release_storage(retained).map_err(Into::into))
        });
        selected?;
        settled
    }
    /// Reports the complete source-role conjunction checked during construction.
    pub const fn source_roles_are_complete(&self) -> bool {
        true
    }
    /// Reports completion of fresh conditional native checks on the final graph.
    pub const fn final_native_completion_is_complete(&self) -> bool {
        true
    }
    /// Always false: concrete allocation, alias and launch premises remain open.
    pub const fn runtime_requirements_are_discharged(&self) -> bool {
        false
    }
    /// Always false: conditional native completion is not unconditional final admission.
    pub const fn ranked_verification_is_complete(&self) -> bool {
        false
    }
    /// Always false: final-native completion grants neither artifact nor launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

fn replay_physical(
    pair: &Pair<'_>,
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    before: &Physical<'_, '_>,
    after: &Physical<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<()> {
    budget.charge_work(6)?;
    if !before.is_for(input)
        || !after.is_for(output)
        || before.latest_stores().len() != input.operations().len()
        || after.latest_stores().len() != output.operations().len()
    {
        return Err(mismatch("LICM physical report owner or complete census"));
    }
    for (index, definition) in input.definitions().iter().enumerate() {
        budget.charge_work(12)?;
        let target = coordinates::expected_definition(pair, input, definition.coordinate, budget)?;
        let target = coordinates::definition_index(output, target)?;
        match (before.address(index), after.address(target)) {
            (None, None) => (),
            (Some(a), Some(b)) => {
                let allocation = input
                    .operations()
                    .get(a.allocation())
                    .ok_or_else(|| mismatch("LICM private allocation index"))?
                    .coordinate;
                let allocation = coordinates::operation(pair, input, allocation, budget)?;
                if coordinates::operation_index(output, allocation)? != b.allocation()
                    || a.start() != b.start()
                    || a.length() != b.length()
                    || a.offset() != b.offset()
                    || a.alignment() != b.alignment()
                    || a.stride() != b.stride()
                {
                    return Err(mismatch("LICM private address identity changed"));
                }
            }
            _ => return Err(mismatch("LICM private definition classification changed")),
        }
    }
    for (index, row) in pair.origins().iter().enumerate() {
        budget.charge_work(7)?;
        let target = coordinates::operation_index(output, row.output)?;
        if before.operation(index) != after.operation(target)
            || (before.operation(index) && row.hoist.is_some())
        {
            return Err(mismatch("LICM private occurrence classification changed"));
        }
        let expected = before.latest_stores()[index]
            .map(|at| {
                let coordinate = input
                    .operations()
                    .get(at)
                    .ok_or_else(|| mismatch("LICM reaching-store index"))?
                    .coordinate;
                coordinates::operation_index(
                    output,
                    coordinates::operation(pair, input, coordinate, budget)?,
                )
                .map_err(Error::from)
            })
            .transpose()?;
        if expected != after.latest_stores()[target] {
            return Err(mismatch("LICM private reaching store changed"));
        }
    }
    Ok(())
}

fn with_pending(
    inventory: &Inventory<'_>,
    layouts: fe2o3_kernel_ir::StorageLayoutLimitsV1,
    budget: &mut ArgumentBudgetV1<'_>,
    consume: impl FnOnce(
        &mut fe2o3_pliron::PendingCanonicalRankedSourceRolesV18<'_, '_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<()>,
) -> Result<()> {
    let metadata = Metadata::new(inventory.owner(), &[]);
    let metadata_storage = metadata.storage_extent(budget)?;
    budget.reserve_storage(metadata_storage)?;
    let (candidate, receipt) = build_canonical_ranked_candidate_v18(inventory, &metadata, budget)?;
    budget.reserve_storage(receipt.retained_storage())?;
    let mut selected = None;
    let result = with_checked_canonical_ranked_view_v18(
        inventory,
        &metadata,
        &candidate,
        budget,
        |checked, budget| {
            Ok::<_, RankedError>(with_pending_canonical_ranked_source_roles_v18(
                checked,
                layouts,
                budget,
                |pending, budget| {
                    selected = Some(consume(pending, budget));
                    Ok(())
                },
            ))
        },
    );
    drop(candidate);
    drop(metadata);
    let released = budget.release_storage(argument_sum_v1(&[
        metadata_storage,
        receipt.retained_storage(),
    ])?);
    let called = selected.is_some();
    if let Some(Err(error)) = selected {
        return Err(error);
    }
    result??;
    released?;
    if !called {
        return Err(mismatch("LICM pending source-role callback absent"));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn join_final<'view, 'source, P: ProductionMixedPrefixOwnerV29<'view, 'source>>(
    relocated: &ProductionMixedLicmRelocationV28<'_, 'view, 'source, P>,
    output: &Inventory<'_>,
    native: &Native<'_, '_>,
    globals: &Globals<'_, '_>,
    source_roles: &[Option<Requirement>],
    role_count: usize,
    parameter_premises: &[Option<usize>],
    occurrences: &[ProductionMixedLicmRuntimeOccurrenceV28],
    seen: &mut [bool],
    histories: &mut Vec<Option<History>>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<()> {
    relocated.check(budget)?;
    budget.charge_work(9)?;
    if !std::ptr::eq(native.owner(budget)?, relocated.tail.output())
        || !std::ptr::eq(
            globals
                .owner(budget)
                .map_err(NativeError::ConditionalGlobalsV26)?,
            relocated.tail.output(),
        )
        || native.function_count(budget)? != output.functions().len()
        || globals
            .function_count(budget)
            .map_err(NativeError::ConditionalGlobalsV26)?
            != output.functions().len()
        || source_roles.len() != output.operations().len()
        || seen.len() != output.operations().len()
        || parameter_premises.len() != output.definitions().len()
        || !histories.is_empty()
        || histories.capacity() < output.functions().len()
    {
        return Err(mismatch(
            "LICM final native owner or complete function census",
        ));
    }
    let obligations = native.obligations(budget)?;
    if obligations.len() != role_count {
        return Err(mismatch("LICM final source obligation count"));
    }
    for obligation in obligations {
        budget.charge_work(5)?;
        let ordinal = coordinates::operation_index(output, obligation.coordinate())?;
        if seen[ordinal] || source_roles[ordinal] != Some(obligation.requirement()) {
            return Err(mismatch(
                "LICM final source obligation changed or duplicated",
            ));
        }
        seen[ordinal] = true;
    }
    for (ordinal, role) in source_roles.iter().enumerate() {
        budget.charge_work(2)?;
        if seen[ordinal] != role.is_some() {
            return Err(mismatch("LICM final source obligation omitted"));
        }
    }
    budget.charge_work(seen.len())?;
    seen.fill(false);
    for function in output.functions() {
        budget.charge_work(4)?;
        let ordinal = function.coordinate.0 as usize;
        let report = native.report(ordinal, budget)?;
        let history = native.history(ordinal, budget)?;
        match (function.function.body.is_some(), report, history) {
            (true, Some(report), Some(_))
                if report.reports().is_clean() && report.paired_stage_count() == 9 =>
            {
                ()
            }
            (false, None, None) => (),
            _ => {
                return Err(mismatch(
                    "LICM final function lacks all nine clean native stages",
                ));
            }
        }
        histories.push(history);
        for parameter in 0..function.function.signature.parameters.len() {
            budget.charge_work(8)?;
            let coordinate = Definition::FunctionArgument {
                function: function.coordinate,
                argument: u32::try_from(parameter).map_err(|_| ArgumentResourceV1::Arithmetic)?,
            };
            let definition = coordinates::definition_index(output, coordinate)?;
            let native = globals
                .parameter(function.coordinate, parameter as u32, budget)
                .map_err(NativeError::ConditionalGlobalsV26)?;
            match (parameter_premises[definition], native) {
                (None, None) => (),
                (Some(index), Some(native)) => {
                    let premise = relocated
                        .prefix
                        .runtime_premises(budget)?
                        .get(index)
                        .ok_or_else(|| mismatch("LICM parameter premise index"))?;
                    if premise.parameter() != coordinate
                        || native.scalar() != premise.scalar()
                        || [native.reads(), native.writes()] != premise.access_counts()
                        || native.invocation_axis() != premise.invocation_axis()
                        || native.requires_valid_aligned_extent()
                            != premise.requires_valid_aligned_extent()
                        || native.requires_initialized_extent()
                            != premise.requires_initialized_extent()
                        || native.requires_exclusive_runtime_binding()
                            != premise.requires_exclusive_nonoverlapping_runtime_binding()
                        || native.requires_exact_launch_binding()
                            != premise.requires_exact_launch_binding()
                        || output.definitions()[definition].value != Some(native.value())
                    {
                        return Err(mismatch("LICM complete original slice premise changed"));
                    }
                    let conditions = globals
                        .function_conditions(function.coordinate, budget)
                        .map_err(NativeError::ConditionalGlobalsV26)?
                        .ok_or_else(|| mismatch("LICM root conditions absent"))?;
                    if conditions.0 != premise.launch() || conditions.1 != premise.index_width() {
                        return Err(mismatch("LICM source launch or index width changed"));
                    }
                }
                _ => {
                    return Err(mismatch(
                        "LICM native parameter omitted or invented a source premise",
                    ));
                }
            }
        }
    }
    if globals
        .access_count(budget)
        .map_err(NativeError::ConditionalGlobalsV26)?
        != occurrences.len()
    {
        return Err(mismatch("LICM final conditional global access census"));
    }
    for occurrence in occurrences {
        budget.charge_work(9)?;
        let ordinal = coordinates::operation_index(output, occurrence.operation)?;
        let actual = globals
            .access_at(occurrence.operation, budget)
            .map_err(NativeError::ConditionalGlobalsV26)?
            .ok_or_else(|| mismatch("LICM final conditional access absent"))?;
        if seen[ordinal]
            || actual.domain() != occurrence.domain()
            || actual.invocation_projection() != occurrence.invocation_projection()
        {
            return Err(mismatch(
                "LICM final conditional access changed or duplicated",
            ));
        }
        seen[ordinal] = true;
        let operation = output.operations()[ordinal].operation;
        let access = match operation.kind {
            OperationKind::Load { access, .. } | OperationKind::Store { access, .. } => access,
            _ => {
                return Err(mismatch(
                    "LICM final global occurrence changed its operation family",
                ));
            }
        };
        if access != occurrence.memory_access() {
            return Err(mismatch("LICM final access representation changed"));
        }
    }
    relocated.check(budget)?;
    Ok(())
}

impl<'prefix, 'view, 'source, P: ProductionMixedPrefixOwnerV29<'view, 'source>>
    ProductionMixedLicmRelocationV28<'prefix, 'view, 'source, P>
{
    /// Replays actual relocation/private memory, then joins fresh final-native
    /// checks with the complete original source roles and runtime premise roster.
    /// The result remains conditional and borrows this exact relocation owner.
    pub fn complete_native_v28<'native>(
        &'native self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<ProductionConditionalMixedLicmOutputHandoffV28<'native, 'prefix, 'view, 'source, P>>
    {
        self.complete_native_inner_v28(
            budget,
            #[cfg(test)]
            None,
        )
    }

    #[cfg(test)]
    pub(crate) fn complete_native_fault_v28<'native>(
        &'native self,
        fault: u8,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<ProductionConditionalMixedLicmOutputHandoffV28<'native, 'prefix, 'view, 'source, P>>
    {
        self.complete_native_inner_v28(budget, Some(fault))
    }

    fn complete_native_inner_v28<'native>(
        &'native self,
        budget: &mut ArgumentBudgetV1<'_>,
        #[cfg(test)] fault: Option<u8>,
    ) -> Result<ProductionConditionalMixedLicmOutputHandoffV28<'native, 'prefix, 'view, 'source, P>>
    {
        self.check(budget)?;
        let source = self.prefix.source_owned_v29();
        let floor = budget.storage();
        let (occurrences, histories, retained) =
            scoped_source_attempt_v29(source.cleanup, budget, floor, |budget| -> Result<_> {
                let entry = budget.storage();
                type Handoff<'a, 'v, 's, P> =
                    ProductionConditionalMixedLicmOutputHandoffV28<'a, 'a, 'v, 's, P>;
                type Vectors = (
                    Vec<Option<Requirement>>,
                    Vec<Option<usize>>,
                    Vec<bool>,
                    Vec<bool>,
                    Vec<fe2o3_kernel_ir::ExplicitLaunchExtent>,
                );
                let owner_header = argument_sum_v1(&[
                    size_of::<Handoff<'_, 'view, 'source, P>>(),
                    align_of::<Handoff<'_, 'view, 'source, P>>(),
                ])?;
                let scratch_header = argument_sum_v1(&[
                    ProductionCheckedMixedPrefixViewV29::inspection_storage_v29()?,
                    2 * size_of::<Inventory<'_>>(),
                    2 * size_of::<Physical<'_, '_>>(),
                    size_of::<Pair<'_>>(),
                    size_of::<Metadata<'_, '_>>(),
                    size_of::<Vectors>(),
                    12 * size_of::<Result<()>>(),
                    4 * size_of::<Option<Result<()>>>(),
                    size_of::<[usize; 64]>(),
                    size_of::<[&(); 64]>(),
                    size_of::<fe2o3_kernel_ir::StorageLayoutLimitsV1>(),
                ])?;
                budget.reserve_storage(argument_sum_v1(&[owner_header, scratch_header])?)?;
                self.replay(budget)?;
                let prefix = self.prefix.checked_prefix_v29(budget)?.owner();
                let layouts = source.limits(budget)?.storage_layout_limits();
                let (pair, ps) = self.tail.replay_against(prefix, budget)?;
                budget.reserve_storage(ps.retained_storage())?;
                let (input, is) = Inventory::derive_v18(prefix, budget)?;
                budget.reserve_storage(is.retained_storage())?;
                let (output, os) = Inventory::derive_v18(self.tail.output(), budget)?;
                budget.reserve_storage(os.retained_storage())?;
                self.projection.replay(&pair, &input, &output, budget)?;
                let (before, bs) = check_canonical_kir_private_memory_v18(
                    &input,
                    PhysicalLimits {
                        max_cells: input.definitions().len(),
                    },
                    budget,
                )?;
                budget.reserve_storage(bs.retained_storage())?;
                let (after, as_) = check_canonical_kir_private_memory_v18(
                    &output,
                    PhysicalLimits {
                        max_cells: output.definitions().len(),
                    },
                    budget,
                )?;
                budget.reserve_storage(as_.retained_storage())?;
                replay_physical(&pair, &input, &output, &before, &after, budget)?;

                let scratch_start = budget.storage();
                let mut roles = vector(output.operations().len(), budget)?;
                let mut parameters = vector(output.definitions().len(), budget)?;
                let mut seen = vector(output.operations().len(), budget)?;
                let mut roots_seen = vector(output.functions().len(), budget)?;
                let mut launches = vector(output.functions().len(), budget)?;
                budget.charge_work(argument_sum_v1(&[
                    output.operations().len(),
                    output.operations().len(),
                    output.definitions().len(),
                    output.functions().len(),
                    output.functions().len(),
                ])?)?;
                roles.resize(output.operations().len(), None);
                parameters.resize(output.definitions().len(), None);
                seen.resize(output.operations().len(), false);
                roots_seen.resize(output.functions().len(), false);
                launches.resize(
                    output.functions().len(),
                    fe2o3_kernel_ir::ExplicitLaunchExtent::Exact {
                        rank: 1,
                        extents: [1, 1, 1],
                    },
                );
                let scratch_vectors = budget
                    .storage()
                    .checked_sub(scratch_start)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                let (root_launches, width) = self.prefix.launch_context(budget)?;
                if root_launches.len() != source.root_count(budget)? {
                    return Err(mismatch("LICM source launch roster differs"));
                }
                for (root, launch) in root_launches.iter().enumerate() {
                    budget.charge_work(6)?;
                    let ordinal = source.root(root, budget)?.1;
                    let original = source
                        .canonical(budget)?
                        .module()
                        .functions
                        .get(ordinal)
                        .ok_or_else(|| mismatch("LICM original root ordinal"))?;
                    let final_root = output
                        .function_for_name(original.id.as_str(), budget)?
                        .ok_or_else(|| mismatch("LICM final original root absent"))?;
                    let ordinal = final_root.coordinate.0 as usize;
                    if roots_seen[ordinal] {
                        return Err(mismatch("LICM repeated root mapping"));
                    }
                    roots_seen[ordinal] = true;
                    launches[ordinal] = *launch;
                }
                for (index, premise) in self.prefix.runtime_premises(budget)?.iter().enumerate() {
                    budget.charge_work(6)?;
                    let definition = coordinates::definition_index(&output, premise.parameter())?;
                    if parameters[definition].replace(index).is_some() {
                        return Err(mismatch("LICM repeated original slice premise"));
                    }
                }
                let mut role_count = 0usize;
                with_pending(&input, layouts, budget, |pending, budget| {
                    for role in pending.obligations(budget)? {
                        budget.charge_work(6)?;
                        let at = coordinates::operation(&pair, &input, role.coordinate(), budget)?;
                        let index = coordinates::operation_index(&output, at)?;
                        if roles[index].replace(role.requirement()).is_some() {
                            return Err(mismatch("LICM repeated prefix source role"));
                        }
                        role_count = role_count
                            .checked_add(1)
                            .ok_or(ArgumentResourceV1::Arithmetic)?;
                    }
                    Ok(())
                })?;

                let payload_start = budget.storage();
                let source_occurrences = self.prefix.runtime_occurrences(budget)?;
                let mut occurrences = vector(source_occurrences.len(), budget)?;
                let mut histories = vector(output.functions().len(), budget)?;
                let payload = budget
                    .storage()
                    .checked_sub(payload_start)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                for occurrence in source_occurrences {
                    budget.charge_work(5)?;
                    occurrences.push(ProductionMixedLicmRuntimeOccurrenceV28 {
                        prefix: *occurrence,
                        operation: coordinates::operation(
                            &pair,
                            &input,
                            occurrence.output_operation(),
                            budget,
                        )?,
                        formation: coordinates::operation(
                            &pair,
                            &input,
                            occurrence.output_address_formation(),
                            budget,
                        )?,
                        address_index: coordinates::expected_definition(
                            &pair,
                            &input,
                            occurrence.output_address_index(),
                            budget,
                        )?,
                        condition: coordinates::expected_definition(
                            &pair,
                            &input,
                            occurrence.output_guard_condition(),
                            budget,
                        )?,
                    });
                }
                #[cfg(test)]
                if let Some(fault) = fault {
                    match fault {
                        1 => *roles.iter_mut().find(|row| row.is_some()).unwrap() = None,
                        2 => {
                            let role = roles.iter_mut().find(|row| row.is_some()).unwrap();
                            *role = Some(if *role == Some(Requirement::Call) {
                                Requirement::Memory
                            } else {
                                Requirement::Call
                            });
                        }
                        3 => {
                            assert!(occurrences.len() > 1);
                            occurrences[1] = occurrences[0];
                        }
                        4 => occurrences[0].operation = occurrences[0].formation,
                        5 => *parameters.iter_mut().find(|row| row.is_some()).unwrap() = None,
                        6 => {
                            *parameters.iter_mut().find(|row| row.is_some()).unwrap() =
                                Some(self.prefix.runtime_premises(budget)?.len())
                        }
                        7 => {
                            occurrences.pop();
                        }
                        8 => {
                            seen.pop();
                        }
                        9 => {
                            roles.pop();
                        }
                        10 => role_count -= 1,
                        _ => panic!("unknown final native join fault"),
                    }
                }
                with_pending(&output, layouts, budget, |pending, budget| {
                    let mut selected = None;
                    let mut native_result = None;
                    let family = fe2o3_kernel_ir::with_canonical_guarded_global_reads_v18(
                        self.tail.output(),
                        Default::default(),
                        budget,
                        |reads, budget| {
                            fe2o3_kernel_ir::with_canonical_guarded_global_stores_v24(
                                self.tail.output(),
                                Default::default(),
                                budget,
                                |stores, budget| {
                                    fe2o3_kernel_ir::with_canonical_conditional_slice_domains_v26(
                                        reads,
                                        stores,
                                        &launches,
                                        width,
                                        budget,
                                        |globals, budget| {
                                            native_result =
                                                Some(pending.with_mixed_memory_observations_v26(
                                                    &after,
                                                    globals,
                                                    budget,
                                                    |native, budget| {
                                                        selected = Some(join_final(
                                                            self,
                                                            &output,
                                                            native,
                                                            globals,
                                                            &roles,
                                                            role_count,
                                                            &parameters,
                                                            &occurrences,
                                                            &mut seen,
                                                            &mut histories,
                                                            budget,
                                                        ));
                                                        Ok(())
                                                    },
                                                ));
                                            Ok(())
                                        },
                                    )
                                },
                            )
                        },
                    );
                    let called = selected.is_some();
                    if let Some(Err(error)) = selected {
                        return Err(error);
                    }
                    if family
                        .map_err(NativeError::ConditionalGlobalsV26)?
                        .is_none()
                    {
                        return Err(mismatch(
                            "LICM final conditional global family is incomplete",
                        ));
                    }
                    native_result
                        .ok_or_else(|| mismatch("LICM final native callback absent"))??;
                    if !called {
                        return Err(mismatch("LICM final source conjunction absent"));
                    }
                    Ok(())
                })?;
                self.check(budget)?;
                drop((roles, parameters, seen, roots_seen, launches));
                drop(before);
                drop(after);
                drop(input);
                drop(output);
                drop(pair);
                budget.release_storage(argument_sum_v1(&[
                    scratch_header,
                    scratch_vectors,
                    ps.retained_storage(),
                    is.retained_storage(),
                    os.retained_storage(),
                    bs.retained_storage(),
                    as_.retained_storage(),
                ])?)?;
                let retained = argument_sum_v1(&[owner_header, payload])?;
                if entry.checked_add(retained) != Some(budget.storage()) {
                    source.cleanup.deny_refund();
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                Ok((occurrences, histories, retained))
            })?;
        Ok(ProductionConditionalMixedLicmOutputHandoffV28 {
            relocation: self,
            occurrences,
            histories,
            retained,
            required: budget.storage(),
            slot: std::ptr::from_ref(budget) as usize,
            ledger: budget.work_ledger_identity_v1(),
        })
    }
}
