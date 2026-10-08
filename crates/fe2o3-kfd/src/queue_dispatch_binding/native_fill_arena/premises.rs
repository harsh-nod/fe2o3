//! Private retained range certificate. No suballocation authority is created.

use super::*;
use crate::shared_memory::SharedGttMappedResourceFactsV1;

type Image = [u8; conditional_fill::KERNARG_BYTES];

#[derive(Clone, Copy)]
struct Slot {
    image: Image,
    packet: Option<PreparedDispatchPacketV1>,
    range: CompletedWritableRangeV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Root {
    capacity: ArenaCapacityV1,
    order: ArenaOrderV1,
    code: ResolvedCodeIdentityV1,
    code_facts: SharedGttMappedResourceFactsV1,
    code_storage: SharedGttAllocationIdentityV1,
    kernarg_facts: SharedGttMappedResourceFactsV1,
    kernarg_storage: SharedGttAllocationIdentityV1,
    output_facts: SharedGttMappedResourceFactsV1,
    output_storage: SharedGttAllocationIdentityV1,
    output_bytes: u64,
    recipe: u64,
    generation: u64,
}

pub(in crate::queue) struct ArenaPremisesV1 {
    pub(in crate::queue) capacity: ArenaCapacityV1,
    pub(in crate::queue) order: ArenaOrderV1,
    slots: HostMetadataTableV1<Slot>,
    root: Option<Root>,
    checked: usize,
    captured: bool,
}

impl ArenaPremisesV1 {
    pub(super) fn preallocate(
        account: &ResourceCreditAccountV1,
    ) -> Result<Self, Gfx942DispatchBindingErrorV1> {
        Self::preallocate_with_capacity(account, ArenaCapacityV1::Original1024)
    }

    pub(super) fn preallocate_with_capacity(
        account: &ResourceCreditAccountV1,
        capacity: ArenaCapacityV1,
    ) -> Result<Self, Gfx942DispatchBindingErrorV1> {
        let slots = HostMetadataTableV1::try_new(capacity.slots(), Some(account), || Slot {
            image: [0; conditional_fill::KERNARG_BYTES],
            packet: None,
            range: CompletedWritableRangeV1 {
                offset: 0,
                byte_len: 0,
            },
        })
        .map_err(|_| rejected(0, "arena retained slot capacity"))?;
        Ok(Self {
            capacity,
            order: ArenaOrderV1::Ordered,
            slots,
            root: None,
            checked: 0,
            captured: false,
        })
    }

    pub(in crate::queue::dispatch_binding) fn capture(
        &mut self,
        bytes: &[u8],
        plan: &FixedDispatchPreparationPlanV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        if self.captured
            || self.checked != 0
            || self.root.is_some()
            || !self.capacity.permits(self.order)
            || self.slots.len() != self.capacity.slots()
            || plan.packets.len() != self.capacity.slots()
            || bytes.len() != self.capacity.slots() * conditional_fill::KERNARG_BYTES
        {
            return Err(rejected(0, "arena fresh exact kernarg image"));
        }
        for (index, slot) in self.slots.iter_mut().enumerate() {
            let offset = index * conditional_fill::KERNARG_BYTES;
            if plan.packets[index].kernarg_offset != offset {
                return Err(rejected(index, "arena kernarg slot order"));
            }
            slot.image
                .copy_from_slice(&bytes[offset..offset + conditional_fill::KERNARG_BYTES]);
        }
        self.captured = true;
        Ok(())
    }

