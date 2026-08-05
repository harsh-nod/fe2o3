use crate::{IdentityText, Name, PayloadDigest, ValidationError};

pub const MAX_CONFIGURATION_ENTRIES: usize = 256;
pub const MAX_PROOF_PROPERTIES: usize = 32;
pub const MAX_TRUSTED_ITEMS: usize = 128;

/// Result reported by a proof invocation. This is evidence, not assurance.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProofOutcome {
    Proved,
    Failed,
    TimedOut,
}

/// Stable target-neutral properties understood by the verification model.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProofProperty {
    Bounds,
    AddressOverflowFreedom,
    MemorySafety,
    Initialization,
    RaceFreedom,
    LaunchValidity,
    FunctionalCorrectness,
}

/// Artifact and build identities independently reconstructed during finalization.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProofArtifactIdentity {
    kernel_id: PayloadDigest,
    instance_digest: PayloadDigest,
    source_tree_digest: PayloadDigest,
    crate_graph_digest: PayloadDigest,
    executable_digest: PayloadDigest,
    environment_digest: PayloadDigest,
    artifact_selection_digest: PayloadDigest,
    artifact_contract_digest: PayloadDigest,
}

impl ProofArtifactIdentity {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        kernel_id: PayloadDigest,
        instance_digest: PayloadDigest,
        source_tree_digest: PayloadDigest,
        crate_graph_digest: PayloadDigest,
        executable_digest: PayloadDigest,
        environment_digest: PayloadDigest,
        artifact_selection_digest: PayloadDigest,
        artifact_contract_digest: PayloadDigest,
    ) -> Self {
        Self {
            kernel_id,
            instance_digest,
            source_tree_digest,
            crate_graph_digest,
            executable_digest,
            environment_digest,
            artifact_selection_digest,
            artifact_contract_digest,
        }
    }

    pub const fn kernel_id(self) -> PayloadDigest {
        self.kernel_id
    }

    pub const fn instance_digest(self) -> PayloadDigest {
        self.instance_digest
    }

    pub const fn source_tree_digest(self) -> PayloadDigest {
        self.source_tree_digest
    }

    pub const fn crate_graph_digest(self) -> PayloadDigest {
        self.crate_graph_digest
    }

    pub const fn executable_digest(self) -> PayloadDigest {
        self.executable_digest
    }

    pub const fn environment_digest(self) -> PayloadDigest {
        self.environment_digest
    }

    pub const fn artifact_selection_digest(self) -> PayloadDigest {
        self.artifact_selection_digest
    }

    pub const fn artifact_contract_digest(self) -> PayloadDigest {
        self.artifact_contract_digest
    }
}

/// Source-contract identities not representable by manifest v1.
///
/// Their producers must use reviewed, domain-separated canonical encodings.
/// Matching these values does not validate how they were produced.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceContractIdentity {
    memory_digest: PayloadDigest,
    effects_digest: PayloadDigest,
    type_layout_digest: PayloadDigest,
    capability_semantics_digest: PayloadDigest,
    functional_specification_digest: PayloadDigest,
}

impl SourceContractIdentity {
    pub const fn new(
        memory_digest: PayloadDigest,
        effects_digest: PayloadDigest,
        type_layout_digest: PayloadDigest,
        capability_semantics_digest: PayloadDigest,
        functional_specification_digest: PayloadDigest,
    ) -> Self {
        Self {
            memory_digest,
            effects_digest,
            type_layout_digest,
            capability_semantics_digest,
            functional_specification_digest,
        }
    }

    pub const fn memory_digest(self) -> PayloadDigest {
        self.memory_digest
    }

    pub const fn effects_digest(self) -> PayloadDigest {
        self.effects_digest
    }

    pub const fn type_layout_digest(self) -> PayloadDigest {
        self.type_layout_digest
    }

    pub const fn capability_semantics_digest(self) -> PayloadDigest {
        self.capability_semantics_digest
    }

    pub const fn functional_specification_digest(self) -> PayloadDigest {
        self.functional_specification_digest
    }
}

