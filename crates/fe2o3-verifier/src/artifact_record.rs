use std::fmt;

use fe2o3_artifacts::{
    ConfigurationEntry as ArtifactConfigurationEntry, DigestAlgorithm, DigestBytes, IdentityText,
    MeasuredToolIdentity as ArtifactToolIdentity, Name, PayloadDigest,
    ProofArtifactIdentity as ArtifactIdentity, ProofExecutionIdentity as ArtifactExecutionIdentity,
    ProofOutcome as ArtifactOutcome, ProofProperty as ArtifactProperty,
    ProofRecordV1 as ArtifactProofRecordV1, ProofTargetIdentity as ArtifactTargetIdentity,
    SourceContractIdentity, TrustedItem as ArtifactTrustedItem,
    ValidationError as ArtifactValidationError, VerificationModelIdentity as ArtifactModelIdentity,
};

use crate::{
    CorrelationId, Digest, ExecutionTools, InvocationPlan, MeasuredToolIdentity, ProofOutcome,
    ProofProperty, ProofResultV1, ProofTargetIdentity, TrustedItem, VerificationModelIdentity,
};

/// Independently reviewed identity of the verifier invocation being recorded.
///
/// Requiring this value prevents conversion from silently blessing whichever
/// plan happens to be in memory. The expected digest must come from the review
/// or persistence boundary, not from untrusted recorder output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReviewedInvocationIdentityV1 {
    correlation_id: CorrelationId,
    canonical_invocation_digest: Digest,
}

impl ReviewedInvocationIdentityV1 {
    pub const fn new(correlation_id: CorrelationId, canonical_invocation_digest: Digest) -> Self {
        Self {
            correlation_id,
            canonical_invocation_digest,
        }
    }

    pub fn from_hex(
        correlation_id: CorrelationId,
        canonical_invocation_digest: &str,
    ) -> Result<Self, ArtifactRecordConversionError> {
        let canonical_invocation_digest = Digest::from_hex(canonical_invocation_digest)
            .map_err(|_| ArtifactRecordConversionError::MalformedInvocationDigest)?;
        Ok(Self::new(correlation_id, canonical_invocation_digest))
    }

    pub const fn correlation_id(self) -> CorrelationId {
        self.correlation_id
    }

    pub const fn canonical_invocation_digest(self) -> Digest {
        self.canonical_invocation_digest
    }
}

/// Reviewed conversion output containing descriptive proof evidence only.
///
/// The correlation is retained for audit lookup and is also committed by the
/// canonical invocation digest stored in the artifact record. This type grants
/// no module-loading or kernel-launch authority and makes no claim that Verus
/// semantics refine compiler IR or emitted machine code.
///
/// ```compile_fail
/// # fn cannot_launch(evidence: fe2o3_verifier::ArtifactProofEvidenceV1) {
/// evidence.launch();
/// # }
/// ```
///
/// ```compile_fail
/// # fn cannot_claim_refinement(evidence: fe2o3_verifier::ArtifactProofEvidenceV1) {
/// evidence.compiler_refinement();
/// # }
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactProofEvidenceV1 {
    correlation_id: CorrelationId,
    canonical_invocation_digest: Digest,
    record: ArtifactProofRecordV1,
}

impl ArtifactProofEvidenceV1 {
    pub const fn correlation_id(&self) -> CorrelationId {
        self.correlation_id
    }

    pub const fn canonical_invocation_digest(&self) -> Digest {
        self.canonical_invocation_digest
    }

    pub const fn record(&self) -> &ArtifactProofRecordV1 {
        &self.record
    }

    pub fn into_record(self) -> ArtifactProofRecordV1 {
        self.record
    }
}

/// Computes the SHA-256 committed to `ProofExecutionIdentity` by conversion.
pub fn canonical_invocation_digest(plan: &InvocationPlan) -> Digest {
    let digest = DigestAlgorithm::Sha256.calculate(&plan.canonical_invocation_bytes());
    Digest::from_bytes(*digest.bytes().as_bytes())
}

