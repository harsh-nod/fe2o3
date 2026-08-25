//! Non-vacuous composition of production coverage and functional-refinement facts.
//!
//! This is a post-analysis admission gate, not another graph analysis pass. It
//! composes the exact reports produced by the mandatory PLIRON passes with the
//! retained safe-reference receipts and typed semantic evidence. Its theorem
//! ends at the safe-reference-MIR to kernel-MIR boundary.

use std::{error::Error, fmt};

use super::middle_end_evidence_v4::derive_ranked_kernel_identity;
use super::{
    ProductionMiddleEndEvidenceV5, ProductionRankedKernelErrorV1,
    ProductionRankedKernelLoweringInputV1, ProductionTypedSemanticObligationSummaryV2,
    typed_semantic_obligation_summary_v2,
};

/// Arithmetic strength represented by one admitted total-output refinement.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionArithmeticAssuranceV2 {
    /// Exact interpreted Boolean and fixed-width integer values at the MIR boundary.
    ExactBitVectorValues,
    /// IEEE operator identity/congruence only, not a target IEEE value theorem.
    IeeeOperatorCongruenceOnly,
    /// Exact bitvector values plus separately bounded IEEE operator congruence.
    ExactBitVectorsAndIeeeOperatorCongruence,
}

/// Non-forgeable-by-construction summary returned only by the aggregate gate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionTotalOutputRefinementReportV2 {
    total_view_contracts: u64,
    reference_obligations: u64,
    effect_contracts: u64,
    collective_contracts: u64,
    typed_expression_roots: u64,
    retained_receipts: u64,
    arithmetic: ProductionArithmeticAssuranceV2,
    evidence_identity: [u8; 32],
}

impl ProductionTotalOutputRefinementReportV2 {
    pub const fn total_view_contracts(self) -> u64 {
        self.total_view_contracts
    }

    pub const fn reference_obligations(self) -> u64 {
        self.reference_obligations
    }

    pub const fn effect_contracts(self) -> u64 {
        self.effect_contracts
    }

    pub const fn collective_contracts(self) -> u64 {
        self.collective_contracts
    }

    pub const fn typed_expression_roots(self) -> u64 {
        self.typed_expression_roots
    }

    pub const fn retained_receipts(self) -> u64 {
        self.retained_receipts
    }

    pub const fn arithmetic_assurance(self) -> ProductionArithmeticAssuranceV2 {
        self.arithmetic
    }

    pub const fn evidence_identity(&self) -> &[u8; 32] {
        &self.evidence_identity
    }

    /// Every finite output coordinate is owned once and its unique global write
    /// is related to the safe CPU reference at the MIR boundary.
    pub const fn proves_total_output_refinement_at_mir_boundary(self) -> bool {
        true
    }

    pub const fn grants_source_to_mir_authority(self) -> bool {
        false
    }

    pub const fn grants_lowering_or_machine_code_authority(self) -> bool {
        false
    }

    pub const fn grants_target_ieee_value_authority(self) -> bool {
        false
    }

    pub const fn grants_artifact_load_launch_or_hardware_authority(self) -> bool {
        false
    }

    pub const fn proves_universal_kernel_correctness(self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionTotalOutputRefinementErrorV2 {
    RankedKernel(ProductionRankedKernelErrorV1),
    CounterOverflow,
    EvidenceIdentityMismatch,
    MandatoryReportNotClean,
    CoverageSummaryMismatch,
    SemanticSummaryMismatch,
    TypedSummaryMismatch,
    MissingTotalViewProof,
    MissingEffectProof,
    MissingRetainedReceipt,
    MissingTypedSemanticProof,
    UndischargedTypedSemanticDomain,
    TypedSemanticReconciliationMismatch,
    CollectiveParticipationWithoutValueProof,
}

impl fmt::Display for ProductionTotalOutputRefinementErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RankedKernel(error) => write!(formatter, "ranked kernel is invalid: {error}"),
            Self::CounterOverflow => formatter.write_str("total-output proof counter overflow"),
            Self::EvidenceIdentityMismatch => formatter.write_str(
                "V5 evidence does not identify the live ranked kernel or its canonical record",
            ),
            Self::MandatoryReportNotClean => {
                formatter.write_str("a mandatory production PLIRON report is not clean")
            }
            Self::CoverageSummaryMismatch => formatter
                .write_str("V5 coverage counters do not match the live ownership report"),
            Self::SemanticSummaryMismatch => formatter
                .write_str("V5 semantic counters do not match the live semantic report"),
            Self::TypedSummaryMismatch => formatter
                .write_str("V5 typed-expression counters do not match the live ranked recipe"),
            Self::MissingTotalViewProof => formatter.write_str(
                "total-output refinement requires at least one non-vacuous TotalView proof",
            ),
            Self::MissingEffectProof => formatter.write_str(
                "total-output refinement requires a proved reference-effect contract for every global write",
            ),
            Self::MissingRetainedReceipt => formatter.write_str(
                "total-output refinement requires retained authenticated functional-refinement receipts",
            ),
            Self::MissingTypedSemanticProof => formatter.write_str(
                "total-output refinement requires at least one typed semantic expression root",
            ),
            Self::UndischargedTypedSemanticDomain => formatter.write_str(
                "a typed semantic expression has an arithmetic domain that was not statically discharged",
            ),
            Self::TypedSemanticReconciliationMismatch => formatter.write_str(
                "retained typed expressions do not reconcile exactly with the live PLIRON graph",
            ),
            Self::CollectiveParticipationWithoutValueProof => formatter.write_str(
                "collective participation coverage is present without a non-vacuous proved finite collective value contract",
            ),
        }
    }
}

