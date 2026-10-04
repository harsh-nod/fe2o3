//! Generic declared semantic-equivalence verification for PLIRON SSA.

use std::{
    collections::{HashMap, HashSet},
    fmt::{self, Write as _},
};

use dialect_kernel::{
    MAX_SEMANTIC_TYPED_EXPRESSION_NODES_V1, OwnershipContractOp, OwnershipCoverageAttr,
    RequireEquivalentOp, RequireFiniteFoldOp, RequireFiniteRecurrenceOp,
    RequirePermutationGatherOp, SemanticBinaryKindAttr, SemanticBinaryOp, SemanticConstantOp,
    SemanticCoverageBindingAttr, SemanticExpressionCommitmentOp, SemanticNumericalContractV1,
    SemanticNumericalPolicyAttr, SemanticSymbolOp, SemanticTypedBinaryOp, SemanticTypedCastOp,
    SemanticTypedCompareOp, SemanticTypedConstantOp, SemanticTypedExpressionRootOp,
    SemanticTypedExpressionV1, SemanticTypedScalarV1, SemanticTypedSelectOp, SemanticTypedSymbolOp,
    SemanticTypedUnaryOp, TensorResultComponentOp,
};
use dialect_proof::{
    CoveredBoundaryAttr, EvidenceRefOp, EvidenceStatusAttr, ObligationOp, PropertyAttr,
    RequireEffectRefinementOp, RequireNumericalRefinementOp, RequireRefinementOp,
    RequireTensorRefinementOp,
};
use fe2o3_kernel_ir::{MatrixElement, TensorOperandRoleV1};
use pliron::{builtin::ops::FuncOp, common_traits::Named, context::Context, operation::Operation};

use crate::production_analysis::pliron_analysis_manager::PlironAnalysisManagerV1;
use crate::production_analysis::pliron_effect_refinement::{
    PlironEffectRefinementReportV1, clean_effect_refinement_report_v1,
    run_pliron_effect_refinement_with_observation_v1,
};
use crate::production_analysis::pliron_function_inventory::BoundedPlironFunctionInventoryV1;
use crate::production_analysis::pliron_progress::PlironProgressReportV1;
#[cfg(test)]
use crate::production_analysis::pliron_progress::run_pliron_progress_check_v1;
#[cfg(test)]
use crate::production_analysis::pliron_ranked_bounds::run_pliron_ranked_bounds_check_with_analyses_v1;
use crate::production_analysis::pliron_resource_envelope::{
    ProductionAnalysisInputCensusV1, ProductionAnalysisResourceLimitV1,
    ProductionAnalysisResourceLimitsV1, ProductionAnalysisResourcePhaseV1,
    ProductionAnalysisResourceUpperBoundV1,
};
use crate::{KernelCheckPassKindV1, KernelCheckStatusV1};

pub const MAX_PLIRON_SEMANTIC_NODES_V1: usize = 65_536;
pub const MAX_PLIRON_SEMANTIC_FINDINGS_V1: usize = 4_096;
pub(crate) const MAX_PLIRON_SEMANTIC_DIAGNOSTIC_BYTES_V1: usize = 4_096;

struct BoundedSemanticDescriptionWriterV1 {
    output: String,
    truncated: bool,
}

impl BoundedSemanticDescriptionWriterV1 {
    fn new() -> Self {
        Self {
            output: String::with_capacity(MAX_PLIRON_SEMANTIC_DIAGNOSTIC_BYTES_V1),
            truncated: false,
        }
    }

    fn finish(mut self) -> String {
        if self.truncated {
            self.output.push_str("...");
        }
        self.output
    }
}

impl fmt::Write for BoundedSemanticDescriptionWriterV1 {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if self.truncated {
            return Err(fmt::Error);
        }
        let payload_limit = MAX_PLIRON_SEMANTIC_DIAGNOSTIC_BYTES_V1 - 3;
        let remaining = payload_limit.saturating_sub(self.output.len());
        if value.len() <= remaining {
            self.output.push_str(value);
            return Ok(());
        }
        let mut end = remaining;
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        self.output.push_str(&value[..end]);
        self.truncated = true;
        Err(fmt::Error)
    }
}

