//! Full original incoming roster through the existing source replay owner.
//! Rows are source/canonical correspondence only, not loan or phase authority.
use super::definitions::{Definition, normalize};
use crate::collector::production_importer_v1::{
    ProductionSemanticImportErrorV1, numerical_policy_v1::defined_body_v1::DefinedSourceRosterV1,
    source_body_v1,
};
use crate::rustc_semantic_plan_v1::ProductionSemanticPreflightPlanV1;
use fe2o3_mir_model::semantic_mir_v1::*;
use rustc_middle::{
    mir::{BasicBlock, Body, Operand, TerminatorKind, UnwindAction},
    ty::{Instance, TyCtxt, TyKind, TypingEnv},
};

type Result<T> = std::result::Result<T, ProductionSemanticImportErrorV1>;
fn rejected(detail: &'static str) -> ProductionSemanticImportErrorV1 {
    ProductionSemanticImportErrorV1::KernelContextBinding(detail)
}
pub(super) fn spend(work: &mut usize, amount: usize) -> Result<()> {
    *work = work
        .checked_sub(amount)
        .ok_or_else(|| rejected("phase live source-work ceiling"))?;
    Ok(())
}

/// Charges requested storage before allocation, then the actual excess
/// capacity. No refund is made: simultaneously live and temporary vectors are
/// bounded by their cumulative charge against the same existing work ceiling.
/// The underlying allocator's transient allocation and rustc caches are not
/// measured by this helper. Existing roster/replay resource owners remain.
pub(super) fn reserve<T>(count: usize, work: &mut usize) -> Result<Vec<T>> {
    let words = |capacity: usize| {
        capacity
            .checked_mul(std::mem::size_of::<T>())
            .and_then(|n| n.checked_add(std::mem::size_of::<Vec<T>>()))
            .and_then(|n| n.checked_add(std::mem::size_of::<usize>() - 1))
            .map(|n| n / std::mem::size_of::<usize>())
            .ok_or_else(|| rejected("phase incoming storage overflow"))
    };
    let requested = words(count)?;
    spend(work, requested)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| rejected("phase incoming allocation failed"))?;
    spend(
        work,
        words(result.capacity())?
            .checked_sub(requested)
            .ok_or_else(|| rejected("phase incoming capacity accounting"))?,
    )?;
    Ok(result)
}

pub(super) struct Call<'a, 'tcx> {
    pub caller: SemanticFunctionIdV1,
    pub callee: SemanticFunctionIdV1,
    pub caller_instance: Instance<'tcx>,
    pub callee_instance: Instance<'tcx>,
    pub original: &'a Body<'tcx>,
    pub raw_block: BasicBlock,
    pub block: SemanticBlockIdV1,
    pub normal: SemanticBlockIdV1,
    pub call: &'a SemanticDirectCallV1,
}

