//! Immutable per-member observations alongside the original aggregate owners.

use super::*;
use crate::shared_memory::SharedGttMappedResourceFactsV1;

type Image = [u8; conditional_fill::KERNARG_BYTES];

pub(in crate::queue::dispatch_binding) struct CohortStorageV1 {
    images: Vec<Image>,
    members: Vec<Member>,
}

struct Member {
    code: ResolvedCodeIdentityV1,
    code_facts: SharedGttMappedResourceFactsV1,
    code_storage: SharedGttAllocationIdentityV1,
    kernarg_facts: SharedGttMappedResourceFactsV1,
    kernarg_storage: SharedGttAllocationIdentityV1,
    output_facts: SharedGttMappedResourceFactsV1,
    output_storage: SharedGttAllocationIdentityV1,
    output_bytes: u64,
    packet: PreparedDispatchPacketV1,
    recipe: u64,
    generation: u64,
}

pub(in crate::queue::dispatch_binding) struct NativeMemberCustodyV1<'a> {
    pub(in crate::queue::dispatch_binding) code: &'a CodeAuthority,
    pub(in crate::queue::dispatch_binding) code_identity: ResolvedCodeIdentityV1,
    pub(in crate::queue::dispatch_binding) kernarg: &'a KernargAuthority,
    pub(in crate::queue::dispatch_binding) output: &'a DispatchDataAuthorityV1,
    pub(in crate::queue::dispatch_binding) generation: &'a DispatchGenerationOwnerV1,
}

impl CohortStorageV1 {
    pub(in crate::queue::dispatch_binding) fn new(
        count: usize,
    ) -> Result<Self, Gfx942DispatchBindingErrorV1> {
        validate_count(count)?;
        let mut images = Vec::new();
        let mut members = Vec::new();
        images
            .try_reserve_exact(count)
            .map_err(|_| rejected(0, "cohort image backing"))?;
        members
            .try_reserve_exact(count)
            .map_err(|_| rejected(0, "cohort premise backing"))?;
        images.resize(count, [0; conditional_fill::KERNARG_BYTES]);
        Ok(Self { images, members })
    }

    pub(in crate::queue::dispatch_binding) fn capture(
        &mut self,
        bytes: &[u8],
        plan: &FixedDispatchPreparationPlanV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        if self.images.len() != plan.packets.len() || bytes.len() != plan.kernarg_arena_bytes {
            return Err(rejected(0, "cohort complete kernarg image"));
        }
        for (index, (image, packet)) in self.images.iter_mut().zip(&plan.packets).enumerate() {
            let end = packet
                .kernarg_offset
                .checked_add(image.len())
                .ok_or_else(|| rejected(index, "cohort kernarg image overflow"))?;
            image.copy_from_slice(
                bytes
                    .get(packet.kernarg_offset..end)
                    .ok_or_else(|| rejected(index, "cohort kernarg image extent"))?,
            );
        }
        Ok(())
    }

    pub(in crate::queue::dispatch_binding) fn check_member(
        &mut self,
        index: usize,
        model: &Gfx942FillKernelV1<'_>,
        input: &Gfx942FixedDispatchPacketV1,
        plan: &FixedDispatchPreparationPlanV1,
        custody: NativeMemberCustodyV1<'_>,
        packet: PreparedDispatchPacketV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        if index != self.members.len()
            || index >= self.images.len()
            || self.members.capacity() < self.images.len()
        {
            return Err(rejected(index, "cohort premise order or capacity"));
        }
        let NativeMemberCustodyV1 {
            code,
            code_identity,
            kernarg,
            output,
            generation,
        } = custody;
        let DispatchDataAuthorityV1::HostVisible(output) = output else {
            return Err(rejected(index, "cohort coherent original output"));
        };
        let output_bytes = plan.data[index].layout.requested_bytes();
        let offset = plan.packets[index].kernarg_offset;
        let vm = kernarg.facts().mapping().allocation.vm;
        let address = output
            .facts()
            .checked_gpu_subrange(0, output_bytes, 4)
            .ok_or_else(|| rejected(index, "cohort output extent"))?;
        let kernarg_address = kernarg
            .facts()
            .checked_gpu_subrange(offset as u64, conditional_fill::KERNARG_BYTES as u64, 8)
            .ok_or_else(|| rejected(index, "cohort complete member kernarg extent"))?;
        if output.layout().requested_bytes() as u64 != output_bytes
            || kernarg.layout().requested_bytes() != plan.kernarg_arena_bytes
            || kernarg.facts().logical_bytes() != plan.kernarg_arena_bytes
            || output.facts().mapping().allocation.vm != vm
            || code.facts().mapping().allocation.vm != vm
            || !code
                .storage_identity()
                .same_retained_session_v1(kernarg.storage_identity())
            || !output
                .storage_identity()
                .same_retained_session_v1(kernarg.storage_identity())
            || code_identity.mapping != code.facts().mapping()
            || packet.kernarg_mapping != kernarg.facts().mapping()
            || packet.kernarg_address.raw() != kernarg_address
            || packet.code_index != index
            || !packet.conditional_fill
        {
            return Err(rejected(
                index,
                "cohort original native mapping association",
            ));
        }
        let mut expected: Image = input
            .kernarg_bytes
            .as_ref()
            .try_into()
            .map_err(|_| rejected(index, "cohort member kernarg shape"))?;
        expected[..8].copy_from_slice(&address.to_le_bytes());
        match (
            &plan.programs[index].implicit_kernarg,
            plan.packets[index].implicit_kernarg,
        ) {
            (Some(layout), Some(values)) => {
                initialize_cov6_implicit_kernarg(&mut expected, layout, values)
            }
            _ => return Err(rejected(index, "cohort implicit argument plan")),
        }
        if self.images[index] != expected {
            return Err(rejected(index, "cohort exact patched member image"));
        }
        let _checked = model
            .check_dispatch(
                expected[..16].try_into().unwrap(),
                kernarg_address,
                address,
                output_bytes,
                packet.geometry.grid(),
                packet.geometry.workgroup().map(u32::from),
            )
            .map_err(|_| rejected(index, "cohort native member spatial premises"))?;
        self.members.push(Member {
            code: code_identity,
            code_facts: *code.facts(),
            code_storage: code.storage_identity(),
            kernarg_facts: *kernarg.facts(),
            kernarg_storage: kernarg.storage_identity(),
            output_facts: *output.facts(),
            output_storage: output.storage_identity(),
            output_bytes,
            packet,
            recipe: generation.recipe_occurrence,
            generation: generation.next_generation,
        });
        Ok(())
    }

