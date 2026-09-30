//! Native mixed-memory admission retains, but does not discharge, slice premises.
use super::*;
use crate::native_conditional_domains_v30::NativeConditionalDomainsV30 as Domains;
use fe2o3_kernel_ir::{
    CanonicalGuardedGlobalReadErrorV1, CheckedCanonicalConditionalSliceDomainsV26 as Globals,
    IndexKind, IntrinsicKind,
};

/// A closed adapter for the exact imported owner and current native epoch.
/// Global rows are a separate conditional family, never private memory facts.
pub(crate) struct NativeCanonicalMixedAdmissionV26<'a> {
    private: NativeCanonicalPrivateAdmissionV18<'a>,
    globals: Domains<'a>,
    kinds: &'a [Option<Kind>],
    counts: [usize; 2],
}

impl NativeCanonicalMixedAdmissionV26<'_> {
    pub(crate) fn global_counts_v26(&self) -> [usize; 2] {
        self.counts
    }

    fn extra_kind(&self, index: usize) -> Option<Kind> {
        self.kinds.get(index).copied().flatten()
    }

    fn is_trap_end(&self, coordinate: KirBridgeCoordinateV1) -> bool {
        let KirBridgeCoordinateV1::Terminator { function, block } = coordinate else {
            return false;
        };
        if function as usize != self.ordinal() {
            return false;
        }
        let Some(function) = self.private.inventory().functions().get(function as usize) else {
            return false;
        };
        let Some(index) = function.blocks.start.checked_add(block as usize) else {
            return false;
        };
        let Some(row) = self.private.inventory().blocks().get(index) else {
            return false;
        };
        index < function.blocks.end
            && row.coordinate.block == block
            && matches!(row.terminator, Terminator::Unreachable)
            && row.operations.end.checked_sub(1).is_some_and(|last| {
                last >= row.operations.start && self.extra_kind(last) == Some(Kind::TrapCall)
            })
    }

    fn check_trap(&self, pointer: Ptr<Operation>, index: usize) -> Result<(), Failure> {
        let context = self.context();
        let expected = self.private.inventory().operations()[index].operation;
        let OperationKind::Call { callee, arguments } = &expected.kind else {
            return Err(Failure::ExactGraph);
        };
        let native = Operation::get_op::<CallOp>(pointer, context).ok_or(Failure::NativeSchema)?;
        let raw = pointer.deref(context);
        if !arguments.is_empty()
            || !expected.results.is_empty()
            || raw.num_regions() != 0
            || raw.get_num_successors() != 0
            || raw.get_num_operands() != 0
            || raw.get_num_results() != 0
            || raw.attributes.0.len() != 2
            || native
                .get_attr_gpu_call_callee(context)
                .is_none_or(|actual| actual.as_str() != callee.as_str())
            || native.signature(context).is_none()
        {
            return Err(Failure::ExactGraph);
        }
        // The mandatory same-epoch snapshot verifies CallOp's immutable
        // signature against these zero arities, without an unpaid getter clone.
        Ok(())
    }

    fn check_index(&self, pointer: Ptr<Operation>, index: usize) -> Result<(), Failure> {
        let context = self.context();
        let expected = self.private.inventory().operations()[index].operation;
        let OperationKind::Intrinsic(intrinsic) = &expected.kind else {
            return Err(Failure::ExactGraph);
        };
        if !matches!(
            intrinsic.kind,
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Global,
                ..
            }
        ) || !matches!(expected.results.as_slice(), [result] if result.ty == Type::INDEX)
        {
            return Err(Failure::NativeSchema);
        }
        let native = Operation::get_op::<PreservedOperationOp>(pointer, context)
            .ok_or(Failure::NativeSchema)?;
        let raw = pointer.deref(context);
        if native.kind(context) != Some(PreservedOperationKindAttr::Intrinsic)
            || raw.num_regions() != 0
            || raw.get_num_successors() != 0
            || raw.get_num_operands() != 0
            || raw.get_num_results() != 1
            || raw.attributes.0.len() != 1
            || self
                .private
                .identity
                .origins
                .preserved_operations
                .get(&pointer)
                != Some(&expected.kind)
            || self.private.identity.origins.values.get(&raw.get_result(0))
                != Some(&expected.results[0].id)
        {
            return Err(Failure::ExactGraph);
        }
        let ty = raw.get_result(0).get_type(context);
        if !ty.deref(context).is::<IndexType>() {
            return Err(Failure::NativeSchema);
        }
        Ok(())
    }

    fn validate_complete(
        &self,
        profile: storage_v18::ProfileV18<'_>,
        budget: &mut Budget<'_>,
    ) -> Result<(), Failure> {
        self.private
            .validate_complete_with(profile, budget, |pointer, index, _| {
                match self.extra_kind(index) {
                    Some(Kind::ConditionalGlobalIndexV26) => self.check_index(pointer, index)?,
                    Some(Kind::TrapCall) => self.check_trap(pointer, index)?,
                    Some(Kind::ConditionalGlobalReadV26 | Kind::ConditionalGlobalWriteV26) => {
                        // The whole-module pending carrier census validated these
                        // exact operands, results and fixed attributes at this epoch.
                    }
                    None => return Ok(false),
                    _ => return Err(Failure::ExactGraph),
                }
                Ok(true)
            })
    }
}

