#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlironSemanticRefinementFindingV1 {
    BoundsPrerequisiteRejected,
    UnresolvedExpression {
        block: usize,
        operation: usize,
        value: String,
    },
    ExpressionMismatch {
        block: usize,
        operation: usize,
        actual: String,
        expected: String,
    },
    ReferenceContractIncomplete {
        block: usize,
        operation: usize,
        obligation: [u64; 4],
        reason: &'static str,
    },
    ReferenceContractRejected {
        block: usize,
        operation: usize,
        obligation: [u64; 4],
        reason: &'static str,
    },
    CollectiveContractIncomplete {
        block: usize,
        operation: usize,
        reason: &'static str,
    },
    CollectiveContractRejected {
        block: usize,
        operation: usize,
        reason: &'static str,
    },
    TypedExpressionRejected {
        reason: &'static str,
    },
    NumericalProofIncomplete {
        block: usize,
        operation: usize,
        actual: String,
        reference: String,
        reason: &'static str,
    },
    ResourceLimitExceeded,
}

impl PlironSemanticRefinementFindingV1 {
    pub const fn status(&self) -> KernelCheckStatusV1 {
        match self {
            Self::ExpressionMismatch { .. }
            | Self::ReferenceContractRejected { .. }
            | Self::CollectiveContractRejected { .. }
            | Self::TypedExpressionRejected { .. } => KernelCheckStatusV1::Rejected,
            Self::BoundsPrerequisiteRejected
            | Self::UnresolvedExpression { .. }
            | Self::ReferenceContractIncomplete { .. }
            | Self::CollectiveContractIncomplete { .. }
            | Self::NumericalProofIncomplete { .. }
            | Self::ResourceLimitExceeded => KernelCheckStatusV1::Incomplete,
        }
    }
}

impl fmt::Display for PlironSemanticRefinementFindingV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BoundsPrerequisiteRejected => formatter.write_str(
                "error[FE2O3-SEMANTIC-000]: bounds prerequisite rejected before declared semantic refinement",
            ),
            Self::UnresolvedExpression {
                block,
                operation,
                value,
            } => write!(
                formatter,
                "error[FE2O3-SEMANTIC-002]: cannot resolve declared semantic expression {value} at block {block} op {operation}",
            ),
            Self::ExpressionMismatch {
                block,
                operation,
                actual,
                expected,
            } => write!(
                formatter,
                "error[FE2O3-SEMANTIC-001]: declared semantic refinement failed at block {block} op {operation}; actual expression `{actual}` is not equivalent to required expression `{expected}`; help: preserve the frontend-declared target-neutral semantic formula",
            ),
            Self::ReferenceContractIncomplete {
                block,
                operation,
                obligation,
                reason,
            } => write!(
                formatter,
                "error[FE2O3-SEMANTIC-003]: functional-reference obligation {} is incomplete at block {block} op {operation}: {reason}",
                proof_identity(*obligation),
            ),
            Self::ReferenceContractRejected {
                block,
                operation,
                obligation,
                reason,
            } => write!(
                formatter,
                "error[FE2O3-SEMANTIC-004]: functional-reference obligation {} is invalid at block {block} op {operation}: {reason}",
                proof_identity(*obligation),
            ),
            Self::CollectiveContractIncomplete {
                block,
                operation,
                reason,
            } => write!(
                formatter,
                "error[FE2O3-SEMANTIC-005]: finite collective contract is incomplete at block {block} op {operation}: {reason}",
            ),
            Self::CollectiveContractRejected {
                block,
                operation,
                reason,
            } => write!(
                formatter,
                "error[FE2O3-SEMANTIC-006]: finite collective contract is invalid at block {block} op {operation}: {reason}",
            ),
            Self::TypedExpressionRejected { reason } => write!(
                formatter,
                "error[FE2O3-SEMANTIC-007]: typed semantic expression payload is invalid: {reason}",
            ),
            Self::NumericalProofIncomplete {
                block,
                operation,
                actual,
                reference,
                reason,
            } => write!(
                formatter,
                "error[FE2O3-NUMERIC-001]: numerical refinement is incomplete at block {block} op {operation}: {reason}; actual `{actual}` differs from reference `{reference}`; help: preserve the exact typed operator tree, or supply a future supported interval/error proof for the changed operation order",
            ),
            Self::ResourceLimitExceeded => formatter.write_str(
                "error[FE2O3-SEMANTIC-002]: semantic refinement analysis resource limit exceeded",
            ),
        }
    }
}