    pub(in crate::queue::dispatch_binding) fn check_member(
        &mut self,
        index: usize,
        model: &Gfx942FillKernelV1<'_>,
        input: &Gfx942FixedDispatchPacketV1,
        plan: &FixedDispatchPreparationPlanV1,
        custody: conditional_fill::NativeFillCustodyV1<'_>,
        packet: PreparedDispatchPacketV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        if !self.captured
            || index != self.checked
            || index >= self.capacity.slots()
            || self.slots.len() != self.capacity.slots()
        {
            return Err(rejected(index, "arena checked slot order"));
        }
        let conditional_fill::NativeFillCustodyV1 {
            code,
            code_identity,
            kernarg,
            output,
            generation,
        } = custody;
        let DispatchDataAuthorityV1::HostVisible(output) = output else {
            return Err(rejected(index, "arena original coherent output"));
        };
        let [binding] = input.buffers.as_ref() else {
            return Err(rejected(index, "arena exact slot binding"));
        };
        let range = CompletedWritableRangeV1 {
            offset: binding.data_byte_offset,
            byte_len: binding.byte_len,
        };
        let address = output
            .facts()
            .checked_gpu_subrange(range.offset, range.byte_len, 4)
            .ok_or_else(|| rejected(index, "arena native output subrange"))?;
        let kernarg_address = kernarg
            .facts()
            .checked_gpu_subrange(
                (index * conditional_fill::KERNARG_BYTES) as u64,
                conditional_fill::KERNARG_BYTES as u64,
                8,
            )
            .ok_or_else(|| rejected(index, "arena native kernarg subrange"))?;
        let root = Root {
            capacity: self.capacity,
            order: self.order,
            code: code_identity,
            code_facts: *code.facts(),
            code_storage: code.storage_identity(),
            kernarg_facts: *kernarg.facts(),
            kernarg_storage: kernarg.storage_identity(),
            output_facts: *output.facts(),
            output_storage: output.storage_identity(),
            output_bytes: plan.data[0].layout.requested_bytes(),
            recipe: generation.recipe_occurrence,
            generation: generation.next_generation,
        };
        root.check_native()?;
        if root.output_bytes != output.layout().requested_bytes() as u64
            || input.ordering != self.order.packet_order()
            || packet.ordering != self.order.packet_order()
            || kernarg.layout().requested_bytes() != plan.kernarg_arena_bytes
            || plan.data[0].writable_ranges[index] != range
            || packet.code_index != 0
            || !packet.conditional_fill
            || packet.kernarg_address.raw() != kernarg_address
            || packet.kernarg_mapping != root.kernarg_facts.mapping()
            || self.root.is_some_and(|original| original != root)
        {
            return Err(rejected(index, "arena original native root association"));
        }
        let mut expected: Image = input
            .kernarg_bytes
            .as_ref()
            .try_into()
            .map_err(|_| rejected(index, "arena complete kernarg shape"))?;
        expected[..8].copy_from_slice(&address.to_le_bytes());
        match (
            &plan.programs[0].implicit_kernarg,
            plan.packets[index].implicit_kernarg,
        ) {
            (Some(layout), Some(values)) => {
                initialize_cov6_implicit_kernarg(&mut expected, layout, values)
            }
            _ => return Err(rejected(index, "arena exact implicit kernarg plan")),
        }
        if self.slots[index].image != expected {
            return Err(rejected(index, "arena original patched kernarg image"));
        }
        let _checked = model
            .check_dispatch(
                expected[..16].try_into().unwrap(),
                kernarg_address,
                address,
                range.byte_len,
                packet.geometry.grid(),
                packet.geometry.workgroup().map(u32::from),
            )
            .map_err(|_| rejected(index, "arena actual slot machine premises"))?;
        self.root = Some(root);
        self.slots[index].range = range;
        self.slots[index].packet = Some(packet);
        self.checked += 1;
        Ok(())
    }

    fn require_root(
        &self,
        owner: &DispatchResourceOwnerV1,
    ) -> Result<Root, Gfx942DispatchBindingErrorV1> {
        let root = self
            .root
            .ok_or_else(|| rejected(0, "arena absent original root"))?;
        if !self.captured
            || root.capacity != self.capacity
            || !self.capacity.permits(self.order)
            || root.order != self.order
            || self.checked != self.capacity.slots()
            || self.slots.len() != self.capacity.slots()
            || owner.code.len() != 1
            || owner.code_identity.len() != 1
            || owner.data.len() != 1
            || owner.data_premises.len() != 1
            || owner.packets.len() != self.capacity.slots()
            || !matches!(
                owner.persistent_control,
                PersistentFixedDispatchControlStateV1::Ordinary
            )
        {
            return Err(rejected(0, "arena complete retained roster"));
        }
        owner.generation.ensure_pristine()?;
        let DispatchDataAuthorityV1::HostVisible(output) = &owner.data[0] else {
            return Err(rejected(0, "arena changed coherent DATA family"));
        };
        let data = &owner.data_premises[0];
        if owner.code_identity[0] != root.code
            || owner.code[0].facts() != &root.code_facts
            || owner.code[0].storage_identity() != root.code_storage
            || owner.kernarg.facts() != &root.kernarg_facts
            || owner.kernarg.storage_identity() != root.kernarg_storage
            || output.facts() != &root.output_facts
            || output.storage_identity() != root.output_storage
            || owner.generation.recipe_occurrence != root.recipe
            || owner.generation.next_generation != root.generation
            || data.layout.requested_bytes() != root.output_bytes
            || data.valid_bytes != root.output_bytes
            || data.layout.kind() != Gfx942FixedDispatchDataKindV1::HostVisibleCoherent
            || data.effect != Some(DeviceDataEffectV1::WriteOnly)
            || data.writable_ranges.len() != self.capacity.slots()
            || !data.completed_snapshots.is_empty()
        {
            return Err(rejected(0, "arena original common owner substitution"));
        }
        Ok(root)
    }