#[cfg(test)]
#[path = "kir_bridge_mixed_memory_v26_tests.rs"]
mod tests;

impl super::super::super::native_private_seal::Sealed for NativeCanonicalMixedAdmissionV26<'_> {}

impl NativePrivateInputV1 for NativeCanonicalMixedAdmissionV26<'_> {
    fn context(&self) -> &Context {
        self.private.context()
    }
    fn function(&self) -> &FuncOp {
        self.private.function()
    }
    fn ordinal(&self) -> usize {
        self.private.ordinal()
    }
    fn epoch(&self) -> u64 {
        self.private.epoch()
    }
    fn operation_count(&self) -> usize {
        self.private.operation_count()
    }
    fn cfg_domain_v26(&self) -> crate::kir_bridge_v1::NativeCfgDomainV26 {
        // This adapter retains the complete exact V18 graph, including original
        // failure blocks left disconnected by source lowering. Policy9 does not
        // run CFG deletion. Neither source nor memory coverage is inferred from
        // a disconnected block; their complete original censuses remain intact.
        crate::kir_bridge_v1::NativeCfgDomainV26::EntryReachableSubgraphV26
    }
    fn supports_conditional_globals_v26(&self) -> bool {
        true
    }
    fn conditional_global_counts_v26(&self) -> Option<[usize; 2]> {
        Some(self.global_counts_v26())
    }
    fn authenticate(&self, context: &Context, function: &FuncOp) -> bool {
        self.private.authenticate(context, function)
    }
    fn operation(&self, context: &Context, pointer: Ptr<Operation>) -> Option<Kind> {
        if !self.authenticate(context, self.function()) {
            return None;
        }
        let coordinate = *self.private.coordinates.get(&pointer)?;
        if let Some(index) = self.private.ordinal_for(coordinate)
            && let Some(kind) = self.extra_kind(index)
        {
            return Some(kind);
        }
        if self.is_trap_end(coordinate) {
            return self
                .private
                .identity
                .unreachable(context, pointer)
                .then_some(Kind::TrapEnd);
        }
        self.private.operation(context, pointer)
    }
    fn attribute(
        &self,
        context: &Context,
        pointer: Ptr<Operation>,
        key: &str,
        dialect: &str,
        name: &str,
    ) -> bool {
        match self.operation(context, pointer) {
            Some(Kind::ConditionalGlobalReadV26) => {
                dialect == "gpu"
                    && matches!(
                        (key, name),
                        ("gpu_load_address_space", "address_space")
                            | ("gpu_load_alignment", "memory_alignment")
                            | ("gpu_load_volatile", "volatile")
                    )
            }
            Some(Kind::ConditionalGlobalWriteV26) => {
                dialect == "gpu"
                    && matches!(
                        (key, name),
                        ("gpu_store_address_space", "address_space")
                            | ("gpu_store_alignment", "memory_alignment")
                            | ("gpu_store_volatile", "volatile")
                    )
            }
            Some(Kind::ConditionalGlobalIndexV26) => {
                dialect == "gpu"
                    && key == "gpu_preserved_operation_kind"
                    && name == "preserved_operation_kind"
            }
            Some(Kind::TrapCall) => false,
            Some(Kind::TrapEnd) => self
                .private
                .identity
                .attribute(context, pointer, key, dialect, name),
            Some(_) => self.private.attribute(context, pointer, key, dialect, name),
            None => false,
        }
    }
    fn identity_lookup_work(&self) -> Option<usize> {
        self.private
            .identity_lookup_work()?
            .checked_add(self.operation_count().checked_mul(8)?)
    }
    fn stage_lookup_work(&self) -> Option<usize> {
        self.private
            .stage_lookup_work()?
            .checked_add(self.operation_count().checked_mul(8)?)
    }
    fn run_fixed(
        &self,
        _: crate::ProductionAnalysisResourceLimitsV1,
        _: Option<&mut crate::invocation_receipt_v1::InvocationReceiptV1>,
    ) -> Result<
        crate::canonical_private_v1::CanonicalPrivatePipelineOutcomeV1,
        crate::PipelineErrorV1,
    > {
        Err(crate::PipelineErrorV1::CanonicalPrivateInput)
    }
}

