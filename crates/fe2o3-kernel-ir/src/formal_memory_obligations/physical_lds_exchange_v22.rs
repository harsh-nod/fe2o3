//! Exact typed global read/write and kernarg obligations from the actual KIR22
//! owner. No caller effect plan and no second physical-state interpreter.
use super::*;
use crate::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, FunctionBody,
    FunctionOperationLocation as Location, Gfx942PhysicalEntrySourceSiteVNext as Site,
    Gfx942PhysicalLdsExchangeOpcodeV1 as Opcode, Gfx942PhysicalLdsExchangeStepV1 as Step,
    VerifiedCanonicalKernelIrModuleV22, gfx942_physical_lds_exchange_declaration_v22,
};
#[path = "physical_lds_exchange_types_v22.rs"]
mod types;
pub use types::*;
const WORK: usize = 131_072;
const SCRATCH: usize = 64 * 1024;

fn invalid(message: &'static str) -> PhysicalLdsExchangeMemoryErrorV22 {
    PhysicalLdsExchangeMemoryErrorV22::Profile(message)
}
fn retained() -> Result<usize, PhysicalLdsExchangeMemoryErrorV22> {
    // Fixed four ABI reads are inline. Existing affine report routines have
    // two allocations/accesses/bounds and one alias pair; reserve their bounded
    // vector/string payload conservatively. Their old allocation domain is not
    // rebranded as exact heap accounting.
    std::mem::size_of::<PhysicalLdsExchangeMemoryObligationsV22>()
        .checked_add(4096)
        .ok_or(Resource::Arithmetic.into())
}
fn vector<T>(count: usize) -> Result<Vec<T>, PhysicalLdsExchangeMemoryErrorV22> {
    let mut v = Vec::new();
    v.try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    if v.capacity() > count {
        return Err(Resource::Accounting.into());
    }
    Ok(v)
}
/// Derives the entire conditional report from one immutable verified body.
/// Launch is descriptive, not runtime authority. Input is unguarded/full-EXEC;
/// output deliberately retains LaunchEnvelope rather than tail-exact bounds.
/// Exact launch128 is required; both real allocations require512B. Readability, initialized
/// input, write permission, alias and compiler-owned kernarg conditions remain
/// unresolved. Generic formal extraction remains Unmodeled for these opcodes.
///
/// Work uses the same cumulative ledger; incoming storage is restored on all
/// exits. Reserve the returned logical receipt while retaining the report.
pub fn derive_physical_lds_exchange_memory_obligations_v22(
    owner: &VerifiedCanonicalKernelIrModuleV22,
    kernel: &KernelId,
    launch: ExplicitLaunchExtent,
    width: FormalIndexWidth,
    budget: &mut Budget<'_>,
) -> Result<
    (
        PhysicalLdsExchangeMemoryObligationsV22,
        PhysicalLdsExchangeMemoryStorageV22,
    ),
    PhysicalLdsExchangeMemoryErrorV22,
