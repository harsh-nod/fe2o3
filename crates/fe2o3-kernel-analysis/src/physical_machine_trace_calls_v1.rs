//! Exact static call-site census over the already byte-bound complete trace.
use super::*;
use crate::physical_machine_effect::MAX_DIRECT_CALL_SITES_V1;

pub(super) fn validate<'a>(
    request: &PhysicalMachineEffectRequestV1,
    effects: &PhysicalMachineEffectEvidenceV1,
    functions: &BTreeMap<&'a str, &'a crate::PhysicalMachineFunctionEvidenceV1>,
    instructions: &BTreeMap<&str, Vec<&PhysicalMachineInstructionTraceV1>>,
) -> Result<(), PhysicalMachineTraceEvidenceErrorV1> {
    let by_offset = functions
        .values()
        .map(|function| (function.code_offset(), function.symbol()))
        .collect::<BTreeMap<_, _>>();
    if by_offset.len() != functions.len() {
        return Err(PhysicalMachineTraceEvidenceErrorV1::AmbiguousFunctionAddress);
    }
    let mut counts = BTreeMap::new();
    let mut total_sites = 0usize;
    for function in effects.functions() {
        let mut callees = BTreeSet::new();
        let mut sites = 0usize;
        for instruction in &instructions[function.symbol()] {
            if instruction.branch_kind != PhysicalMachineBranchKindV1::DirectCall {
                continue;
            }
            // Global bound is checked before increment. Exact offsets are already
            // strictly ordered and cover the function bytes without overlap/gaps.
            if total_sites == MAX_DIRECT_CALL_SITES_V1 {
                return Err(PhysicalMachineTraceEvidenceErrorV1::DirectCallSiteCount);
            }
            total_sites += 1;
            sites += 1;
            let branch_target = instruction
                .branch_target
                .ok_or(PhysicalMachineTraceEvidenceErrorV1::InvalidDirectCall)?;
            let callee = by_offset
                .get(&branch_target)
                .ok_or(PhysicalMachineTraceEvidenceErrorV1::InvalidDirectCall)?;
            callees.insert(*callee);
        }
        if callees
            != function
                .direct_callees()
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>()
        {
            return Err(PhysicalMachineTraceEvidenceErrorV1::InvalidDirectCall);
        }
        counts.insert(function.symbol(), sites);
    }

    for entry in request.entries() {
        let mut closure = BTreeSet::new();
        let mut pending = vec![entry.symbol()];
        let mut sites = 0usize;
        while let Some(symbol) = pending.pop() {
            if !closure.insert(symbol) {
                continue;
            }
            // Each function contributes its static sites once. Shared helpers
            // do not expand into dynamic invocations or duplicate effect rows.
            sites = sites
                .checked_add(counts[symbol])
                .ok_or(PhysicalMachineTraceEvidenceErrorV1::DirectCallSiteCount)?;
            if sites > entry.budget().max_direct_calls() as usize {
                return Err(PhysicalMachineTraceEvidenceErrorV1::DirectCallBudget);
            }
            pending.extend(
                functions[symbol]
                    .direct_callees()
                    .iter()
                    .map(String::as_str),
            );
        }
    }
    Ok(())
}