fn formal_error(error: CanonicalGuardedGlobalReadErrorV1) -> Failure {
    Failure::ConditionalGlobalsV26(error)
}

fn mixed_rows_v26(
    physical: &CheckedCanonicalKirPrivateMemoryV18<'_, '_>,
    globals: Domains<'_>,
    budget: &mut Budget<'_>,
) -> Result<Vec<Option<Kind>>, Failure> {
    if !std::ptr::eq(
        physical.inventory().owner(),
        globals.owner(budget).map_err(formal_error)?,
    ) {
        return Err(Failure::ExactGraph);
    }
    let inventory = physical.inventory();
    budget.charge_work(4)?;
    let mut kinds = Vec::new();
    budget.reserve_storage(
        std::mem::size_of::<Vec<Option<Kind>>>()
            .checked_add(
                inventory
                    .operations()
                    .len()
                    .checked_mul(std::mem::size_of::<Option<Kind>>())
                    .ok_or(ResourceError::Arithmetic)?,
            )
            .ok_or(ResourceError::Arithmetic)?,
    )?;
    kinds
        .try_reserve_exact(inventory.operations().len())
        .map_err(|_| ResourceError::Allocation)?;
    budget.reserve_storage(
        kinds
            .capacity()
            .checked_sub(inventory.operations().len())
            .and_then(|n| n.checked_mul(std::mem::size_of::<Option<Kind>>()))
            .ok_or(ResourceError::Arithmetic)?,
    )?;
    let mut accesses = 0usize;
    for (index, row) in inventory.operations().iter().enumerate() {
        budget.charge_work(12)?;
        let global = globals
            .access_at(row.coordinate, budget)
            .map_err(formal_error)?;
        let kind = if let Some(global) = global {
            if physical.operation(index) {
                return Err(Failure::ExactGraph);
            }
            accesses = accesses.checked_add(1).ok_or(ResourceError::Arithmetic)?;
            // The closed checked domain retains every actual source choice.
            // Operation shape alone neither selects a leaf nor discharges
            // source, initialization, alias or physical runtime premises.
            match (global, &row.operation.kind) {
                ((false, checked_pointer), OperationKind::Load { pointer, access })
                    if checked_pointer == *pointer
                        && matches!(
                            access.address_space,
                            fe2o3_kernel_ir::AddressSpace::Global
                                | fe2o3_kernel_ir::AddressSpace::Generic
                        )
                        && !access.volatile =>
                {
                    Some(Kind::ConditionalGlobalReadV26)
                }
                (
                    (true, checked_pointer),
                    OperationKind::Store {
                        pointer, access, ..
                    },
                ) if checked_pointer == *pointer
                    && matches!(
                        access.address_space,
                        fe2o3_kernel_ir::AddressSpace::Global
                            | fe2o3_kernel_ir::AddressSpace::Generic
                    )
                    && !access.volatile =>
                {
                    Some(Kind::ConditionalGlobalWriteV26)
                }
                _ => return Err(Failure::ExactGraph),
            }
        } else if matches!(row.operation.kind, OperationKind::Call { .. })
            && row
                .operation
                .has_registered_trap_contract_with_budget_v26(budget)?
        {
            if physical.operation(index) {
                return Err(Failure::ExactGraph);
            }
            check_trap_pair_v26(inventory, index, budget)?;
            Some(Kind::TrapCall)
        } else if matches!(&row.operation.kind, OperationKind::Intrinsic(intrinsic)
            if matches!(intrinsic.kind, IntrinsicKind::InvocationIndex { kind: IndexKind::Global, .. }))
        {
            if physical.operation(index) || !row.effects.is_empty() {
                return Err(Failure::ExactGraph);
            }
            Some(Kind::ConditionalGlobalIndexV26)
        } else {
            None
        };
        kinds.push(kind);
    }
    if accesses != globals.access_count(budget).map_err(formal_error)? {
        return Err(Failure::ExactGraph);
    }
    Ok(kinds)
}

