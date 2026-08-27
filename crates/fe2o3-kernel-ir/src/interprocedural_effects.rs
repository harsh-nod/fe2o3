//! Bounded interprocedural memory-effect summaries for verified Kernel IR.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    FunctionId, FunctionOperationLocation, MemoryEffect, MemoryEffectSummary, Module,
    OperationKind, VerificationErrors, VerifiedKernelIrModuleV1, verify_module_ref,
};

pub const MAX_INTERPROCEDURAL_EFFECT_FUNCTIONS_V1: usize = 4_096;
pub const MAX_INTERPROCEDURAL_EFFECT_CALL_EDGES_V1: usize = 65_536;

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
    Complete(MemoryEffectSummary),
    Incomplete {
        partial: MemoryEffectSummary,
        reasons: Vec<InterproceduralEffectIncompleteReasonV1>,
    },
}

impl InterproceduralEffectDecisionV1 {
    pub const fn is_complete(&self) -> bool {
        matches!(self, Self::Complete(_))
    }

    pub fn summary(&self) -> &MemoryEffectSummary {
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
                    partial: MemoryEffectSummary::pure(),
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
}

impl EffectSummaryBuilderV1<'_> {
    fn summarize(&mut self, function_id: &FunctionId) -> InterproceduralEffectDecisionV1 {
        if let Some(decision) = self.decisions.get(function_id) {
            return decision.clone();
        }
        if !self.visiting.insert(function_id.clone()) {
            return incomplete(
                BTreeSet::new(),
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
        let mut reasons = BTreeSet::new();
        for block in &body.blocks {
            for (operation_index, operation) in block.operations.iter().enumerate() {
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
                        reasons.extend(callee.incomplete_reasons().iter().cloned());
                    }
                    OperationKind::InlineAssembly(_) => {
                        effects.extend(operation.memory_effects());
                        reasons.insert(InterproceduralEffectIncompleteReasonV1::InlineAssembly {
                            function: function_id.clone(),
                            location: FunctionOperationLocation::new(block.id, operation_index),
                        });
                    }
                    _ => effects.extend(operation.memory_effects()),
                }
            }
        }
        self.visiting.remove(function_id);
        let decision = if reasons.is_empty() {
            InterproceduralEffectDecisionV1::Complete(MemoryEffectSummary::new(effects))
        } else {
            incomplete(effects, reasons)
        };
        self.decisions.insert(function_id.clone(), decision.clone());
        decision
    }
}

fn incomplete(
    effects: impl IntoIterator<Item = MemoryEffect>,
    reasons: impl IntoIterator<Item = InterproceduralEffectIncompleteReasonV1>,
) -> InterproceduralEffectDecisionV1 {
    InterproceduralEffectDecisionV1::Incomplete {
        partial: MemoryEffectSummary::new(effects),
        reasons: reasons
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
    }
}