impl Call<'_, '_> {
    pub(super) fn key(&self) -> (SemanticFunctionIdV1, SemanticBlockIdV1) {
        (self.caller, self.block)
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn observe<'a, 'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &'a ProductionSemanticPreflightPlanV1<'tcx>,
    functions: &'a [SemanticFunctionDeclV1],
    callables: &[SemanticCallableDeclV1],
    definitions: &[(SemanticFunctionIdV1, Definition<'tcx>)],
    roster: &DefinedSourceRosterV1<'a, 'tcx>,
    replay: &mut source_body_v1::Replay<'a, 'tcx>,
    work: &mut usize,
) -> Result<Vec<Call<'a, 'tcx>>> {
    spend(work, functions.len().saturating_add(definitions.len()))?;
    if functions.len() != plan.function_producers().len() {
        return Err(rejected("phase incoming complete function roster"));
    }
    let target = |id| definitions.iter().any(|(function, _)| *function == id);
    let mut expected = 0usize;
    for edge in plan.direct_call_producers() {
        spend(work, 1 + definitions.len())?;
        if target(edge.callee) {
            expected = expected
                .checked_add(1)
                .ok_or_else(|| rejected("phase incoming count overflow"))?;
        }
    }
    if expected == 0 && !definitions.is_empty() {
        return Err(rejected(
            "phase definitions have no retained incoming occurrences",
        ));
    }
    let mut rows = reserve(expected, work)?;
    for (index, caller) in plan.function_producers().iter().enumerate() {
        let caller_id = SemanticFunctionIdV1::from_index(index as u32);
        let mut needed = false;
        for edge in plan.direct_call_producers() {
            spend(work, 1 + definitions.len())?;
            needed |= edge.caller == caller_id && target(edge.callee);
        }
        if !needed {
            continue;
        }
        let mapping = roster.reconstructed_body(caller_id, replay, work)?;
        let original = plan
            .function_mir(caller_id)
            .filter(|raw| std::ptr::eq(*raw, tcx.instance_mir(caller.instance.def)))
            .ok_or_else(|| rejected("phase incoming caller is not the retained original body"))?;
        for (raw_block, data) in original.basic_blocks.iter_enumerated() {
            spend(work, 1)?;
            let TerminatorKind::Call {
                func,
                args,
                destination,
                target: normal,
                unwind,
                ..
            } = &data.terminator().kind
            else {
                continue;
            };
            let ty = normalize(tcx, caller.instance, func.ty(&original.local_decls, tcx))
                .map_err(|_| rejected("phase original caller normalization"))?;
            let TyKind::FnDef(def, arguments) = *ty.kind() else {
                continue;
            };
            let Some(instance) = Instance::try_resolve(
                tcx,
                TypingEnv::fully_monomorphized(),
                def,
                tcx.erase_and_anonymize_regions(arguments),
            )
            .ok()
            .flatten() else {
                return Err(rejected("phase incoming original callee did not resolve"));
            };
            let mut selected = None;
            for (function, definition) in definitions {
                spend(work, 1)?;
                if definition.instance == instance && selected.replace(*function).is_some() {
                    return Err(rejected("phase duplicate original definition instance"));
                }
            }
            let Some(callee) = selected else { continue };
            let mut matches = 0;
            for edge in plan.direct_call_producers() {
                spend(work, 1)?;
                if edge.caller == caller_id && edge.block == raw_block.as_u32() {
                    if edge.callee != callee {
                        return Err(rejected("phase original incoming callee substitution"));
                    }
                    matches += 1;
                }
            }
            if matches != 1
                || caller_id == callee
                || !matches!(func, Operand::Constant(_))
                || !destination.projection.is_empty()
                || *unwind != UnwindAction::Unreachable
            {
                return Err(rejected(
                    "phase incoming source occurrence is not exact and unique",
                ));
            }
            let block = mapping.block(raw_block.as_u32())?;
            let normal = mapping.block(
                normal
                    .ok_or_else(|| rejected("phase incoming has no normal result"))?
                    .as_u32(),
            )?;
            let SemanticTerminatorKindV1::Call(call) = functions[index]
                .blocks()
                .get(block.index() as usize)
                .ok_or_else(|| rejected("phase mapped incoming block"))?
                .terminator()
                .kind()
            else {
                return Err(rejected("phase mapped incoming is not its retained call"));
            };
            let result = call
                .destination()
                .ok_or_else(|| rejected("phase mapped incoming has no result"))?;
            if callables.get(call.callee().index() as usize)
                != Some(&SemanticCallableDeclV1::defined(callee))
                || result.place().local() != mapping.local(destination.local.as_u32())?
                || !result.place().projections().is_empty()
                || result.edge().role() != SemanticEdgeRoleV1::CallReturn
                || result.edge().target() != normal
                || call.unwind() != SemanticUnwindActionV1::Unreachable
                || !call.variadic_argument_abis().is_empty()
                || call.arguments().len() != args.len()
            {
                return Err(rejected(
                    "phase incoming canonical callee, operand roster or normal-edge substitution",
                ));
            }
            for (original_arg, canonical_arg) in args.iter().zip(call.arguments()) {
                spend(work, plan.type_producers().len().saturating_add(1))?;
                let ty = normalize(
                    tcx,
                    caller.instance,
                    original_arg.node.ty(&original.local_decls, tcx),
                )
                .map_err(|_| rejected("phase incoming original operand normalization"))?;
                if roster.types([ty])?[0] != canonical_arg.ty() {
                    return Err(rejected("phase incoming source operand type substitution"));
                }
            }
            if rows.len() == expected {
                return Err(rejected("phase incoming original roster has an extra call"));
            }
            rows.push(Call {
                caller: caller_id,
                callee,
                caller_instance: caller.instance,
                callee_instance: instance,
                original,
                raw_block,
                block,
                normal,
                call,
            });
        }
    }
    if rows.len() != expected {
        return Err(rejected("phase incoming omitted an original call"));
    }
    // Canonical ordering is independent of rustc block numbering. Insertion
    // comparisons/swaps share the caller's finite work budget; no extra buffer.
    for index in 1..rows.len() {
        let mut slot = index;
        while slot != 0 {
            spend(work, 1)?;
            if rows[slot - 1].key() == rows[slot].key() {
                return Err(rejected("phase incoming duplicate canonical call"));
            }
            if rows[slot - 1].key() < rows[slot].key() {
                break;
            }
            rows.swap(slot - 1, slot);
            slot -= 1;
        }
    }
    // An added inert canonical call cannot hide outside the live producer list.
    let mut observed = 0;
    for (function, body) in functions.iter().enumerate() {
        for (block, data) in body.blocks().iter().enumerate() {
            spend(work, 1 + definitions.len())?;
            let SemanticTerminatorKindV1::Call(call) = data.terminator().kind() else {
                continue;
            };
            let Some(SemanticCallableDeclV1::Defined { function: callee }) =
                callables.get(call.callee().index() as usize)
            else {
                continue;
            };
            if !target(*callee) {
                continue;
            }
            let row = rows
                .get(observed)
                .ok_or_else(|| rejected("phase canonical incoming has an extra call"))?;
            if row.key()
                != (
                    SemanticFunctionIdV1::from_index(function as u32),
                    SemanticBlockIdV1::from_index(block as u32),
                )
                || row.callee != *callee
                || !std::ptr::eq(row.call, call)
            {
                return Err(rejected(
                    "phase canonical incoming roster differs from original source",
                ));
            }
            observed += 1;
        }
    }
    if observed != rows.len() {
        return Err(rejected("phase canonical incoming omitted a live call"));
    }
    Ok(rows)
}
