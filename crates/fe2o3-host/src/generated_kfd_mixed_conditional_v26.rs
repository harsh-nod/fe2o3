//! Complete generated packing to the distinct mixed runtime family.
//! This structural join is inert: a production Worker must retain authenticated
//! source/contract, proof, finalizer, machine and currentness owners separately.

use super::conditional::{
    GeneratedConditionalPremiseErrorV1 as Error, Result, abi, exact_row_storage, word,
};
use super::*;
use fe2o3_artifacts::{Access, AddressSpace, AliasClass, ArgumentOwnership, Mutability};
use fe2o3_kernel_descriptor::mixed_conditional_v26::{
    MixedContractV26, MixedIndexEnvelopeV26, MixedScalarV26, mixed_descriptor_subject_v26,
};
use fe2o3_kernel_descriptor::mixed_conditional_v86::MixedContractV86;
use fe2o3_kernel_descriptor::{
    DeviceDescriptorTableV3, KernelDescriptorRefV3, MAX_ARGUMENTS_PER_KERNEL, OwnershipSemantics,
    SourceTypeDescriptorV3,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_kfd::{
    ConditionalDispatchSliceV1 as Slice, MixedConditionalAccessV26 as AccessRow,
    MixedConditionalDispatchPremisesV26 as Premises, MixedConditionalIndexDomainV26 as Domain,
    MixedConditionalUnusedSliceV26 as Unused,
};

#[path = "generated_kfd_mixed_contract_v88.rs"]
mod contract_v88;
use contract_v88::MixedPackingContractV88;

fn binding(reason: &'static str) -> Error {
    Error::Binding(reason)
}
fn sum(values: &[usize]) -> Result<usize> {
    values
        .iter()
        .try_fold(0usize, |n, v| n.checked_add(*v))
        .ok_or_else(|| Resource::Arithmetic.into())
}

/// The actual generated borrows/completion and full numeric premises move
/// together. Neither this type nor runtime preparation supplies launch authority.
#[must_use]
pub(crate) struct GeneratedMixedKfdArgumentsV26<'allocation> {
    packed: GeneratedKfdPackedArguments<'allocation>,
    premises: Premises,
    retained_storage: usize,
}
impl<'allocation> GeneratedMixedKfdArgumentsV26<'allocation> {
    pub(crate) const fn retained_storage(&self) -> usize {
        self.retained_storage
    }
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
            inputs.with_mixed_conditional_premises_v26(self.premises)?,
            completion,
        ))
    }
    #[cfg(test)]
    fn into_parts(self) -> (GeneratedKfdPackedArguments<'allocation>, Premises) {
        (self.packed, self.premises)
    }
}