> {
    let retained = retained()?;
    budget.with_prepaid_scope(budget.storage(), 1, WORK, retained + SCRATCH, |_| {
        derive(owner, kernel, launch, width)
            .map(|report| (report, PhysicalLdsExchangeMemoryStorageV22 { retained }))
    })
}
/// Independently reconstructs and compares every typed field, actual source
/// occurrence, SSA identity and unresolved condition. No digest-only shortcut.
/// Caller must already reserve its retained report; temporary replay storage
/// is prepaid here and the incoming floor is never reset.
pub fn validate_physical_lds_exchange_memory_obligations_v22(
    owner: &VerifiedCanonicalKernelIrModuleV22,
    kernel: &KernelId,
    launch: ExplicitLaunchExtent,
    width: FormalIndexWidth,
    report: &PhysicalLdsExchangeMemoryObligationsV22,
    budget: &mut Budget<'_>,
) -> Result<(), PhysicalLdsExchangeMemoryErrorV22> {
    let retained = retained()?;
    budget.with_prepaid_scope(retained, 1, WORK, retained + SCRATCH, |_| {
        if derive(owner, kernel, launch, width)? != *report {
            return Err(invalid("LDS exchange complete memory report changed"));
        }
        Ok(())
    })
}
fn definition(
    body: &FunctionBody,
    value: ValueId,
) -> Result<(Location, &Operation, Step), PhysicalLdsExchangeMemoryErrorV22> {
    body.blocks
        .iter()
        .find_map(|block| {
            block
                .operations
                .iter()
                .enumerate()
                .find(|(_, op)| op.results.iter().any(|r| r.id == value))
                .map(|(index, op)| (Location::new(block.id, index), op))
        })
        .ok_or_else(|| invalid("LDS exchange memory SSA producer missing"))
        .and_then(|(location, operation)| match operation.kind {
            OperationKind::Gfx942PhysicalLdsExchangeStep(step) => Ok((location, operation, step)),
            _ => Err(invalid("LDS exchange memory SSA producer is not a step")),
        })
}
fn following(
    block: &crate::BasicBlock,
    index: usize,
    opcode: Opcode,
) -> Result<(Location, Site), PhysicalLdsExchangeMemoryErrorV22> {
    match block.operations.get(index).map(|op| &op.kind) {
        Some(OperationKind::Gfx942PhysicalLdsExchangeStep(step))
            if step.instruction.opcode == opcode =>
        {
            Ok((Location::new(block.id, index), step.site))
        }
        _ => Err(invalid(
            "LDS exchange actual following wait or restore differs",
        )),
    }
}
fn address_index(
    body: &FunctionBody,
    high: ValueId,
) -> Result<[ValueId; 2], PhysicalLdsExchangeMemoryErrorV22> {
    // Attribute the exact index through the actual SSA displacement producer.
    // Root/half/carry/current-generation semantics are already proved by the
    // immutable canonical owner, not reinvented by this memory summary.
    let (_, high_op, add) = definition(body, high)?;
    if add.instruction.opcode != Opcode::VectorAddCarryIn
        || high_op.results.first().map(|r| r.id) != Some(high)
    {
        return Err(invalid("LDS exchange memory high address producer"));
    }
    let offset = add.operands[1].ok_or_else(|| invalid("LDS exchange high offset absent"))?;
    let (_, shift_op, shift) = definition(body, offset)?;
    if shift.instruction.opcode != Opcode::VectorLshlrev64
        || shift_op.results.get(1).map(|r| r.id) != Some(offset)
    {
        return Err(invalid("LDS exchange exact index displacement producer"));
    }
    match shift.operands {
        [Some(low), Some(high), Some(_), None, None] => Ok([low, high]),
        _ => Err(invalid("LDS exchange displacement operand roster")),
    }
}
fn derive(
    owner: &VerifiedCanonicalKernelIrModuleV22,
    kernel_id: &KernelId,
    launch: ExplicitLaunchExtent,
    width: FormalIndexWidth,
) -> Result<PhysicalLdsExchangeMemoryObligationsV22, PhysicalLdsExchangeMemoryErrorV22> {
    let declaration = gfx942_physical_lds_exchange_declaration_v22(owner)
        .ok_or_else(|| invalid("exact V22 LDS-exchange profile required"))?;
    let [kernel] = owner.module().kernels.as_slice() else {
        return Err(invalid("LDS exchange one kernel"));
    };
    let [function] = owner.module().functions.as_slice() else {
        return Err(invalid("LDS exchange one function"));
    };
    if kernel_id != &kernel.id || kernel.entry != function.id {
        return Err(invalid("LDS exchange actual kernel entry join"));
    }
    if width != FormalIndexWidth::Bits64 {
        return Err(PhysicalLdsExchangeMemoryErrorV22::Incomplete(
            FormalMemoryIncompleteReason::UnsupportedIndexWidth { width },
        ));
    }
    let ExplicitLaunchExtent::Exact {
        rank: 1,
        extents: [count, 1, 1],
    } = launch
    else {
        return Err(invalid("LDS exchange formal launch must be exact rank1"));
    };
    if count != 128
        || declaration.workgroup != [128, 1, 1]
        || declaration.maximum_workgroups != [1, 1, 1]
    {
        return Err(invalid(
            "LDS exchange formal launch exceeds full-workgroup profile",
        ));
    }
    let mut reasons = BTreeSet::new();
    let invocations = resolve_invocations(&kernel.domain, launch, &mut reasons)?;
    if let Some(reason) = reasons.into_iter().next() {
        return Err(PhysicalLdsExchangeMemoryErrorV22::Incomplete(reason));
    }
    let invocations =
        invocations.ok_or_else(|| invalid("LDS exchange invocation universe absent"))?;
    let body = function
        .body
        .as_ref()
        .ok_or_else(|| invalid("LDS exchange body absent"))?;
    let [block] = body.blocks.as_slice() else {
        return Err(invalid("LDS exchange one actual block"));
    };
    let [input, output] = body.parameters.as_slice() else {
        return Err(invalid("LDS exchange two real parameters"));
    };
    if [*input, *output] != declaration.parameters {
        return Err(invalid("LDS exchange actual logical parameter join"));
    }
    let mut allocations = vector(2)?;
    for (index, (value, access)) in [
        (*input, AccessMode::ReadOnly),
        (*output, AccessMode::ReadWrite),
    ]
    .into_iter()
    .enumerate()
    {
        let Some(Type::Slice(slice)) = function.signature.parameters.get(index) else {
            return Err(invalid("LDS exchange actual slice signature"));
        };
        if slice.address_space != AddressSpace::Global
            || slice.access != access
            || *slice.element != Type::Scalar(ScalarType::U32)
        {
            return Err(invalid("LDS exchange source slice permission"));
        }
        allocations.push(FormalAllocationParameter {
            identity: FormalAllocationIdentity {
                parameter_index: index as u32,
            },
            value,
            kind: FormalParameterKind::Slice,
            address_space: AddressSpace::Global,
            access,
        });
    }
    let input_allocation = allocations[0].identity;
    let output_allocation = allocations[1].identity;
    let entry = block
        .operations
        .first()
        .ok_or_else(|| invalid("LDS exchange entry declaration missing"))?;
    let [_, _, _, local_x, full_exec] = entry.results.as_slice() else {
        return Err(invalid("LDS exchange actual entry live-ins"));
    };
    let mut reads = [None; 4];
    let mut read_count = 0;
    let mut global_read = None;
    let mut global_store = None;
    let mut lds_write = None;
    let mut lds_read = None;
    let mut publication = None;
    for (index, operation) in block.operations.iter().enumerate() {
        let location = Location::new(block.id, index);
        let step = match operation.kind {
            OperationKind::Gfx942PhysicalLdsExchangeDeclaration(_) if index == 0 => continue,
            OperationKind::Gfx942PhysicalLdsExchangeStep(step) => step,
            _ => return Err(invalid("LDS exchange unaccounted memory operation")),
        };
        match step.instruction.opcode {
            Opcode::LoadKernargPair => {
                let slot = match step.instruction.immediate {
                    0 => PhysicalLdsExchangeKernargSlotV22::InputPointer,
                    8 => PhysicalLdsExchangeKernargSlotV22::InputLength,
                    16 => PhysicalLdsExchangeKernargSlotV22::OutputPointer,
                    24 => PhysicalLdsExchangeKernargSlotV22::OutputLength,
                    _ => return Err(invalid("LDS exchange kernarg slot range")),
                };
                let [Some(low), Some(high), None, None, None] = step.operands else {
                    return Err(invalid("LDS exchange kernarg base roster"));
                };
                let [a, b] = operation.results.as_slice() else {
                    return Err(invalid("LDS exchange kernarg results"));
                };
                let wait = block.operations[index + 1..]
                    .iter()
                    .enumerate()
                    .find_map(|(relative, op)| match op.kind {
                        OperationKind::Gfx942PhysicalLdsExchangeStep(s)
                            if s.instruction.opcode == Opcode::WaitLgkm0 =>
                        {
                            Some((Location::new(block.id, index + 1 + relative), s.site))
                        }
                        _ => None,
                    })
                    .ok_or_else(|| invalid("LDS exchange kernarg readiness missing"))?;
                let row = reads
                    .get_mut(read_count)
                    .ok_or_else(|| invalid("LDS exchange excessive kernarg reads"))?;
                *row = Some(PhysicalLdsExchangeKernargReadV22 {
                    location,
                    site: step.site,
                    slot,
                    offset: step.instruction.immediate,
                    base: [low, high],
                    results: [a.id, b.id],
                    ready_at: wait.0,
                    ready_site: wait.1,
                });
                read_count += 1;
            }
            Opcode::GlobalLoadDword => {
                let [Some(low), Some(high), Some(exec), None, None] = step.operands else {
                    return Err(invalid("LDS exchange read operand roster"));
                };
                let [result] = operation.results.as_slice() else {
                    return Err(invalid("LDS exchange single pending read result"));
                };
                if exec != full_exec.id {
                    return Err(invalid("LDS exchange read requires actual full EXEC"));
                }
                let ready = following(block, index + 1, Opcode::WaitVm0)?;
                let access = PhysicalLdsExchangeAccessV22 {
                    location,
                    site: step.site,
                    allocation: input_allocation,
                    parameter: *input,
                    address: [low, high],
                    index: address_index(body, high)?,
                    exec,
                };
                if global_read
                    .replace(PhysicalLdsExchangeReadV22 {
                        access,
                        result: result.id,
                        ready_at: ready.0,
                        ready_site: ready.1,
                    })
                    .is_some()
                {
                    return Err(invalid("LDS exchange duplicate global read"));
                }
            }
            Opcode::LdsWriteB32 => {
                let [Some(address), Some(value), Some(exec), None, None] = step.operands else {
                    return Err(invalid("LDS exchange write operand roster"));
                };
                if exec != full_exec.id || !operation.results.is_empty() {
                    return Err(invalid("LDS exchange full-EXEC resultless write"));
                }
                let (_, address_op, shift) = definition(body, address)?;
                if shift.instruction.opcode != Opcode::VectorLshlrev32
                    || shift.instruction.immediate != 2
                    || address_op.results.first().map(|v| v.id) != Some(address)
                    || shift.operands[0] != Some(local_x.id)
                {
                    return Err(invalid("LDS exchange localX byte address"));
                }
                let ready = following(block, index + 1, Opcode::WaitLgkm0)?;
                if lds_write
                    .replace(PhysicalLdsExchangeLdsAccessV22 {
                        location,
                        site: step.site,
                        kind: FormalMemoryAccessKind::Write,
                        address,
                        value,
                        exec,
                        complete_at: ready.0,
                        complete_site: ready.1,
                    })
                    .is_some()
                {
                    return Err(invalid("LDS exchange duplicate LDS write"));
                }
            }
            Opcode::WorkgroupPublishBarrier => {
                if !operation.results.is_empty() || step.operands.iter().any(Option::is_some) {
                    return Err(invalid("LDS exchange barrier is not an SSA definition"));
                }
                if publication
                    .replace(PhysicalLdsExchangePublicationV22 {
                        location,
                        site: step.site,
                        epoch: declaration.lds_frame.publication_epoch,
                        participants: 128,
                    })
                    .is_some()
                {
                    return Err(invalid("LDS exchange duplicate publication"));
                }
            }
            Opcode::LdsReadB32 => {
                let [Some(address), Some(exec), None, None, None] = step.operands else {
                    return Err(invalid("LDS exchange read operand roster"));
                };
                let [result] = operation.results.as_slice() else {
                    return Err(invalid("LDS exchange read one pending result"));
                };
                if exec != full_exec.id {
                    return Err(invalid("LDS exchange full-EXEC peer read"));
                }
                let (_, address_op, shift) = definition(body, address)?;
                if shift.instruction.opcode != Opcode::VectorLshlrev32
                    || shift.instruction.immediate != 2
                    || address_op.results.first().map(|r| r.id) != Some(address)
                {
                    return Err(invalid("LDS exchange peer byte displacement"));
                }
                let peer =
                    shift.operands[0].ok_or_else(|| invalid("LDS exchange peer address SSA"))?;
                let (_, peer_op, xor) = definition(body, peer)?;
                if xor.instruction.opcode != Opcode::VectorXor32
                    || xor.instruction.immediate != 64
                    || peer_op.results.first().map(|r| r.id) != Some(peer)
                    || xor.operands[0] != Some(local_x.id)
                {
                    return Err(invalid("LDS exchange actual opposite-wave address"));
                }
                let ready = following(block, index + 1, Opcode::WaitLgkm0)?;
                if lds_read
                    .replace(PhysicalLdsExchangeLdsAccessV22 {
                        location,
                        site: step.site,
                        kind: FormalMemoryAccessKind::Read,
                        address,
                        value: result.id,
                        exec,
                        complete_at: ready.0,
                        complete_site: ready.1,
                    })
                    .is_some()
                {
                    return Err(invalid("LDS exchange duplicate LDS read"));
                }
            }
            Opcode::GlobalStoreDword => {
                let [Some(low), Some(high), Some(value), Some(exec), None] = step.operands else {
                    return Err(invalid("LDS exchange store operand roster"));
                };
                let (mask_at, mask_op, mask) = definition(body, exec)?;
                if mask.instruction.opcode != Opcode::SaveAndMaskExec
                    || mask_op.results.get(2).map(|r| r.id) != Some(exec)
                {
                    return Err(invalid("LDS exchange actual store EXEC producer"));
                }
                let vcc =
                    mask.operands[0].ok_or_else(|| invalid("LDS exchange store mask input"))?;
                let (comparison_at, compare_op, comparison) = definition(body, vcc)?;
                if comparison.instruction.opcode != Opcode::VectorCompareGtU64
                    || compare_op.results.first().map(|r| r.id) != Some(vcc)
                {
                    return Err(invalid("LDS exchange actual store comparison producer"));
                }
                let [
                    Some(len_low),
                    Some(len_high),
                    Some(idx_low),
                    Some(idx_high),
                    Some(_),
                ] = comparison.operands
                else {
                    return Err(invalid("LDS exchange bounds roster"));
                };
                if address_index(body, high)? != [idx_low, idx_high] {
                    return Err(invalid("LDS exchange store address index differs"));
                }
                let ready = following(block, index + 1, Opcode::WaitVm0)?;
                let restore = following(block, index + 2, Opcode::RestoreExec)?;
                let access = PhysicalLdsExchangeAccessV22 {
                    location,
                    site: step.site,
                    allocation: output_allocation,
                    parameter: *output,
                    address: [low, high],
                    index: [idx_low, idx_high],
                    exec,
                };
                if global_store
                    .replace(PhysicalLdsExchangeStoreV22 {
                        access,
                        value,
                        mask_at,
                        mask_site: mask.site,
                        comparison_at,
                        comparison_site: comparison.site,
                        length: [len_low, len_high],
                        ready_at: ready.0,
                        ready_site: ready.1,
                        restore_at: restore.0,
                        restore_site: restore.1,
                    })
                    .is_some()
                {
                    return Err(invalid("LDS exchange duplicate output store"));
                }
            }
            Opcode::WaitLgkm0
            | Opcode::ScalarLshl32
            | Opcode::VectorAddU32
            | Opcode::VectorMove32
            | Opcode::VectorLshlrev64
            | Opcode::VectorAddCarry
            | Opcode::VectorAddCarryIn
            | Opcode::WaitVm0
            | Opcode::VectorCompareGtU64
            | Opcode::SaveAndMaskExec
            | Opcode::RestoreExec
            | Opcode::VectorLshlrev32
            | Opcode::VectorXor32 => {}
            Opcode::Endpgm0 => return Err(invalid("LDS exchange end must be actual CFG")),
        }
    }
    let [Some(a), Some(b), Some(c), Some(d)] = reads else {
        return Err(invalid("LDS exchange incomplete kernarg read census"));
    };
    let read = global_read.ok_or_else(|| invalid("LDS exchange global read missing"))?;
    let store = global_store.ok_or_else(|| invalid("LDS exchange store missing"))?;
    let lds_write = lds_write.ok_or_else(|| invalid("LDS exchange write missing"))?;
    let lds_read = lds_read.ok_or_else(|| invalid("LDS exchange read missing"))?;
    let publication = publication.ok_or_else(|| invalid("LDS exchange publication missing"))?;
    if read.result != lds_write.value
        || lds_read.value != store.value
        || read.ready_at.operation_index >= lds_write.location.operation_index
        || lds_write.location.operation_index >= lds_write.complete_at.operation_index
        || lds_write.complete_at.operation_index >= publication.location.operation_index
        || publication.location.operation_index >= lds_read.location.operation_index
        || lds_read.location.operation_index >= lds_read.complete_at.operation_index
        || lds_read.complete_at.operation_index >= store.access.location.operation_index
        || read.access.index != store.access.index
        || read.ready_at.operation_index >= store.access.location.operation_index
    {
        return Err(invalid(
            "LDS exchange original ready input result and store join",
        ));
    }
    let mut accesses = vector(2)?;
    for (access, kind) in [
        (read.access, FormalMemoryAccessKind::Read),
        (store.access, FormalMemoryAccessKind::Write),
    ] {
        accesses.push(FormalMemoryAccess {
            location: access.location,
            allocation: access.allocation,
            kind,
            address_space: AddressSpace::Global,
            byte_offset: ByteExpression::invocation_affine(0, 4),
            byte_width: 4,
            alignment: 4,
            invocations,
            domain: FormalAccessDomainV1::LaunchEnvelope,
        });
    }
    let mut reasons = BTreeSet::new();
    let mut guarded = None;
    let bounds_requirements = derive_bounds_requirements(&accesses, &mut reasons, &mut guarded)?;
    let runtime_alias_requirements = derive_alias_requirements(&accesses, &mut guarded)?;
    let inter_invocation_conflicts = derive_inter_invocation_conflicts(&accesses, &mut guarded)?;
    if let Some(reason) = reasons.into_iter().next() {
        return Err(PhysicalLdsExchangeMemoryErrorV22::Incomplete(reason));
    }
    let minimum = count * 4;
    if bounds_requirements.len() != 2
        || bounds_requirements
            .iter()
            .any(|b| b.minimum_byte_len() != Some(minimum))
        || runtime_alias_requirements.len() != 1
        || !inter_invocation_conflicts.is_empty()
    {
        return Err(invalid(
            "LDS exchange bounds alias or race derivation differs",
        ));
    }
    let alias = runtime_alias_requirements[0];
    if alias.left() != input_allocation
        || alias.right() != output_allocation
        || [alias.left_accessed_bytes(), alias.right_accessed_bytes()]
            .iter()
            .any(|r| r.is_none_or(|r| r.start() != 0 || r.end_exclusive() != minimum))
    {
        return Err(invalid(
            "LDS exchange actual input output alias regions differ",
        ));
    }
    let global = FormalMemoryObligations {
        kernel: kernel.id.clone(),
        entry: kernel.entry.clone(),
        index_width: width,
        invocations: Some(invocations),
        allocations,
        accesses,
        bounds_requirements,
        runtime_alias_requirements,
        inter_invocation_conflicts,
    };
    Ok(PhysicalLdsExchangeMemoryObligationsV22 {
        canonical_identity: *owner.identity().digest(),
        global,
        kernarg_reads: [a, b, c, d],
        lds_frame: PhysicalLdsExchangeFrameObligationV22 {
            declaration: Location::new(block.id, 0),
            site: declaration.begin_site,
            descriptor: declaration.lds_frame,
            local_x: local_x.id,
        },
        lds_write,
        lds_read,
        publication,
        read,
        store,
        runtime: PhysicalLdsExchangeRuntimeRequirementsV22 {
            input: input_allocation,
            output: output_allocation,
            minimum_input_bytes: minimum,
            minimum_output_bytes: minimum,
            input_readable: true,
            input_initialized: true,
            output_writable: true,
            input_output_disjoint: true,
            required_workgroup: declaration.workgroup,
            required_workgroups: declaration.maximum_workgroups,
        },
        abi: PhysicalLdsExchangeKernargAbiRequirementV22 {
            minimum_bytes: 32,
            alignment: 8,
            disjoint_output: output_allocation,
            readable: true,
            live: true,
            immutable: true,
        },
    })
}
#[cfg(test)]
#[path = "physical_lds_exchange_v22_tests.rs"]
mod tests;
