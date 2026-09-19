//! Bounded interprocedural physical-memory and compiler-order summaries for verified Kernel IR.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    AssemblyOption, CompilerOrderingEffectSummaryV12, Function, FunctionId,
    FunctionOperationLocation, MemoryEffect, MemoryEffectSummary, Module, Operation,
    OperationEffectSummaryV12, OperationKind, ScalarType, Type, ValueId, VerificationErrors,
    VerifiedKernelIrModuleV1, validate_gfx942_inline_assembly_v1, verify_module_ref,
};

pub const MAX_INTERPROCEDURAL_EFFECT_FUNCTIONS_V1: usize = 4_096;
pub const MAX_INTERPROCEDURAL_EFFECT_CALL_EDGES_V1: usize = 65_536;
// Cumulative across the complete analysis, charged before allocating any
// assembly type table. No table is retained across recursive call traversal.
const MAX_INTERPROCEDURAL_ASSEMBLY_TYPE_WORK_V1: usize = 1_048_576;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum InterproceduralEffectIncompleteReasonV1 {
    FunctionDeclaration {
        function: FunctionId,
    },
    RecursiveCallCycle {
        function: FunctionId,
    },
    InlineAssembly {
        function: FunctionId,
        location: FunctionOperationLocation,
    },
    ResourceLimit {
        resource: &'static str,
        limit: usize,
        actual: usize,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InterproceduralEffectDecisionV1 {
    Complete(OperationEffectSummaryV12),
    Incomplete {
        partial: OperationEffectSummaryV12,
        reasons: Vec<InterproceduralEffectIncompleteReasonV1>,
    },
}

impl InterproceduralEffectDecisionV1 {
    pub const fn is_complete(&self) -> bool {
        matches!(self, Self::Complete(_))
    }

    pub fn summary(&self) -> &OperationEffectSummaryV12 {
        match self {
            Self::Complete(summary)
            | Self::Incomplete {
                partial: summary, ..
            } => summary,
        }
    }

    pub fn incomplete_reasons(&self) -> &[InterproceduralEffectIncompleteReasonV1] {
        match self {
            Self::Complete(_) => &[],
            Self::Incomplete { reasons, .. } => reasons,
        }
    }

    pub fn is_complete_and_pure(&self) -> bool {
        matches!(self, Self::Complete(summary) if summary.is_pure())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InterproceduralEffectAnalysisV1 {
    functions: BTreeMap<FunctionId, InterproceduralEffectDecisionV1>,
}

impl InterproceduralEffectAnalysisV1 {
    pub fn function(&self, function: &FunctionId) -> Option<&InterproceduralEffectDecisionV1> {
        self.functions.get(function)
    }

    pub fn functions(&self) -> &BTreeMap<FunctionId, InterproceduralEffectDecisionV1> {
        &self.functions
    }
}

pub fn analyze_interprocedural_effects_v1(
    module: &Module,
) -> Result<InterproceduralEffectAnalysisV1, VerificationErrors> {
    analyze_interprocedural_effects_from_verified_v1(verify_module_ref(module)?)
}

pub fn analyze_interprocedural_effects_from_verified_v1(
    verified: VerifiedKernelIrModuleV1<'_>,
) -> Result<InterproceduralEffectAnalysisV1, VerificationErrors> {
    let module = verified.module();
    let mut analysis = EffectSummaryBuilderV1 {
        module,
        decisions: BTreeMap::new(),
        visiting: BTreeSet::new(),
        call_edges: 0,
        assembly_type_budget: AssemblyTypeBudgetV1 {
            used: 0,
            limit: MAX_INTERPROCEDURAL_ASSEMBLY_TYPE_WORK_V1,
        },
    };
    if module.functions.len() > MAX_INTERPROCEDURAL_EFFECT_FUNCTIONS_V1 {
        let reason = InterproceduralEffectIncompleteReasonV1::ResourceLimit {
            resource: "function",
            limit: MAX_INTERPROCEDURAL_EFFECT_FUNCTIONS_V1,
            actual: module.functions.len(),
        };
        for function in &module.functions {
            analysis.decisions.insert(
                function.id.clone(),
                InterproceduralEffectDecisionV1::Incomplete {
                    partial: OperationEffectSummaryV12::pure(),
                    reasons: vec![reason.clone()],
                },
            );
        }
        return Ok(InterproceduralEffectAnalysisV1 {
            functions: analysis.decisions,
        });
    }
    for function in &module.functions {
        analysis.summarize(&function.id);
    }
    Ok(InterproceduralEffectAnalysisV1 {
        functions: analysis.decisions,
    })
}

struct EffectSummaryBuilderV1<'module> {
    module: &'module Module,
    decisions: BTreeMap<FunctionId, InterproceduralEffectDecisionV1>,
    visiting: BTreeSet<FunctionId>,
    call_edges: usize,
    assembly_type_budget: AssemblyTypeBudgetV1,
}

impl EffectSummaryBuilderV1<'_> {
    fn summarize(&mut self, function_id: &FunctionId) -> InterproceduralEffectDecisionV1 {
        if let Some(decision) = self.decisions.get(function_id) {
            return decision.clone();
        }
        if !self.visiting.insert(function_id.clone()) {
            return incomplete(
                BTreeSet::new(),
                CompilerOrderingEffectSummaryV12::empty(),
                [
                    InterproceduralEffectIncompleteReasonV1::RecursiveCallCycle {
                        function: function_id.clone(),
                    },
                ],
            );
        }
        let function = self
            .module
            .function(function_id)
            .expect("verified call graph references only declared functions");
        let Some(body) = &function.body else {
            let decision = incomplete(
                BTreeSet::new(),
                CompilerOrderingEffectSummaryV12::empty(),
                [
                    InterproceduralEffectIncompleteReasonV1::FunctionDeclaration {
                        function: function_id.clone(),
                    },
                ],
            );
            self.visiting.remove(function_id);
            self.decisions.insert(function_id.clone(), decision.clone());
            return decision;
        };

        let mut effects = BTreeSet::<MemoryEffect>::new();
        let mut compiler_ordering = CompilerOrderingEffectSummaryV12::empty();
        let mut reasons = assembly_incomplete_reasons_v30(function, &mut self.assembly_type_budget);
        for block in &body.blocks {
            for operation in &block.operations {
                compiler_ordering =
                    compiler_ordering.union(operation.compiler_ordering_effects_v12());
                match &operation.kind {
                    OperationKind::Call { .. } if operation.has_complete_effect_summary() => {
                        effects.extend(operation.memory_effects());
                    }
                    OperationKind::Call { callee, .. } => {
                        self.call_edges = self
                            .call_edges
                            .saturating_add(1)
                            .min(MAX_INTERPROCEDURAL_EFFECT_CALL_EDGES_V1 + 1);
                        if self.call_edges > MAX_INTERPROCEDURAL_EFFECT_CALL_EDGES_V1 {
                            reasons.insert(
                                InterproceduralEffectIncompleteReasonV1::ResourceLimit {
                                    resource: "call edge",
                                    limit: MAX_INTERPROCEDURAL_EFFECT_CALL_EDGES_V1,
                                    actual: self.call_edges,
                                },
                            );
                            continue;
                        }
                        let callee = self.summarize(callee);
                        effects.extend(callee.summary().effects().iter().cloned());
                        compiler_ordering =
                            compiler_ordering.union(callee.summary().compiler_ordering());
                        reasons.extend(callee.incomplete_reasons().iter().cloned());
                    }
                    OperationKind::InlineAssembly(_) => {
                        // The nonrecursive leaf checked every assembly operation
                        // against this function's complete actual SSA type table.
                        effects.extend(operation.memory_effects());
                    }
                    _ => effects.extend(operation.memory_effects()),
                }
            }
        }
        self.visiting.remove(function_id);
        let decision = if reasons.is_empty() {
            InterproceduralEffectDecisionV1::Complete(OperationEffectSummaryV12::new(
                MemoryEffectSummary::new(effects),
                compiler_ordering,
            ))
        } else {
            incomplete(effects, compiler_ordering, reasons)
        };
        self.decisions.insert(function_id.clone(), decision.clone());
        decision
    }
}

struct AssemblyTypeBudgetV1 {
    used: usize,
    limit: usize,
}

// Keep assembly-only iterator, map, validation, and failure temporaries out of
// the recursive summary frame, including for long assembly-free call chains.
// The table is discarded before following any callee; the cumulative work
// budget and all malformed-operation reasons remain unchanged.
#[inline(never)]
fn assembly_incomplete_reasons_v30(
    function: &Function,
    budget: &mut AssemblyTypeBudgetV1,
) -> BTreeSet<InterproceduralEffectIncompleteReasonV1> {
    let body = function
        .body
        .as_ref()
        .expect("verified definition has a body");
    let mut reasons = BTreeSet::new();
    if !body
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .any(|operation| matches!(operation.kind, OperationKind::InlineAssembly(_)))
    {
        return reasons;
    }
    let types = match collect_assembly_value_types_v1(function, budget) {
        Ok(types) => Some(types),
        Err(reason) => {
            reasons.insert(reason);
            None
        }
    };
    for block in &body.blocks {
        for (operation_index, operation) in block.operations.iter().enumerate() {
            if matches!(operation.kind, OperationKind::InlineAssembly(_))
                && !types
                    .as_ref()
                    .is_some_and(|types| is_closed_u32_assembly_effect_v30(operation, types))
            {
                reasons.insert(InterproceduralEffectIncompleteReasonV1::InlineAssembly {
                    function: function.id.clone(),
                    location: FunctionOperationLocation::new(block.id, operation_index),
                });
            }
        }
    }
    reasons
}

impl AssemblyTypeBudgetV1 {
    fn charge(&mut self, amount: usize) -> Result<(), InterproceduralEffectIncompleteReasonV1> {
        self.used = self.used.saturating_add(amount);
        if self.used > self.limit {
            return Err(InterproceduralEffectIncompleteReasonV1::ResourceLimit {
                resource: "inline assembly type work",
                limit: self.limit,
                actual: self.used,
            });
        }
        Ok(())
    }
}

fn collect_assembly_value_types_v1<'a>(
    function: &'a Function,
    budget: &mut AssemblyTypeBudgetV1,
) -> Result<BTreeMap<ValueId, &'a Type>, InterproceduralEffectIncompleteReasonV1> {
    let body = function
        .body
        .as_ref()
        .expect("verified definition has a body");
    budget.charge(body.parameters.len())?;
    budget.charge(body.blocks.len())?;
    for block in &body.blocks {
        budget.charge(block.parameters.len())?;
        budget.charge(block.operations.len())?;
        for operation in &block.operations {
            budget.charge(operation.results.len())?;
        }
    }
    // Verification has already established unique SSA definitions, exact
    // signature cardinality, references, and dominance. Borrow types rather
    // than cloning arbitrarily nested type payloads into analysis storage.
    let mut types: BTreeMap<_, _> = body
        .parameters
        .iter()
        .copied()
        .zip(function.signature.parameters.iter())
        .collect();
    for block in &body.blocks {
        types.extend(block.parameters.iter().map(|value| (value.id, &value.ty)));
        types.extend(
            block
                .operations
                .iter()
                .flat_map(|operation| &operation.results)
                .map(|value| (value.id, &value.ty)),
        );
    }
    Ok(types)
}