/// Complete identities a proof record must match to caller-supplied evidence.
///
/// Equality of these identities is never sufficient to promote assurance.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProofTargetIdentity {
    artifact: ProofArtifactIdentity,
    source_contracts: SourceContractIdentity,
}

impl ProofTargetIdentity {
    pub const fn new(
        artifact: ProofArtifactIdentity,
        source_contracts: SourceContractIdentity,
    ) -> Self {
        Self {
            artifact,
            source_contracts,
        }
    }

    pub const fn artifact(self) -> ProofArtifactIdentity {
        self.artifact
    }

    pub const fn source_contracts(self) -> SourceContractIdentity {
        self.source_contracts
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ConfigurationEntry {
    key: Name,
    value: IdentityText,
}

impl ConfigurationEntry {
    pub const fn new(key: Name, value: IdentityText) -> Self {
        Self { key, value }
    }

    pub const fn key(&self) -> &Name {
        &self.key
    }

    pub const fn value(&self) -> &IdentityText {
        &self.value
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationModelIdentity {
    version: IdentityText,
    axioms_digest: PayloadDigest,
}

impl VerificationModelIdentity {
    pub const fn new(version: IdentityText, axioms_digest: PayloadDigest) -> Self {
        Self {
            version,
            axioms_digest,
        }
    }

    pub const fn version(&self) -> &IdentityText {
        &self.version
    }

    pub const fn axioms_digest(&self) -> PayloadDigest {
        self.axioms_digest
    }
}

/// Exact identity of a tool participating in proof production.
///
/// The executable digest measures the tool binary or immutable distribution.
/// The configuration digest measures all settings that can affect its result.
/// These values remain evidence supplied by a future audited proof driver; this
/// type does not authenticate either measurement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MeasuredToolIdentity {
    name: IdentityText,
    version: IdentityText,
    executable_digest: PayloadDigest,
    configuration_digest: PayloadDigest,
}

impl MeasuredToolIdentity {
    pub const fn new(
        name: IdentityText,
        version: IdentityText,
        executable_digest: PayloadDigest,
        configuration_digest: PayloadDigest,
    ) -> Self {
        Self {
            name,
            version,
            executable_digest,
            configuration_digest,
        }
    }

    pub const fn name(&self) -> &IdentityText {
        &self.name
    }

    pub const fn version(&self) -> &IdentityText {
        &self.version
    }

    pub const fn executable_digest(&self) -> PayloadDigest {
        self.executable_digest
    }

    pub const fn configuration_digest(&self) -> PayloadDigest {
        self.configuration_digest
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofExecutionIdentity {
    model: VerificationModelIdentity,
    verifier: MeasuredToolIdentity,
    solver: MeasuredToolIdentity,
    evidence_recorder: MeasuredToolIdentity,
    invocation_digest: PayloadDigest,
}

impl ProofExecutionIdentity {
    pub const fn new(
        model: VerificationModelIdentity,
        verifier: MeasuredToolIdentity,
        solver: MeasuredToolIdentity,
        evidence_recorder: MeasuredToolIdentity,
        invocation_digest: PayloadDigest,
    ) -> Self {
        Self {
            model,
            verifier,
            solver,
            evidence_recorder,
            invocation_digest,
        }
    }

    pub const fn model(&self) -> &VerificationModelIdentity {
        &self.model
    }

    pub const fn verifier(&self) -> &MeasuredToolIdentity {
        &self.verifier
    }

    pub const fn solver(&self) -> &MeasuredToolIdentity {
        &self.solver
    }

    pub const fn evidence_recorder(&self) -> &MeasuredToolIdentity {
        &self.evidence_recorder
    }

    pub const fn invocation_digest(&self) -> PayloadDigest {
        self.invocation_digest
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TrustedItem {
    name: Name,
    contract_digest: PayloadDigest,
}

impl TrustedItem {
    pub const fn new(name: Name, contract_digest: PayloadDigest) -> Self {
        Self {
            name,
            contract_digest,
        }
    }

    pub const fn name(&self) -> &Name {
        &self.name
    }

    pub const fn contract_digest(&self) -> PayloadDigest {
        self.contract_digest
    }
}

/// A validated in-memory proof record. Wire encoding is versioned separately.
///
/// Construction records a tool result. It does not establish that the tool ran
/// or that the result is trustworthy, and therefore cannot elevate assurance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofRecordV1 {
    target: ProofTargetIdentity,
    configuration: Vec<ConfigurationEntry>,
    execution: ProofExecutionIdentity,
    outcome: ProofOutcome,
    proved_properties: Vec<ProofProperty>,
    trusted_items: Vec<TrustedItem>,
}

impl ProofRecordV1 {
    pub fn new(
        target: ProofTargetIdentity,
        mut configuration: Vec<ConfigurationEntry>,
        execution: ProofExecutionIdentity,
        outcome: ProofOutcome,
        mut proved_properties: Vec<ProofProperty>,
        mut trusted_items: Vec<TrustedItem>,
    ) -> Result<Self, ValidationError> {
        canonicalize_configuration(&mut configuration)?;
        canonicalize_properties(&mut proved_properties)?;
        canonicalize_trusted_items(&mut trusted_items)?;
        if outcome == ProofOutcome::Proved && proved_properties.is_empty() {
            return Err(ValidationError::EmptyCollection {
                field: "proved properties",
            });
        }

        Ok(Self {
            target,
            configuration,
            execution,
            outcome,
            proved_properties,
            trusted_items,
        })
    }

    pub const fn target(&self) -> ProofTargetIdentity {
        self.target
    }

    pub fn configuration(&self) -> &[ConfigurationEntry] {
        &self.configuration
    }

    pub const fn execution(&self) -> &ProofExecutionIdentity {
        &self.execution
    }

    pub const fn outcome(&self) -> ProofOutcome {
        self.outcome
    }

    pub fn proved_properties(&self) -> &[ProofProperty] {
        &self.proved_properties
    }

    pub fn trusted_items(&self) -> &[TrustedItem] {
        &self.trusted_items
    }
}

pub(crate) fn canonicalize_configuration(
    configuration: &mut [ConfigurationEntry],
) -> Result<(), ValidationError> {
    require_at_most(
        configuration.len(),
        "proof configuration",
        MAX_CONFIGURATION_ENTRIES,
    )?;
    configuration.sort_unstable();
    if configuration
        .windows(2)
        .any(|pair| pair[0].key() == pair[1].key())
    {
        Err(ValidationError::Duplicate {
            field: "proof configuration key",
        })
    } else {
        Ok(())
    }
}

pub(crate) fn canonicalize_properties(
    properties: &mut [ProofProperty],
) -> Result<(), ValidationError> {
    require_at_most(properties.len(), "proved properties", MAX_PROOF_PROPERTIES)?;
    sort_unique(properties, "proved property")
}

pub(crate) fn canonicalize_trusted_items(
    trusted_items: &mut [TrustedItem],
) -> Result<(), ValidationError> {
    require_at_most(trusted_items.len(), "trusted items", MAX_TRUSTED_ITEMS)?;
    trusted_items.sort_unstable();
    if trusted_items
        .windows(2)
        .any(|pair| pair[0].name() == pair[1].name())
    {
        Err(ValidationError::Duplicate {
            field: "trusted item name",
        })
    } else {
        Ok(())
    }
}

fn require_at_most(count: usize, field: &'static str, max: usize) -> Result<(), ValidationError> {
    if count > max {
        Err(ValidationError::TooMany { field, max })
    } else {
        Ok(())
    }
}

fn sort_unique<T: Ord>(values: &mut [T], field: &'static str) -> Result<(), ValidationError> {
    values.sort_unstable();
    if values.windows(2).any(|pair| pair[0] == pair[1]) {
        Err(ValidationError::Duplicate { field })
    } else {
        Ok(())
    }
}
