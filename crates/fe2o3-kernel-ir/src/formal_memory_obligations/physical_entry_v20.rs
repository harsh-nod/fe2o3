//! Exact memory obligations selected only by an immutable physical-entry owner.
//! Kernarg is a compiler ABI allocation, never a fabricated logical parameter.
use super::*;
use crate::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, FunctionBody,
    Gfx942PhysicalEntryOpcodeV20 as Opcode, Gfx942PhysicalEntrySourceSiteVNext as Site,
    VerifiedCanonicalKernelIrModuleV20, gfx942_physical_entry_declaration_v20,
};
#[path = "physical_entry_types_v20.rs"]
mod types;
pub use types::*;

const WORK: usize = 65_536;
const SCRATCH: usize = 64 * 1024;

fn invalid(message: &'static str) -> PhysicalEntryMemoryErrorV20 {
    PhysicalEntryMemoryErrorV20::Profile(message)
}
fn reserve<T>(count: usize) -> Result<Vec<T>, PhysicalEntryMemoryErrorV20> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    if values.capacity() > count {
        return Err(Resource::Accounting.into());
    }
    Ok(values)
}
/// Derives all modeled memory families from the actual verified physical body.
///
/// The ordinary output report uses the actual logical output parameter and store
/// location; the existing affine bounds/alias/race derivation remains unchanged.
/// The conservative LaunchEnvelope is intentional: launch128 requires512B even
/// when the authored EXEC bounds mask would suppress a ragged tail.
///
/// Exact kernarg reads remain separate ABI obligations with actual SSA results,
/// source sites and wait locations. They cannot be consumed as user allocations
/// or dropped by a normal descriptor/native continuation. No runtime kernarg
/// allocation, immutability, output nonalias relation or launch is authenticated.
///
/// Generic formal extraction remains Unmodeled for this profile. This function
/// accepts no caller-selected memory plan and grants no source/runtime authority.
pub fn derive_physical_entry_memory_obligations_v20(
    owner: &VerifiedCanonicalKernelIrModuleV20,
    kernel_id: &KernelId,
    launch: ExplicitLaunchExtent,
    index_width: FormalIndexWidth,
    budget: &mut Budget<'_>,
) -> Result<
    (
        PhysicalEntryMemoryObligationsV20,
        PhysicalEntryMemoryStorageV20,
    ),
    PhysicalEntryMemoryErrorV20,
