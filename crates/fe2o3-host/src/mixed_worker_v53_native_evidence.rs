//! Exact boxed evidence reservation on the caller's ledger. No proof is checked here.
use super::*;
use crate::{
    MAX_WORKER_V3_MACHINE_EFFECT_EVIDENCE_BYTES_V1,
    MAX_WORKER_V3_SEMANTIC_MACHINE_REFINEMENT_PROOF_BYTES_V1,
};

/// Private and move-only; a safe constructor establishes accounting only.
/// Only the unsafe backend contract can authenticate the payload's semantics.
pub(crate) struct ReservedNativeEvidenceV53 {
    evidence: WorkerV3ProtectedSemanticMachineRefinementEvidenceV1,
    subject: NativeSubjectV53,
    ledger: CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
    retained: usize,
}
impl ReservedNativeEvidenceV53 {
    fn extent(effect_bytes: usize, proof_bytes: usize) -> Result<usize> {
        if effect_bytes == 0
            || effect_bytes > MAX_WORKER_V3_MACHINE_EFFECT_EVIDENCE_BYTES_V1
            || proof_bytes == 0
            || proof_bytes > MAX_WORKER_V3_SEMANTIC_MACHINE_REFINEMENT_PROOF_BYTES_V1
        {
            return Err(binding("V53 native evidence declared extent"));
        }
        effect_bytes
            .checked_add(proof_bytes)
            .and_then(|n| n.checked_add(size_of::<Self>()))
            .ok_or_else(|| Resource::Arithmetic.into())
    }

    /// The callback must construct exact-sized boxed payloads after this prepay.
    /// A provider converting Vecs must separately prepay live capacities and any
    /// conversion coexistence. Provider computation/storage is not host RSS.
    pub(crate) fn construct(
        subject: NativeSubjectV53,
        effect_bytes: usize,
        proof_bytes: usize,
        budget: &mut Budget<'_>,
        construct: impl FnOnce(
            &mut Budget<'_>,
        ) -> Result<WorkerV3ProtectedSemanticMachineRefinementEvidenceV1>,
    ) -> Result<Self> {
        budget.check_prior_denials_v1()?;
        if subject.versions != VERSIONS {
            return Err(binding("V53 native evidence versions"));
        }
        let retained = Self::extent(effect_bytes, proof_bytes)?;
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let evidence = codec_on_budget(budget, retained, |budget| {
            budget.charge_work(
                effect_bytes
                    .checked_add(proof_bytes)
                    .ok_or(Resource::Arithmetic)?,
            )?;
            let evidence = construct(budget)?;
            budget.check_prior_denials_v1()?;
            if evidence.machine_effect_evidence_bytes().len() != effect_bytes
                || evidence.refinement_proof_bytes().len() != proof_bytes
            {
                return Err(binding("V53 native evidence actual boxed extent"));
            }
            Ok(evidence)
        })?;
        budget.reserve_storage(retained)?;
        Ok(Self {
            evidence,
            subject,
            ledger,
            floor,
            retained,
        })
    }

    pub(super) fn into_checked(
        self,
        expected: NativeSubjectV53,
        budget: &mut Budget<'_>,
    ) -> Result<(WorkerV3ProtectedSemanticMachineRefinementEvidenceV1, usize)> {
        budget.check_prior_denials_v1()?;
        budget.charge_work(size_of::<NativeSubjectV53>())?;
        if self.ledger != budget.work_ledger_identity_v1()
            || budget.storage()
                < self
                    .floor
                    .checked_add(self.retained)
                    .ok_or(Resource::Arithmetic)?
        {
            return Err(binding("V53 native evidence ledger or retained floor"));
        }
        if self.subject.versions != VERSIONS || expected != self.subject {
            return Err(binding("V53 native evidence exact subject"));
        }
        if self.retained
            != Self::extent(
                self.evidence.machine_effect_evidence_bytes().len(),
                self.evidence.refinement_proof_bytes().len(),
            )?
        {
            return Err(binding("V53 native evidence retained extent"));
        }
        Ok((self.evidence, self.retained))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_v53_native_evidence_retained_headers_match_independent_owner_shape() {
        #[allow(dead_code)]
        struct OwnerShape {
            evidence: WorkerV3ProtectedSemanticMachineRefinementEvidenceV1,
            subject: NativeSubjectV53,
            ledger: CanonicalKernelIrWorkLedgerIdentityV1,
            floor: usize,
            retained: usize,
        }
        assert_eq!(
            size_of::<ReservedNativeEvidenceV53>(),
            size_of::<OwnerShape>()
        );
        assert_eq!(
            ReservedNativeEvidenceV53::extent(7, 11).unwrap(),
            size_of::<OwnerShape>() + 18
        );
    }
}