fn check_trap_pair_v26(
    inventory: &CanonicalKirInventoryV18<'_>,
    index: usize,
    budget: &mut Budget<'_>,
) -> Result<(), Failure> {
    budget.charge_work(64)?;
    let row = &inventory.operations()[index];
    let coordinate = row.coordinate;
    let function = inventory
        .functions()
        .get(coordinate.block.function.0 as usize)
        .ok_or(Failure::ExactGraph)?;
    let block_index = function
        .blocks
        .start
        .checked_add(coordinate.block.block as usize)
        .filter(|n| *n < function.blocks.end)
        .ok_or(Failure::ExactGraph)?;
    let block = inventory
        .blocks()
        .get(block_index)
        .ok_or(Failure::ExactGraph)?;
    let calls = inventory
        .calls()
        .get(function.calls.clone())
        .ok_or(Failure::ExactGraph)?;
    // Inventory construction preserves coordinate order; search only this
    // function's call range rather than scanning the module for every pair.
    let levels = (usize::BITS - calls.len().leading_zeros()) as usize;
    budget.charge_work(
        levels
            .checked_add(1)
            .and_then(|n| n.checked_mul(8))
            .ok_or(ResourceError::Arithmetic)?,
    )?;
    let call_index = calls
        .binary_search_by_key(&coordinate, |call| call.coordinate)
        .map_err(|_| Failure::ExactGraph)?;
    let call = &calls[call_index];
    let target = call
        .target
        .and_then(|target| inventory.functions().get(target.0 as usize))
        .ok_or(Failure::ExactGraph)?;
    // Prepay both this declaration name comparison and the later exact native
    // carrier name comparison. Unequal native string lengths return immediately.
    budget.charge_work(
        call.callee
            .len()
            .checked_add(1)
            .and_then(|n| n.checked_mul(2))
            .ok_or(ResourceError::Arithmetic)?,
    )?;
    if block.coordinate != coordinate.block
        || block.operations.end.checked_sub(1) != Some(index)
        || !matches!(block.terminator, Terminator::Unreachable)
        || target.function.body.is_some()
        || target.function.id.as_str() != call.callee
        || !target.function.signature.parameters.is_empty()
        || !target.function.signature.results.is_empty()
    {
        return Err(Failure::ExactGraph);
    }
    // The inventory retains the exact verified V18 owner. Its reserved-call
    // verifier authenticated the declaration, capability and terminal position.
    // This additional pair census grants neither source truth nor launch rights.
    Ok(())
}

