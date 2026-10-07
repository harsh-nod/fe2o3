//! Native V5 artifact/refinement composition; protected currentness stays a separate gate.
use fe2o3_artifact_transaction::InertCompilerExecutionSubjectV3;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionReceiptCarriageV3, VerifiedCompilerExecutionCurrentRecordV3,
};
use fe2o3_hsaco_finalize::RecoveredConditionalWorkerHsacoPublicationV5;
use fe2o3_kernel_analysis::AuthenticatedPhysicalMachineAnalysisExecutionV1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_verifier::{
    FunctionalRefinementVerusRuntimeLeaseV1, NativeConditionalFillRefinementErrorV1,
    NativeConditionalFillRefinementExecutionV1, execute_native_conditional_fill_refinement_v1,
};
use std::{fmt, mem::size_of};
mod abi;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod intake;
pub use abi::{
    CheckedNativeConditionalFillAbiV1, NativeConditionalFillAbiErrorV1,
    check_native_conditional_fill_abi_v1,
};
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub use intake::{
    CurrentnessReadyNativeConditionalFillApplicationV1, NativeConditionalFillIntakeErrorV1,
    NativeConditionalFillIntakeStorageV1, NativeConditionalFillInvocationScopeV1,
    PreparedNativeConditionalFillApplicationV1, ProvedNativeConditionalFillApplicationV1,
    RegisteredNativeConditionalFillApplicationV1,
};

#[cfg(target_arch = "x86_64")]
mod custodian;
#[cfg(target_arch = "x86_64")]
mod provenance;
#[cfg(target_arch = "x86_64")]
pub use custodian::{
    CheckedCustodiedNativeConditionalFillContentV1,
    check_custodied_native_conditional_fill_content_v1,
};
#[cfg(target_arch = "x86_64")]
pub use provenance::{
    CheckedRegisteredNativeConditionalFillArtifactV1,
    check_registered_native_conditional_fill_artifact_v1,
};

/// Borrows the actual recovered V5 publication, V3 carriage and independently
/// verified current-record response, retaining their exact final-to-machine proof.
///
/// Cryptographic response verification does NOT admit the service peer, protected
/// key provenance, a live publication lock or application process custody. Those
/// remain separate production gates. No runtime authority conversion is exposed.
///
/// ```compile_fail
/// use fe2o3_host::{CheckedNativeConditionalFillArtifactV1 as Native,
///     RemoteConditionalFillArtifactV1 as Legacy};
/// fn downgrade<'a, K>(value: Native<'a>) -> Legacy<K> { value.into() }
/// ```
/// ```compile_fail
/// use fe2o3_host::CheckedNativeConditionalFillArtifactV1 as Checked;
/// fn escape<'a>(value: Checked<'a>) -> Checked<'static> { value }
/// ```
#[must_use = "reserve the returned charge while retaining every borrowed owner"]
pub struct CheckedNativeConditionalFillArtifactV1<'a> {
    publication: &'a RecoveredConditionalWorkerHsacoPublicationV5,
    carriage: &'a CompilerExecutionReceiptCarriageV3,
    current_record: &'a VerifiedCompilerExecutionCurrentRecordV3,
    refinement: NativeConditionalFillRefinementExecutionV1<'a>,
}
impl CheckedNativeConditionalFillArtifactV1<'_> {
    pub const fn publication(&self) -> &RecoveredConditionalWorkerHsacoPublicationV5 {
        self.publication
    }
    pub const fn carriage(&self) -> &CompilerExecutionReceiptCarriageV3 {
        self.carriage
    }
    pub const fn current_record(&self) -> &VerifiedCompilerExecutionCurrentRecordV3 {
        self.current_record
    }
    pub const fn refinement(&self) -> &NativeConditionalFillRefinementExecutionV1<'_> {
        &self.refinement
    }
    pub const fn authenticates_protected_currentness(&self) -> bool {
        false
    }
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeConditionalFillArtifactStorageV1(usize);
impl NativeConditionalFillArtifactStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

#[derive(Debug)]
pub enum NativeConditionalFillArtifactErrorV1 {
    Resource(Resource),
    Subject(fe2o3_artifact_transaction::CompilerExecutionSubjectErrorV3),
    Binding,
    Refinement(NativeConditionalFillRefinementErrorV1),
    #[cfg(target_arch = "x86_64")]
    Provenance(fe2o3_compiler_execution_client::NativeApplicationChannelErrorV1),
}
impl From<Resource> for NativeConditionalFillArtifactErrorV1 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for NativeConditionalFillArtifactErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native conditional fill artifact rejected: {self:?}")
    }
}
impl std::error::Error for NativeConditionalFillArtifactErrorV1 {}
type Error = NativeConditionalFillArtifactErrorV1;

