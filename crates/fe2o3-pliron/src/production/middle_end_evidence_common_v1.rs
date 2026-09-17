// Shared facts and scalar codecs are not an evidence record or wire envelope.
// Each version separately chooses its policy, extra claims, and identity domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CommonMiddleEndFactsV1 {
    pub(super) source_semantic_identity: [u8; SHA256_BYTES],
    pub(super) ranked_kernel_identity: [u8; SHA256_BYTES],
    pub(super) coverage: ProductionMiddleEndCoverageSummaryV5,
    pub(super) semantics: ProductionMiddleEndSemanticSummaryV5,
    pub(super) typed_summary: ProductionTypedSemanticObligationSummaryV2,
    pub(super) reconciliation: ProductionMiddleEndTypedSemanticReconciliationV5,
}

impl CommonMiddleEndFactsV1 {
    #[cfg(test)]
    pub(super) fn test_fixture() -> Self {
        Self {
            source_semantic_identity: [1; 32],
            ranked_kernel_identity: [2; 32],
            coverage: ProductionMiddleEndCoverageSummaryV5::default(),
            semantics: ProductionMiddleEndSemanticSummaryV5::default(),
            typed_summary: ProductionTypedSemanticObligationSummaryV2::default(),
            reconciliation: ProductionMiddleEndTypedSemanticReconciliationV5 {
                recipe_expression_roots: 0,
                pliron_commitment_roots: 0,
                ordered_commitments_sha256: [3; 32],
            },
        }
    }
    pub(super) fn revalidate(
        semantic: &ProductionSemanticMirOwnerV1,
        ranked: &ProductionRankedKernelLoweringInputV1,
    ) -> Result<Self, ProductionMiddleEndEvidenceCodecErrorV5> {
        let source_semantic_identity = revalidated_source_semantic_identity(semantic)
            .map_err(ProductionMiddleEndEvidenceCodecErrorV5::HistoricalV4)?;
        ranked
            .revalidate_structure()
            .map_err(ProductionMiddleEndEvidenceCodecErrorV5::RankedKernel)?;
        validate_v5_live_reports(ranked)?;
        let observed = ranked.ownership_report().coverage_summary();
        let coverage = ProductionMiddleEndCoverageSummaryV5 {
            total_view_declared: usize_to_u64(observed.total_view_declared())?,
            total_view_proved: usize_to_u64(observed.total_view_proved())?,
            collective_contributions_declared: usize_to_u64(
                observed.collective_contributions_declared(),
            )?,
            collective_contributions_proved: usize_to_u64(
                observed.collective_contributions_proved(),
            )?,
        };
        validate_coverage_v5(coverage)?;
        let semantic_report = ranked.semantic_report();
        let effect_report = semantic_report.effect_refinement();
        let reference_obligations_declared =
            usize_to_u64(semantic_report.reference_obligation_count())?
                .checked_add(usize_to_u64(semantic_report.numerical_obligation_count())?)
                .ok_or(ProductionMiddleEndEvidenceCodecErrorV5::CounterOverflow)?;
        let reference_obligations_policy_checked =
            usize_to_u64(semantic_report.policy_checked_reference_obligation_count())?
                .checked_add(usize_to_u64(
                    semantic_report.policy_checked_numerical_obligation_count(),
                )?)
                .ok_or(ProductionMiddleEndEvidenceCodecErrorV5::CounterOverflow)?;
        let semantics = ProductionMiddleEndSemanticSummaryV5 {
            reference_obligations_declared,
            reference_obligations_policy_checked,
            effect_contracts_declared: usize_to_u64(effect_report.contract_count())?,
            effect_contracts_proved: usize_to_u64(effect_report.proved_contract_count())?,
            collective_contracts_declared: usize_to_u64(
                semantic_report.collective_contract_count(),
            )?,
            collective_contracts_policy_checked: usize_to_u64(
                semantic_report.policy_checked_collective_contract_count(),
            )?,
        };
        validate_semantics_v5(semantics)?;
        let typed_summary = typed_semantic_obligation_summary_v2(ranked.kernel())
            .map_err(ProductionMiddleEndEvidenceCodecErrorV5::RankedKernel)?;
        validate_typed_summary_v5(typed_summary)?;
        let observed_reconciliation = typed_semantic_commitment_reconciliation_v2(ranked)
            .map_err(ProductionMiddleEndEvidenceCodecErrorV5::RankedKernel)?;
        let reconciliation = ProductionMiddleEndTypedSemanticReconciliationV5 {
            recipe_expression_roots: usize_to_u64(
                observed_reconciliation.recipe_expression_roots(),
            )?,
            pliron_commitment_roots: usize_to_u64(
                observed_reconciliation.pliron_commitment_roots(),
            )?,
            ordered_commitments_sha256: *observed_reconciliation.ordered_commitments_sha256(),
        };
        validate_reconciliation_v5(typed_summary, reconciliation)?;
        let ranked_kernel_identity = derive_ranked_kernel_identity(ranked);
        if ranked_kernel_identity == [0; SHA256_BYTES] {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::ZeroRankedKernelIdentity);
        }
        Ok(Self {
            source_semantic_identity,
            ranked_kernel_identity,
            coverage,
            semantics,
            typed_summary,
            reconciliation,
        })
    }

    pub(super) fn validate(
        &self,
        ir: &[u8],
    ) -> Result<(), ProductionMiddleEndEvidenceCodecErrorV5> {
        validate_ranked_ir_v5(ir)?;
        if self.source_semantic_identity == [0; SHA256_BYTES] {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::ZeroSemanticIdentity);
        }
        if self.ranked_kernel_identity == [0; SHA256_BYTES] {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::ZeroRankedKernelIdentity);
        }
        validate_coverage_v5(self.coverage)?;
        validate_semantics_v5(self.semantics)?;
        validate_typed_summary_v5(self.typed_summary)?;
        validate_reconciliation_v5(self.typed_summary, self.reconciliation)
    }

    pub(super) fn encode_body(
        &self,
        output: &mut Vec<u8>,
        ir: &str,
    ) -> Result<Range<usize>, ProductionMiddleEndEvidenceCodecErrorV5> {
        self.validate(ir.as_bytes())?;
        output.extend_from_slice(&self.source_semantic_identity);
        output.extend_from_slice(&self.ranked_kernel_identity);
        output.extend_from_slice(&(ir.len() as u32).to_le_bytes());
        let start = output.len();
        output.extend_from_slice(ir.as_bytes());
        let range = start..output.len();
        output.push(PASS_COUNT_V5 as u8);
        for pass in PRODUCTION_MIDDLE_END_EVIDENCE_PASS_ORDER_V5 {
            output.extend_from_slice(&[pass.tag(), CLEAN_STATUS_V5, 0, 0, 0, 0, 0, 0, 0, 0]);
        }
        for counter in [
            self.coverage.total_view_declared,
            self.coverage.total_view_proved,
            self.coverage.collective_contributions_declared,
            self.coverage.collective_contributions_proved,
            self.semantics.reference_obligations_declared,
            self.semantics.reference_obligations_policy_checked,
            self.semantics.effect_contracts_declared,
            self.semantics.effect_contracts_proved,
            self.semantics.collective_contracts_declared,
            self.semantics.collective_contracts_policy_checked,
        ] {
            output.extend_from_slice(&counter.to_le_bytes());
        }
        for counter in typed_summary_counters_v5(self.typed_summary)? {
            output.extend_from_slice(&counter.to_le_bytes());
        }
        output.extend_from_slice(&self.reconciliation.recipe_expression_roots.to_le_bytes());
        output.extend_from_slice(&self.reconciliation.pliron_commitment_roots.to_le_bytes());
        output.extend_from_slice(&self.reconciliation.ordered_commitments_sha256);
        Ok(range)
    }

    pub(super) fn decode_body(
        reader: &mut ReaderV5<'_>,
    ) -> Result<(Self, Range<usize>), ProductionMiddleEndEvidenceCodecErrorV5> {
        let source_semantic_identity = reader.fixed()?;
        let ranked_kernel_identity = reader.fixed()?;
        let length = reader.u32()? as usize;
        let start = reader.offset();
        let ir = reader.take(length)?;
        let range = start..reader.offset();
        let count = reader.u8()?;
        if usize::from(count) != PASS_COUNT_V5 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::InvalidPassCount(
                count,
            ));
        }
        for (index, expected) in PRODUCTION_MIDDLE_END_EVIDENCE_PASS_ORDER_V5
            .into_iter()
            .enumerate()
        {
            let actual = reader.u8()?;
            if actual != expected.tag() {
                return Err(ProductionMiddleEndEvidenceCodecErrorV5::InvalidPassOrder {
                    index,
                    expected,
                    actual,
                });
            }
            if reader.u8()? != CLEAN_STATUS_V5 || reader.u32()? != 0 {
                return Err(ProductionMiddleEndEvidenceCodecErrorV5::PassNotClean(
                    expected,
                ));
            }
            if reader.u8()? != 0 || reader.u8()? != 0 {
                return Err(ProductionMiddleEndEvidenceCodecErrorV5::AuthorityClaim(
                    expected,
                ));
            }
            if reader.u16()? != 0 {
                return Err(ProductionMiddleEndEvidenceCodecErrorV5::NonzeroReserved);
            }
        }
        let coverage = ProductionMiddleEndCoverageSummaryV5 {
            total_view_declared: reader.u64()?,
            total_view_proved: reader.u64()?,
            collective_contributions_declared: reader.u64()?,
            collective_contributions_proved: reader.u64()?,
        };
        let semantics = ProductionMiddleEndSemanticSummaryV5 {
            reference_obligations_declared: reader.u64()?,
            reference_obligations_policy_checked: reader.u64()?,
            effect_contracts_declared: reader.u64()?,
            effect_contracts_proved: reader.u64()?,
            collective_contracts_declared: reader.u64()?,
            collective_contracts_policy_checked: reader.u64()?,
        };
        let typed_summary = ProductionTypedSemanticObligationSummaryV2 {
            expression_roots: u64_to_usize(reader.u64()?)?,
            expression_nodes: u64_to_usize(reader.u64()?)?,
            arithmetic_operations: u64_to_usize(reader.u64()?)?,
            comparisons: u64_to_usize(reader.u64()?)?,
            selects: u64_to_usize(reader.u64()?)?,
            casts: u64_to_usize(reader.u64()?)?,
            checked_operations: u64_to_usize(reader.u64()?)?,
            statically_discharged_domain_roots: u64_to_usize(reader.u64()?)?,
            exact_bitvector_operator_congruence_roots: u64_to_usize(reader.u64()?)?,
            exact_ieee_operator_congruence_roots: u64_to_usize(reader.u64()?)?,
        };
        let reconciliation = ProductionMiddleEndTypedSemanticReconciliationV5 {
            recipe_expression_roots: reader.u64()?,
            pliron_commitment_roots: reader.u64()?,
            ordered_commitments_sha256: reader.fixed()?,
        };
        let fields = Self {
            source_semantic_identity,
            ranked_kernel_identity,
            coverage,
            semantics,
            typed_summary,
            reconciliation,
        };
        fields.validate(ir)?;
        Ok((fields, range))
    }
}

#[cfg(test)]
mod common_body_tests {
    use super::*;
    #[test]
    fn common_body_preserves_exact_legacy_v5_encoding() {
        let facts = CommonMiddleEndFactsV1::test_fixture();
        let ir = "func @legacy {\n  kernel.return\n}\n";
        let old = encode_record_v5(
            facts.source_semantic_identity,
            facts.ranked_kernel_identity,
            ir,
            facts.coverage,
            facts.semantics,
            facts.typed_summary,
            facts.reconciliation,
        )
        .unwrap();
        let mut body = Vec::new();
        facts.encode_body(&mut body, ir).unwrap();
        let header = 32
            + PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V5.len()
            + PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V5.len();
        assert_eq!(
            &old.canonical_bytes[header..old.canonical_bytes.len() - 32],
            body
        );
        let mut reader = ReaderV5::new(&body);
        let (decoded, range) = CommonMiddleEndFactsV1::decode_body(&mut reader).unwrap();
        assert_eq!(decoded, facts);
        assert_eq!(&body[range], ir.as_bytes());
        assert!(reader.is_empty());
    }
}
