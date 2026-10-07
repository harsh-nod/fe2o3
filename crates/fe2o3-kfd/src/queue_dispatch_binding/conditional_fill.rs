//! Additional spatial/storage premises, never compiler or launch authority.

use super::*;
use crate::shared_memory::SharedGttMappedResourceFactsV1;
use fe2o3_kernel_analysis::Gfx942FillKernelV1;

pub(super) const KERNARG_BYTES: usize = 272;

/// Allocated before native effects; ordinary queues pay only an optional pointer.
#[derive(Default)]
pub(super) struct ConditionalFillStorageV1 {
    pub(super) kernarg: Option<[u8; KERNARG_BYTES]>,
    pub(super) premises: Option<PreparedConditionalFillPremisesV1>,
}

impl ConditionalFillStorageV1 {
    pub(super) fn revalidate(
        &self,
        owner: &DispatchResourceOwnerV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.premises
            .as_ref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?
            .revalidate(owner)
    }
}

fn rejected(detail: &'static str) -> Gfx942DispatchBindingErrorV1 {
    Gfx942DispatchBindingErrorV1::InvalidKernarg { packet: 0, detail }
}

pub(super) fn check_plan<'a, const N: usize>(
    programs: &[ValidatedKernelEnvelope<'a>],
    packets: &[Gfx942FixedDispatchPacketV1; N],
    plan: &FixedDispatchPreparationPlanV1,
    control: PersistentFixedDispatchControlStateV1,
) -> Result<Option<Gfx942FillKernelV1<'a>>, Gfx942DispatchBindingErrorV1> {
    if !packets.iter().any(|packet| packet.conditional_fill) {
        return Ok(None);
    }
    if N != 1
        || programs.len() != 1
        || plan.data.len() != 1
        || !matches!(control, PersistentFixedDispatchControlStateV1::Ordinary)
    {
        return Err(rejected(
            "conditional fill requires one ordinary program, packet and output",
        ));
    }
    let input = &packets[0];
    let data = &plan.data[0];
    let [binding] = input.buffers.as_ref() else {
        return Err(rejected("conditional fill output cardinality"));
    };
    if input.program_index != 0
        || input.kernarg_bytes.len() != KERNARG_BYTES
        || input.dynamic_group_segment_bytes != 0
        || binding.explicit_argument_index != 0
        || binding.data_index != 0
        || binding.data_byte_offset != 0
        || binding.byte_len == 0
        || binding.byte_len != data.layout.requested_bytes()
        || data.layout.kind() != Gfx942FixedDispatchDataKindV1::HostVisibleCoherent
        || binding.completed_snapshot.is_some()
        || data.effect != Some(DeviceDataEffectV1::WriteOnly)
    {
        return Err(rejected(
            "conditional fill whole write-only output or kernarg shape",
        ));
    }
    let count = u64::from_le_bytes(input.kernarg_bytes[8..16].try_into().unwrap());
    let grid = input.geometry.grid();
    if count.checked_mul(4) != Some(binding.byte_len)
        || input.geometry.workgroup() != [64, 1, 1]
        || grid[0] == 0
        || !grid[0].is_multiple_of(64)
        || grid[1..] != [1, 1]
        || count > u64::from(grid[0])
    {
        return Err(rejected("conditional fill full64 coverage"));
    }
    let kernel = &programs[0];
    let model =
        Gfx942FillKernelV1::inspect(kernel.envelope().bytes(), kernel.selected_kernel_index())
            .map_err(|_| rejected("conditional fill exact machine profile"))?;
    if model.binding() != kernel.selected_binding()
        || model.kernarg_storage_bytes() != KERNARG_BYTES as u64
    {
        return Err(rejected("conditional fill selected descriptor"));
    }
    Ok(Some(model))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OutputFacts {
    storage: SharedGttAllocationIdentityV1,
    mapped: SharedGttMappedResourceFactsV1,
}

impl OutputFacts {
    fn from_authority(data: &DispatchDataAuthorityV1) -> Option<Self> {
        let DispatchDataAuthorityV1::HostVisible(data) = data else {
            return None;
        };
        Some(Self {
            storage: data.storage_identity(),
            mapped: *data.facts(),
        })
    }
}

/// A snapshot alongside original, inaccessible resource owners, not an exportable permit.
pub(super) struct PreparedConditionalFillPremisesV1 {
    code: ResolvedCodeIdentityV1,
    code_facts: SharedGttMappedResourceFactsV1,
    code_storage: SharedGttAllocationIdentityV1,
    packet: PreparedDispatchPacketV1,
    kernarg_facts: SharedGttMappedResourceFactsV1,
    kernarg_storage: SharedGttAllocationIdentityV1,
    kernarg_image: [u8; KERNARG_BYTES],
    output: OutputFacts,
    output_bytes: u64,
    recipe_occurrence: u64,
    generation: u64,
}

pub(super) struct NativeFillCustodyV1<'a> {
    pub code: &'a CodeAuthority,
    pub code_identity: ResolvedCodeIdentityV1,
    pub kernarg: &'a KernargAuthority,
    pub output: &'a DispatchDataAuthorityV1,
    pub generation: &'a DispatchGenerationOwnerV1,
}