impl<'allocation> GeneratedKfdPackedArguments<'allocation> {
    /// Reuses the actual sealed packing plan. Callers retain/prepay the complete
    /// descriptor/contract backing and inherited packing; returned addition is
    /// unreserved, as on the historical conditional path. This does not accept
    /// public digests as source/proof/currentness authority.
    pub(crate) fn bind_mixed_conditional_premises_v26(
        self,
        table: &DeviceDescriptorTableV3<'_>,
        contract: &MixedContractV26<'_>,
        geometry: AqlDispatchGeometryV1,
        budget: &mut Budget<'_>,
    ) -> Result<GeneratedMixedKfdArgumentsV26<'allocation>> {
        self.bind_mixed_contract_v88(table, contract, geometry, budget)
    }

    /// V86 retains the exact explicit-predicate contract and occurrence identities.
    /// Numeric runtime premises are shared with V26, not its CFG-only decoding or
    /// source-proof admission. The owning worker still supplies those authorities.
    pub(crate) fn bind_predicated_mixed_conditional_premises_v88(
        self,
        table: &DeviceDescriptorTableV3<'_>,
        contract: &MixedContractV86<'_>,
        geometry: AqlDispatchGeometryV1,
        budget: &mut Budget<'_>,
    ) -> Result<GeneratedMixedKfdArgumentsV26<'allocation>> {
        self.bind_mixed_contract_v88(table, contract, geometry, budget)
    }

    fn bind_mixed_contract_v88<C: MixedPackingContractV88>(
        self,
        table: &DeviceDescriptorTableV3<'_>,
        contract: &C,
        geometry: AqlDispatchGeometryV1,
        budget: &mut Budget<'_>,
    ) -> Result<GeneratedMixedKfdArgumentsV26<'allocation>> {
        let floor = budget.storage();
        if floor < self.source_plan_storage {
            return Err(Resource::Accounting.into());
        }
        let row_bytes = contract
            .argument_count()
            .checked_mul(size_of::<Slice>())
            .and_then(|n| {
                contract
                    .occurrence_count()
                    .checked_mul(size_of::<AccessRow>())
                    .and_then(|v| n.checked_add(v))
            })
            .ok_or(Resource::Arithmetic)?;
        let scratch = sum(&[
            abi::ABI_SCRATCH_STORAGE,
            fe2o3_kernel_descriptor::DESCRIPTOR_QUERY_STORAGE_V3,
            C::CODEC_STORAGE,
            contract_v88::PACKING_CONTRACT_STORAGE_V88,
            size_of::<KernelDescriptorRefV3<'_, '_>>(),
            size_of::<[Option<usize>; MAX_ARGUMENTS_PER_KERNEL]>() * 2,
            size_of::<Vec<Slice>>()
                + size_of::<Vec<AccessRow>>()
                + size_of::<Vec<Unused>>()
                + size_of::<Premises>(),
            size_of::<Sha256>(),
            row_bytes,
            row_bytes,
            contract
                .argument_count()
                .checked_mul(2 * size_of::<Unused>())
                .ok_or(Resource::Arithmetic)?,
            1024,
        ])?;
        let (premises, retained_storage) =
            budget.with_prepaid_scope(floor, 1, 1, scratch, |budget| -> Result<_> {
                let kernel = table.find_kernel(self.kernel_id, &mut |n| budget.charge_work(n))?;
                let plan = self
                    .source_plan
                    .as_ref()
                    .ok_or_else(|| binding("missing sealed plan"))?;
                abi::validate(plan, &self.packing_observation, table, &kernel, budget)?;
                // Replay the exact retained observation, including row order,
                // before its identity is carried into the numerical payload.
                let observation_work = self
                    .packing_observation
                    .components
                    .len()
                    .checked_mul(128)
                    .and_then(|n| {
                        self.packing_observation
                            .buffers
                            .len()
                            .checked_mul(160)
                            .and_then(|m| n.checked_add(m))
                    })
                    .and_then(|n| n.checked_add(256))
                    .ok_or(Resource::Arithmetic)?;
                budget.charge_work(observation_work)?;
                if packing_observation_identity(&self.packing_observation)
                    .map_err(Error::Packing)?
                    != *self.packing_observation.identity()
                    || self.packing_observation.kernarg_alignment() != self.alignment
                {
                    return Err(binding("mixed retained packing observation"));
                }
                let subjects = contract.subjects();
                budget.charge_work(256)?;
                budget.charge_work(self.explicit_kernarg.len())?;
                if subjects.kernel_id != *self.kernel_id.as_bytes()
                    || subjects.descriptor_identity
                        != mixed_descriptor_subject_v26(table, &mut |n| budget.charge_work(n))?
                    || usize::try_from(subjects.generated_field_count).ok()
                        != Some(plan.argument_count())
                    || u64::from(subjects.explicit_argument_bytes) != plan.kernarg_size()
                    || subjects.kernarg_alignment != plan.kernarg_alignment()
                    || self.alignment != plan.kernarg_alignment()
                    || plan.kernarg_size() != self.explicit_kernarg.len() as u64
                    || !self
                        .packing_observation
                        .matches_explicit_kernarg(&self.explicit_kernarg)
                    || contract.argument_count() != self.packing_observation.buffers.len()
                {
                    return Err(binding(
                        "mixed complete descriptor/packing/contract subject",
                    ));
                }
                let premises = prepare_rows(&self, table, &kernel, contract, geometry, budget)?;
                let retained = sum(&[
                    row_bytes,
                    premises
                        .unused_slices()
                        .len()
                        .checked_mul(size_of::<Unused>())
                        .ok_or(Resource::Arithmetic)?,
                    size_of::<GeneratedMixedKfdArgumentsV26<'_>>()
                        .checked_sub(size_of::<Self>())
                        .ok_or(Resource::Arithmetic)?,
                ])?;
                Ok((premises, retained))
            })?;
        Ok(GeneratedMixedKfdArgumentsV26 {
            packed: self,
            premises,
            retained_storage,
        })
    }
}

