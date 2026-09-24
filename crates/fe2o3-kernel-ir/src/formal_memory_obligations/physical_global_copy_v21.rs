//! Exact typed global read/write and kernarg obligations from the actual KIR21
//! owner. No caller effect plan and no second physical-state interpreter.
use super::*;
use crate::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, FunctionBody,
    FunctionOperationLocation as Location, Gfx942PhysicalEntrySourceSiteVNext as Site,
    Gfx942PhysicalGlobalCopyOpcodeV1 as Opcode, Gfx942PhysicalGlobalCopyStepV1 as Step,
    VerifiedCanonicalKernelIrModuleV21, gfx942_physical_global_copy_declaration_v21,
};
#[path = "physical_global_copy_types_v21.rs"]
mod types;
pub use types::*;
const WORK: usize = 131_072;
const SCRATCH: usize = 64 * 1024;

fn invalid(message: &'static str) -> PhysicalGlobalCopyMemoryErrorV21 {
    PhysicalGlobalCopyMemoryErrorV21::Profile(message)
}
fn retained() -> Result<usize, PhysicalGlobalCopyMemoryErrorV21> {
    // Fixed four ABI reads are inline. Existing affine report routines have
    // two allocations/accesses/bounds and one alias pair; reserve their bounded
    // vector/string payload conservatively. Their old allocation domain is not
    // rebranded as exact heap accounting.
    std::mem::size_of::<PhysicalGlobalCopyMemoryObligationsV21>()
        .checked_add(4096)
        .ok_or(Resource::Arithmetic.into())
}
fn vector<T>(count: usize) -> Result<Vec<T>, PhysicalGlobalCopyMemoryErrorV21> {
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
/// At launch128 both real allocations require512B. Readability, initialized
/// input, write permission, alias and compiler-owned kernarg conditions remain
/// unresolved. Generic formal extraction remains Unmodeled for these opcodes.
///
/// Work uses the same cumulative ledger; incoming storage is restored on all
/// exits. Reserve the returned logical receipt while retaining the report.
pub fn derive_physical_global_copy_memory_obligations_v21(
    owner: &VerifiedCanonicalKernelIrModuleV21,
    kernel: &KernelId,
    launch: ExplicitLaunchExtent,
    width: FormalIndexWidth,
    budget: &mut Budget<'_>,
) -> Result<
    (
        PhysicalGlobalCopyMemoryObligationsV21,
        PhysicalGlobalCopyMemoryStorageV21,
    ),
    PhysicalGlobalCopyMemoryErrorV21,
> {
    let retained = retained()?;
    budget.with_prepaid_scope(budget.storage(), 1, WORK, retained + SCRATCH, |_| {
        derive(owner, kernel, launch, width)
            .map(|report| (report, PhysicalGlobalCopyMemoryStorageV21 { retained }))
    })
}
/// Independently reconstructs and compares every typed field, actual source
/// occurrence, SSA identity and unresolved condition. No digest-only shortcut.
/// Caller must already reserve its retained report; temporary replay storage
/// is prepaid here and the incoming floor is never reset.
pub fn validate_physical_global_copy_memory_obligations_v21(
    owner: &VerifiedCanonicalKernelIrModuleV21,
    kernel: &KernelId,
    launch: ExplicitLaunchExtent,
    width: FormalIndexWidth,
    report: &PhysicalGlobalCopyMemoryObligationsV21,
    budget: &mut Budget<'_>,
) -> Result<(), PhysicalGlobalCopyMemoryErrorV21> {
    let retained = retained()?;
    budget.with_prepaid_scope(retained, 1, WORK, retained + SCRATCH, |_| {
        if derive(owner, kernel, launch, width)? != *report {
            return Err(invalid("global copy complete memory report changed"));
        }
        Ok(())
    })
}
fn definition(
    body: &FunctionBody,
    value: ValueId,
) -> Result<(Location, &Operation, Step), PhysicalGlobalCopyMemoryErrorV21> {
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
        .ok_or_else(|| invalid("global copy memory SSA producer missing"))
        .and_then(|(location, operation)| match operation.kind {
            OperationKind::Gfx942PhysicalGlobalCopyStep(step) => Ok((location, operation, step)),
            _ => Err(invalid("global copy memory SSA producer is not a step")),
        })
}
fn following(
    block: &crate::BasicBlock,
    index: usize,
    opcode: Opcode,
) -> Result<(Location, Site), PhysicalGlobalCopyMemoryErrorV21> {
    match block.operations.get(index).map(|op| &op.kind) {
        Some(OperationKind::Gfx942PhysicalGlobalCopyStep(step))
            if step.instruction.opcode == opcode =>
        {
            Ok((Location::new(block.id, index), step.site))
        }
        _ => Err(invalid(
            "global copy actual following wait or restore differs",
        )),
    }
}
fn address_index(
    body: &FunctionBody,
    high: ValueId,
) -> Result<[ValueId; 2], PhysicalGlobalCopyMemoryErrorV21> {
    // Attribute the exact index through the actual SSA displacement producer.
    // Root/half/carry/current-generation semantics are already proved by the
    // immutable canonical owner, not reinvented by this memory summary.
    let (_, high_op, add) = definition(body, high)?;
    if add.instruction.opcode != Opcode::VectorAddCarryIn
        || high_op.results.first().map(|r| r.id) != Some(high)
    {
        return Err(invalid("global copy memory high address producer"));
    }
    let offset = add.operands[1].ok_or_else(|| invalid("global copy high offset absent"))?;
    let (_, shift_op, shift) = definition(body, offset)?;
    if shift.instruction.opcode != Opcode::VectorLshlrev64
        || shift_op.results.get(1).map(|r| r.id) != Some(offset)
    {
        return Err(invalid("global copy exact index displacement producer"));
    }
    match shift.operands {
        [Some(low), Some(high), Some(_), None, None] => Ok([low, high]),
        _ => Err(invalid("global copy displacement operand roster")),
    }
}
fn derive(
    owner: &VerifiedCanonicalKernelIrModuleV21,
    kernel_id: &KernelId,
    launch: ExplicitLaunchExtent,
    width: FormalIndexWidth,
) -> Result<PhysicalGlobalCopyMemoryObligationsV21, PhysicalGlobalCopyMemoryErrorV21> {
    let declaration = gfx942_physical_global_copy_declaration_v21(owner)
        .ok_or_else(|| invalid("exact V21 global-copy profile required"))?;
    let [kernel] = owner.module().kernels.as_slice() else {
        return Err(invalid("global copy one kernel"));
    };
    let [function] = owner.module().functions.as_slice() else {
        return Err(invalid("global copy one function"));
    };
    if kernel_id != &kernel.id || kernel.entry != function.id {
        return Err(invalid("global copy actual kernel entry join"));
    }
    if width != FormalIndexWidth::Bits64 {
        return Err(PhysicalGlobalCopyMemoryErrorV21::Incomplete(
            FormalMemoryIncompleteReason::UnsupportedIndexWidth { width },
        ));
    }
    let ExplicitLaunchExtent::Exact {
        rank: 1,
        extents: [count, 1, 1],
    } = launch
    else {
        return Err(invalid("global copy formal launch must be exact rank1"));
    };
    if !matches!(count, 64 | 128)
        || declaration.workgroup != [64, 1, 1]
        || declaration.maximum_workgroups != [2, 1, 1]
    {
        return Err(invalid(
            "global copy formal launch exceeds full-workgroup profile",
        ));
    }
    let mut reasons = BTreeSet::new();
    let invocations = resolve_invocations(&kernel.domain, launch, &mut reasons)?;
    if let Some(reason) = reasons.into_iter().next() {
        return Err(PhysicalGlobalCopyMemoryErrorV21::Incomplete(reason));
    }
    let invocations =
        invocations.ok_or_else(|| invalid("global copy invocation universe absent"))?;
    let body = function
        .body
        .as_ref()
        .ok_or_else(|| invalid("global copy body absent"))?;
    let [block] = body.blocks.as_slice() else {
        return Err(invalid("global copy one actual block"));
    };
    let [input, output] = body.parameters.as_slice() else {
        return Err(invalid("global copy two real parameters"));
    };
    if [*input, *output] != declaration.parameters {
        return Err(invalid("global copy actual logical parameter join"));
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
            return Err(invalid("global copy actual slice signature"));
        };
        if slice.address_space != AddressSpace::Global
            || slice.access != access
            || *slice.element != Type::Scalar(ScalarType::U32)
        {
            return Err(invalid("global copy source slice permission"));
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
        .ok_or_else(|| invalid("global copy entry declaration missing"))?;
    let [_, _, _, _, full_exec] = entry.results.as_slice() else {
        return Err(invalid("global copy actual entry live-ins"));
    };
    let mut reads = [None; 4];
    let mut read_count = 0;
    let mut global_read = None;
    let mut global_store = None;
    for (index, operation) in block.operations.iter().enumerate() {
        let location = Location::new(block.id, index);
        let step = match operation.kind {
            OperationKind::Gfx942PhysicalGlobalCopyDeclaration(_) if index == 0 => continue,
            OperationKind::Gfx942PhysicalGlobalCopyStep(step) => step,
            _ => return Err(invalid("global copy unaccounted memory operation")),
        };
        match step.instruction.opcode {
            Opcode::LoadKernargPair => {
                let slot = match step.instruction.immediate {
                    0 => PhysicalGlobalCopyKernargSlotV21::InputPointer,
                    8 => PhysicalGlobalCopyKernargSlotV21::InputLength,
                    16 => PhysicalGlobalCopyKernargSlotV21::OutputPointer,
                    24 => PhysicalGlobalCopyKernargSlotV21::OutputLength,
                    _ => return Err(invalid("global copy kernarg slot range")),
                };
                let [Some(low), Some(high), None, None, None] = step.operands else {
                    return Err(invalid("global copy kernarg base roster"));
                };
                let [a, b] = operation.results.as_slice() else {
                    return Err(invalid("global copy kernarg results"));
                };
                let wait = block.operations[index + 1..]
                    .iter()
                    .enumerate()
                    .find_map(|(relative, op)| match op.kind {
                        OperationKind::Gfx942PhysicalGlobalCopyStep(s)
                            if s.instruction.opcode == Opcode::WaitLgkm0 =>
                        {
                            Some((Location::new(block.id, index + 1 + relative), s.site))
                        }
                        _ => None,
                    })
                    .ok_or_else(|| invalid("global copy kernarg readiness missing"))?;
                let row = reads
                    .get_mut(read_count)
                    .ok_or_else(|| invalid("global copy excessive kernarg reads"))?;
                *row = Some(PhysicalGlobalCopyKernargReadV21 {
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
                    return Err(invalid("global copy read operand roster"));
                };
                let [result] = operation.results.as_slice() else {
                    return Err(invalid("global copy single pending read result"));
                };
                if exec != full_exec.id {
                    return Err(invalid("global copy read requires actual full EXEC"));
                }
                let ready = following(block, index + 1, Opcode::WaitVm0)?;
                let access = PhysicalGlobalCopyAccessV21 {
                    location,
                    site: step.site,
                    allocation: input_allocation,
                    parameter: *input,
                    address: [low, high],
                    index: address_index(body, high)?,
                    exec,
                };
                if global_read
                    .replace(PhysicalGlobalCopyReadV21 {
                        access,
                        result: result.id,
                        ready_at: ready.0,
                        ready_site: ready.1,
                    })
                    .is_some()
                {
                    return Err(invalid("global copy duplicate global read"));
                }
            }
            Opcode::GlobalStoreDword => {
                let [Some(low), Some(high), Some(value), Some(exec), None] = step.operands else {
                    return Err(invalid("global copy store operand roster"));
                };
                let (mask_at, mask_op, mask) = definition(body, exec)?;
                if mask.instruction.opcode != Opcode::SaveAndMaskExec
                    || mask_op.results.get(2).map(|r| r.id) != Some(exec)
                {
                    return Err(invalid("global copy actual store EXEC producer"));
                }
                let vcc =
                    mask.operands[0].ok_or_else(|| invalid("global copy store mask input"))?;
                let (comparison_at, compare_op, comparison) = definition(body, vcc)?;
                if comparison.instruction.opcode != Opcode::VectorCompareGtU64
                    || compare_op.results.first().map(|r| r.id) != Some(vcc)
                {
                    return Err(invalid("global copy actual store comparison producer"));
                }
                let [
                    Some(len_low),
                    Some(len_high),
                    Some(idx_low),
                    Some(idx_high),
                    Some(_),
                ] = comparison.operands
                else {
                    return Err(invalid("global copy bounds roster"));
                };
                if address_index(body, high)? != [idx_low, idx_high] {
                    return Err(invalid("global copy store address index differs"));
                }
                let ready = following(block, index + 1, Opcode::WaitVm0)?;
                let restore = following(block, index + 2, Opcode::RestoreExec)?;
                let access = PhysicalGlobalCopyAccessV21 {
                    location,
                    site: step.site,
                    allocation: output_allocation,
                    parameter: *output,
                    address: [low, high],
                    index: [idx_low, idx_high],
                    exec,
                };
                if global_store
                    .replace(PhysicalGlobalCopyStoreV21 {
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
                    return Err(invalid("global copy duplicate output store"));
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
            | Opcode::RestoreExec => {}
            Opcode::Endpgm0 => return Err(invalid("global copy end must be actual CFG")),
        }
    }
    let [Some(a), Some(b), Some(c), Some(d)] = reads else {
        return Err(invalid("global copy incomplete kernarg read census"));
    };
    let read = global_read.ok_or_else(|| invalid("global copy global read missing"))?;
    let store = global_store.ok_or_else(|| invalid("global copy store missing"))?;
    if read.result != store.value
        || read.access.index != store.access.index
        || read.ready_at.operation_index >= store.access.location.operation_index
    {
        return Err(invalid(
            "global copy original ready input result and store join",
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
        return Err(PhysicalGlobalCopyMemoryErrorV21::Incomplete(reason));
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
            "global copy bounds alias or race derivation differs",
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
            "global copy actual input output alias regions differ",
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
    Ok(PhysicalGlobalCopyMemoryObligationsV21 {
        canonical_identity: *owner.identity().digest(),
        global,
        kernarg_reads: [a, b, c, d],
        read,
        store,
        runtime: PhysicalGlobalCopyRuntimeRequirementsV21 {
            input: input_allocation,
            output: output_allocation,
            minimum_input_bytes: minimum,
            minimum_output_bytes: minimum,
            input_readable: true,
            input_initialized: true,
            output_writable: true,
            input_output_disjoint: true,
        },
        abi: PhysicalGlobalCopyKernargAbiRequirementV21 {
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
#[path = "physical_global_copy_v21_tests.rs"]
mod tests;