fn mixed_headers_v26(capture: usize, alignment: usize) -> Result<usize, Failure> {
    use fe2o3_kernel_analysis::CanonicalKirOperationRefV1;
    use fe2o3_kernel_ir::{ExplicitLaunchExtent, FormalIndexWidth};
    use std::{cell::Ref, mem::size_of};
    fn h<T>() -> Result<usize, Failure> {
        size_of::<T>()
            .checked_add(
                size_of::<Result<T, Failure>>()
                    .checked_mul(2)
                    .ok_or(ResourceError::Arithmetic)?,
            )
            .ok_or_else(|| ResourceError::Arithmetic.into())
    }
    type Rows<'a> = (
        &'a CheckedCanonicalKirPrivateMemoryV18<'a, 'a>,
        Domains<'a>,
        &'a CanonicalKirInventoryV18<'a>,
        &'a mut Budget<'a>,
        Vec<Option<Kind>>,
        usize,
        usize,
        std::iter::Enumerate<std::slice::Iter<'a, CanonicalKirOperationRefV1<'a>>>,
        Option<(usize, &'a CanonicalKirOperationRefV1<'a>)>,
        Option<(bool, ValueId)>,
        Option<Kind>,
        (bool, ValueId),
        &'a OperationKind,
        &'a fe2o3_kernel_ir::IntrinsicOperation,
    );
    type Index<'a> = (
        &'a NativeCanonicalMixedAdmissionV26<'a>,
        &'a Context,
        Ptr<Operation>,
        usize,
        &'a KirOperation,
        &'a fe2o3_kernel_ir::IntrinsicOperation,
        Ref<'a, Operation>,
        PreservedOperationOp,
        Option<PreservedOperationOp>,
        Option<PreservedOperationKindAttr>,
        Option<&'a OperationKind>,
        Option<&'a ValueId>,
        TypeHandle,
        Ref<'a, dyn pliron::r#type::Type>,
    );
    type Snapshot<'a> = (
        &'a mut KirPlironGraphV18<'a>,
        [Option<&'a crate::kir_bridge_v1::StructuralBridgeWitnessV18>; 2],
        [&'a crate::kir_bridge_v1::StructuralBridgeWitnessV18; 2],
        fe2o3_kernel_ir::StorageLayoutLimitsV1,
        u64,
        &'a mut Budget<'a>,
        (
            VerifiedCanonicalKernelIrModuleV18,
            KirBridgeReportV18,
            KirBridgeStorageV18,
        ),
    );
    type Conditions<'a> = (
        Domains<'a>,
        fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
        Option<(ExplicitLaunchExtent, FormalIndexWidth, usize, usize)>,
        ExplicitLaunchExtent,
        FormalIndexWidth,
        [usize; 2],
        &'a NativeCanonicalMixedAdmissionV26<'a>,
        &'a NativeCanonicalPrivateAdmissionV18<'a>,
        (&'a NativeCanonicalMixedAdmissionV26<'a>,),
        Vec<Option<Kind>>,
    );
    type Trap<'a> = (
        &'a CanonicalKirInventoryV18<'a>,
        &'a fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'a>,
        &'a fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'a>,
        &'a fe2o3_kernel_analysis::CanonicalKirBlockRefV1<'a>,
        &'a fe2o3_kernel_analysis::CanonicalKirCallRefV1<'a>,
        &'a [fe2o3_kernel_analysis::CanonicalKirCallRefV1<'a>],
        std::ops::Range<usize>,
        Result<usize, usize>,
        &'a NativeCanonicalMixedAdmissionV26<'a>,
        &'a Context,
        &'a KirOperation,
        &'a fe2o3_kernel_ir::FunctionId,
        &'a [ValueId],
        Ref<'a, Operation>,
        Option<Ref<'a, pliron::builtin::attributes::StringAttr>>,
        Option<Ref<'a, pliron::builtin::attributes::TypeAttr>>,
        CallOp,
        Option<CallOp>,
        Option<TypeHandle>,
        Option<Ptr<Operation>>,
        KirBridgeCoordinateV1,
        fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
        [usize; 16],
        [bool; 4],
    );
    let mut bytes = native_private_headers_v18(capture, alignment)?
        .checked_add(KirPlironGraphV18::pending_global_scan_headers_v18()?)
        .ok_or(ResourceError::Arithmetic)?;
    for item in [
        crate::native_conditional_domains_v30::conditional_query_headers_v30()?,
        h::<Rows<'_>>()?,
        h::<Index<'_>>()?,
        h::<Snapshot<'_>>()?,
        h::<Conditions<'_>>()?,
        h::<Trap<'_>>()?,
        h::<NativeCanonicalMixedAdmissionV26<'_>>()?,
        h::<Result<Vec<Option<Kind>>, Failure>>()?,
    ] {
        bytes = bytes.checked_add(item).ok_or(ResourceError::Arithmetic)?;
    }
    Ok(bytes)
}

