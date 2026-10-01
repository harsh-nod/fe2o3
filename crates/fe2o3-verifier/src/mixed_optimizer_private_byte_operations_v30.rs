//! Physical scalar/typed-object Alloca execution, not a source-frame transition.
use super::byte_function_v30::{ByteAllocationResolverV30, ByteAllocationSiteV30};
use super::byte_memory_v30::ByteMemoryStateNamesV30;
use super::pointer_byte_operations_v30::scalar_bytes;
use super::{Error, Inventory, Result, Writer};
use fe2o3_kernel_ir::{AccessMode, AddressSpace, FormalIndexWidth, OperationKind, Type};
use std::{fmt::Write as _, mem::size_of};

macro_rules! emit {
    ($out:expr, $($arg:tt)*) => { write!($out, $($arg)*).map_err(|_| $out.error())? };
}

#[derive(Clone, Copy)]
pub(super) struct AllocaByteOperationV30 {
    pub result: usize,
    pub extent: usize,
    pub alignment: u32,
    site: ByteAllocationSiteV30,
}

impl AllocaByteOperationV30 {
    pub(super) fn derive(
        inventory: &Inventory<'_>,
        operation: usize,
        width: FormalIndexWidth,
        allocations: &impl ByteAllocationResolverV30,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<Self>> {
        out.budget.charge_work(6)?;
        let row = inventory
            .operations()
            .get(operation)
            .ok_or(Error::Statement("physical byte Alloca coordinate"))?;
        let OperationKind::Alloca {
            element,
            count,
            address_space,
            alignment,
        } = &row.operation.kind
        else {
            return Ok(None);
        };
        out.budget.charge_work(24)?;
        if count.is_some()
            || !row.operands.is_empty()
            || row.results.len() != 1
            || *address_space != AddressSpace::Private
            || !alignment.is_power_of_two()
        {
            return Err(Error::Statement(
                "physical byte Alloca shape is not modeled",
            ));
        }
        let extent = match element {
            Type::Scalar(scalar) => scalar_bytes(*scalar, width)?,
            Type::StorageObject(id) => {
                let layout = inventory
                    .owner()
                    .module()
                    .storage_layouts
                    .get(id.0 as usize)
                    .ok_or(Error::Statement("physical byte Alloca layout is absent"))?;
                if *alignment < layout.alignment || !layout.alignment.is_power_of_two() {
                    return Err(Error::Statement(
                        "physical byte Alloca layout alignment differs",
                    ));
                }
                usize::try_from(layout.size).map_err(|_| super::Resource::Arithmetic)?
            }
            _ => {
                return Err(Error::Statement(
                    "physical byte Alloca layout is not modeled",
                ));
            }
        };
        let result = row.results.start;
        let ty = inventory
            .definitions()
            .get(result)
            .ok_or(Error::Statement("physical byte Alloca result coordinate"))?
            .ty;
        if !matches!(ty, Type::Pointer(pointer) if pointer.pointee.as_ref() == element
            && pointer.address_space == AddressSpace::Private && pointer.access == AccessMode::ReadWrite)
        {
            return Err(Error::Statement("physical byte Alloca result type differs"));
        }
        allocations.check_owner(inventory.owner(), out)?;
        let site = allocations.site(row.coordinate, out)?;
        Ok(Some(Self {
            result,
            extent,
            alignment: *alignment,
            site,
        }))
    }

    pub(super) fn emit_step(
        &self,
        before: ByteMemoryStateNamesV30<'_>,
        after: ByteMemoryStateNamesV30<'_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        out.budget.charge_work(1)?;
        let operation = self.site.original;
        let owner = self.site.physical_root_owner;
        let values = before.values;
        let memory = before.memory;
        let generations = before.generations;
        let frames = before.frames;
        let valid = before.valid;
        let result = self.result;
        let extent = self.extent;
        let alignment = self.alignment;
        emit!(
            out,
            " let allocation_site_v30 = MemoryPrivateSiteV30 {{ owner: {owner}, invocation: 0, site: MemorySourceOperationV30 {{ function: {}, block: {}, operation: {} }} }};\n",
            operation.block.function.0,
            operation.block.block,
            operation.operation
        );
        emit!(
            out,
            " let allocation_generation_v30 = private_generation_v30({generations}, allocation_site_v30);\n let allocation_v30 = private_allocation_v30(allocation_site_v30, allocation_generation_v30);\n"
        );
        // Inlined callee entry/return annotations do not execute an allocation
        // or change its physical frame. Repeated execution advances only this
        // Alloca site's generation; it never retires older generations.
        emit!(
            out,
            " let {} = {valid} && byte_frame_runtime_well_formed_v30({frames}) && {frames}.active.len() == 1 && {frames}.active[0] == MemoryDynamicFrameV30 {{ owner: {owner}, invocation: 0 }} && private_generation_counters_valid_v30({generations}, {memory}) && 0 <= allocation_generation_v30 && !{memory}.live.contains_key(allocation_v30);\n",
            after.valid
        );
        emit!(
            out,
            " let {} = if {} {{ byte_allocate_v30({memory}, allocation_v30, {extent}, {alignment}) }} else {{ {memory} }};\n",
            after.memory,
            after.valid
        );
        emit!(
            out,
            " let {} = if {} {{ {generations}.insert(allocation_site_v30, allocation_generation_v30 + 1) }} else {{ {generations} }};\n",
            after.generations,
            after.valid
        );
        emit!(
            out,
            " let {} = {values}.update({result}, if {} {{ MemoryValueV30::Pointer(MemoryPointerV30 {{ allocation: allocation_v30, byte_offset: 0, view: None }}) }} else {{ MemoryValueV30::Undefined }});\n let {} = {frames};\n",
            after.values,
            after.valid,
            after.frames
        );
        Ok(())
    }
}

pub(super) fn headers() -> usize {
    2 * size_of::<AllocaByteOperationV30>()
        + 2 * size_of::<Result<Option<AllocaByteOperationV30>>>()
        + 2 * size_of::<ByteMemoryStateNamesV30<'_>>()
        + size_of::<ByteAllocationSiteV30>()
        + size_of::<FormalIndexWidth>()
        + size_of::<([usize; 16], [&(); 12], [Result<()>; 3])>()
}