fn prepare_rows<C: MixedPackingContractV88>(
    packed: &GeneratedKfdPackedArguments<'_>,
    table: &DeviceDescriptorTableV3<'_>,
    kernel: &KernelDescriptorRefV3<'_, '_>,
    contract: &C,
    geometry: AqlDispatchGeometryV1,
    budget: &mut Budget<'_>,
) -> Result<Premises> {
    let plan = packed
        .source_plan
        .as_ref()
        .ok_or_else(|| binding("missing sealed plan"))?;
    budget.charge_work(2 * MAX_ARGUMENTS_PER_KERNEL + contract.argument_count() + 32)?;
    let mut by_field = [None; MAX_ARGUMENTS_PER_KERNEL];
    let mut observations = [None; MAX_ARGUMENTS_PER_KERNEL];
    for i in 0..contract.argument_count() {
        let a = contract.argument(i, budget)?;
        let slot = by_field
            .get_mut(usize::from(a.generated_field))
            .ok_or_else(|| binding("mixed generated field range"))?;
        if slot.replace(i).is_some() {
            return Err(binding("mixed duplicate generated field"));
        }
    }
    for (i, observation) in packed.packing_observation.buffers.iter().enumerate() {
        budget.charge_work(8)?;
        let slot = observations
            .get_mut(observation.argument_index)
            .ok_or_else(|| binding("mixed buffer field range"))?;
        if slot.replace(i).is_some() {
            return Err(binding("mixed duplicate generated buffer"));
        }
    }
    let mut slices = Vec::new();
    slices
        .try_reserve_exact(contract.argument_count())
        .map_err(|_| binding("allocation"))?;
    let mut slices = exact_row_storage(slices, contract.argument_count())?;
    let mut unused = Vec::new();
    unused
        .try_reserve_exact(contract.argument_count())
        .map_err(|_| binding("allocation"))?;
    let mut unused = exact_row_storage(unused, contract.argument_count())?;
    let empty = Slice {
        generated_field: 0,
        pointer_offset: 0,
        length_offset: 8,
        buffer_index: None,
        buffer_byte_offset: 0,
        length: 0,
        element_bytes: 1,
        alignment: 1,
    };
    slices.resize(contract.argument_count(), empty);
    let mut cursor = kernel.arguments();
    let mut visited = 0usize;
    for field_index in 0..plan.argument_count() {
        let descriptor = cursor
            .next(&mut |n| budget.charge_work(n))?
            .ok_or_else(|| binding("mixed descriptor field"))?;
        let field = plan
            .argument(field_index)
            .ok_or_else(|| binding("mixed generated field"))?;
        budget.charge_work(24)?;
        let Some(index) = by_field[field_index] else {
            if observations[field_index].is_some()
                || matches!(field.kind(), fe2o3_artifacts::AbiKind::Slice { .. })
            {
                return Err(binding("mixed omitted generated slice"));
            }
            continue;
        };
        let a = contract.argument(index, budget)?;
        if a.reads == 0 && a.writes == 0 {
            unused.push(Unused {
                slice: u16::try_from(index).map_err(|_| Resource::Arithmetic)?,
                source_argument_identity: contract.argument_identity(index, budget)?,
            });
        }
        let observation = packed
            .packing_observation
            .buffers
            .get(
                observations[field_index]
                    .ok_or_else(|| binding("mixed missing generated buffer"))?,
            )
            .ok_or_else(|| binding("mixed generated buffer ordinal"))?;
        let source = table.source_type(descriptor.source_type(), &mut |n| budget.charge_work(n))?;
        let (scalar, exclusive) = match source.descriptor() {
            SourceTypeDescriptorV3::SharedSlice(s) => (s, false),
            SourceTypeDescriptorV3::DisjointSlice(s) => (s, true),
            _ => return Err(binding("mixed source slice kind")),
        };
        let fe2o3_artifacts::AbiKind::Slice {
            element_size,
            element_alignment,
        } = field.kind()
        else {
            return Err(binding("mixed generated slice layout"));
        };
        budget.charge_work(192)?;
        if descriptor.source_type().as_bytes() != &a.descriptor_type_identity
            || descriptor.device_layout().as_bytes() != &a.device_layout_identity
            || a.scalar != scalar_kind(scalar)
            || element_size != a.scalar.element_bytes()
            || u64::from(a.pointer_offset) != field.offset()
            || a.pointer_offset.checked_add(8) != Some(a.length_offset)
            || a.source_exclusive != exclusive
            || exclusive != (descriptor.ownership() == OwnershipSemantics::UniqueBorrow)
            || field.address_space() != AddressSpace::Global
            || (a.reads != 0 && !matches!(field.access(), Access::ReadOnly | Access::ReadWrite))
            || (a.writes != 0 && !matches!(field.access(), Access::WriteOnly | Access::ReadWrite))
        {
            return Err(binding(
                "mixed source/descriptor/actual generated slice join",
            ));
        }
        let effect = if exclusive {
            field.ownership() == ArgumentOwnership::UniqueBorrow
                && field.alias_class() == AliasClass::Exclusive
                && field.mutability() == Mutability::Mutable
        } else {
            field.ownership() == ArgumentOwnership::SharedBorrow
                && field.alias_class() == AliasClass::SharedReadOnly
                && field.mutability() == Mutability::Immutable
                && field.access() == Access::ReadOnly
        };
        if !effect {
            return Err(binding("mixed generated borrow class"));
        }
        let pointer_offset = usize::try_from(a.pointer_offset).map_err(|_| Resource::Arithmetic)?;
        let length_offset = usize::try_from(a.length_offset).map_err(|_| Resource::Arithmetic)?;
        let length = word(&packed.explicit_kernarg, length_offset)?;
        let bytes = length
            .checked_mul(element_size)
            .ok_or(Resource::Arithmetic)?;
        if bytes != observation.initial_bytes as u64 {
            return Err(binding("mixed complete initialized logical span"));
        }
        if let Some(i) = observation.buffer_index {
            let buffer = packed
                .buffers
                .get(i)
                .ok_or_else(|| binding("mixed actual buffer"))?;
            let access = match field.access() {
                Access::ReadOnly => Gfx942RuntimeBufferAccessV1::ReadOnly,
                Access::WriteOnly => Gfx942RuntimeBufferAccessV1::WriteOnly,
                Access::ReadWrite => Gfx942RuntimeBufferAccessV1::ReadWrite,
                _ => return Err(binding("mixed buffer access")),
            };
            budget.charge_work(buffer.bytes().len())?;
            if observation.access != Some(access)
                || buffer.access() != access
                || buffer.bytes().len() != observation.initial_bytes
                || <[u8; 32]>::from(Sha256::digest(buffer.bytes())) != observation.initial_sha256
            {
                return Err(binding("mixed retained buffer contents/access"));
            }
        } else if length != 0 || observation.access.is_some() {
            return Err(binding("mixed empty buffer binding"));
        }
        slices[index] = Slice {
            generated_field: a.generated_field,
            pointer_offset,
            length_offset,
            buffer_index: observation.buffer_index,
            buffer_byte_offset: 0,
            length,
            element_bytes: element_size,
            alignment: element_alignment,
        };
        visited += 1;
    }
    if visited != contract.argument_count()
        || cursor.next(&mut |n| budget.charge_work(n))?.is_some()
    {
        return Err(binding("mixed complete descriptor slice roster"));
    }
    let mut accesses = Vec::new();
    accesses
        .try_reserve_exact(contract.occurrence_count())
        .map_err(|_| binding("allocation"))?;
    let mut accesses = exact_row_storage(accesses, contract.occurrence_count())?;
    for i in 0..contract.occurrence_count() {
        let row = contract.occurrence(i, budget)?;
        let slice = slices
            .get(usize::from(row.argument))
            .ok_or_else(|| binding("mixed occurrence argument"))?;
        budget.charge_work(32)?;
        if slice.element_bytes != row.element_bytes || slice.alignment < row.alignment {
            return Err(binding("mixed exact occurrence element layout"));
        }
        accesses.push(AccessRow {
            slice: row.argument,
            writing: row.writing,
            occurrence_identity: contract.occurrence_identity(i, budget)?,
            access_domain: domain(row.access_envelope),
            address_domain: domain(row.formation_envelope),
            invocation_axis: (row.invocation_axis != 255).then_some(row.invocation_axis),
        });
    }
    // Prepay KFD comparisons, including 32-byte identity equality in duplicate
    // scans, scalar domain checks, row copies and hashing. Fixed caps make all
    // inner factors bounded; outer additions remain checked on the caller account.
    let work = slices
        .len()
        .checked_mul(8 * slices.len() + 4 * accesses.len() + 4 * unused.len() + 512)
        .and_then(|n| {
            accesses
                .len()
                .checked_mul(64 * accesses.len() + 512)
                .and_then(|m| n.checked_add(m))
        })
        .and_then(|n| {
            unused
                .len()
                .checked_mul(40 * unused.len() + 128)
                .and_then(|m| n.checked_add(m))
        })
        .and_then(|n| n.checked_add(packed.explicit_kernarg.len()))
        .and_then(|n| n.checked_add(2048))
        .ok_or(Resource::Arithmetic)?;
    budget.charge_work(work)?;
    let subjects = contract.subjects();
    budget.charge_work(32)?;
    let launch = kernel.launch();
    let max_grid = launch.max_grid();
    if subjects.source_rank != launch.rank()
        || [max_grid.x(), max_grid.y(), max_grid.z()]
            .into_iter()
            .zip(geometry.workgroup())
            .zip(subjects.exact_grid)
            .any(|((groups, workgroup), envelope)| {
                u64::from(groups) * u64::from(workgroup) != envelope
            })
    {
        return Err(binding("mixed physical envelope and exact workgroup"));
    }
    Ok(Premises::new_for_physical_envelope_v26(
        *contract.identity(),
        *packed.packing_observation.identity(),
        *packed.kernel_id.as_bytes(),
        &packed.explicit_kernarg,
        geometry,
        subjects.source_rank,
        subjects.exact_grid,
        subjects.index_width,
        &slices,
        &accesses,
        &unused,
    )?)
}

