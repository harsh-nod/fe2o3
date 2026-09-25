//! Runtime-premise preparation on the existing generated argument path.
//!
//! This module cannot authenticate V4 descriptors or discharge a memory theorem.
//! The production invocation owner must retain V4 proof/finalizer/currentness and
//! machine-refinement custody, then consume this bundle through normal runtime
//! preparation. The mandatory premise transport must not be discarded.

use super::*;
use fe2o3_artifacts::{
    AbiKind, Access, AddressSpace, AliasClass, ArgumentOwnership, ConditionalAddressDomainV1,
    ConditionalArgumentRoleV1, ConditionalInvocationContractV1, ConditionalInvocationWireErrorV1,
    MAX_CONDITIONAL_ARGUMENTS_V1, MAX_CONDITIONAL_READS_V1, Mutability,
};
use fe2o3_kernel_descriptor::{
    DescriptorWireErrorV4, DeviceDescriptorTableV4, KernelDescriptorRefV4, SourceTypeDescriptorV3,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_kfd::{
    ConditionalDispatchDomainV1 as Domain, ConditionalDispatchErrorV1,
    ConditionalDispatchPremisesV1, ConditionalDispatchReadV1, ConditionalDispatchSliceV1,
};

#[path = "generated_kfd_conditional_abi_v1.rs"]
mod abi;

#[derive(Debug)]
pub(crate) enum GeneratedConditionalPremiseErrorV1 {
    Resource(Resource),
    Contract(ConditionalInvocationWireErrorV1<Resource>),
    Descriptor(DescriptorWireErrorV4<Resource>),
    Binding(&'static str),
    Runtime(ConditionalDispatchErrorV1),
    Packing(GeneratedKfdArgumentError),
}
impl fmt::Display for GeneratedConditionalPremiseErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for GeneratedConditionalPremiseErrorV1 {}
impl From<Resource> for GeneratedConditionalPremiseErrorV1 {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl From<ConditionalInvocationWireErrorV1<Resource>> for GeneratedConditionalPremiseErrorV1 {
    fn from(e: ConditionalInvocationWireErrorV1<Resource>) -> Self {
        Self::Contract(e)
    }
}
impl From<DescriptorWireErrorV4<Resource>> for GeneratedConditionalPremiseErrorV1 {
    fn from(e: DescriptorWireErrorV4<Resource>) -> Self {
        Self::Descriptor(e)
    }
}
impl From<fe2o3_kernel_descriptor::DescriptorWireErrorV3<Resource>>
    for GeneratedConditionalPremiseErrorV1
{
    fn from(e: fe2o3_kernel_descriptor::DescriptorWireErrorV3<Resource>) -> Self {
        Self::Descriptor(DescriptorWireErrorV4::Nominal(e))
    }
}
impl From<ConditionalDispatchErrorV1> for GeneratedConditionalPremiseErrorV1 {
    fn from(e: ConditionalDispatchErrorV1) -> Self {
        Self::Runtime(e)
    }
}
pub(super) type Result<T> = std::result::Result<T, GeneratedConditionalPremiseErrorV1>;
fn binding(s: &'static str) -> GeneratedConditionalPremiseErrorV1 {
    GeneratedConditionalPremiseErrorV1::Binding(s)
}

impl<'allocation> GeneratedKfdArgumentBinding<'allocation> {
    /// Retains the exact plan used by the existing packer. Only the additional
    /// plan clone is accounted here; ordinary packing/input ownership must
    /// already be covered by the caller's owning transition. The returned plan
    /// charge is unreserved and must be retained until the packed owner drops or
    /// moves into runtime inputs (which drops the plan). No authority is created.
    pub(crate) fn pack_with_conditional_plan_v1(
        self,
        plan: &GeneratedArgumentPackingPlanV1,
        budget: &mut Budget<'_>,
    ) -> Result<(GeneratedKfdPackedArguments<'allocation>, usize)> {
        budget.with_prepaid_scope(budget.storage(), 1, 1, 0, |budget| {
            let (source_plan, retained) = plan.clone_for_conditional_v1(budget)?;
            budget.reserve_storage(retained)?;
            let mut packed = self
                .pack(plan)
                .map_err(GeneratedConditionalPremiseErrorV1::Packing)?;
            packed.source_plan = Some(source_plan);
            packed.source_plan_storage = retained;
            Ok((packed, retained))
        })
    }
}

/// Move-only preparation. The original generated borrows and completion sink
/// remain inside `packed`; premise data is still inert until live mapping checks.
#[must_use]
pub(crate) struct GeneratedConditionalKfdArgumentsV1<'allocation> {
    packed: GeneratedKfdPackedArguments<'allocation>,
    premises: ConditionalDispatchPremisesV1,
    retained_storage: usize,
}
impl<'allocation> GeneratedConditionalKfdArgumentsV1<'allocation> {
    /// Moves the complete transport through ordinary runtime preparation without
    /// constructing authority. The conditional payload cannot be omitted here.
    /// Caller-retained ledger credit follows this move; no fresh budget is made.
    pub(crate) fn into_runtime_inputs(
        self,
        geometry: AqlDispatchGeometryV1,
        dynamic_group_segment_bytes: u32,
        timeout_milliseconds: u32,
    ) -> std::result::Result<
        (
            Gfx942RuntimeDispatchInputsV1,
            GeneratedKfdCompletion<'allocation>,
        ),
        fe2o3_runtime::Gfx942RuntimePreparationErrorV1,
    > {
        let (inputs, completion) = self.packed.into_runtime_inputs(
            geometry,
            dynamic_group_segment_bytes,
            timeout_milliseconds,
        );
        Ok((
            inputs.with_conditional_premises_v1(self.premises)?,
            completion,
        ))
    }

    /// Unreserved premise addition, excluding the already-owned generated
    /// arguments and retained plan. The caller reserves it before controlled use.
    /// Keep both reservations until their owners drop; into_runtime_inputs drops
    /// the plan, while the premise reservation follows the runtime payload.
    pub(crate) const fn retained_storage(&self) -> usize {
        self.retained_storage
    }
    // Tests inspect inert components; production can only move them together.
    #[cfg(test)]
    pub(crate) fn into_parts(
        self,
    ) -> (
        GeneratedKfdPackedArguments<'allocation>,
        ConditionalDispatchPremisesV1,
    ) {
        (self.packed, self.premises)
    }
}

impl<'allocation> GeneratedKfdPackedArguments<'allocation> {
    /// Called only while the production invocation retains the authenticated V4
    /// table. V4 parsing and this structural join do not supply that authority.
    /// All decode/traversal work uses the caller's existing cumulative budget.
    pub(crate) fn bind_conditional_premises_v1(
        self,
        table: &DeviceDescriptorTableV4<'_>,
        geometry: AqlDispatchGeometryV1,
        budget: &mut Budget<'_>,
    ) -> Result<GeneratedConditionalKfdArgumentsV1<'allocation>> {
        let floor = budget.storage();
        if budget.storage() < self.source_plan_storage {
            return Err(Resource::Accounting.into());
        }
        let scratch = fe2o3_kernel_descriptor::DESCRIPTOR_QUERY_STORAGE_V4
            + size_of::<KernelDescriptorRefV4<'_, '_>>()
            + size_of::<ConditionalInvocationContractV1<'_>>()
            + abi::ABI_SCRATCH_STORAGE;
        let (premises, retained_storage) =
            budget.with_prepaid_scope(floor, 1, 1, scratch, |budget| -> Result<_> {
                let kernel = table.find_kernel(self.kernel_id, &mut |n| budget.charge_work(n))?;
                let plan = self
                    .source_plan
                    .as_ref()
                    .ok_or_else(|| binding("missing sealed plan"))?;
                abi::validate(plan, &self.packing_observation, table, &kernel, budget)?;
                let contract = kernel.conditional_contract(&mut |n| budget.charge_work(n))?;
                let retained_storage = contract
                    .argument_count()
                    .checked_mul(size_of::<ConditionalDispatchSliceV1>())
                    .and_then(|s| {
                        contract
                            .read_count()
                            .checked_mul(size_of::<ConditionalDispatchReadV1>())
                            .and_then(|r| s.checked_add(r))
                    })
                    .and_then(|s| {
                        size_of::<GeneratedConditionalKfdArgumentsV1<'_>>()
                            .checked_sub(size_of::<Self>())
                            .and_then(|shell| s.checked_add(shell))
                    })
                    .ok_or(Resource::Arithmetic)?;
                let premises = prepare(&self, table, &kernel, &contract, geometry, budget)?;
                Ok((premises, retained_storage))
            })?;
        Ok(GeneratedConditionalKfdArgumentsV1 {
            packed: self,
            premises,
            retained_storage,
        })
    }
}

fn prepare(
    packed: &GeneratedKfdPackedArguments<'_>,
    table: &DeviceDescriptorTableV4<'_>,
    kernel: &KernelDescriptorRefV4<'_, '_>,
    contract: &ConditionalInvocationContractV1<'_>,
    geometry: AqlDispatchGeometryV1,
    budget: &mut Budget<'_>,
) -> Result<ConditionalDispatchPremisesV1> {
    budget.charge_work(64)?;
    budget.charge_work(packed.explicit_kernarg.len())?;
    let plan = packed
        .source_plan
        .as_ref()
        .ok_or_else(|| binding("missing sealed plan"))?;
    if packed.kernel_id != plan.kernel_id()
        || kernel.kernel_id() != packed.kernel_id
        || contract.subjects().kernel_id != *packed.kernel_id.as_bytes()
        || kernel.argument_count() != plan.argument_count()
        || u64::from(kernel.abi_layout().explicit_argument_size()) != plan.kernarg_size()
        || kernel.abi_layout().kernarg_segment_alignment() != plan.kernarg_alignment()
        || plan.kernarg_size() != packed.explicit_kernarg.len() as u64
        || !packed
            .packing_observation
            .matches_explicit_kernarg(&packed.explicit_kernarg)
        || packed.alignment != plan.kernarg_alignment()
    {
        return Err(binding("kernel/actual packing plan/bytes"));
    }
    if contract.argument_count() > MAX_CONDITIONAL_ARGUMENTS_V1
        || contract.read_count() > MAX_CONDITIONAL_READS_V1
    {
        return Err(binding("conditional roster limit"));
    }
    // Both temporary rows and the resulting KFD-owned copies coexist. Precharge
    // the bounded structural engine and hashes before any collection allocation.
    let storage = contract
        .argument_count()
        .checked_mul(size_of::<ConditionalDispatchSliceV1>())
        .and_then(|n| {
            contract
                .read_count()
                .checked_mul(size_of::<ConditionalDispatchReadV1>())
                .and_then(|r| n.checked_add(r))
        })
        .and_then(|n| n.checked_mul(2))
        .and_then(|n| {
            n.checked_add(
                size_of::<ConditionalDispatchPremisesV1>()
                    + size_of::<Vec<ConditionalDispatchSliceV1>>()
                    + size_of::<Vec<ConditionalDispatchReadV1>>()
                    + size_of::<Sha256>()
                    + 256,
            )
        })
        .ok_or(Resource::Arithmetic)?;
    let floor = budget.storage();
    budget.with_prepaid_scope(floor, 1, 1, storage, |budget| {
        prepare_rows(packed, table, kernel, contract, geometry, budget)
    })
}

fn prepare_rows(
    packed: &GeneratedKfdPackedArguments<'_>,
    table: &DeviceDescriptorTableV4<'_>,
    kernel: &KernelDescriptorRefV4<'_, '_>,
    contract: &ConditionalInvocationContractV1<'_>,
    geometry: AqlDispatchGeometryV1,
    budget: &mut Budget<'_>,
) -> Result<ConditionalDispatchPremisesV1> {
    budget.charge_work(4)?;
    let mut slices = Vec::new();
    slices
        .try_reserve_exact(contract.argument_count())
        .map_err(|_| binding("allocation"))?;
    let mut slices = exact_row_storage(slices, contract.argument_count())?;
    let mut reads = Vec::new();
    reads
        .try_reserve_exact(contract.read_count())
        .map_err(|_| binding("allocation"))?;
    let mut reads = exact_row_storage(reads, contract.read_count())?;
    let plan = packed
        .source_plan
        .as_ref()
        .ok_or_else(|| binding("missing sealed plan"))?;
    let mut arguments = contract.arguments();
    while let Some(a) = arguments.next(&mut |n| budget.charge_work(n))? {
        let field_index = usize::from(a.generated_field);
        let field = plan
            .argument(field_index)
            .ok_or_else(|| binding("generated field"))?;
        let mut cursor = kernel.arguments();
        let mut descriptor = None;
        for index in 0..=field_index {
            let row = cursor
                .next(&mut |n| budget.charge_work(n))?
                .ok_or_else(|| binding("descriptor field"))?;
            if index == field_index {
                descriptor = Some(row);
            }
        }
        let descriptor = descriptor.ok_or_else(|| binding("descriptor field"))?;
        budget.charge_work(128 + descriptor.name().len() + field.name().as_str().len())?;
        if descriptor.source_type().as_bytes() != &a.source_type_identity
            || descriptor.device_layout().as_bytes() != &a.device_layout_identity
            || u32::from(descriptor.source_index()) != a.source_argument
            || descriptor.name() != field.name().as_str()
            || descriptor.component_count() != 2
        {
            return Err(binding("source/generated field correspondence"));
        }
        for i in 0..2 {
            let component = descriptor.component(i, &mut |n| budget.charge_work(n))?;
            let expected_kind = if i == 0 {
                fe2o3_kernel_descriptor::PhysicalAbiComponentKind::GlobalPointer
            } else {
                fe2o3_kernel_descriptor::PhysicalAbiComponentKind::SliceLengthU64
            };
            if component.kind != expected_kind
                || u64::from(component.offset) != field.offset() + 8 * i as u64
                || component.size != 8
                || component.alignment != 8
            {
                return Err(binding("generated physical components"));
            }
        }
        let source = table.source_type(descriptor.source_type(), &mut |n| budget.charge_work(n))?;
        match (a.role, source.descriptor()) {
            (ConditionalArgumentRoleV1::Input, SourceTypeDescriptorV3::SharedSlice(_))
            | (ConditionalArgumentRoleV1::Output, SourceTypeDescriptorV3::DisjointSlice(_)) => {}
            _ => return Err(binding("conditional source slice role")),
        }
        let AbiKind::Slice {
            element_size,
            element_alignment,
        } = field.kind()
        else {
            return Err(binding("slice layout"));
        };
        let output = a.role == ConditionalArgumentRoleV1::Output;
        let descriptor_access = match descriptor.access() {
            fe2o3_kernel_descriptor::AccessMode::ReadOnly => Access::ReadOnly,
            fe2o3_kernel_descriptor::AccessMode::WriteOnly => Access::WriteOnly,
            fe2o3_kernel_descriptor::AccessMode::ReadWrite => Access::ReadWrite,
            fe2o3_kernel_descriptor::AccessMode::ByValue => Access::ByValue,
        };
        let access_matches = if output {
            matches!(field.access(), Access::WriteOnly | Access::ReadWrite)
                && field.ownership() == ArgumentOwnership::UniqueBorrow
                && field.alias_class() == AliasClass::Exclusive
                && field.mutability() == Mutability::Mutable
        } else {
            field.access() == Access::ReadOnly
                && field.ownership() == ArgumentOwnership::SharedBorrow
                && field.alias_class() == AliasClass::SharedReadOnly
                && field.mutability() == Mutability::Immutable
        };
        if !access_matches
            || field.access() != descriptor_access
            || field.address_space() != AddressSpace::Global
        {
            return Err(binding("generated borrow/access"));
        }
        let pointer_offset =
            usize::try_from(field.offset()).map_err(|_| binding("field offset"))?;
        let length_offset = pointer_offset.checked_add(8).ok_or(Resource::Arithmetic)?;
        let length = word(&packed.explicit_kernarg, length_offset)?;
        budget.charge_work(packed.packing_observation.buffers.len())?;
        let mut candidates = packed
            .packing_observation
            .buffers
            .iter()
            .filter(|b| b.argument_index == field_index);
        let observation = candidates
            .next()
            .ok_or_else(|| binding("generated buffer roster"))?;
        if candidates.next().is_some() {
            return Err(binding("duplicate buffer field"));
        }
        let bytes = length
            .checked_mul(element_size)
            .ok_or(Resource::Arithmetic)?;
        if bytes != observation.initial_bytes as u64 {
            return Err(binding("whole logical buffer span"));
        }
        if let Some(i) = observation.buffer_index {
            let buffer = packed
                .buffers
                .get(i)
                .ok_or_else(|| binding("buffer index"))?;
            let access = match field.access() {
                Access::ReadOnly => Gfx942RuntimeBufferAccessV1::ReadOnly,
                Access::WriteOnly => Gfx942RuntimeBufferAccessV1::WriteOnly,
                Access::ReadWrite => Gfx942RuntimeBufferAccessV1::ReadWrite,
                _ => return Err(binding("buffer access")),
            };
            budget.charge_work(buffer.bytes().len())?;
            if observation.access != Some(access)
                || buffer.access() != access
                || buffer.bytes().len() != observation.initial_bytes
                || <[u8; 32]>::from(Sha256::digest(buffer.bytes())) != observation.initial_sha256
            {
                return Err(binding("retained buffer contents/access"));
            }
        } else if length != 0 || observation.access.is_some() {
            return Err(binding("empty buffer binding"));
        }
        slices.push(ConditionalDispatchSliceV1 {
            generated_field: a.generated_field,
            pointer_offset,
            length_offset,
            buffer_index: observation.buffer_index,
            buffer_byte_offset: 0,
            length,
            element_bytes: element_size,
            alignment: element_alignment,
        });
    }
    if slices.len() != packed.packing_observation.buffers.len() {
        return Err(binding("complete logical slice roster"));
    }
    let output = contract.output();
    let out = slices
        .get(usize::from(output.argument))
        .ok_or_else(|| binding("output field"))?;
    if out.element_bytes != output.element_bytes || out.alignment < output.alignment {
        return Err(binding("output element layout"));
    }
    let mut cursor = contract.reads();
    while let Some(r) = cursor.next(&mut |n| budget.charge_work(n))? {
        let s = slices
            .get(usize::from(r.argument))
            .ok_or_else(|| binding("read field"))?;
        if s.element_bytes != r.element_bytes || s.alignment < r.alignment {
            return Err(binding("read element layout"));
        }
        reads.push(ConditionalDispatchReadV1 {
            slice: r.argument,
            access_domain: domain(r.access_domain),
            address_domain: domain(r.address_domain),
        });
    }
    // Includes row scans, duplicate/coverage checks, copies and serialized hash
    // bytes in the bounded KFD constructor, all on the inherited account.
    let structural_work = slices
        .len()
        .checked_mul(slices.len() + reads.len() + 256)
        .and_then(|n| reads.len().checked_mul(64).and_then(|r| n.checked_add(r)))
        .and_then(|n| n.checked_add(packed.explicit_kernarg.len()))
        .and_then(|n| n.checked_add(512))
        .ok_or(Resource::Arithmetic)?;
    budget.charge_work(structural_work)?;
    Ok(ConditionalDispatchPremisesV1::new(
        *contract.identity().as_bytes(),
        *packed.packing_observation.identity(),
        *packed.kernel_id.as_bytes(),
        &packed.explicit_kernarg,
        geometry,
        &slices,
        usize::from(output.argument),
        domain(output.address_domain),
        &reads,
    )?)
}

fn exact_row_storage<T>(rows: Vec<T>, count: usize) -> Result<Vec<T>> {
    // The enclosing scope prepaid count * size_of::<T>(); Vec may expose more
    // than requested even after try_reserve_exact. Never use that unpaid storage.
    if !rows.is_empty() || rows.capacity() != count {
        return Err(Resource::Accounting.into());
    }
    Ok(rows)
}

fn domain(value: ConditionalAddressDomainV1) -> Domain {
    match value {
        ConditionalAddressDomainV1::GuardedOutput => Domain::GuardedOutput,
        ConditionalAddressDomainV1::GlobalLaunch => Domain::GlobalLaunch,
    }
}

fn word(bytes: &[u8], offset: usize) -> Result<u64> {
    let end = offset.checked_add(8).ok_or(Resource::Arithmetic)?;
    Ok(u64::from_le_bytes(
        bytes
            .get(offset..end)
            .ok_or_else(|| binding("slice component range"))?
            .try_into()
            .map_err(|_| binding("slice component width"))?,
    ))
}

#[cfg(test)]
mod capacity_tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

    #[test]
    fn exact_scratch_accepts_empty_reads_and_full_slice_roster_without_growth() {
        let empty = exact_row_storage(Vec::<ConditionalDispatchReadV1>::new(), 0).unwrap();
        assert_eq!(empty.capacity(), 0);
        let rows = Vec::<ConditionalDispatchSliceV1>::with_capacity(MAX_CONDITIONAL_ARGUMENTS_V1);
        let pointer = rows.as_ptr();
        let rows = exact_row_storage(rows, MAX_CONDITIONAL_ARGUMENTS_V1).unwrap();
        assert_eq!(rows.as_ptr(), pointer);
        assert_eq!(rows.capacity(), MAX_CONDITIONAL_ARGUMENTS_V1);
        assert!(rows.is_empty());
    }

    #[test]
    fn excess_scratch_capacity_refuses_before_row_use_and_restores_original_scope() {
        let mut work = Work::new(100);
        let mut budget = Budget::new(&mut work, 4096);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(73).unwrap();
        let account = budget.work_ledger_identity_v1();
        for requested in [0, 1, MAX_CONDITIONAL_READS_V1] {
            let used = std::cell::Cell::new(false);
            let result = budget.with_prepaid_scope(
                73,
                1,
                1,
                requested * size_of::<ConditionalDispatchReadV1>(),
                |_| -> Result<()> {
                    // Inject the allocator outcome without replacing a global
                    // allocator or exposing a production allocation callback.
                    let rows = Vec::<ConditionalDispatchReadV1>::with_capacity(requested + 1);
                    let _rows = exact_row_storage(rows, requested)?;
                    used.set(true);
                    Ok(())
                },
            );
            assert!(matches!(
                result,
                Err(GeneratedConditionalPremiseErrorV1::Resource(
                    Resource::Accounting
                ))
            ));
            assert!(!used.get());
            assert_eq!(budget.storage(), 73);
            assert!(account == budget.work_ledger_identity_v1());
        }
        assert_eq!(budget.work(), 10);
    }
}
