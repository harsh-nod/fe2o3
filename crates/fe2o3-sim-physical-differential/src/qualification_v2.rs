use serde::Serialize;

/// Agent-readable production qualification schema for the protected physical route.
pub const PHYSICAL_DIFFERENTIAL_QUALIFICATION_SCHEMA_V2: &str =
    "fe2o3-simulator-direct-kfd-differential-qualification-v2";

/// Furthest production mechanism implemented without claiming a protected verifier deployment.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PhysicalDifferentialAvailableBoundaryV2 {
    AuthenticatedGeneratedInvocationToSingleUseExecuteAndCompare,
}

/// One independently tracked prerequisite for a production physical differential.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PhysicalDifferentialPrerequisiteV2 {
    SealedProtectedVerifierAdapter,
    ConcreteProtectedVerifierBackend,
    ProtectedVerifierKeyCustody,
    ProtectedWorkerLedgerReacquisition,
    ExternalMonotonicRollbackAuthority,
    IndependentFinalizerReplay,
    SignedCompilerProofAndTargetLineage,
    ProofToExecutableMachineRefinement,
    RustTypeLayoutRefinement,
    RustEffectRefinement,
    InheritedWorkerV3ApplicationHandoff,
    CheckedCurrentGfx942KfdDevice,
    GeneratedAddressFreeArgumentPacking,
    SealedUnambiguousKfdCompletion,
}

/// Static implementation state versus state that can be decided only for one exact invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PhysicalDifferentialPrerequisiteStatusV2 {
    ImplementedMechanism,
    PerInvocation,
    Unavailable,
}

/// Exact reason a protected prerequisite is unavailable in the current production tree.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PhysicalDifferentialPrerequisiteUnavailableV2 {
    ConcreteBackendNotImplemented,
    ProtectedKeyDeploymentNotProvisioned,
    ProtectedWorkerLedgerDeploymentNotProvisioned,
    ExternalMonotonicRollbackDeploymentNotProvisioned,
    MachineRefinementReceiptProducerNotImplemented,
    RustTypeLayoutRefinementReceiptProducerNotImplemented,
    RustEffectRefinementReceiptProducerNotImplemented,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct PhysicalDifferentialPrerequisiteRecordV2 {
    pub prerequisite: PhysicalDifferentialPrerequisiteV2,
    pub status: PhysicalDifferentialPrerequisiteStatusV2,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<PhysicalDifferentialPrerequisiteUnavailableV2>,
}

const fn implemented(
    prerequisite: PhysicalDifferentialPrerequisiteV2,
) -> PhysicalDifferentialPrerequisiteRecordV2 {
    PhysicalDifferentialPrerequisiteRecordV2 {
        prerequisite,
        status: PhysicalDifferentialPrerequisiteStatusV2::ImplementedMechanism,
        unavailable_reason: None,
    }
}

const fn per_invocation(
    prerequisite: PhysicalDifferentialPrerequisiteV2,
) -> PhysicalDifferentialPrerequisiteRecordV2 {
    PhysicalDifferentialPrerequisiteRecordV2 {
        prerequisite,
        status: PhysicalDifferentialPrerequisiteStatusV2::PerInvocation,
        unavailable_reason: None,
    }
}

const fn unavailable(
    prerequisite: PhysicalDifferentialPrerequisiteV2,
    reason: PhysicalDifferentialPrerequisiteUnavailableV2,
) -> PhysicalDifferentialPrerequisiteRecordV2 {
    PhysicalDifferentialPrerequisiteRecordV2 {
        prerequisite,
        status: PhysicalDifferentialPrerequisiteStatusV2::Unavailable,
        unavailable_reason: Some(reason),
    }
}

/// Exact, authority-free qualification result for the current production tree.
///
/// This record is descriptive. It accepts no caller claims or pass counts and grants no compiler,
/// proof, KFD, launch, hardware-observation, or parity authority. Per-invocation records must be
/// decided by the existing move-only production owners, not by changing this response.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PhysicalDifferentialQualificationV2 {
    pub schema: &'static str,
    pub production_ready: bool,
    pub direct_kfd_only: bool,
    pub generated_worker_v3_only: bool,
    pub hardware_observed: bool,
    pub parity_observed: bool,
    pub grants_authority: bool,
    pub synthetic_verifier_admitted: bool,
    pub execution_failure_can_mint_report: bool,
    pub stale_publication_can_mint_report: bool,
    pub stale_device_can_mint_report: bool,
    pub ambiguous_completion_can_mint_report: bool,
    pub available_boundary: PhysicalDifferentialAvailableBoundaryV2,
    pub current_blocker: PhysicalDifferentialPrerequisiteUnavailableV2,
    pub prerequisites: [PhysicalDifferentialPrerequisiteRecordV2; 14],
}