    /// Constant-roster-time check, callable only while the private queue keeps
    /// every other immutable slot inaccessible. This is not a generic slice proof.
    pub(in crate::queue::dispatch_binding) fn selected(
        &self,
        owner: &DispatchResourceOwnerV1,
        index: usize,
        queue: Option<QueueKeyV1>,
    ) -> Result<CompletedWritableRangeV1, Gfx942DispatchBindingErrorV1> {
        let root = self.require_root(owner)?;
        let slot = self
            .slots
            .get(index)
            .ok_or_else(|| rejected(index, "arena slot ordinal"))?;
        if slot.packet != owner.packets.get(index).copied()
            || slot.packet.is_none()
            || slot
                .packet
                .is_some_and(|packet| packet.ordering != self.order.packet_order())
            || owner.data_premises[0].writable_ranges[index] != slot.range
            || queue.is_some_and(|q| q.vm != root.code.mapping.allocation.vm)
            || owner.data[0].checked_gpu_subrange(slot.range.offset, slot.range.byte_len, 4)
                != Some(u64::from_le_bytes(slot.image[..8].try_into().unwrap()))
        {
            return Err(rejected(index, "arena selected original slot substitution"));
        }
        Ok(slot.range)
    }

    pub(in crate::queue::dispatch_binding) fn revalidate(
        &self,
        owner: &DispatchResourceOwnerV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.require_root(owner)?.check_native()?;
        let mut end = 0u64;
        for index in 0..self.capacity.slots() {
            let range = self.selected(owner, index, None)?;
            if range.offset != end || range.byte_len == 0 {
                return Err(rejected(index, "arena dense immutable partition"));
            }
            end = end
                .checked_add(range.byte_len)
                .ok_or_else(|| rejected(index, "arena extent overflow"))?;
        }
        if end != owner.data_premises[0].valid_bytes {
            return Err(rejected(0, "arena complete partition extent"));
        }
        Ok(())
    }
}

impl Root {
    fn check_native(&self) -> Result<(), Gfx942DispatchBindingErrorV1> {
        let facts = [self.code_facts, self.kernarg_facts, self.output_facts];
        let vm = self.code.mapping.allocation.vm;
        if !self.capacity.permits(self.order)
            || self.code.mapping != self.code_facts.mapping()
            || !self
                .code_storage
                .same_retained_session_v1(self.kernarg_storage)
            || !self
                .output_storage
                .same_retained_session_v1(self.kernarg_storage)
            || self.kernarg_facts.logical_bytes()
                != self.capacity.slots() * conditional_fill::KERNARG_BYTES
            || self.output_facts.logical_bytes() as u64 != self.output_bytes
        {
            return Err(rejected(0, "arena same original native session"));
        }
        for (index, right) in facts.iter().enumerate() {
            let end = right
                .gpu_va()
                .checked_add(right.gpu_va_bytes())
                .filter(|_| right.gpu_va_bytes() != 0)
                .ok_or_else(|| rejected(index, "arena mapped extent"))?;
            if right.mapping().allocation.vm != vm {
                return Err(rejected(index, "arena native VM mismatch"));
            }
            for left in &facts[..index] {
                let left_end = left
                    .gpu_va()
                    .checked_add(left.gpu_va_bytes())
                    .ok_or_else(|| rejected(index, "arena mapped extent"))?;
                if left.gpu_va() < end && right.gpu_va() < left_end {
                    return Err(rejected(index, "arena CODE/kernarg/DATA overlap"));
                }
            }
        }
        Ok(())
    }
}
