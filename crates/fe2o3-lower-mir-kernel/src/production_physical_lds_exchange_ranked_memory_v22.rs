//! Exact typed LDS rows joined to the unchanged canonical graph. Ranked memory
//! models access/publication safety; pending completion remains in the complete
//! typed canonical/formal report and is never manufactured by a barrier.
use super::*;
use dialect_gpu::{AddressSpaceAttr, HierarchyAttr, MemoryOrderAttr, MemoryScopeAttr};
use fe2o3_kernel_ir::{
    AddressSpace, FormalMemoryAccessKind, FunctionOperationLocation as Location,
    Gfx942PhysicalLdsExchangeFrameV1 as Frame, Gfx942PhysicalLdsExchangeStepV1 as Step,
    MemoryOrdering, SynchronizationScope,
};

pub(super) fn required_conditions(report: &PhysicalLdsExchangeMemoryObligationsV22) -> bool {
    let frame = report.lds_frame();
    let write = report.lds_write();
    let read = report.lds_read();
    let publication = report.publication();
    let runtime = report.runtime_requirements();
    frame.descriptor()
        == Frame {
            byte_offset: 0,
            byte_length: 512,
            alignment: 4,
            publication_epoch: 1,
        }
        && frame.peer_xor_mask() == 64
        && frame.byte_scale() == 4
        && runtime.required_workgroup() == [128, 1, 1]
        && runtime.required_workgroups() == [1, 1, 1]
        && runtime.requires_all_workgroup_invocations()
        && write.kind() == FormalMemoryAccessKind::Write
        && read.kind() == FormalMemoryAccessKind::Read
        && [write, read]
            .iter()
            .all(|r| r.byte_width() == 4 && r.alignment() == 4)
        && write.value() == report.input_read().result()
        && read.value() == report.output_store().value()
        && publication.publication_epoch() == 1
        && publication.participant_count() == 128
        && publication.address_space() == AddressSpace::Workgroup
        && publication.memory_scope() == SynchronizationScope::Workgroup
        && publication.ordering() == MemoryOrdering::AcquireRelease
        && !publication.completes_pending_accesses()
        && !publication.grants_global_happens_before()
        && report.input_read().ready_at().operation_index < write.location().operation_index
        && write.location().operation_index < write.complete_at().operation_index
        && write.complete_at().operation_index < publication.location().operation_index
        && publication.location().operation_index < read.location().operation_index
        && read.location().operation_index < read.complete_at().operation_index
        && read.complete_at().operation_index
            < report.output_store().access().location().operation_index
}
pub(super) fn setup(ops: &mut Operations) -> Result<(), PhysicalLdsExchangeAuxErrorV22> {
    // A real compiler-owned workgroup frame, NOT a third logical argument or a
    // conditional host allocation. Its extent is derived from the typed frame.
    ops.push(OpR::ViewInSpace {
        result: LDS,
        element_width: 32,
        writable: true,
        shape: copy(&[128])?,
        dynamic_extents: Vec::new(),
        memory_space: dialect_kernel::MemorySpaceAttr::Workgroup,
        allocation_origin: 4,
        noalias_class: 4,
    })
}
fn step_at<'a>(
    block: &'a fe2o3_kernel_ir::BasicBlock,
    location: Location,
    site: fe2o3_kernel_ir::Gfx942PhysicalEntrySourceSiteVNext,
    opcode: Opcode,
) -> Result<(&'a fe2o3_kernel_ir::Operation, Step), PhysicalLdsExchangeAuxErrorV22> {
    if location.block != block.id {
        return Err(invalid("LDS ranked report block differs"));
    }
    let op = block
        .operations
        .get(location.operation_index)
        .ok_or_else(|| invalid("LDS ranked report operation absent"))?;
    let Op::Gfx942PhysicalLdsExchangeStep(step) = op.kind else {
        return Err(invalid("LDS ranked report operation kind differs"));
    };
    if step.site != site || step.instruction.opcode != opcode {
        return Err(invalid("LDS ranked actual source site or opcode differs"));
    }
    Ok((op, step))
}
pub(super) fn join_actual_rows(
    owner: &VerifiedCanonicalKernelIrModuleV22,
    report: &PhysicalLdsExchangeMemoryObligationsV22,
) -> Result<(), PhysicalLdsExchangeAuxErrorV22> {
    let block = &owner.module().functions[0]
        .body
        .as_ref()
        .ok_or_else(|| invalid("LDS ranked actual body absent"))?
        .blocks[0];
    let entry = &block.operations[0];
    let Op::Gfx942PhysicalLdsExchangeDeclaration(declaration) = entry.kind else {
        return Err(invalid("LDS ranked actual declaration differs"));
    };
    let frame = report.lds_frame();
    if frame.declaration() != Location::new(block.id, 0)
        || frame.source_site() != declaration.begin_site
        || frame.descriptor() != declaration.lds_frame
        || entry.results.get(3).map(|v| v.id) != Some(frame.local_x())
    {
        return Err(invalid("LDS ranked actual frame or local-X differs"));
    }
    for read in report.kernarg_reads() {
        let (op, _) = step_at(
            block,
            read.ready_at(),
            read.ready_source_site(),
            Opcode::WaitLgkm0,
        )?;
        if !op.results.is_empty() {
            return Err(invalid("LDS ranked kernarg wait defines data"));
        }
    }
    let input = report.input_read();
    let write = report.lds_write();
    let read = report.lds_read();
    let store = report.output_store();
    for (location, site, opcode) in [
        (input.ready_at(), input.ready_source_site(), Opcode::WaitVm0),
        (
            write.complete_at(),
            write.complete_source_site(),
            Opcode::WaitLgkm0,
        ),
        (
            read.complete_at(),
            read.complete_source_site(),
            Opcode::WaitLgkm0,
        ),
        (store.ready_at(), store.ready_source_site(), Opcode::WaitVm0),
        (
            store.restore_at(),
            store.restore_source_site(),
            Opcode::RestoreExec,
        ),
        (
            store.mask_at(),
            store.mask_source_site(),
            Opcode::SaveAndMaskExec,
        ),
        (
            store.comparison_at(),
            store.comparison_source_site(),
            Opcode::VectorCompareGtU64,
        ),
    ] {
        step_at(block, location, site, opcode)?;
    }
    let full_exec = entry
        .results
        .get(4)
        .map(|v| v.id)
        .ok_or_else(|| invalid("LDS ranked actual full EXEC absent"))?;
    if write.exec() != full_exec || read.exec() != full_exec {
        return Err(invalid("LDS ranked full-workgroup participation differs"));
    }
    Ok(())
}
#[derive(Default)]
pub(super) struct Census {
    write: bool,
    publication: bool,
    read: bool,
    // Exact source byte-address SSA -> (retained byte arithmetic, word index).
    // Both rows come only from the actual shift-by2 instructions below.
    addresses: [Option<(ValueId, ValueR, ValueR)>; 2],
}
impl Census {
    pub(super) fn complete(&self) -> bool {
        self.write && self.publication && self.read && self.addresses.iter().all(Option::is_some)
    }
    pub(super) fn record_address(
        &mut self,
        source: ValueId,
        byte_value: ValueR,
        element_index: ValueR,
    ) -> Result<(), PhysicalLdsExchangeAuxErrorV22> {
        if self
            .addresses
            .iter()
            .flatten()
            .any(|(id, _, _)| *id == source)
        {
            return Err(invalid("LDS ranked duplicate byte-address relation"));
        }
        let row = self
            .addresses
            .iter_mut()
            .find(|row| row.is_none())
            .ok_or_else(|| invalid("LDS ranked byte-address relation bound"))?;
        *row = Some((source, byte_value, element_index));
        Ok(())
    }
    pub(super) fn address_index(
        &self,
        source: ValueId,
        byte_value: ValueR,
    ) -> Result<ValueR, PhysicalLdsExchangeAuxErrorV22> {
        self.addresses
            .iter()
            .flatten()
            .find_map(|(id, bytes, index)| {
                (*id == source && *bytes == byte_value).then_some(*index)
            })
            .ok_or_else(|| invalid("LDS ranked byte-address relation absent or changed"))
    }
}
pub(super) struct Projection<'a> {
    pub(super) formal: &'a PhysicalLdsExchangeMemoryObligationsV22,
    pub(super) values: &'a Values,
    pub(super) ops: &'a mut Operations,
    pub(super) special: &'a mut [Option<ValueR>; 4],
    pub(super) census: &'a mut Census,
}
impl Projection<'_> {
    pub(super) fn project(
        self,
        step: Step,
        operation: &fe2o3_kernel_ir::Operation,
        location: Location,
    ) -> Result<(), PhysicalLdsExchangeAuxErrorV22> {
        let Self {
            formal,
            values,
            ops,
            special,
            census,
        } = self;
        match step.instruction.opcode {
            Opcode::VectorLshlrev32 => {
                if step.instruction.immediate != 2 {
                    return Err(invalid("LDS ranked byte shift differs"));
                }
                let source = values.get(
                    step.operands[0].ok_or_else(|| invalid("LDS ranked shift source absent"))?,
                )?;
                let [result] = operation.results.as_slice() else {
                    return Err(invalid("LDS ranked byte-address result roster"));
                };
                if result.id != formal.lds_write().address()
                    && result.id != formal.lds_read().address()
                {
                    return Err(invalid(
                        "LDS ranked byte-address not an actual memory operand",
                    ));
                }
                let scale = ops.constant(4)?;
                let bytes = ops.binary(Binary::Multiply, source, scale)?;
                special[0] = Some(bytes);
                // The verified zero-base u32 frame uses byte_address=index*4.
                // Keep that actual byte expression, but project the exact source
                // index instead of asking bounds analysis to invert a product.
                // Canonical/formal prove index0..127, so no shift overflow exists.
                census.record_address(result.id, bytes, source)?;
            }
            Opcode::VectorXor32 => {
                if step.instruction.immediate != 64
                    || step.operands[0] != Some(formal.lds_frame().local_x())
                {
                    return Err(invalid("LDS ranked peer mapping differs"));
                }
                // For the independently verified localX in 0..128, xor64 equals
                // (localX +64)%128 exactly. This computes an address, never data.
                let local = values.get(formal.lds_frame().local_x())?;
                let half = ops.constant(64)?;
                let count = ops.constant(128)?;
                let sum = ops.binary(Binary::Add, local, half)?;
                special[0] = Some(ops.binary(Binary::Remainder, sum, count)?);
            }
            Opcode::LdsWriteB32 => {
                let row = formal.lds_write();
                if census.write
                    || census.publication
                    || census.read
                    || row.location() != location
                    || row.source_site() != step.site
                    || step.operands
                        != [
                            Some(row.address()),
                            Some(row.value()),
                            Some(row.exec()),
                            None,
                            None,
                        ]
                    || !operation.results.is_empty()
                {
                    return Err(invalid("LDS ranked actual write join"));
                }
                let address = values.get(row.address())?;
                let _data = values.get(row.value())?;
                let index = census.address_index(row.address(), address)?;
                ops.push(OpR::Access {
                    kind: Access::Write,
                    view: ValueR::Local(LDS),
                    indices: copy(&[index])?,
                })?;
                census.write = true;
            }
            Opcode::WorkgroupPublishBarrier => {
                let row = formal.publication();
                if !census.write
                    || census.publication
                    || census.read
                    || row.location() != location
                    || row.source_site() != step.site
                    || !operation.results.is_empty()
                    || step.operands.iter().any(Option::is_some)
                {
                    return Err(invalid("LDS ranked actual publication join"));
                }
                ops.push(OpR::Barrier {
                    execution_scope: HierarchyAttr::Workgroup,
                    memory_scope: MemoryScopeAttr::Workgroup,
                    address_space: AddressSpaceAttr::Workgroup,
                    order: MemoryOrderAttr::AcquireRelease,
                })?;
                census.publication = true;
            }
            Opcode::LdsReadB32 => {
                let row = formal.lds_read();
                if !census.write
                    || !census.publication
                    || census.read
                    || row.location() != location
                    || row.source_site() != step.site
                    || step.operands != [Some(row.address()), Some(row.exec()), None, None, None]
                    || operation.results.len() != 1
                    || operation.results.first().map(|r| r.id) != Some(row.value())
                {
                    return Err(invalid("LDS ranked actual peer read join"));
                }
                let address = values.get(row.address())?;
                let index = census.address_index(row.address(), address)?;
                ops.push(OpR::Access {
                    kind: Access::Read,
                    view: ValueR::Local(LDS),
                    indices: copy(&[index])?,
                })?;
                // Different invocation's memory data is not a deterministic address
                // expression. Readiness is the same SSA at the retained actual wait.
                special[0] = Some(ops.unknown()?);
                census.read = true;
            }
            _ => return Err(invalid("LDS ranked closed memory projector opcode")),
        }
        Ok(())
    }
}