/// Reports the exact implemented boundary and every remaining authority prerequisite.
pub const fn physical_differential_qualification_v2() -> PhysicalDifferentialQualificationV2 {
    PhysicalDifferentialQualificationV2 {
        schema: PHYSICAL_DIFFERENTIAL_QUALIFICATION_SCHEMA_V2,
        production_ready: false,
        direct_kfd_only: true,
        generated_worker_v3_only: true,
        hardware_observed: false,
        parity_observed: false,
        grants_authority: false,
        synthetic_verifier_admitted: false,
        execution_failure_can_mint_report: false,
        stale_publication_can_mint_report: false,
        stale_device_can_mint_report: false,
        ambiguous_completion_can_mint_report: false,
        available_boundary:
            PhysicalDifferentialAvailableBoundaryV2::AuthenticatedGeneratedInvocationToSingleUseExecuteAndCompare,
        current_blocker:
            PhysicalDifferentialPrerequisiteUnavailableV2::ConcreteBackendNotImplemented,
        prerequisites: [
            implemented(PhysicalDifferentialPrerequisiteV2::SealedProtectedVerifierAdapter),
            unavailable(
                PhysicalDifferentialPrerequisiteV2::ConcreteProtectedVerifierBackend,
                PhysicalDifferentialPrerequisiteUnavailableV2::ConcreteBackendNotImplemented,
            ),
            unavailable(
                PhysicalDifferentialPrerequisiteV2::ProtectedVerifierKeyCustody,
                PhysicalDifferentialPrerequisiteUnavailableV2::ProtectedKeyDeploymentNotProvisioned,
            ),
            unavailable(
                PhysicalDifferentialPrerequisiteV2::ProtectedWorkerLedgerReacquisition,
                PhysicalDifferentialPrerequisiteUnavailableV2::ProtectedWorkerLedgerDeploymentNotProvisioned,
            ),
            unavailable(
                PhysicalDifferentialPrerequisiteV2::ExternalMonotonicRollbackAuthority,
                PhysicalDifferentialPrerequisiteUnavailableV2::ExternalMonotonicRollbackDeploymentNotProvisioned,
            ),
            implemented(PhysicalDifferentialPrerequisiteV2::IndependentFinalizerReplay),
            per_invocation(
                PhysicalDifferentialPrerequisiteV2::SignedCompilerProofAndTargetLineage,
            ),
            unavailable(
                PhysicalDifferentialPrerequisiteV2::ProofToExecutableMachineRefinement,
                PhysicalDifferentialPrerequisiteUnavailableV2::MachineRefinementReceiptProducerNotImplemented,
            ),
            unavailable(
                PhysicalDifferentialPrerequisiteV2::RustTypeLayoutRefinement,
                PhysicalDifferentialPrerequisiteUnavailableV2::RustTypeLayoutRefinementReceiptProducerNotImplemented,
            ),
            unavailable(
                PhysicalDifferentialPrerequisiteV2::RustEffectRefinement,
                PhysicalDifferentialPrerequisiteUnavailableV2::RustEffectRefinementReceiptProducerNotImplemented,
            ),
            per_invocation(
                PhysicalDifferentialPrerequisiteV2::InheritedWorkerV3ApplicationHandoff,
            ),
            per_invocation(PhysicalDifferentialPrerequisiteV2::CheckedCurrentGfx942KfdDevice),
            per_invocation(
                PhysicalDifferentialPrerequisiteV2::GeneratedAddressFreeArgumentPacking,
            ),
            per_invocation(
                PhysicalDifferentialPrerequisiteV2::SealedUnambiguousKfdCompletion,
            ),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qualification_is_complete_unique_and_fail_closed() {
        let qualification = physical_differential_qualification_v2();
        assert!(!qualification.production_ready);
        assert!(!qualification.hardware_observed);
        assert!(!qualification.parity_observed);
        assert!(!qualification.grants_authority);
        assert!(!qualification.synthetic_verifier_admitted);
        assert!(!qualification.execution_failure_can_mint_report);
        assert!(!qualification.stale_publication_can_mint_report);
        assert!(!qualification.stale_device_can_mint_report);
        assert!(!qualification.ambiguous_completion_can_mint_report);
        assert_eq!(
            qualification.current_blocker,
            PhysicalDifferentialPrerequisiteUnavailableV2::ConcreteBackendNotImplemented
        );
        for (index, record) in qualification.prerequisites.iter().enumerate() {
            assert_eq!(
                record.status == PhysicalDifferentialPrerequisiteStatusV2::Unavailable,
                record.unavailable_reason.is_some(),
                "record {index} has inconsistent unavailable state"
            );
            assert!(
                !qualification.prerequisites[..index]
                    .iter()
                    .any(|prior| prior.prerequisite == record.prerequisite),
                "duplicate prerequisite {:?}",
                record.prerequisite
            );
        }
    }

    #[test]
    fn qualification_has_no_pass_count_fields() {
        let encoded = serde_json::to_string(&physical_differential_qualification_v2()).unwrap();
        assert!(!encoded.contains("hardware_pass"));
        assert!(!encoded.contains("parity_pass"));
        assert!(encoded.contains("concrete_backend_not_implemented"));
    }
}