impl PreparedConditionalFillPremisesV1 {
    pub(super) fn check(
        model: &Gfx942FillKernelV1<'_>,
        input: &Gfx942FixedDispatchPacketV1,
        plan: &FixedDispatchPreparationPlanV1,
        custody: NativeFillCustodyV1<'_>,
        image: [u8; KERNARG_BYTES],
        packet: PreparedDispatchPacketV1,
    ) -> Result<Self, Gfx942DispatchBindingErrorV1> {
        let NativeFillCustodyV1 {
            code,
            code_identity,
            kernarg,
            output,
            generation,
        } = custody;
        let vm = kernarg.facts().mapping().allocation.vm;
        let output_facts = OutputFacts::from_authority(output)
            .ok_or_else(|| rejected("conditional fill requires coherent host output"))?;
        let output_bytes = plan.data[0].layout.requested_bytes();
        let retained_output_bytes = match output {
            DispatchDataAuthorityV1::Device(output) => output.layout().requested_bytes(),
            DispatchDataAuthorityV1::HostVisible(output) => {
                output.layout().requested_bytes() as u64
            }
        };
        if kernarg.layout().requested_bytes() != KERNARG_BYTES
            || kernarg.facts().logical_bytes() != KERNARG_BYTES
            || retained_output_bytes != output_bytes
        {
            return Err(rejected("conditional fill logical allocation extents"));
        }
        let address = output
            .checked_gpu_subrange(0, output_bytes, 4)
            .ok_or_else(|| rejected("conditional fill retained output range"))?;
        let kernarg_address = kernarg
            .facts()
            .checked_gpu_subrange(0, KERNARG_BYTES as u64, 8)
            .ok_or_else(|| rejected("conditional fill complete kernarg range"))?;
        if output.vm() != vm
            || !code
                .storage_identity()
                .same_retained_session_v1(kernarg.storage_identity())
            || matches!(output, DispatchDataAuthorityV1::HostVisible(data)
                if !data.storage_identity().same_retained_session_v1(kernarg.storage_identity()))
            || code.facts().mapping().allocation.vm != vm
            || code_identity.mapping != code.facts().mapping()
            || packet.kernarg_mapping != kernarg.facts().mapping()
            || packet.kernarg_address.raw() != kernarg_address
            || plan.packets[0].kernarg_offset != 0
        {
            return Err(rejected("conditional fill native mapping association"));
        }
        let mut expected: [u8; KERNARG_BYTES] = input
            .kernarg_bytes
            .as_ref()
            .try_into()
            .map_err(|_| rejected("conditional fill complete kernarg image"))?;
        expected[..8].copy_from_slice(&address.to_le_bytes());
        match (
            &plan.programs[0].implicit_kernarg,
            plan.packets[0].implicit_kernarg,
        ) {
            (Some(layout), Some(values)) => {
                initialize_cov6_implicit_kernarg(&mut expected, layout, values)
            }
            _ => return Err(rejected("conditional fill implicit argument plan")),
        }
        if image != expected {
            return Err(rejected("conditional fill patched kernarg image"));
        }
        let _dispatch = model
            .check_dispatch(
                image[..16].try_into().unwrap(),
                kernarg_address,
                address,
                output_bytes,
                packet.geometry.grid(),
                packet.geometry.workgroup().map(u32::from),
            )
            .map_err(|_| rejected("conditional fill native spatial premises"))?;
        Ok(Self {
            code: code_identity,
            code_facts: *code.facts(),
            code_storage: code.storage_identity(),
            packet,
            kernarg_facts: *kernarg.facts(),
            kernarg_storage: kernarg.storage_identity(),
            kernarg_image: image,
            output: output_facts,
            output_bytes,
            recipe_occurrence: generation.recipe_occurrence,
            generation: generation.next_generation,
        })
    }

    pub(super) fn revalidate(
        &self,
        owner: &DispatchResourceOwnerV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        if owner.code.len() != 1
            || owner.code_identity.as_slice() != [self.code]
            || owner.packets.as_slice() != [self.packet]
            || owner.data.len() != 1
            || owner.data_premises.len() != 1
            || !matches!(
                owner.persistent_control,
                PersistentFixedDispatchControlStateV1::Ordinary
            )
            || owner.generation.recipe_occurrence != self.recipe_occurrence
            || owner.generation.next_generation != self.generation
            || owner.code[0].facts() != &self.code_facts
            || owner.code[0].storage_identity() != self.code_storage
            || owner.kernarg.facts() != &self.kernarg_facts
            || owner.kernarg.storage_identity() != self.kernarg_storage
            || OutputFacts::from_authority(&owner.data[0]) != Some(self.output)
            || owner.data_premises[0].layout.requested_bytes() != self.output_bytes
            || owner.data_premises[0].valid_bytes != self.output_bytes
            || owner.data_premises[0].effect != Some(DeviceDataEffectV1::WriteOnly)
            || owner.data[0].checked_gpu_subrange(0, self.output_bytes, 4)
                != Some(u64::from_le_bytes(
                    self.kernarg_image[..8].try_into().unwrap(),
                ))
        {
            return Err(rejected(
                "conditional fill prepared owner substitution or replay",
            ));
        }
        Ok(())
    }
}