/// Converts a strict parsed verifier result into artifact proof evidence.
///
/// This boundary revalidates the result against the full request and plan even
/// though the recorder parser already performed those checks. Every identity is
/// copied without inference. A successful conversion is still only evidence;
/// artifact matching and runtime launch authorization remain separate steps.
pub fn convert_to_artifact_proof_record(
    plan: &InvocationPlan,
    result: &ProofResultV1,
    reviewed: ReviewedInvocationIdentityV1,
) -> Result<ArtifactProofEvidenceV1, ArtifactRecordConversionError> {
    let request = plan.request();
    if reviewed.correlation_id != request.correlation_id()
        || result.correlation_id() != request.correlation_id()
    {
        return Err(ArtifactRecordConversionError::CorrelationMismatch);
    }

    let invocation_digest = canonical_invocation_digest(plan);
    if reviewed.canonical_invocation_digest != invocation_digest {
        return Err(ArtifactRecordConversionError::InvocationDigestMismatch);
    }

    ensure_complete_identities(
        request.target(),
        request.model(),
        plan.tools(),
        request.trusted_items(),
    )?;
    require_equal(result.target(), request.target(), "proof target")?;
    require_equal(
        result.configuration(),
        request.configuration(),
        "proof configuration",
    )?;
    require_equal(result.model(), request.model(), "verification model")?;
    require_equal(result.tools(), plan.tools(), "measured tools")?;
    require_equal(
        result.trusted_items(),
        request.trusted_items(),
        "trusted items",
    )?;

    match result.outcome() {
        ProofOutcome::Proved if result.proved_properties() != request.properties() => {
            return Err(ArtifactRecordConversionError::PropertyMismatch);
        }
        ProofOutcome::Failed | ProofOutcome::TimedOut if !result.proved_properties().is_empty() => {
            return Err(ArtifactRecordConversionError::ClaimsOnIncompleteProof);
        }
        ProofOutcome::Proved | ProofOutcome::Failed | ProofOutcome::TimedOut => {}
    }

    let target = artifact_target(request.target());
    let configuration = request
        .configuration()
        .entries()
        .iter()
        .map(|entry| {
            Ok(ArtifactConfigurationEntry::new(
                Name::new(entry.key().as_str())?,
                IdentityText::new(entry.value().as_str())?,
            ))
        })
        .collect::<Result<Vec<_>, ArtifactValidationError>>()?;
    let execution = ArtifactExecutionIdentity::new(
        artifact_model(request.model())?,
        artifact_tool(plan.tools().verifier())?,
        artifact_tool(plan.tools().solver())?,
        artifact_tool(plan.tools().evidence_recorder())?,
        payload(invocation_digest),
    );
    let properties = result
        .proved_properties()
        .iter()
        .copied()
        .map(artifact_property)
        .collect();
    let trusted_items = result
        .trusted_items()
        .iter()
        .map(artifact_trusted_item)
        .collect::<Result<Vec<_>, ArtifactValidationError>>()?;
    let record = ArtifactProofRecordV1::new(
        target,
        configuration,
        execution,
        artifact_outcome(result.outcome()),
        properties,
        trusted_items,
    )?;

    Ok(ArtifactProofEvidenceV1 {
        correlation_id: request.correlation_id(),
        canonical_invocation_digest: invocation_digest,
        record,
    })
}

fn ensure_complete_identities(
    target: ProofTargetIdentity,
    model: &VerificationModelIdentity,
    tools: &ExecutionTools,
    trusted_items: &[TrustedItem],
) -> Result<(), ArtifactRecordConversionError> {
    const TARGET_FIELDS: [&str; 13] = [
        "kernel identity",
        "instance identity",
        "source-tree identity",
        "crate-graph identity",
        "executable identity",
        "environment identity",
        "artifact-selection identity",
        "artifact-contract identity",
        "memory-contract identity",
        "effects-contract identity",
        "type-layout identity",
        "capability-semantics identity",
        "functional-specification identity",
    ];
    for (digest, field) in target.digests().into_iter().zip(TARGET_FIELDS) {
        require_measured(digest, field)?;
    }
    require_measured(model.axioms_digest(), "verification-model axioms")?;
    for (tool, role) in [
        (tools.verifier(), "verifier"),
        (tools.solver(), "solver"),
        (tools.evidence_recorder(), "evidence recorder"),
    ] {
        if is_zero(tool.executable_digest()) || is_zero(tool.configuration_digest()) {
            return Err(ArtifactRecordConversionError::UnmeasuredIdentity(role));
        }
    }
    for item in trusted_items {
        require_measured(item.contract_digest(), "trusted-item contract")?;
    }
    Ok(())
}

fn require_measured(
    digest: Digest,
    field: &'static str,
) -> Result<(), ArtifactRecordConversionError> {
    if is_zero(digest) {
        Err(ArtifactRecordConversionError::UnmeasuredIdentity(field))
    } else {
        Ok(())
    }
}

fn is_zero(digest: Digest) -> bool {
    digest.as_bytes().iter().all(|byte| *byte == 0)
}

