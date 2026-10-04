//! Private borrowed checks for the compiler-appended Trap declaration only.
//! Coordinates are consumed from the SAME actual pre-ranked owner at the caller.
//! Pure tests cannot turn these predicates into a source/SSA/canonical owner.
use super::*;
use fe2o3_kernel_ir::{
    AMDGPU_DIAGNOSTICS_CAPABILITY_NAME, AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE, FunctionRole,
    TargetCapability,
};
use fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1;

#[cfg(test)]
#[path = "production_tiled_region_declaration_v1_tests.rs"]
mod tests;

const TRAP: &str = "__fe2o3_ir_amdgpu_diagnostics_gfx942_v1_trap";
// Actual emitter: <=2 prologue spans per source block and <=1 shared Trap.
const SYNTHETIC_SPANS: usize = 2 * SOURCE_BLOCKS + 1;

pub(super) fn select_root<'a>(
    module: &'a Module,
    relations: &[SemanticKirFunctionCorrespondenceV1],
    root: SemanticFunctionIdV1,
    budget: &mut Budget<'_>,
) -> Result<(&'a Function, bool)> {
    budget.charge_work(8)?;
    let ([kernel], [relation]) = (module.kernels.as_slice(), relations) else {
        return unavailable("one actual kernel/source function relation required");
    };
    let (function, declaration) = match module.functions.as_slice() {
        [function] => (function, None),
        [function, declaration] => (function, Some(declaration)),
        _ => return unavailable("only actual root then optional compiler Trap required"),
    };
    // str equality first compares lengths, then at most the left-hand bytes.
    // Pay every possible byte comparison before inspecting the two identities.
    let id_work = kernel
        .entry
        .as_str()
        .len()
        .checked_add(relation.kernel_ir_function().as_str().len())
        .and_then(|work| work.checked_add(10))
        .ok_or(Resource::Arithmetic)?;
    budget.charge_work(id_work)?;
    if function.role != FunctionRole::KernelEntry
        || function.body.is_none()
        || kernel.entry != function.id
        || relation.semantic_function() != root
        || relation.correspondence_owner() != root
        || relation.role() != SemanticKirFunctionRoleV1::KernelEntry
        || relation.kernel_ir_function() != &function.id
    {
        return unavailable("canonical/source function relation differs");
    }
    if let Some(declaration) = declaration {
        exact_trap_declaration(declaration, budget)?;
    }
    Ok((function, declaration.is_some()))
}

fn exact_trap_declaration(function: &Function, budget: &mut Budget<'_>) -> Result<()> {
    // Complete Function contract by borrowed fields. No declaration/ID/capability
    // constructor or generic external-import predicate is invoked.
    budget.charge_work(
        TRAP.len()
            + AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE.len()
            + AMDGPU_DIAGNOSTICS_CAPABILITY_NAME.len()
            + 16,
    )?;
    if function.id.as_str() != TRAP
        || function.role != FunctionRole::ExternalImport
        || function.body.is_some()
        || !function.signature.parameters.is_empty()
        || !function.signature.results.is_empty()
        || function.required_capabilities.len() != 1
    {
        return unavailable("appended Trap declaration contract differs");
    }
    if !matches!(function.required_capabilities.iter().next(),
        Some(TargetCapability::Extension { namespace, name })
            if namespace == AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE
                && name == AMDGPU_DIAGNOSTICS_CAPABILITY_NAME)
    {
        return unavailable("appended Trap capability is not the current emitter contract");
    }
    Ok(())
}

pub(super) fn require_declaration_use(
    declared: bool,
    calls: usize,
    budget: &mut Budget<'_>,
) -> Result<()> {
    budget.charge_work(1)?;
    if declared != (calls != 0) {
        return unavailable("Trap declaration/call origin roster differs");
    }
    Ok(())
}

// Borrowed stack-only selectors. They cannot escape seal, own a receipt, or
// authorize another source/canonical graph. Existing original ledger pays scans.
pub(super) struct Origins<'a> {
    root: SemanticFunctionIdV1,
    blocks: &'a [SemanticBasicBlockV1],
    callables: &'a [SemanticCallableDeclV1],
    terminators: &'a [SemanticKirTerminatorOperationSpanV1],
    synthetic: &'a [SemanticKirSyntheticOperationSpanV1],
}
impl<'a> Origins<'a> {
    pub(super) fn from_owner(
        owner: &'a ProductionPreRankedKirOwnerV1,
        root: SemanticFunctionIdV1,
        budget: &mut Budget<'_>,
    ) -> Result<Self> {
        budget.charge_work(3)?;
        let semantic = owner.semantic_ssa().source_semantic();
        let [function] = semantic.functions() else {
            return unavailable("Trap origins require the same single source root");
        };
        if root.index() != 0 {
            return unavailable("Trap source root coordinate differs");
        }
        Self::new(
            root,
            function.blocks(),
            semantic.callables(),
            owner.correspondence.terminator_operation_spans(),
            owner.correspondence.synthetic_operation_spans(),
            budget,
        )
    }
    fn new(
        root: SemanticFunctionIdV1,
        blocks: &'a [SemanticBasicBlockV1],
        callables: &'a [SemanticCallableDeclV1],
        terminators: &'a [SemanticKirTerminatorOperationSpanV1],
        synthetic: &'a [SemanticKirSyntheticOperationSpanV1],
        budget: &mut Budget<'_>,
    ) -> Result<Self> {
        budget.charge_work(4)?;
        if blocks.len() > SOURCE_BLOCKS
            || terminators.len() > SOURCE_BLOCKS
            || synthetic.len() > SYNTHETIC_SPANS
        {
            return unavailable("Trap origin roster exceeds finite source bounds");
        }
        Ok(Self {
            root,
            blocks,
            callables,
            terminators,
            synthetic,
        })
    }