include!("pliron_semantic_refinement/resources_v1.rs");
include!("pliron_semantic_refinement/expression_table_v1.rs");
include!("pliron_semantic_refinement/reports_v1.rs");

#[path = "pliron_semantic_refinement/conditional_v1.rs"]
pub(crate) mod conditional_semantic_v1;

struct SemanticExecutionV1<'a> {
    context: &'a Context,
    function: &'a FuncOp,
    analyses: &'a mut PlironAnalysisManagerV1,
}

type SemanticObserverV1<'o, 'p, 'r> = Option<&'o InvocationObserverV1<'p, 'r>>;

trait SemanticModeV1: Sized {
    type Effect;
    fn effect(
        self,
        execution: SemanticExecutionV1<'_>,
        observer: SemanticObserverV1<'_, '_, '_>,
    ) -> Self::Effect;
    fn selected_view(&self, _: pliron::value::Value) -> bool {
        false
    }
}

#[derive(Debug, Eq, PartialEq)]
enum EffectRunV1<E> {
    NotRun,
    Executed(E),
}

#[derive(Debug, Eq, PartialEq)]
struct SemanticBodyV1<E> {
    findings: Vec<PlironSemanticRefinementFindingV1>,
    reference_obligations: usize,
    policy_checked_reference_obligations: usize,
    numerical_obligations: usize,
    policy_checked_numerical_obligations: usize,
    collective_contracts: usize,
    policy_checked_collective_contracts: usize,
    typed_root_commitments: Vec<[u64; 4]>,
    numerical_certificates: Vec<PlironNumericalBoundCertificateV1>,
    progress: PlironProgressReportV1,
    effect_refinement: EffectRunV1<E>,
}

struct OrdinarySemanticModeV1;

impl SemanticModeV1 for OrdinarySemanticModeV1 {
    type Effect = PlironEffectRefinementReportV1;
    fn effect(
        self,
        execution: SemanticExecutionV1<'_>,
        observer: SemanticObserverV1<'_, '_, '_>,
    ) -> Self::Effect {
        let SemanticExecutionV1 {
            context,
            function,
            analyses,
        } = execution;
        run_pliron_effect_refinement_with_observation_v1(context, function, analyses, observer)
    }
}

fn semantic_early_v1<E>(
    findings: Vec<PlironSemanticRefinementFindingV1>,
    progress: PlironProgressReportV1,
) -> SemanticBodyV1<E> {
    SemanticBodyV1 {
        findings,
        reference_obligations: 0,
        policy_checked_reference_obligations: 0,
        numerical_obligations: 0,
        policy_checked_numerical_obligations: 0,
        collective_contracts: 0,
        policy_checked_collective_contracts: 0,
        typed_root_commitments: Vec::new(),
        numerical_certificates: Vec::new(),
        progress,
        effect_refinement: EffectRunV1::NotRun,
    }
}

impl SemanticBodyV1<PlironEffectRefinementReportV1> {
    fn into_ordinary(self) -> PlironSemanticRefinementReportV1 {
        // Preserve the historical ordinary early-error representation only in
        // this adapter. Conditional reports keep actual progress and NotRun.
        let (progress, effect_refinement) = match self.effect_refinement {
            EffectRunV1::Executed(effect) => (self.progress, effect),
            EffectRunV1::NotRun => (
                PlironProgressReportV1::clean(),
                clean_effect_refinement_report_v1(),
            ),
        };
        PlironSemanticRefinementReportV1 {
            findings: self.findings,
            reference_obligations: self.reference_obligations,
            policy_checked_reference_obligations: self.policy_checked_reference_obligations,
            numerical_obligations: self.numerical_obligations,
            policy_checked_numerical_obligations: self.policy_checked_numerical_obligations,
            collective_contracts: self.collective_contracts,
            policy_checked_collective_contracts: self.policy_checked_collective_contracts,
            typed_root_commitments: self.typed_root_commitments,
            numerical_certificates: self.numerical_certificates,
            progress,
            effect_refinement,
        }
    }
}

include!("pliron_semantic_refinement/contracts_v1.rs");
include!("pliron_semantic_refinement/resource_tests.rs");