impl Error for ProductionTotalOutputRefinementErrorV2 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::RankedKernel(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
struct ObservedTotalOutputFactsV2 {
    reports_clean: bool,
    total_view_declared: u64,
    total_view_proved: u64,
    collective_contributions_declared: u64,
    collective_contributions_proved: u64,
    reference_declared: u64,
    reference_proved: u64,
    effect_declared: u64,
    effect_proved: u64,
    collective_declared: u64,
    collective_proved: u64,
    typed: ProductionTypedSemanticObligationSummaryV2,
    reconciliation_recipe_roots: u64,
    reconciliation_pliron_roots: u64,
    reconciliation_exact: bool,
    retained_receipts: u64,
    evidence_identity: [u8; 32],
}

/// Composes total output coverage with independently proved CPU-reference effects.
pub fn require_total_output_refinement_v2(
    ranked: &ProductionRankedKernelLoweringInputV1,
    evidence: &ProductionMiddleEndEvidenceV5,
) -> Result<ProductionTotalOutputRefinementReportV2, ProductionTotalOutputRefinementErrorV2> {
    if derive_ranked_kernel_identity(ranked) != *evidence.ranked_kernel_identity()
        || !evidence
            .identity()
            .matches_canonical_bytes(evidence.canonical_bytes())
    {
        return Err(ProductionTotalOutputRefinementErrorV2::EvidenceIdentityMismatch);
    }

    let coverage = ranked.ownership_report().coverage_summary();
    let semantic = ranked.semantic_report();
    let effects = semantic.effect_refinement();
    let typed = typed_semantic_obligation_summary_v2(ranked.kernel())
        .map_err(ProductionTotalOutputRefinementErrorV2::RankedKernel)?;
    let reconciliation = evidence.typed_semantic_reconciliation();
    let facts = ObservedTotalOutputFactsV2 {
        reports_clean: ranked.all_mandatory_reports_are_clean(),
        total_view_declared: as_u64(coverage.total_view_declared())?,
        total_view_proved: as_u64(coverage.total_view_proved())?,
        collective_contributions_declared: as_u64(coverage.collective_contributions_declared())?,
        collective_contributions_proved: as_u64(coverage.collective_contributions_proved())?,
        reference_declared: as_u64(semantic.reference_obligation_count())?
            .checked_add(as_u64(semantic.numerical_obligation_count())?)
            .ok_or(ProductionTotalOutputRefinementErrorV2::CounterOverflow)?,
        reference_proved: as_u64(semantic.proved_reference_obligation_count())?
            .checked_add(as_u64(semantic.proved_numerical_obligation_count())?)
            .ok_or(ProductionTotalOutputRefinementErrorV2::CounterOverflow)?,
        effect_declared: as_u64(effects.contract_count())?,
        effect_proved: as_u64(effects.proved_contract_count())?,
        collective_declared: as_u64(semantic.collective_contract_count())?,
        collective_proved: as_u64(semantic.proved_collective_contract_count())?,
        typed,
        reconciliation_recipe_roots: reconciliation.recipe_expression_roots(),
        reconciliation_pliron_roots: reconciliation.pliron_commitment_roots(),
        reconciliation_exact: reconciliation.is_exact(),
        retained_receipts: as_u64(ranked.retained_functional_refinement_receipts().len())?,
        evidence_identity: *evidence.identity().sha256(),
    };

    let encoded_coverage = evidence.coverage_summary();
    if encoded_coverage.total_view_declared() != facts.total_view_declared
        || encoded_coverage.total_view_proved() != facts.total_view_proved
        || encoded_coverage.collective_contributions_declared()
            != facts.collective_contributions_declared
        || encoded_coverage.collective_contributions_proved()
            != facts.collective_contributions_proved
    {
        return Err(ProductionTotalOutputRefinementErrorV2::CoverageSummaryMismatch);
    }
    let encoded_semantics = evidence.semantic_summary();
    if encoded_semantics.reference_obligations_declared() != facts.reference_declared
        || encoded_semantics.reference_obligations_proved() != facts.reference_proved
        || encoded_semantics.effect_contracts_declared() != facts.effect_declared
        || encoded_semantics.effect_contracts_proved() != facts.effect_proved
        || encoded_semantics.collective_contracts_declared() != facts.collective_declared
        || encoded_semantics.collective_contracts_proved() != facts.collective_proved
    {
        return Err(ProductionTotalOutputRefinementErrorV2::SemanticSummaryMismatch);
    }
    if evidence.typed_semantic_summary() != facts.typed {
        return Err(ProductionTotalOutputRefinementErrorV2::TypedSummaryMismatch);
    }
    validate_observed_facts_v2(facts)
}

fn validate_observed_facts_v2(
    facts: ObservedTotalOutputFactsV2,
) -> Result<ProductionTotalOutputRefinementReportV2, ProductionTotalOutputRefinementErrorV2> {
    if !facts.reports_clean {
        return Err(ProductionTotalOutputRefinementErrorV2::MandatoryReportNotClean);
    }
    if facts.total_view_declared == 0 || facts.total_view_declared != facts.total_view_proved {
        return Err(ProductionTotalOutputRefinementErrorV2::MissingTotalViewProof);
    }
    if facts.effect_declared == 0 || facts.effect_declared != facts.effect_proved {
        return Err(ProductionTotalOutputRefinementErrorV2::MissingEffectProof);
    }
    if facts.reference_declared != facts.reference_proved
        || facts.collective_declared != facts.collective_proved
    {
        return Err(ProductionTotalOutputRefinementErrorV2::SemanticSummaryMismatch);
    }
    if facts.retained_receipts == 0 {
        return Err(ProductionTotalOutputRefinementErrorV2::MissingRetainedReceipt);
    }
    if !facts.typed.is_non_vacuous() {
        return Err(ProductionTotalOutputRefinementErrorV2::MissingTypedSemanticProof);
    }
    if facts.typed.statically_discharged_domain_roots != facts.typed.expression_roots {
        return Err(ProductionTotalOutputRefinementErrorV2::UndischargedTypedSemanticDomain);
    }
    let typed_roots = as_u64(facts.typed.expression_roots)?;
    if !facts.reconciliation_exact
        || facts.reconciliation_recipe_roots != typed_roots
        || facts.reconciliation_pliron_roots != typed_roots
    {
        return Err(ProductionTotalOutputRefinementErrorV2::TypedSemanticReconciliationMismatch);
    }
    if facts.collective_contributions_declared != facts.collective_contributions_proved
        || (facts.collective_contributions_declared != 0 && facts.collective_declared == 0)
    {
        return Err(
            ProductionTotalOutputRefinementErrorV2::CollectiveParticipationWithoutValueProof,
        );
    }
    let bitvector = facts.typed.exact_bitvector_operator_congruence_roots;
    let ieee = facts.typed.exact_ieee_operator_congruence_roots;
    let arithmetic = match (bitvector != 0, ieee != 0) {
        (true, false) => ProductionArithmeticAssuranceV2::ExactBitVectorValues,
        (false, true) => ProductionArithmeticAssuranceV2::IeeeOperatorCongruenceOnly,
        (true, true) => ProductionArithmeticAssuranceV2::ExactBitVectorsAndIeeeOperatorCongruence,
        (false, false) => {
            return Err(ProductionTotalOutputRefinementErrorV2::MissingTypedSemanticProof);
        }
    };
    Ok(ProductionTotalOutputRefinementReportV2 {
        total_view_contracts: facts.total_view_declared,
        reference_obligations: facts.reference_declared,
        effect_contracts: facts.effect_declared,
        collective_contracts: facts.collective_declared,
        typed_expression_roots: typed_roots,
        retained_receipts: facts.retained_receipts,
        arithmetic,
        evidence_identity: facts.evidence_identity,
    })
}

fn as_u64(value: usize) -> Result<u64, ProductionTotalOutputRefinementErrorV2> {
    u64::try_from(value).map_err(|_| ProductionTotalOutputRefinementErrorV2::CounterOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> ObservedTotalOutputFactsV2 {
        ObservedTotalOutputFactsV2 {
            reports_clean: true,
            total_view_declared: 1,
            total_view_proved: 1,
            collective_contributions_declared: 0,
            collective_contributions_proved: 0,
            reference_declared: 0,
            reference_proved: 0,
            effect_declared: 1,
            effect_proved: 1,
            collective_declared: 0,
            collective_proved: 0,
            typed: ProductionTypedSemanticObligationSummaryV2 {
                expression_roots: 2,
                expression_nodes: 6,
                arithmetic_operations: 2,
                comparisons: 0,
                selects: 0,
                casts: 0,
                checked_operations: 0,
                statically_discharged_domain_roots: 2,
                exact_bitvector_operator_congruence_roots: 2,
                exact_ieee_operator_congruence_roots: 0,
            },
            reconciliation_recipe_roots: 2,
            reconciliation_pliron_roots: 2,
            reconciliation_exact: true,
            retained_receipts: 1,
            evidence_identity: [7; 32],
        }
    }

    #[test]
    fn accepts_non_vacuous_total_output_refinement() {
        let report = validate_observed_facts_v2(facts()).unwrap();
        assert!(report.proves_total_output_refinement_at_mir_boundary());
        assert_eq!(
            report.arithmetic_assurance(),
            ProductionArithmeticAssuranceV2::ExactBitVectorValues
        );
        assert!(!report.grants_source_to_mir_authority());
        assert!(!report.grants_lowering_or_machine_code_authority());
        assert!(!report.proves_universal_kernel_correctness());
    }

    #[test]
    fn every_non_vacuity_boundary_fails_closed() {
        let cases = [
            (|facts: &mut ObservedTotalOutputFactsV2| facts.reports_clean = false) as fn(&mut _),
            |facts: &mut ObservedTotalOutputFactsV2| facts.total_view_declared = 0,
            |facts: &mut ObservedTotalOutputFactsV2| facts.total_view_proved = 0,
            |facts: &mut ObservedTotalOutputFactsV2| facts.effect_declared = 0,
            |facts: &mut ObservedTotalOutputFactsV2| facts.effect_proved = 0,
            |facts: &mut ObservedTotalOutputFactsV2| facts.retained_receipts = 0,
            |facts: &mut ObservedTotalOutputFactsV2| facts.typed.expression_roots = 0,
            |facts: &mut ObservedTotalOutputFactsV2| {
                facts.typed.statically_discharged_domain_roots = 1
            },
            |facts: &mut ObservedTotalOutputFactsV2| facts.reconciliation_pliron_roots = 1,
        ];
        for mutate in cases {
            let mut hostile = facts();
            mutate(&mut hostile);
            assert!(validate_observed_facts_v2(hostile).is_err());
        }
    }

    #[test]
    fn collective_participation_requires_an_independent_value_contract() {
        let mut hostile = facts();
        hostile.collective_contributions_declared = 1;
        hostile.collective_contributions_proved = 1;
        assert_eq!(
            validate_observed_facts_v2(hostile),
            Err(ProductionTotalOutputRefinementErrorV2::CollectiveParticipationWithoutValueProof)
        );

        let mut proved = facts();
        proved.collective_contributions_declared = 1;
        proved.collective_contributions_proved = 1;
        proved.collective_declared = 1;
        proved.collective_proved = 1;
        assert_eq!(
            validate_observed_facts_v2(proved)
                .unwrap()
                .collective_contracts(),
            1
        );
    }

    #[test]
    fn mixed_arithmetic_never_promotes_ieee_target_authority() {
        let mut mixed = facts();
        mixed.typed.exact_bitvector_operator_congruence_roots = 1;
        mixed.typed.exact_ieee_operator_congruence_roots = 1;
        let report = validate_observed_facts_v2(mixed).unwrap();
        assert_eq!(
            report.arithmetic_assurance(),
            ProductionArithmeticAssuranceV2::ExactBitVectorsAndIeeeOperatorCongruence
        );
        assert!(!report.grants_target_ieee_value_authority());
        assert!(!report.grants_artifact_load_launch_or_hardware_authority());
    }
}