impl KirPlironGraphV18<'_> {
    fn require_mixed_snapshot_v26(
        &mut self,
        layouts: fe2o3_kernel_ir::StorageLayoutLimitsV1,
        epoch: u64,
        structural: Option<&crate::kir_bridge_v1::StructuralBridgeWitnessV18>,
        budget: &mut Budget<'_>,
    ) -> Result<(), Failure> {
        self.check_ranked_policy_epoch_v18(epoch)?;
        let (owner, report, credit) = match structural {
            Some(witness) => self.extract_structural_native_v30(layouts, budget, witness)?,
            None => self.extract_canonical_v18_o0(layouts, budget)?,
        };
        budget.reserve_storage(credit.retained_storage())?;
        drop((owner, report));
        budget.release_storage(credit.retained_storage())?;
        self.check_ranked_policy_epoch_v18(epoch)
    }

    /// Internal conditional bridge. Exact current graph replay is mandatory;
    /// source/runtime premises remain a separate required conjunction.
    pub(crate) fn visit_mixed_policy_functions_v26(
        &mut self,
        physical: &CheckedCanonicalKirPrivateMemoryV18<'_, '_>,
        globals: &Globals<'_, '_>,
        epoch: u64,
        layouts: fe2o3_kernel_ir::StorageLayoutLimitsV1,
        structural: Option<&crate::kir_bridge_v1::StructuralBridgeWitnessV18>,
        budget: &mut Budget<'_>,
        consume: impl FnMut(usize, &NativeCanonicalMixedAdmissionV26<'_>) -> Result<(), Failure>,
    ) -> Result<(), Failure> {
        self.visit_conditional_policy_functions_v30(
            physical,
            Domains::Legacy(globals),
            epoch,
            layouts,
            structural,
            budget,
            consume,
        )
    }

    pub(crate) fn visit_conditional_policy_functions_v30(
        &mut self,
        physical: &CheckedCanonicalKirPrivateMemoryV18<'_, '_>,
        globals: Domains<'_>,
        epoch: u64,
        layouts: fe2o3_kernel_ir::StorageLayoutLimitsV1,
        structural: Option<&crate::kir_bridge_v1::StructuralBridgeWitnessV18>,
        budget: &mut Budget<'_>,
        mut consume: impl FnMut(usize, &NativeCanonicalMixedAdmissionV26<'_>) -> Result<(), Failure>,
    ) -> Result<(), Failure> {
        self.validate_custody(budget)?;
        self.check_ranked_policy_epoch_v18(epoch)?;
        if !std::ptr::eq(physical.inventory().owner(), self.profile.owner()) {
            return Err(Failure::ExactGraph);
        }
        budget.reserve_storage(mixed_headers_v26(
            std::mem::size_of_val(&consume),
            std::mem::align_of_val(&consume),
        )?)?;
        self.require_mixed_snapshot_v26(layouts, epoch, structural, budget)?;
        let kinds = mixed_rows_v26(physical, globals, budget)?;
        self.check_pending_global_carriers_v18(self.profile.owner(), epoch, budget)?;
        budget.charge_work(
            self.session
                .operations
                .len()
                .checked_add(1)
                .ok_or(ResourceError::Arithmetic)?,
        )?;
        let context = &self.session.context;
        let root = self.session.operations[&self.root.identity];
        let region = root.deref(context).get_region(0);
        let block = region
            .deref(context)
            .iter(context)
            .next()
            .ok_or(Failure::NativeSchema)?;
        let live_block = block.deref(context);
        let mut live_functions = live_block.iter(context);
        for (ordinal, source) in self.profile.owner().module().functions.iter().enumerate() {
            budget.charge_work(4)?;
            let Some(body) = &source.body else {
                continue;
            };
            let pointer = live_functions.next().ok_or(Failure::NativeSchema)?;
            budget.charge_work(
                self.origins
                    .functions
                    .len()
                    .checked_add(1)
                    .ok_or(ResourceError::Arithmetic)?,
            )?;
            if self.origins.functions.get(&pointer) != Some(&ordinal) {
                return Err(Failure::ExactGraph);
            }
            let function =
                Operation::get_op::<FuncOp>(pointer, context).ok_or(Failure::NativeSchema)?;
            self.check_ranked_policy_epoch_v18(epoch)?;
            budget.charge_work(body.blocks.len())?;
            let operations = body
                .blocks
                .iter()
                .try_fold(0usize, |n, b| {
                    n.checked_add(b.operations.len())?.checked_add(1)
                })
                .ok_or(ResourceError::Arithmetic)?;
            let coordinate = fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
                u32::try_from(ordinal).map_err(|_| ResourceError::Arithmetic)?,
            );
            let (_, _, reads, writes) = globals
                .function_conditions(coordinate, budget)
                .map_err(formal_error)?
                .ok_or(Failure::ExactGraph)?;
            let identity = NativeLifecycleIdentityAdmissionV18 {
                context,
                function: &function,
                origins: &self.origins,
                ordinal,
                epoch,
                operations,
            };
            let private = NativeCanonicalPrivateAdmissionV18 {
                identity: &identity,
                coordinates: &self.coordinates,
                physical,
            };
            let input = NativeCanonicalMixedAdmissionV26 {
                private,
                globals,
                kinds: &kinds,
                counts: [reads, writes],
            };
            input.validate_complete(self.profile, budget)?;
            consume(ordinal, &input)?;
            self.check_ranked_policy_epoch_v18(epoch)?;
            input.globals.owner(budget).map_err(formal_error)?;
        }
        budget.charge_work(1)?;
        if live_functions.next().is_some() {
            return Err(Failure::NativeSchema);
        }
        drop(live_functions);
        drop(live_block);
        globals.owner(budget).map_err(formal_error)?;
        self.validate_custody(budget)?;
        self.require_mixed_snapshot_v26(layouts, epoch, structural, budget)
    }
}