fn is_closed_u32_assembly_effect_v30(
    operation: &Operation,
    types: &BTreeMap<ValueId, &Type>,
) -> bool {
    let OperationKind::InlineAssembly(assembly) = &operation.kind else {
        return false;
    };
    if assembly.options.len() != 1
        || !assembly.options.contains(&AssemblyOption::NoMemory)
        || !assembly.declared_effects.is_empty()
    {
        return false;
    }
    validate_gfx942_inline_assembly_v1(operation, |value| {
        types.get(&value).and_then(|ty| ty.as_scalar())
    })
    .is_ok_and(|validated| {
        use crate::Gfx942InlineAssemblyInstructionV1 as Instruction;
        validated.scalar_type() == ScalarType::U32
            && matches!(
                validated.instruction(),
                Instruction::VMovB32
                    | Instruction::VAddU32
                    | Instruction::VSubU32
                    | Instruction::VAndB32
                    | Instruction::VOrB32
                    | Instruction::VXorB32
            )
    })
}

#[cfg(test)]
#[path = "interprocedural_effects_gfx942_v30_tests.rs"]
mod gfx942_v30_tests;

fn incomplete(
    effects: impl IntoIterator<Item = MemoryEffect>,
    compiler_ordering: CompilerOrderingEffectSummaryV12,
    reasons: impl IntoIterator<Item = InterproceduralEffectIncompleteReasonV1>,
) -> InterproceduralEffectDecisionV1 {
    InterproceduralEffectDecisionV1::Incomplete {
        partial: OperationEffectSummaryV12::new(
            MemoryEffectSummary::new(effects),
            compiler_ordering,
        ),
        reasons: reasons
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
    }
}