    pub(in crate::queue::dispatch_binding) fn revalidate(
        &self,
        owner: &DispatchResourceOwnerV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        let count = self.members.len();
        validate_count(count)?;
        if self.images.len() != count
            || owner.code.len() != count
            || owner.code_identity.len() != count
            || owner.packets.len() != count
            || owner.data.len() != count
            || owner.data_premises.len() != count
            || !matches!(
                owner.persistent_control,
                PersistentFixedDispatchControlStateV1::Ordinary
            )
        {
            return Err(rejected(0, "cohort original aggregate cardinality"));
        }
        for (index, member) in self.members.iter().enumerate() {
            let DispatchDataAuthorityV1::HostVisible(output) = &owner.data[index] else {
                return Err(rejected(index, "cohort original coherent output changed"));
            };
            if owner.code_identity[index] != member.code
                || owner.code[index].facts() != &member.code_facts
                || owner.code[index].storage_identity() != member.code_storage
                || owner.kernarg.facts() != &member.kernarg_facts
                || owner.kernarg.storage_identity() != member.kernarg_storage
                || output.facts() != &member.output_facts
                || output.storage_identity() != member.output_storage
                || owner.packets[index] != member.packet
                || owner.generation.recipe_occurrence != member.recipe
                || owner.generation.next_generation != member.generation
                || owner.data_premises[index].layout.requested_bytes() != member.output_bytes
                || owner.data_premises[index].valid_bytes != member.output_bytes
                || owner.data_premises[index].effect != Some(DeviceDataEffectV1::WriteOnly)
                || owner.data[index].checked_gpu_subrange(0, member.output_bytes, 4)
                    != Some(u64::from_le_bytes(
                        self.images[index][..8].try_into().unwrap(),
                    ))
            {
                return Err(rejected(
                    index,
                    "cohort member owner substitution or replay",
                ));
            }
        }
        // Compare complete mapped ranges, not only equal-sized logical prefixes.
        let facts = |index: usize| -> &SharedGttMappedResourceFactsV1 {
            if index == count * 2 {
                owner.kernarg.facts()
            } else if index < count {
                owner.code[index].facts()
            } else {
                &self.members[index - count].output_facts
            }
        };
        for right in 0..count * 2 + 1 {
            let right_facts = facts(right);
            let right_end = right_facts
                .gpu_va()
                .checked_add(right_facts.gpu_va_bytes())
                .filter(|_| right_facts.gpu_va_bytes() != 0)
                .ok_or_else(|| rejected(right, "cohort mapped range overflow"))?;
            for left in 0..right {
                let left_facts = facts(left);
                let left_end = left_facts
                    .gpu_va()
                    .checked_add(left_facts.gpu_va_bytes())
                    .ok_or_else(|| rejected(left, "cohort mapped range overflow"))?;
                if left_facts.mapping().allocation.vm != right_facts.mapping().allocation.vm
                    || (left_facts.gpu_va() < right_end && right_facts.gpu_va() < left_end)
                {
                    return Err(rejected(
                        right,
                        "cohort mapped owners overlap or use different VMs",
                    ));
                }
            }
        }
        Ok(())
    }
}