/// A finite-error theorem derived from the live typed operator trees.
///
/// V1 proves only structural operator congruence, which makes both derived
/// error bounds exactly zero. Imported evidence selects the obligation but is
/// not used to infer these values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironNumericalBoundCertificateV1 {
    block: usize,
    operation: usize,
    requested_absolute_error_f64_bits: u64,
    requested_relative_error_f64_bits: u64,
}

impl PlironNumericalBoundCertificateV1 {
    pub const fn block(&self) -> usize {
        self.block
    }
    pub const fn operation(&self) -> usize {
        self.operation
    }
    pub const fn derived_absolute_error_f64_bits(&self) -> u64 {
        0.0_f64.to_bits()
    }
    pub const fn derived_relative_error_f64_bits(&self) -> u64 {
        0.0_f64.to_bits()
    }
    pub const fn requested_absolute_error_f64_bits(&self) -> u64 {
        self.requested_absolute_error_f64_bits
    }
    pub const fn requested_relative_error_f64_bits(&self) -> u64 {
        self.requested_relative_error_f64_bits
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironSemanticRefinementReportV1 {
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
    effect_refinement: PlironEffectRefinementReportV1,
}

impl PlironSemanticRefinementReportV1 {
    #[cfg(test)]
    pub(super) fn validation_payload_test_report_v1() -> Self {
        let mut typed_root_commitments = Vec::with_capacity(3);
        typed_root_commitments.extend([[1, 2, 3, 4], [5, 6, 7, 8]]);
        let mut numerical_certificates = Vec::with_capacity(4);
        numerical_certificates.push(PlironNumericalBoundCertificateV1 {
            block: 7,
            operation: 11,
            requested_absolute_error_f64_bits: 0,
            requested_relative_error_f64_bits: 1,
        });
        Self {
            findings: Vec::new(),
            reference_obligations: 2,
            policy_checked_reference_obligations: 2,
            numerical_obligations: 1,
            policy_checked_numerical_obligations: 1,
            collective_contracts: 3,
            policy_checked_collective_contracts: 3,
            typed_root_commitments,
            numerical_certificates,
            progress: PlironProgressReportV1::validation_payload_test_report_v1(),
            effect_refinement: super::pliron_effect_refinement::clean_effect_refinement_report_v1(),
        }
    }

    pub(super) fn validation_payload_receipt_v1(
        &self,
        limits: super::pliron_resource_envelope::ProductionAnalysisResourceLimitsV1,
    ) -> Result<
        super::pliron_report_payload_receipt::ProductionAnalysisReportPayloadReceiptV1,
        super::pliron_resource_envelope::ProductionAnalysisResourceLimitV1,
    > {
        use super::pliron_report_payload_receipt::{
            ProductionAnalysisReportPayloadReceiptV1 as Receipt, payload_product_v1,
            payload_sum_v1, require_payload_census_v1,
        };
        // Header reads and arithmetic are fixed. Each progress certificate adds
        // one visit, four length/capacity reads, and six checked additions.
        const HEADER_CENSUS_WORK: usize = 48;
        // Six semantic and two effect counters, six empty vector owners,
        // three report structs, three vector reservation attempts, and six
        // findings length/capacity checks.
        const FIXED_CLONE_WORK: usize = 6 + 2 + 6 + 3 + 3 + 6;
        // The counters, six vector length comparisons, and three report tags.
        const FIXED_COMPARISON_WORK: usize = 6 + 2 + 6 + 3;
        // Each evaluation folds three empty finding vectors and joins twice.
        const STATUS_WORK: usize = 4 * (3 + 2);
        require_payload_census_v1(HEADER_CENSUS_WORK, limits)?;
        if !self.findings.is_empty()
            || self.findings.capacity() != 0
            || !self.progress.has_empty_validation_findings_v1()
            || !self.effect_refinement.has_empty_validation_findings_v1()
        {
            return Ok(Receipt::fallback(self.pass(), HEADER_CENSUS_WORK));
        }
        let certificates = self.progress.certificates().len();
        let census_work =
            payload_sum_v1(&[HEADER_CENSUS_WORK, payload_product_v1(certificates, 11)?])?;
        require_payload_census_v1(census_work, limits)?;
        let (owned_text, copied_text) = self.progress.validation_text_storage_v1()?;
        let owner_storage = payload_sum_v1(&[
            payload_product_v1(self.typed_root_commitments.capacity(), 4)?,
            payload_product_v1(self.numerical_certificates.capacity(), 6)?,
            self.progress.validation_certificate_capacity_v1(),
            owned_text,
        ])?;
        let fixed_payload = payload_sum_v1(&[
            payload_product_v1(self.typed_root_commitments.len(), 4)?,
            payload_product_v1(self.numerical_certificates.len(), 6)?,
            copied_text,
        ])?;
        Ok(Receipt::exact(
            self.pass(),
            census_work,
            owner_storage,
            payload_sum_v1(&[fixed_payload, certificates])?,
            payload_sum_v1(&[
                FIXED_CLONE_WORK,
                fixed_payload,
                payload_product_v1(certificates, 7)?,
            ])?,
            payload_sum_v1(&[
                FIXED_COMPARISON_WORK,
                fixed_payload,
                payload_product_v1(certificates, 7)?,
            ])?,
            STATUS_WORK,
        ))
    }

    pub(super) fn try_clone_validation_payload_v1(
        &self,
    ) -> Result<Self, super::pliron_resource_envelope::ProductionAnalysisResourceLimitV1> {
        use super::pliron_report_payload_receipt::{payload_limit_v1, try_reserve_payload_v1};
        if !self.findings.is_empty() || self.findings.capacity() != 0 {
            return Err(payload_limit_v1("report payload shape changed"));
        }
        let mut typed_root_commitments = Vec::new();
        try_reserve_payload_v1(
            &mut typed_root_commitments,
            self.typed_root_commitments.len(),
        )?;
        typed_root_commitments.extend(self.typed_root_commitments.iter().copied());
        let mut numerical_certificates = Vec::new();
        try_reserve_payload_v1(
            &mut numerical_certificates,
            self.numerical_certificates.len(),
        )?;
        numerical_certificates.extend(self.numerical_certificates.iter().map(|certificate| {
            PlironNumericalBoundCertificateV1 {
                block: certificate.block,
                operation: certificate.operation,
                requested_absolute_error_f64_bits: certificate.requested_absolute_error_f64_bits,
                requested_relative_error_f64_bits: certificate.requested_relative_error_f64_bits,
            }
        }));
        Ok(Self {
            findings: Vec::new(),
            reference_obligations: self.reference_obligations,
            policy_checked_reference_obligations: self.policy_checked_reference_obligations,
            numerical_obligations: self.numerical_obligations,
            policy_checked_numerical_obligations: self.policy_checked_numerical_obligations,
            collective_contracts: self.collective_contracts,
            policy_checked_collective_contracts: self.policy_checked_collective_contracts,
            typed_root_commitments,
            numerical_certificates,
            progress: self.progress.try_clone_validation_payload_v1()?,
            effect_refinement: self.effect_refinement.try_clone_validation_payload_v1()?,
        })
    }

    pub const fn pass(&self) -> KernelCheckPassKindV1 {
        KernelCheckPassKindV1::SemanticRefinement
    }

    pub fn status(&self) -> KernelCheckStatusV1 {
        self.findings
            .iter()
            .fold(KernelCheckStatusV1::Clean, |status, finding| {
                status.join(finding.status())
            })
            .join(self.progress.status())
            .join(self.effect_refinement.status())
    }

    pub fn findings(&self) -> &[PlironSemanticRefinementFindingV1] {
        &self.findings
    }

    pub fn is_clean(&self) -> bool {
        self.status() == KernelCheckStatusV1::Clean
    }

    /// Number of target-neutral expression equalities explicitly bound to a
    /// functional-reference proof obligation.
    pub const fn reference_obligation_count(&self) -> usize {
        self.reference_obligations
    }

    /// Number of reference-bound equalities with one exact policy-checked staging
    /// record and an equal target-neutral expression. This is not proof authority.
    pub const fn policy_checked_reference_obligation_count(&self) -> usize {
        self.policy_checked_reference_obligations
    }

    /// Whether every declared reference obligation has structurally consistent,
    /// policy-checked staging. This does not report functional proof.
    pub fn all_reference_obligations_are_policy_checked(&self) -> bool {
        self.is_clean()
            && self.reference_obligations != 0
            && self.reference_obligations == self.policy_checked_reference_obligations
    }

    /// Number of finite-error relations with policy-checked staging in the live graph.
    pub const fn numerical_obligation_count(&self) -> usize {
        self.numerical_obligations
    }

    /// Number whose typed roots, finite bounds, and exact evidence join passed.
    pub const fn policy_checked_numerical_obligation_count(&self) -> usize {
        self.policy_checked_numerical_obligations
    }

    pub fn all_numerical_obligations_are_policy_checked(&self) -> bool {
        self.is_clean()
            && self.numerical_obligations != 0
            && self.numerical_obligations == self.policy_checked_numerical_obligations
    }

    /// Number of closed finite fold, recurrence, and permutation contracts.
    pub const fn collective_contract_count(&self) -> usize {
        self.collective_contracts
    }

    /// Number structurally joined to one policy-checked MIR staging obligation.
    pub const fn policy_checked_collective_contract_count(&self) -> usize {
        self.policy_checked_collective_contracts
    }

    pub fn all_collective_contracts_are_policy_checked(&self) -> bool {
        self.is_clean()
            && self.collective_contracts != 0
            && self.collective_contracts == self.policy_checked_collective_contracts
    }

    /// Canonical typed-root commitments reconstructed and verified by the
    /// mandatory semantic pass, in function order.
    pub fn typed_root_commitments(&self) -> &[[u64; 4]] {
        &self.typed_root_commitments
    }

    pub fn numerical_certificates(&self) -> &[PlironNumericalBoundCertificateV1] {
        &self.numerical_certificates
    }

    pub const fn progress(&self) -> &PlironProgressReportV1 {
        &self.progress
    }

    pub const fn effect_refinement(&self) -> &PlironEffectRefinementReportV1 {
        &self.effect_refinement
    }

    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }

    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironSemanticRefinementCheckErrorV1 {
    report: PlironSemanticRefinementReportV1,
}

impl PlironSemanticRefinementCheckErrorV1 {
    pub fn report(&self) -> &PlironSemanticRefinementReportV1 {
        &self.report
    }
}

impl fmt::Display for PlironSemanticRefinementCheckErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut wrote = false;
        for (index, finding) in self.report.findings.iter().enumerate() {
            if index != 0 {
                formatter.write_str("\n")?;
            }
            finding.fmt(formatter)?;
            wrote = true;
        }
        for finding in self.report.effect_refinement.findings() {
            if wrote {
                formatter.write_str("\n")?;
            }
            finding.fmt(formatter)?;
            wrote = true;
        }
        for finding in self.report.progress.findings() {
            if wrote {
                formatter.write_str("\n")?;
            }
            finding.fmt(formatter)?;
            wrote = true;
        }
        Ok(())
    }
}

impl std::error::Error for PlironSemanticRefinementCheckErrorV1 {}
