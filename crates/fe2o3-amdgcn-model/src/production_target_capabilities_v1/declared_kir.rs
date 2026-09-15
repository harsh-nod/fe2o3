//! Declared canonical graph adapters into the existing exact target-query core.
use super::*;
use fe2o3_kernel_ir::{CanonicalKernelIrVersionV1 as Version, VerifiedCanonicalKernelIrV1};

#[cfg(test)]
mod tests;

fn subject(neutral: &VerifiedCanonicalKernelIrV1, epoch: u64) -> ProductionCanonicalGraphSubjectV1 {
    ProductionCanonicalGraphSubjectV1 {
        version: match neutral.version() { Version::V13 => ProductionCanonicalGraphVersionV1::V13,
            Version::V14 => ProductionCanonicalGraphVersionV1::V14 },
        digest: *neutral.identity().digest(), canonical_length: neutral.identity().canonical_length(), epoch,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionTargetLaunchEvidenceKirV1 {
    identity: [u8; 32],
    subject: ProductionCanonicalGraphSubjectV1,
    values: Box<[ProductionTargetLaunchEvidenceValueV1]>,
}
impl ProductionTargetLaunchEvidenceKirV1 {
    pub fn for_static_launches(neutral: &VerifiedCanonicalKernelIrV1, epoch: u64) -> Result<Self, ProductionTargetCapabilityErrorV1> {
        Self::from_exact_values(neutral, epoch, [])
    }
    pub fn from_exact_values(neutral: &VerifiedCanonicalKernelIrV1, epoch: u64,
        values: impl IntoIterator<Item=ProductionTargetLaunchEvidenceValueV1>) -> Result<Self, ProductionTargetCapabilityErrorV1> {
        neutral.revalidate().map_err(ProductionTargetCapabilityErrorV1::InvalidCanonicalKirDeclared)?;
        let values = exact_launch_values(values)?;
        let subject = subject(neutral, epoch);
        Ok(Self { identity: launch_evidence_identity(subject, &values), subject, values })
    }
    pub const fn identity(&self) -> [u8;32] { self.identity }
    pub const fn subject(&self) -> ProductionCanonicalGraphSubjectV1 { self.subject }
    pub fn values(&self) -> &[ProductionTargetLaunchEvidenceValueV1] { &self.values }
    pub const fn grants_launch_authority(&self) -> bool { false }
    pub(crate) fn from_v13(value: &ProductionTargetLaunchEvidenceV13) -> Self {
        Self { identity: value.identity, subject: value.subject, values: value.values.clone() }
    }
}
impl ExactLaunchEvidenceV1 for ProductionTargetLaunchEvidenceKirV1 {
    fn identity(&self) -> [u8;32] { self.identity }
    fn subject(&self) -> ProductionCanonicalGraphSubjectV1 { self.subject }
    fn values(&self) -> &[ProductionTargetLaunchEvidenceValueV1] { &self.values }
}

/// Inert target-query evidence tied to one declared graph, epoch and launch.
/// Querying V14 does not produce a source witness or an operational proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionTargetCapabilityClosureKirV1 {
    identity: [u8;32], canonical_bytes: Box<[u8]>,
    subject: ProductionCanonicalGraphSubjectV1, target_model: TargetCapabilityModelIdentityV1,
    launch_evidence: ProductionTargetLaunchEvidenceKirV1,
    decisions: Box<[TargetCapabilityDecisionV1]>, owners: Box<[ProductionAmdCapabilityOwnerV1]>,
    records: Box<[ProductionTargetLegalizationRecordV1]>,
    artifact_only_requirements: Box<[ProductionArtifactOnlyRequirementV1]>,
}
impl ProductionTargetCapabilityClosureKirV1 {
    pub fn decode_canonical(bytes: &[u8], launch: &ProductionTargetLaunchEvidenceKirV1) -> Result<Self, ProductionTargetCapabilityCanonicalErrorV1> {
        let decoded = decode_capability_closure(bytes)?;
        if !matches!(decoded.subject.version(), ProductionCanonicalGraphVersionV1::V13 | ProductionCanonicalGraphVersionV1::V14) {
            return Err(ProductionTargetCapabilityCanonicalErrorV1::GraphVersionMismatch);
        }
        if decoded.subject != launch.subject() || decoded.launch_evidence_identity != launch.identity() {
            return Err(ProductionTargetCapabilityCanonicalErrorV1::LaunchEvidenceMismatch);
        }
        Ok(Self { identity: capability_closure_identity(bytes), canonical_bytes: bytes.to_vec().into_boxed_slice(),
            subject: decoded.subject, target_model: decoded.target_model, launch_evidence: launch.clone(),
            decisions: decoded.decisions, owners: decoded.owners, records: decoded.records,
            artifact_only_requirements: decoded.artifact_only_requirements })
    }
    pub const fn identity(&self) -> [u8;32] { self.identity }
    pub fn canonical_bytes(&self) -> &[u8] { &self.canonical_bytes }
    pub const fn subject(&self) -> ProductionCanonicalGraphSubjectV1 { self.subject }
    pub const fn target_model(&self) -> TargetCapabilityModelIdentityV1 { self.target_model }
    pub const fn launch_evidence(&self) -> &ProductionTargetLaunchEvidenceKirV1 { &self.launch_evidence }
    pub fn decisions(&self) -> &[TargetCapabilityDecisionV1] { &self.decisions }
    pub fn capability_owners(&self) -> &[ProductionAmdCapabilityOwnerV1] { &self.owners }
    pub fn legalization_records(&self) -> &[ProductionTargetLegalizationRecordV1] { &self.records }
    pub fn artifact_only_requirements(&self) -> &[ProductionArtifactOnlyRequirementV1] { &self.artifact_only_requirements }
    pub const fn grants_publication_authority(&self) -> bool { false }
    pub const fn grants_load_authority(&self) -> bool { false }
    pub const fn grants_launch_authority(&self) -> bool { false }

    pub(crate) fn into_v13(self) -> Result<ProductionTargetCapabilityClosureV13, ProductionTargetCapabilityErrorV1> {
        if self.subject.version != ProductionCanonicalGraphVersionV1::V13 {
            return Err(ProductionTargetCapabilityErrorV1::DeclaredVersionMismatch);
        }
        Ok(ProductionTargetCapabilityClosureV13 {
            identity: self.identity, canonical_bytes: self.canonical_bytes, subject: self.subject,
            target_model: self.target_model, launch_evidence: ProductionTargetLaunchEvidenceV13 {
                identity: self.launch_evidence.identity, subject: self.launch_evidence.subject,
                values: self.launch_evidence.values },
            decisions: self.decisions, owners: self.owners, records: self.records,
            artifact_only_requirements: self.artifact_only_requirements,
        })
    }
}

pub fn legalize_production_target_capabilities_kir_v1(neutral: &VerifiedCanonicalKernelIrV1,
    epoch: u64, launch: &ProductionTargetLaunchEvidenceKirV1, profile: ProductionAmdTargetProfileV1)
    -> Result<ProductionTargetCapabilityClosureKirV1, ProductionTargetCapabilityErrorV1> {
    legalize(neutral, epoch, launch, profile)
}

pub(super) fn legalize(neutral: &VerifiedCanonicalKernelIrV1, epoch: u64,
    launch: &impl ExactLaunchEvidenceV1, profile: ProductionAmdTargetProfileV1)
    -> Result<ProductionTargetCapabilityClosureKirV1, ProductionTargetCapabilityErrorV1> {
    neutral.revalidate().map_err(ProductionTargetCapabilityErrorV1::InvalidCanonicalKirDeclared)?;
    let module = match neutral.version() {
        Version::V13 => decode_module_v13(neutral.canonical_bytes()),
        Version::V14 => fe2o3_kernel_ir::decode_module_v14(neutral.canonical_bytes()),
    }.map_err(ProductionTargetCapabilityErrorV1::CanonicalKirDecode)?;
    let subject = subject(neutral, epoch);
    let core = legalize_production_target_capability_module_v1(&module, subject, launch, profile)?;
    Ok(ProductionTargetCapabilityClosureKirV1 {
        identity: core.identity, canonical_bytes: core.canonical_bytes, subject, target_model: core.target_model,
        launch_evidence: ProductionTargetLaunchEvidenceKirV1 { identity: launch.identity(), subject: launch.subject(), values: launch.values().to_vec().into_boxed_slice() },
        decisions: core.decisions, owners: core.owners, records: core.records,
        artifact_only_requirements: core.artifact_only_requirements,
    })
}