fn domain(value: MixedIndexEnvelopeV26) -> Domain {
    match value {
        MixedIndexEnvelopeV26::LogicalExtent { argument } => {
            Domain::LogicalExtent { slice: argument }
        }
        MixedIndexEnvelopeV26::InvocationAxis { axis } => Domain::InvocationAxis { axis },
        MixedIndexEnvelopeV26::UnsignedWidth { bits } => Domain::UnsignedWidth { bits },
    }
}
fn scalar_kind(value: fe2o3_kernel_descriptor::ScalarTypeV1) -> MixedScalarV26 {
    use fe2o3_kernel_descriptor::ScalarTypeV1 as S;
    match value {
        S::I8 => MixedScalarV26::I8,
        S::U8 => MixedScalarV26::U8,
        S::I16 => MixedScalarV26::I16,
        S::U16 => MixedScalarV26::U16,
        S::I32 => MixedScalarV26::I32,
        S::U32 => MixedScalarV26::U32,
        S::I64 => MixedScalarV26::I64,
        S::U64 => MixedScalarV26::U64,
        S::F16 => MixedScalarV26::F16,
        S::F32 => MixedScalarV26::F32,
        S::F64 => MixedScalarV26::F64,
    }
}

#[cfg(test)]
#[path = "generated_kfd_mixed_conditional_v26_tests.rs"]
mod tests;