/// Composes the actual finalized V5 artifact with its exact authenticated machine
/// analysis, source/F owner, native V3 subject and cryptographically verified
/// current-record coordinates. Independently admitted service/lock/application
/// custody is intentionally not inferred from these bytes or protocol objects.
///
/// Caller prepays the recovered publication, carriage and current-record headers
/// on the same account. Local equality scratch is refundable; the subsequent
/// generated proof is terminal on error/unwind and MUST NOT be blanket-refunded.
/// Reserve the returned additional proof/header charge before retaining the view.
pub fn check_native_conditional_fill_artifact_v1<'a>(
    runtime: &FunctionalRefinementVerusRuntimeLeaseV1,
    publication: &'a RecoveredConditionalWorkerHsacoPublicationV5,
    carriage: &'a CompilerExecutionReceiptCarriageV3,
    current_record: &'a VerifiedCompilerExecutionCurrentRecordV3,
    analysis: &'a AuthenticatedPhysicalMachineAnalysisExecutionV1,
    timeout_seconds: u32,
    budget: &mut Budget<'_>,
) -> Result<
    (
        CheckedNativeConditionalFillArtifactV1<'a>,
        NativeConditionalFillArtifactStorageV1,
    ),
    Error,
> {
    let finalized = publication.finalized();
    let owner = finalized.source().recovered_handoff();
    let floor = publication
        .required_retained_storage()
        .checked_add(carriage.retained_storage())
        .and_then(|n| n.checked_add(size_of::<VerifiedCompilerExecutionCurrentRecordV3>()))
        .ok_or(Resource::Arithmetic)?;
    let work = finalized
        .finalized()
        .as_bytes()
        .len()
        .checked_add(analysis.request().exact_payload_bytes().len())
        .and_then(|n| n.checked_add(4096))
        .ok_or(Resource::Arithmetic)?;
    budget.with_prepaid_scope(floor, 8, work, 4096, |b| {
        let coordinates = publication.transcript().coordinates();
        let (subject, storage) = InertCompilerExecutionSubjectV3::from_replay_evidence(
            coordinates.attempt(),
            coordinates.slot(),
            coordinates.transaction_identity(),
            owner.handoff(),
            b,
        )
        .map_err(Error::Subject)?;
        b.reserve_storage(storage.retained_storage())?;
        if !exact_binding(
            finalized.finalized().as_bytes(),
            analysis.request().exact_payload_bytes(),
            subject.canonical_bytes(),
            carriage.request().subject().canonical_bytes(),
            *subject.identity().sha256(),
            current_record.verification().subject_identity(),
            *carriage.identity().as_bytes(),
            current_record.verification().carriage_identity(),
        ) {
            return Err(Error::Binding);
        }
        Ok(())
    })?;
    let (refinement, storage) = execute_native_conditional_fill_refinement_v1(
        runtime,
        owner,
        analysis,
        timeout_seconds,
        budget,
    )
    .map_err(Error::Refinement)?;
    let retained = storage
        .retained_storage()
        .checked_sub(size_of::<NativeConditionalFillRefinementExecutionV1>())
        .and_then(|n| n.checked_add(size_of::<CheckedNativeConditionalFillArtifactV1>()))
        .ok_or(Resource::Arithmetic)?;
    Ok((
        CheckedNativeConditionalFillArtifactV1 {
            publication,
            carriage,
            current_record,
            refinement,
        },
        NativeConditionalFillArtifactStorageV1(retained),
    ))
}

#[allow(clippy::too_many_arguments)]
fn exact_binding(
    finalized: &[u8],
    analyzed: &[u8],
    subject: &[u8],
    issued: &[u8],
    subject_identity: [u8; 32],
    observed_subject: [u8; 32],
    carriage_identity: [u8; 32],
    observed_carriage: [u8; 32],
) -> bool {
    finalized == analyzed
        && subject == issued
        && subject_identity == observed_subject
        && carriage_identity == observed_carriage
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_artifact_requires_exact_payload_subject_and_current_record_binding() {
        for changed in 0..5 {
            assert_eq!(
                exact_binding(
                    b"finalized",
                    if changed == 1 { b"other" } else { b"finalized" },
                    b"V3 subject",
                    if changed == 2 {
                        b"V1 subject"
                    } else {
                        b"V3 subject"
                    },
                    [1; 32],
                    if changed == 3 { [2; 32] } else { [1; 32] },
                    [3; 32],
                    if changed == 4 { [4; 32] } else { [3; 32] }
                ),
                changed == 0
            );
        }
    }
}
