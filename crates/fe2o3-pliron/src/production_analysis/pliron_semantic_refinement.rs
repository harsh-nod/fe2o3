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
    run_pliron_effect_refinement_with_analyses_v1,
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
include!("pliron_semantic_refinement/contracts_v1.rs");
include!("pliron_semantic_refinement/resource_tests.rs");
