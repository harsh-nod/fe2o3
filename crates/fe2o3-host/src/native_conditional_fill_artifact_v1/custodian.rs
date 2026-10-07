//! Content/currentness join with actual retained native controller custody.
use super::*;
use fe2o3_compiler_execution_client::{
    NativeApplicationChannelErrorV1, RetainedNativeApplicationProofV1,
};
use fe2o3_runtime_protocol::NativeConditionalApplicationBindingV1 as Association;
use sha2::{Digest, Sha256};
use std::time::Instant;

/// Exact original publication/content matched to a live native proof custodian.
///
/// This retains the actual proof-capable native currentness owner, two sealed
/// inputs, independently installed profiles and matched controller evidence.
/// Evidence for analyzer execution and generated refinement remains descriptive
/// here: no local authenticated-machine owner or executed-theorem owner is
/// reconstructed from hashes, a key, or a self-signed receipt. Publication lock,
/// runtime machine refinement and native memory authority remain mandatory.
///
/// ```compile_fail
/// use fe2o3_host::CheckedCustodiedNativeConditionalFillContentV1 as Checked;
/// use fe2o3_runtime_protocol::NativeApplicationProofEvidenceV1 as Evidence;
/// fn promote<'a, 'w>(value: &'a Evidence) -> Checked<'a, 'w> { value.into() }
/// ```
pub struct CheckedCustodiedNativeConditionalFillContentV1<'a, 'work> {
    publication: &'a RecoveredConditionalWorkerHsacoPublicationV5,
    carriage: &'a CompilerExecutionReceiptCarriageV3,
    proof: &'a RetainedNativeApplicationProofV1<'work>,
}
impl<'a, 'work> CheckedCustodiedNativeConditionalFillContentV1<'a, 'work> {
    pub const fn publication(&self) -> &'a RecoveredConditionalWorkerHsacoPublicationV5 {
        self.publication
    }
    pub const fn carriage(&self) -> &'a CompilerExecutionReceiptCarriageV3 {
        self.carriage
    }
    pub const fn proof(&self) -> &'a RetainedNativeApplicationProofV1<'work> {
        self.proof
    }
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Joins actual original owners and exact registered readiness, never a bare record.
/// Prepay publication, carriage, readiness and retained proof on the original
/// account. Both matched Probe/Retained round trips are required; failures leave
/// terminal operation charges. No machine or proof authority is synthesized.
pub fn check_custodied_native_conditional_fill_content_v1<'a, 'work>(
    publication: &'a RecoveredConditionalWorkerHsacoPublicationV5,
    carriage: &'a CompilerExecutionReceiptCarriageV3,
    readiness: &[u8],
    proof: &'a mut RetainedNativeApplicationProofV1<'work>,
    deadline: Instant,
    budget: &mut Budget<'work>,
) -> Result<
    (
        CheckedCustodiedNativeConditionalFillContentV1<'a, 'work>,
        NativeConditionalFillArtifactStorageV1,
    ),
    Error,
> {
    budget.charge_work(8)?;
    let floor = budget.storage();
    let required = publication
        .required_retained_storage()
        .checked_add(carriage.retained_storage())
        .and_then(|bytes| bytes.checked_add(readiness.len()))
        .and_then(|bytes| bytes.checked_add(proof.retained_storage().ok()?))
        .ok_or(Resource::Arithmetic)?;
    if floor < required {
        return Err(Resource::Accounting.into());
    }
    let finalized = publication.finalized();
    let owner = finalized.source().recovered_handoff();
    let payload = finalized.finalized().as_bytes();
    let handoff = owner.handoff().canonical_bytes();
    let final_kir = owner.output().canonical().canonical_bytes();
    let work = [
        readiness.len(),
        payload.len(),
        handoff.len(),
        final_kir.len(),
        4096,
    ]
    .into_iter()
    .try_fold(0usize, usize::checked_add)
    .ok_or(Resource::Arithmetic)?;
    budget.charge_work(work)?;
    budget.reserve_storage(16 * 1024)?;
    proof.probe(deadline, budget).map_err(Error::Provenance)?;
    let current = proof.currentness();
    let registration = current.registration().registration();
    let association =
        Association::bind(readiness, registration.compiler_handoff(), carriage, budget).map_err(
            |error| Error::Provenance(NativeApplicationChannelErrorV1::Association(error)),
        )?;
    let coordinates = publication.transcript().coordinates();
    let (subject, storage) = InertCompilerExecutionSubjectV3::from_replay_evidence(
        coordinates.attempt(),
        coordinates.slot(),
        coordinates.transaction_identity(),
        owner.handoff(),
        budget,
    )
    .map_err(Error::Subject)?;
    budget.reserve_storage(storage.retained_storage())?;
    let parts = proof.evidence().parts();
    let actual = [
        digest(handoff),
        digest(final_kir),
        digest(payload),
        digest(readiness),
    ];
    let expected = [
        parts.native_handoff,
        parts.final_kernel_ir,
        proof.inputs().payload(),
        proof.inputs().readiness(),
    ];
    if actual != expected
        || proof.evidence().boundary() != 6
        || association.canonical_bytes() != registration.association().canonical_bytes()
        || subject.canonical_bytes() != carriage.request().subject().canonical_bytes()
        || parts.subject_identity != *subject.identity().sha256()
        || parts.subject_identity != current.verified().verification().subject_identity()
        || parts.carriage_identity != *carriage.identity().as_bytes()
        || parts.carriage_identity != current.verified().verification().carriage_identity()
        || parts.policy_identity != *carriage.policy().identity().as_bytes()
        || parts.policy_identity
            != current
                .registration()
                .proof_profile()
                .configuration()
                .parts()
                .compiler_policy_identity
    {
        return Err(Error::Binding);
    }
    drop(subject);
    proof.probe(deadline, budget).map_err(Error::Provenance)?;
    budget.release_storage(
        budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)?,
    )?;
    Ok((
        CheckedCustodiedNativeConditionalFillContentV1 {
            publication,
            carriage,
            proof,
        },
        NativeConditionalFillArtifactStorageV1(size_of::<(
            CheckedCustodiedNativeConditionalFillContentV1,
            NativeConditionalFillArtifactStorageV1,
        )>()),
    ))
}
fn digest(bytes: &[u8]) -> ([u8; 32], u64) {
    (Sha256::digest(bytes).into(), bytes.len() as u64)
}