fn require_equal<T: PartialEq>(
    actual: T,
    expected: T,
    field: &'static str,
) -> Result<(), ArtifactRecordConversionError> {
    if actual == expected {
        Ok(())
    } else {
        Err(ArtifactRecordConversionError::IdentityMismatch { field })
    }
}

fn artifact_target(target: ProofTargetIdentity) -> ArtifactTargetIdentity {
    ArtifactTargetIdentity::new(
        ArtifactIdentity::new(
            payload(target.kernel_id),
            payload(target.instance_digest),
            payload(target.source_tree_digest),
            payload(target.crate_graph_digest),
            payload(target.executable_digest),
            payload(target.environment_digest),
            payload(target.artifact_selection_digest),
            payload(target.artifact_contract_digest),
        ),
        SourceContractIdentity::new(
            payload(target.memory_contract_digest),
            payload(target.effects_contract_digest),
            payload(target.type_layout_digest),
            payload(target.capability_semantics_digest),
            payload(target.functional_specification_digest),
        ),
    )
}

fn artifact_model(
    model: &VerificationModelIdentity,
) -> Result<ArtifactModelIdentity, ArtifactValidationError> {
    Ok(ArtifactModelIdentity::new(
        IdentityText::new(model.version().as_str())?,
        payload(model.axioms_digest()),
    ))
}

fn artifact_tool(
    tool: &MeasuredToolIdentity,
) -> Result<ArtifactToolIdentity, ArtifactValidationError> {
    Ok(ArtifactToolIdentity::new(
        IdentityText::new(tool.name().as_str())?,
        IdentityText::new(tool.version().as_str())?,
        payload(tool.executable_digest()),
        payload(tool.configuration_digest()),
    ))
}

fn artifact_trusted_item(
    item: &TrustedItem,
) -> Result<ArtifactTrustedItem, ArtifactValidationError> {
    Ok(ArtifactTrustedItem::new(
        Name::new(item.name().as_str())?,
        payload(item.contract_digest()),
    ))
}

const fn artifact_outcome(outcome: ProofOutcome) -> ArtifactOutcome {
    match outcome {
        ProofOutcome::Proved => ArtifactOutcome::Proved,
        ProofOutcome::Failed => ArtifactOutcome::Failed,
        ProofOutcome::TimedOut => ArtifactOutcome::TimedOut,
    }
}

const fn artifact_property(property: ProofProperty) -> ArtifactProperty {
    match property {
        ProofProperty::Bounds => ArtifactProperty::Bounds,
        ProofProperty::AddressOverflowFreedom => ArtifactProperty::AddressOverflowFreedom,
        ProofProperty::MemorySafety => ArtifactProperty::MemorySafety,
        ProofProperty::Initialization => ArtifactProperty::Initialization,
        ProofProperty::RaceFreedom => ArtifactProperty::RaceFreedom,
        ProofProperty::LaunchValidity => ArtifactProperty::LaunchValidity,
        ProofProperty::FunctionalCorrectness => ArtifactProperty::FunctionalCorrectness,
    }
}

fn payload(digest: Digest) -> PayloadDigest {
    PayloadDigest::new(
        DigestAlgorithm::Sha256,
        DigestBytes::from_bytes(*digest.as_bytes()),
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ArtifactRecordConversionError {
    MalformedInvocationDigest,
    CorrelationMismatch,
    InvocationDigestMismatch,
    IdentityMismatch { field: &'static str },
    PropertyMismatch,
    ClaimsOnIncompleteProof,
    UnmeasuredIdentity(&'static str),
    ArtifactValidation(ArtifactValidationError),
}

impl fmt::Display for ArtifactRecordConversionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedInvocationDigest => {
                write!(
                    formatter,
                    "reviewed invocation digest is not canonical SHA-256 hex"
                )
            }
            Self::CorrelationMismatch => write!(formatter, "proof correlation does not match"),
            Self::InvocationDigestMismatch => {
                write!(
                    formatter,
                    "canonical invocation digest does not match review"
                )
            }
            Self::IdentityMismatch { field } => write!(formatter, "{field} does not match request"),
            Self::PropertyMismatch => {
                write!(
                    formatter,
                    "proved properties do not exactly match the request"
                )
            }
            Self::ClaimsOnIncompleteProof => {
                write!(
                    formatter,
                    "incomplete proof contains proved-property claims"
                )
            }
            Self::UnmeasuredIdentity(field) => write!(formatter, "{field} is not measured"),
            Self::ArtifactValidation(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ArtifactRecordConversionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ArtifactValidation(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ArtifactValidationError> for ArtifactRecordConversionError {
    fn from(value: ArtifactValidationError) -> Self {
        Self::ArtifactValidation(value)
    }
}