    pub(super) fn validate_call(
        &self,
        block: &BasicBlock,
        ordinal: usize,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        budget.charge_work(TRAP.len() + 12)?;
        let operation = block
            .operations
            .get(ordinal)
            .ok_or(Error::Unavailable("Trap operation coordinate"))?;
        let OperationKind::Call { callee, arguments } = &operation.kind else {
            return unavailable("non-call cannot establish a Trap origin");
        };
        if callee.as_str() != TRAP
            || !arguments.is_empty()
            || !operation.results.is_empty()
            || ordinal.checked_add(1) != Some(block.operations.len())
            || !matches!(block.terminator, Some(Terminator::Unreachable))
        {
            return unavailable("actual Trap call/continuation shape differs");
        }
        // This registered effects check is allocation-free and charges its own
        // classification. The Trap is retained, never relabeled pure/returning.
        if !operation.has_complete_effect_summary_with_budget_v1(budget)? {
            return unavailable("actual Trap registered effects unavailable");
        }
        let ordinal = u32::try_from(ordinal).map_err(|_| Resource::Arithmetic)?;
        let mut matches = 0usize;
        for span in self.terminators {
            budget.charge_work(12)?;
            if span.kernel_ir_block() != block.id
                || !covers(
                    span.first_operation_ordinal(),
                    span.operation_count(),
                    ordinal,
                )?
            {
                continue;
            }
            if span.correspondence_owner() != self.root
                || span.semantic_function() != self.root
                || span.first_operation_ordinal() != ordinal
                || span.operation_count() != 1
            {
                return unavailable("Trap source span owner/coverage differs");
            }
            self.require_source_trap(span.semantic_block(), budget)?;
            unique_origin(&mut matches)?;
        }
        for span in self.synthetic {
            budget.charge_work(12)?;
            if span.kernel_ir_block() != block.id
                || !covers(
                    span.first_operation_ordinal(),
                    span.operation_count(),
                    ordinal,
                )?
            {
                continue;
            }
            if span.correspondence_owner() != self.root
                || span.semantic_function() != self.root
                || span.rule() != SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap
                || span.first_operation_ordinal() != 0
                || span.operation_count() != 1
                || ordinal != 0
                || block.operations.len() != 1
            {
                return unavailable("Trap synthetic span owner/rule/coverage differs");
            }
            unique_origin(&mut matches)?;
        }
        budget.charge_work(1)?;
        if matches != 1 {
            return unavailable("Trap lacks one actual source or synthetic origin");
        }
        Ok(())
    }

    fn require_source_trap(&self, id: SemanticBlockIdV1, budget: &mut Budget<'_>) -> Result<()> {
        budget.charge_work(12)?;
        let source = self
            .blocks
            .get(id.index() as usize)
            .ok_or(Error::Unavailable(
                "Trap semantic block outside actual root",
            ))?;
        let SemanticTerminatorKindV1::Call(call) = source.terminator().kind() else {
            return unavailable("Trap span is not an actual source call");
        };
        if !call.arguments().is_empty()
            || !call.variadic_argument_abis().is_empty()
            || call.destination().is_some()
            || matches!(call.unwind(), SemanticUnwindActionV1::Cleanup(_))
            || !matches!(
                self.callables.get(call.callee().index() as usize),
                Some(SemanticCallableDeclV1::CompilerIntrinsic {
                    operation: SemanticCompilerIntrinsicOperationV1::Trap,
                    ..
                })
            )
        {
            return unavailable("Trap source call is not the exact compiler intrinsic");
        }
        Ok(())
    }
}
fn covers(first: u32, count: u32, ordinal: u32) -> Result<bool> {
    let end = first.checked_add(count).ok_or(Resource::Arithmetic)?;
    Ok(first <= ordinal && ordinal < end)
}
fn unique_origin(matches: &mut usize) -> Result<()> {
    if *matches != 0 {
        return unavailable("duplicate Trap source/synthetic origin");
    }
    *matches = 1;
    Ok(())
}