> {
    let retained = std::mem::size_of::<PhysicalEntryMemoryObligationsV20>()
        .checked_add(64 * std::mem::size_of::<PhysicalEntryKernargReadV20>())
        .and_then(|n| n.checked_add(4096))
        .ok_or(Resource::Arithmetic)?;
    budget.with_prepaid_scope(budget.storage(), 1, WORK, retained + SCRATCH, |_| {
        derive(owner, kernel_id, launch, index_width)
            .map(|report| (report, PhysicalEntryMemoryStorageV20 { retained }))
    })
}
// Bounded SSA attribution only, not another physical-state interpreter.
fn definition(
    body: &FunctionBody,
    value: ValueId,
) -> Result<(FunctionOperationLocation, &Operation), PhysicalEntryMemoryErrorV20> {
    body.blocks
        .iter()
        .find_map(|block| {
            block
                .operations
                .iter()
                .enumerate()
                .find(|(_, operation)| operation.results.iter().any(|result| result.id == value))
                .map(|(index, operation)| {
                    (FunctionOperationLocation::new(block.id, index), operation)
                })
        })
        .ok_or_else(|| invalid("physical memory attribution lacks actual SSA producer"))
}
fn derive(
    owner: &VerifiedCanonicalKernelIrModuleV20,
    kernel_id: &KernelId,
    launch: ExplicitLaunchExtent,
    index_width: FormalIndexWidth,
) -> Result<PhysicalEntryMemoryObligationsV20, PhysicalEntryMemoryErrorV20> {
    let declaration = gfx942_physical_entry_declaration_v20(owner)
        .ok_or_else(|| invalid("exact V20 physical profile required"))?;
    let module = owner.module();
    let [kernel] = module.kernels.as_slice() else {
        return Err(invalid("physical one kernel"));
    };
    let [function] = module.functions.as_slice() else {
        return Err(invalid("physical one function"));
    };
    if kernel_id != &kernel.id || kernel.entry != function.id {
        return Err(invalid("physical actual kernel/entry join"));
    }
    if index_width != FormalIndexWidth::Bits64 {
        return Err(PhysicalEntryMemoryErrorV20::Incomplete(
            FormalMemoryIncompleteReason::UnsupportedIndexWidth { width: index_width },
        ));
    }
    let ExplicitLaunchExtent::Exact {
        rank: 1,
        extents: [count, 1, 1],
    } = launch
    else {
        return Err(invalid("physical formal launch must be exact rank1"));
    };
    if !matches!(count, 64 | 128)
        || declaration.workgroup != [64, 1, 1]
        || declaration.maximum_workgroups != [2, 1, 1]
    {
        return Err(invalid(
            "physical formal launch exceeds exact full-workgroup profile",
        ));
    }
    let mut reasons = BTreeSet::new();
    let invocations = resolve_invocations(&kernel.domain, launch, &mut reasons)?;
    if let Some(reason) = reasons.into_iter().next() {
        return Err(PhysicalEntryMemoryErrorV20::Incomplete(reason));
    }
    let invocations =
        invocations.ok_or_else(|| invalid("physical formal invocation universe absent"))?;
    let body = function
        .body
        .as_ref()
        .ok_or_else(|| invalid("physical body absent"))?;
    let [output, _, _, _, _] = body.parameters.as_slice() else {
        return Err(invalid("physical logical parameter roster"));
    };
    if *output != declaration.parameters[0] {
        return Err(invalid("physical logical output join"));
    }
    let Type::Slice(slice) = &function.signature.parameters[0] else {
        return Err(invalid("physical output is not slice"));
    };
    if slice.address_space != AddressSpace::Global
        || slice.access != AccessMode::ReadWrite
        || *slice.element != Type::Scalar(ScalarType::U32)
    {
        return Err(invalid("physical output slice contract"));
    }
    let allocation = FormalAllocationIdentity { parameter_index: 0 };
    let mut allocations = reserve(1)?;
    allocations.push(FormalAllocationParameter {
        identity: allocation,
        value: *output,
        kind: FormalParameterKind::Slice,
        address_space: AddressSpace::Global,
        access: AccessMode::ReadWrite,
    });
    let mut reads = reserve(64)?;
    let mut store = None;
    for block in &body.blocks {
        for (index, operation) in block.operations.iter().enumerate() {
            let location = FunctionOperationLocation::new(block.id, index);
            let step = match &operation.kind {
                OperationKind::Gfx942PhysicalEntryDeclaration(_)
                    if block.id == body.blocks[0].id && index == 0 =>
                {
                    continue;
                }
                OperationKind::Gfx942PhysicalEntryStep(step) => step,
                _ => return Err(invalid("physical unaccounted operation")),
            };
            match step.instruction.opcode {
                Opcode::LoadKernargPair | Opcode::LoadKernargDword => {
                    if reads.len() == 64 {
                        return Err(invalid("physical kernarg read bound"));
                    }
                    let width = if step.instruction.opcode == Opcode::LoadKernargPair {
                        8
                    } else {
                        4
                    };
                    let offset = step.instruction.immediate;
                    let slot = match (offset, width) {
                        (0, 8) => PhysicalEntryKernargSlotV20::OutputPointer,
                        (8, 8) => PhysicalEntryKernargSlotV20::OutputLength,
                        (16, 4) => PhysicalEntryKernargSlotV20::ScalarArgument(1),
                        (20, 4) => PhysicalEntryKernargSlotV20::ScalarArgument(2),
                        (24, 4) => PhysicalEntryKernargSlotV20::ScalarArgument(3),
                        (28, 4) => PhysicalEntryKernargSlotV20::ScalarArgument(4),
                        _ => return Err(invalid("physical kernarg slot range differs")),
                    };
                    if offset.checked_add(width).is_none_or(|end| end > 32) {
                        return Err(invalid("physical kernarg extent"));
                    }
                    let wait = block.operations[index + 1..]
                        .iter()
                        .enumerate()
                        .find_map(|(relative, op)| match op.kind {
                            OperationKind::Gfx942PhysicalEntryStep(s)
                                if s.instruction.opcode == Opcode::WaitLgkm0 =>
                            {
                                Some((
                                    FunctionOperationLocation::new(block.id, index + 1 + relative),
                                    s.site,
                                ))
                            }
                            _ => None,
                        })
                        .ok_or_else(|| {
                            invalid("physical pending load lacks same-block lgkm wait")
                        })?;
                    let results = match operation.results.as_slice() {
                        [low] if width == 4 => [Some(low.id), None],
                        [low, high] if width == 8 => [Some(low.id), Some(high.id)],
                        _ => return Err(invalid("physical kernarg result roster")),
                    };
                    let [Some(base_low), Some(base_high), None, None, None, None] = step.operands
                    else {
                        return Err(invalid("physical kernarg base roster"));
                    };
                    reads.push(PhysicalEntryKernargReadV20 {
                        location,
                        site: step.site,
                        offset,
                        width,
                        alignment: width,
                        slot,
                        base: [base_low, base_high],
                        results,
                        ready_at: wait.0,
                        ready_site: wait.1,
                    });
                }
                Opcode::GlobalStoreDword => {
                    let [Some(low), Some(high), Some(value), Some(exec), None, None] =
                        step.operands
                    else {
                        return Err(invalid("physical store operand roster"));
                    };
                    let (mask_at, mask) = definition(body, exec)?;
                    let OperationKind::Gfx942PhysicalEntryStep(mask_step) = mask.kind else {
                        return Err(invalid("physical EXEC producer is not a physical step"));
                    };
                    if mask_step.instruction.opcode != Opcode::SaveAndMaskExec
                        || mask.results.get(2).map(|r| r.id) != Some(exec)
                    {
                        return Err(invalid("physical store EXEC producer differs"));
                    }
                    let vcc = mask_step.operands[0]
                        .ok_or_else(|| invalid("physical bounds VCC absent"))?;
                    let (comparison_at, comparison) = definition(body, vcc)?;
                    let OperationKind::Gfx942PhysicalEntryStep(compare_step) = comparison.kind
                    else {
                        return Err(invalid("physical bounds producer is not a physical step"));
                    };
                    if compare_step.instruction.opcode != Opcode::VectorCompareGtU64
                        || comparison.results.first().map(|r| r.id) != Some(vcc)
                    {
                        return Err(invalid("physical store bounds producer differs"));
                    }
                    let [
                        Some(length_low),
                        Some(length_high),
                        Some(index_low),
                        Some(index_high),
                        Some(_),
                        None,
                    ] = compare_step.operands
                    else {
                        return Err(invalid("physical bounds operand roster"));
                    };
                    if store
                        .replace(PhysicalEntryStoreV20 {
                            location,
                            site: step.site,
                            allocation,
                            output: *output,
                            address: [low, high],
                            value,
                            exec,
                            mask_at,
                            mask_site: mask_step.site,
                            comparison_at,
                            comparison_site: compare_step.site,
                            index: [index_low, index_high],
                            length: [length_low, length_high],
                        })
                        .is_some()
                    {
                        return Err(invalid("physical duplicate output store"));
                    }
                }
                Opcode::WaitLgkm0
                | Opcode::ScalarLshl32
                | Opcode::VectorAddU32
                | Opcode::VectorMove32
                | Opcode::ScalarCompareEqZero
                | Opcode::VectorLshlrev64
                | Opcode::VectorAddCarry
                | Opcode::VectorAddCarryIn
                | Opcode::VectorCompareGtU64
                | Opcode::SaveAndMaskExec
                | Opcode::WaitVm0
                | Opcode::RestoreExec => {}
                Opcode::BranchScc1 | Opcode::Branch | Opcode::Endpgm0 | Opcode::Fallthrough => {
                    return Err(invalid("physical control must be existing CFG"));
                }
            }
        }
    }
    let store = store.ok_or_else(|| invalid("physical output store missing"))?;
    if reads.is_empty() {
        return Err(invalid("physical kernarg read census empty"));
    }
    let mut accesses = reserve(1)?;
    accesses.push(FormalMemoryAccess {
        location: store.location,
        allocation,
        kind: FormalMemoryAccessKind::Write,
        address_space: AddressSpace::Global,
        byte_offset: ByteExpression::invocation_affine(0, 4),
        byte_width: 4,
        alignment: 4,
        invocations,
        domain: FormalAccessDomainV1::LaunchEnvelope,
    });
    // One actual store over the typed profile's proved gid/scale/pointer chain;
    // this is a memory summary, not execution of the authored instructions.
    let mut reasons = BTreeSet::new();
    let mut guarded = None;
    let bounds_requirements = derive_bounds_requirements(&accesses, &mut reasons, &mut guarded)?;
    let runtime_alias_requirements = derive_alias_requirements(&accesses, &mut guarded)?;
    let inter_invocation_conflicts = derive_inter_invocation_conflicts(&accesses, &mut guarded)?;
    if let Some(reason) = reasons.into_iter().next() {
        return Err(PhysicalEntryMemoryErrorV20::Incomplete(reason));
    }
    if bounds_requirements.len() != 1
        || bounds_requirements[0].minimum_byte_len() != Some(count * 4)
        || !runtime_alias_requirements.is_empty()
        || !inter_invocation_conflicts.is_empty()
    {
        return Err(invalid(
            "physical output bounds/alias/race derivation differs",
        ));
    }
    let output = FormalMemoryObligations {
        kernel: kernel.id.clone(),
        entry: kernel.entry.clone(),
        index_width,
        invocations: Some(invocations),
        allocations,
        accesses,
        bounds_requirements,
        runtime_alias_requirements,
        inter_invocation_conflicts,
    };
    Ok(PhysicalEntryMemoryObligationsV20 {
        canonical_identity: *owner.identity().digest(),
        output,
        reads,
        store,
        abi: PhysicalEntryKernargAbiRequirementV20 {
            minimum_bytes: 32,
            alignment: 8,
            disjoint_output: allocation,
        },
    })
}
