//! Cooperative-process Cargo-to-application handoff for production Worker V3 evidence.
//!
//! This module is an inert foundation, not a same-process security boundary. The application must
//! consume the inherited handoff before threads, signal handlers, descendants, or unrelated FD
//! mutation can race it. A malicious application in the same process can bypass these cooperative
//! assumptions and must instead be isolated from authority by a separate broker.

use crate::{
    CompilerGeneratedKernelExpectationRosterV1, KernelId, RecoveredWorkerV3AdmissionErrorV1,
    RecoveredWorkerV3PinnedDescriptorV1, RecoveredWorkerV3PinnedRosterV1,
    admit_recovered_worker_v3_descriptor_v1, admit_recovered_worker_v3_roster_v1,
};
use crate::{RecoveredMixedWorkerV53PinnedRoster, admit_recovered_mixed_worker_v53_roster};
use fe2o3_artifact_transaction::{
    DurableCurrentLinkPublicationTokenV1, WorkerV3LoadReadinessReceiptV1,
};
use fe2o3_compiler_execution_client::{
    ApplicationProofChannelErrorV1, COMPILER_EXECUTION_SERVICE_CHILD_FD_V1,
    RegisteredApplicationProofEndpointV1, RetainedApplicationProofEndpointV1,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as MixedReceiptBudgetV53;
use fe2o3_runtime_protocol::{
    MAX_WORKER_V3_APPLICATION_OCCURRENCE_BYTES_V1, MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V2,
    RecoveredWorkerV3LoadEnvelopeV2, WORKER_V3_APPLICATION_ARTIFACT_DIR_FD_ENV_V1,
    WORKER_V3_APPLICATION_ENVELOPE_FD_ENV_V1, WORKER_V3_APPLICATION_HANDOFF_ACK_FD_ENV_V1,
    WORKER_V3_APPLICATION_HANDOFF_CHALLENGE_BYTES_V1,
    WORKER_V3_APPLICATION_HANDOFF_CHALLENGE_ENV_V1,
    WORKER_V3_APPLICATION_HANDOFF_COMMITMENT_BYTES_V1,
    WORKER_V3_APPLICATION_HANDOFF_COMMITMENT_ENV_V1, WORKER_V3_APPLICATION_OCCURRENCE_ENV_V1,
    WORKER_V3_APPLICATION_PROOF_FD_ENV_V1, WorkerV3ApplicationHandoffChallengeV1,
    WorkerV3ApplicationHandoffCommitmentV1, WorkerV3ApplicationHandoffExpectationV1,
    WorkerV3ApplicationHandoffProtocolErrorV1, WorkerV3ApplicationIdentityV1,
    WorkerV3ApplicationInputOccurrenceV1, WorkerV3ApplicationOccurrenceV1,
    WorkerV3ApplicationRegistrationDescriptorsV1, WorkerV3ApplicationRegistrationErrorV1,
    WorkerV3ApplicationRegistrationInputsV1, WorkerV3LoadEnvelopeErrorV2,
    WorkerV3LoadEnvelopeIdentityV1, WorkerV3LoadEnvelopeWireV2, recover_worker_v3_load_envelope_v2,
};
use rustix::fs::{FileType, OFlags, fcntl_getfl, fcntl_setfl, fstat};
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::fs::FileExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

mod custodian;
use custodian::ApplicationCustodianStateV1;
pub use custodian::RegisteredWorkerV3CustodianApplicationV1;

const MAX_APPLICATION_EXECUTABLE_BYTES_V1: u64 = 1 << 30;
const MAX_APPLICATION_ARTIFACT_DIRECTORY_ENTRIES_V1: usize = 4_096;
const RETIRED_WORKER_V2_ENVELOPE_PREFIX_V1: &str = ".fe2o3-worker-v2-load-envelope-v1-";
const WORKER_V2_APPLICATION_ENVELOPE_FD_ENV_V1: &str = "FE2O3_APPLICATION_ENVELOPE_FD_V1";
const WORKER_V2_APPLICATION_ARTIFACT_DIR_FD_ENV_V1: &str = "FE2O3_APPLICATION_ARTIFACT_DIR_FD_V1";
const WORKER_V2_APPLICATION_HANDOFF_COMMITMENT_ENV_V1: &str =
    "FE2O3_APPLICATION_ENVELOPE_COMMITMENT_V1";
const WORKER_V2_APPLICATION_HANDOFF_ACK_FD_ENV_V1: &str = "FE2O3_APPLICATION_HANDOFF_ACK_FD_V1";
const WORKER_V2_APPLICATION_HANDOFF_CHALLENGE_ENV_V1: &str =
    "FE2O3_APPLICATION_HANDOFF_CHALLENGE_V1";
const STARTUP_DEADLINE_V1: Duration = Duration::from_secs(30);
const WORKER_V3_ENVELOPE_PREFIX_V1: &str = ".fe2o3-worker-v3-load-readiness-v1-";
const WORKER_V3_ENVELOPE_SUFFIX_V1: &str = ".envelope";
const WORKER_V3_ENVELOPE_NAME_BYTES_V1: usize =
    WORKER_V3_ENVELOPE_PREFIX_V1.len() + 64 + WORKER_V3_ENVELOPE_SUFFIX_V1.len();

static INHERITED_HANDOFF_CLAIMED_V1: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DescriptorIdentityV1 {
    device: u64,
    inode: u64,
}

impl DescriptorIdentityV1 {
    const fn from_stat(stat: &rustix::fs::Stat) -> Self {
        Self {
            device: stat.st_dev,
            inode: stat.st_ino,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DirectorySnapshotV1 {
    device: u64,
    inode: u64,
    mode: u32,
    owner: u32,
}

impl DirectorySnapshotV1 {
    const fn from_stat(stat: &rustix::fs::Stat) -> Self {
        Self {
            device: stat.st_dev,
            inode: stat.st_ino,
            mode: stat.st_mode,
            owner: stat.st_uid,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EnvelopeSnapshotV1 {
    device: u64,
    inode: u64,
    mode: u32,
    owner: u32,
    links: u64,
    size: i64,
    modified_seconds: i64,
    modified_nanoseconds: u64,
    changed_seconds: i64,
    changed_nanoseconds: u64,
}

impl EnvelopeSnapshotV1 {
    const fn from_stat(stat: &rustix::fs::Stat) -> Self {
        Self {
            device: stat.st_dev,
            inode: stat.st_ino,
            mode: stat.st_mode,
            owner: stat.st_uid,
            links: stat.st_nlink,
            size: stat.st_size,
            modified_seconds: stat.st_mtime,
            modified_nanoseconds: stat.st_mtime_nsec,
            changed_seconds: stat.st_ctime,
            changed_nanoseconds: stat.st_ctime_nsec,
        }
    }
}

struct InspectedWorkerV3EnvelopeV1 {
    snapshot: EnvelopeSnapshotV1,
    exact_bytes: Box<[u8]>,
    decoded: WorkerV3LoadEnvelopeWireV2,
    canonical_name: String,
}

/// Exact inherited V3 inputs retained through native executable unload.
pub(crate) struct RetainedWorkerV3ApplicationDescriptorsV1 {
    directory: File,
    envelope: File,
    proof: ApplicationRegistrationCustodyV1,
    directory_snapshot: DirectorySnapshotV1,
    envelope_snapshot: EnvelopeSnapshotV1,
    envelope_name: String,
    exact_envelope_bytes: Box<[u8]>,
    expectation: WorkerV3ApplicationHandoffExpectationV1,
    challenge: WorkerV3ApplicationHandoffChallengeV1,
}

enum ApplicationRegistrationCustodyV1 {
    Unregistered(RetainedApplicationProofEndpointV1),
    Registered(RegisteredApplicationProofEndpointV1),
    Custodian(std::sync::Mutex<ApplicationCustodianStateV1>),
}

#[derive(Clone, Copy)]
enum ApplicationRegistrationModeV1 {
    Required,
    Custodian,
    #[cfg(feature = "hardware-test-hooks")]
    EnvelopeOnlyFixture,
}

impl fmt::Debug for RetainedWorkerV3ApplicationDescriptorsV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RetainedWorkerV3ApplicationDescriptorsV1")
            .field("directory", &self.directory_snapshot)
            .field("envelope", &self.envelope_snapshot)
            .field("envelope_name", &self.envelope_name)
            .field("commitment", &self.expectation.commitment())
            .field("challenge", &self.challenge)
            .finish_non_exhaustive()
    }
}

impl RetainedWorkerV3ApplicationDescriptorsV1 {
    pub(crate) fn revalidate(&self) -> Result<(), WorkerV3ApplicationDescriptorHandoffErrorV1> {
        validate_directory(&self.directory, self.directory_snapshot)
            .map_err(WorkerV3ApplicationDescriptorHandoffErrorV1::Descriptor)?;
        validate_envelope(
            &self.directory,
            &self.envelope,
            self.envelope_snapshot,
            &self.exact_envelope_bytes,
            &self.envelope_name,
        )
        .map_err(worker_v3_descriptor_error)?;
        reject_worker_v2_envelope_coexistence(&self.directory)
            .map_err(worker_v3_descriptor_error)?;
        match &self.proof {
            ApplicationRegistrationCustodyV1::Unregistered(proof) => proof.revalidate(),
            ApplicationRegistrationCustodyV1::Registered(proof) => proof.revalidate(),
            ApplicationRegistrationCustodyV1::Custodian(proof) => proof
                .try_lock()
                .map_err(|_| custodian::terminal())?
                .revalidate(),
        }
        .map_err(WorkerV3ApplicationDescriptorHandoffErrorV1::ProofChannel)?;
        Ok(())
    }

    fn register(
        mut self,
        inputs: WorkerV3ApplicationRegistrationInputsV1,
        mode: ApplicationRegistrationModeV1,
        deadline: Instant,
    ) -> Result<Self, WorkerV3ApplicationDescriptorHandoffErrorV1> {
        match mode {
            ApplicationRegistrationModeV1::Required | ApplicationRegistrationModeV1::Custodian => {
                self.proof = match self.proof {
                    ApplicationRegistrationCustodyV1::Unregistered(proof) => {
                        if matches!(mode, ApplicationRegistrationModeV1::Custodian) {
                            ApplicationRegistrationCustodyV1::Custodian(std::sync::Mutex::new(
                                ApplicationCustodianStateV1::register(proof, inputs, deadline)?,
                            ))
                        } else {
                            ApplicationRegistrationCustodyV1::Registered(
                                proof.register_pre_ack(inputs, deadline).map_err(
                                    WorkerV3ApplicationDescriptorHandoffErrorV1::ProofChannel,
                                )?,
                            )
                        }
                    }
                    ApplicationRegistrationCustodyV1::Registered(_)
                    | ApplicationRegistrationCustodyV1::Custodian(_) => {
                        unreachable!("single-use startup registration")
                    }
                };
            }
            #[cfg(feature = "hardware-test-hooks")]
            ApplicationRegistrationModeV1::EnvelopeOnlyFixture => {}
        }
        Ok(self)
    }
}

trait WorkerV3ApplicationHandoffAdmissionV1: Sized {
    fn acquire_currentness(
        &self,
    ) -> Result<DurableCurrentLinkPublicationTokenV1, RecoveredWorkerV3AdmissionErrorV1>;
    fn revalidate_currentness(
        &self,
        token: &DurableCurrentLinkPublicationTokenV1,
    ) -> Result<(), RecoveredWorkerV3AdmissionErrorV1>;

    fn retain_application_descriptors(
        self,
        descriptors: RetainedWorkerV3ApplicationDescriptorsV1,
    ) -> Self;
}

impl WorkerV3ApplicationHandoffAdmissionV1 for RecoveredWorkerV3PinnedDescriptorV1 {
    fn acquire_currentness(
        &self,
    ) -> Result<DurableCurrentLinkPublicationTokenV1, RecoveredWorkerV3AdmissionErrorV1> {
        self.acquire_retained_currentness_token()
    }

    fn revalidate_currentness(
        &self,
        token: &DurableCurrentLinkPublicationTokenV1,
    ) -> Result<(), RecoveredWorkerV3AdmissionErrorV1> {
        self.revalidate_retained_currentness_token(token)
    }

    fn retain_application_descriptors(
        self,
        descriptors: RetainedWorkerV3ApplicationDescriptorsV1,
    ) -> Self {
        RecoveredWorkerV3PinnedDescriptorV1::retain_application_descriptors(self, descriptors)
    }
}

impl<R> WorkerV3ApplicationHandoffAdmissionV1 for RecoveredWorkerV3PinnedRosterV1<R> {
    fn acquire_currentness(
        &self,
    ) -> Result<DurableCurrentLinkPublicationTokenV1, RecoveredWorkerV3AdmissionErrorV1> {
        self.acquire_retained_currentness_token()
    }

    fn revalidate_currentness(
        &self,
        token: &DurableCurrentLinkPublicationTokenV1,
    ) -> Result<(), RecoveredWorkerV3AdmissionErrorV1> {
        self.revalidate_retained_currentness_token(token)
    }

    fn retain_application_descriptors(
        self,
        descriptors: RetainedWorkerV3ApplicationDescriptorsV1,
    ) -> Self {
        RecoveredWorkerV3PinnedRosterV1::retain_application_descriptors(self, descriptors)
    }
}

impl<R> WorkerV3ApplicationHandoffAdmissionV1 for RecoveredMixedWorkerV53PinnedRoster<R> {
    fn acquire_currentness(
        &self,
    ) -> Result<DurableCurrentLinkPublicationTokenV1, RecoveredWorkerV3AdmissionErrorV1> {
        self.acquire_retained_currentness_token()
    }
    fn revalidate_currentness(
        &self,
        token: &DurableCurrentLinkPublicationTokenV1,
    ) -> Result<(), RecoveredWorkerV3AdmissionErrorV1> {
        self.revalidate_retained_currentness_token(token)
    }
    fn retain_application_descriptors(
        self,
        descriptors: RetainedWorkerV3ApplicationDescriptorsV1,
    ) -> Self {
        RecoveredMixedWorkerV53PinnedRoster::retain_application_descriptors(self, descriptors)
    }
}

/// Consumes the same protected Cargo handoff into the complete V53 roster.
/// V26 contracts and V50 source execution content remain mandatory; there is no
/// retry through V1, V3 or V5 descriptor admission. This transition does not
/// substitute for protected compiler/native verification or authorize a load.
///
/// # Safety
/// Call only during cooperative single-threaded startup, before signal handlers,
/// descendants or unrelated descriptor/environment mutation can race recovery.
pub unsafe fn consume_inherited_mixed_worker_v53_application_handoff<R>(
    budget: &mut MixedReceiptBudgetV53<'_>,
) -> Result<RecoveredMixedWorkerV53PinnedRoster<R>, WorkerV3ApplicationDescriptorHandoffErrorV1>
where
    R: CompilerGeneratedKernelExpectationRosterV1,
{
    // SAFETY: the public entry has the same startup contract as the claim.
    let claimed = unsafe { claim_inherited_worker_v3_application_handoff_v1()? };
    consume_worker_v3_application_handoff_with_admission_v1(
        claimed.envelope,
        claimed.directory,
        claimed.acknowledgment,
        claimed.proof,
        claimed.occurrence,
        claimed.commitment,
        claimed.challenge,
        ApplicationRegistrationModeV1::Required,
        |envelope| admit_recovered_mixed_worker_v53_roster::<R>(envelope, budget),
    )
    .map(|(admission, _current)| admission)
}

impl<R> WorkerV3ApplicationHandoffAdmissionV1 for crate::RecoveredMixedWorkerV89PinnedRoster<R> {
    fn acquire_currentness(
        &self,
    ) -> Result<DurableCurrentLinkPublicationTokenV1, RecoveredWorkerV3AdmissionErrorV1> {
        self.acquire_retained_currentness_token()
    }
    fn revalidate_currentness(
        &self,
        token: &DurableCurrentLinkPublicationTokenV1,
    ) -> Result<(), RecoveredWorkerV3AdmissionErrorV1> {
        self.revalidate_retained_currentness_token(token)
    }

    fn retain_application_descriptors(
        self,
        descriptors: RetainedWorkerV3ApplicationDescriptorsV1,
    ) -> Self {
        crate::RecoveredMixedWorkerV89PinnedRoster::retain_application_descriptors(
            self,
            descriptors,
        )
    }
}

/// Consumes the protected handoff into the exact predicated descriptor roster.
/// There is no legacy descriptor retry and no proof, load, or launch authority.
///
/// # Safety
/// Call during cooperative single-threaded startup before handlers, descendants,
/// or unrelated descriptor/environment mutation can race recovery. Transfer each
/// inherited descriptor exactly once, including on refusal or unwind.
pub unsafe fn consume_inherited_mixed_worker_v89_application_handoff<R>(
    budget: &mut MixedReceiptBudgetV53<'_>,
) -> Result<
    crate::RecoveredMixedWorkerV89PinnedRoster<R>,
    WorkerV3ApplicationDescriptorHandoffErrorV1,
>
where
    R: CompilerGeneratedKernelExpectationRosterV1,
{
    // SAFETY: the caller transfers the exact startup custody required by the claim.
    let claimed = unsafe { claim_inherited_worker_v3_application_handoff_v1()? };
    consume_worker_v3_application_handoff_with_admission_v1(
        claimed.envelope,
        claimed.directory,
        claimed.acknowledgment,
        claimed.proof,
        claimed.occurrence,
        claimed.commitment,
        claimed.challenge,
        ApplicationRegistrationModeV1::Required,
        |envelope| crate::admit_recovered_mixed_worker_v89_roster::<R>(envelope, budget),
    )
    .map(|(admission, _current)| admission)
}

struct ClaimedInheritedWorkerV3ApplicationHandoffV1 {
    envelope: OwnedFd,
    directory: OwnedFd,
    acknowledgment: OwnedFd,
    proof: Option<OwnedFd>,
    occurrence: WorkerV3ApplicationOccurrenceV1,
    commitment: WorkerV3ApplicationHandoffCommitmentV1,
    challenge: WorkerV3ApplicationHandoffChallengeV1,
}

/// Consumes Cargo's strict Worker V3 descriptor handoff and recovers its exact publication.
///
/// All V2 and V3 handoff environment values are removed atomically with respect to this
/// operation's startup contract. The supplied occurrence is independently reconstructed from the
/// current executable and inherited descriptor objects. The returned descriptor retains the
/// envelope, artifact-directory and registered root-channel custody through native unload.
/// Four input slots and root registration are mandatory, including when test features are enabled.
/// This grants neither verification nor launch authority.
///
/// # Safety
///
/// The caller must invoke this operation before creating threads, installing signal handlers that
/// can access the environment or descriptor table, spawning descendants, or allowing unrelated
/// descriptor mutation. A hostile same-process caller violates this cooperative contract.
pub unsafe fn consume_inherited_worker_v3_application_handoff_v1(
    kernel_id: KernelId,
) -> Result<RecoveredWorkerV3PinnedDescriptorV1, WorkerV3ApplicationDescriptorHandoffErrorV1> {
    // SAFETY: this function exposes the same cooperative startup contract as the private claim.
    let claimed = unsafe { claim_inherited_worker_v3_application_handoff_v1()? };
    consume_worker_v3_application_handoff_descriptors_v1(
        claimed.envelope,
        claimed.directory,
        claimed.acknowledgment,
        claimed.proof,
        claimed.occurrence,
        claimed.commitment,
        claimed.challenge,
        kernel_id,
    )
}

/// Consumes the strict handoff through the independently installed proof custodian.
///
/// Requires root CustodianReady, emits ACK, and retains the original controller endpoint
/// inside the recovered publication. Legacy Ready is rejected without fallback. This is
/// still inert admission: no proof request, compiler audit or native launch occurs here.
///
/// # Safety
///
/// The same exclusive startup/environment/descriptor contract as
/// [`consume_inherited_worker_v3_application_handoff_v1`] applies.
pub unsafe fn consume_inherited_worker_v3_application_custodian_handoff_v1(
    kernel_id: KernelId,
) -> Result<RegisteredWorkerV3CustodianApplicationV1, WorkerV3ApplicationDescriptorHandoffErrorV1> {
    // SAFETY: caller promises the same exclusive startup contract as the private claim.
    let claimed = unsafe { claim_inherited_worker_v3_application_handoff_v1()? };
    consume_worker_v3_application_handoff_with_admission_v1(
        claimed.envelope,
        claimed.directory,
        claimed.acknowledgment,
        claimed.proof,
        claimed.occurrence,
        claimed.commitment,
        claimed.challenge,
        ApplicationRegistrationModeV1::Custodian,
        |recovered| admit_recovered_worker_v3_descriptor_v1(recovered, kernel_id),
    )
    .map(|(admission, current)| RegisteredWorkerV3CustodianApplicationV1 { admission, current })
}

/// Consumes Cargo's strict Worker V3 descriptor handoff into one exact generated roster.
///
/// The complete marker roster is matched in canonical descriptor-table order. The returned roster
/// retains the envelope, artifact-directory and registered root-channel custody through native
/// unload and grants neither verification nor launch authority. Four input slots and root
/// registration are mandatory. It exposes no raw inherited descriptor or envelope bytes.
///
/// ```compile_fail
/// use fe2o3_host::RecoveredWorkerV3PinnedRosterV1;
///
/// fn extract_raw<R>(roster: RecoveredWorkerV3PinnedRosterV1<R>) {
///     let _ = roster.into_raw_envelope();
/// }
/// ```
///
/// # Safety
///
/// The caller must invoke this operation before creating threads, installing signal handlers that
/// can access the environment or descriptor table, spawning descendants, or allowing unrelated
/// descriptor mutation. A hostile same-process caller violates this cooperative contract.
pub unsafe fn consume_inherited_worker_v3_application_roster_handoff_v1<R>()
-> Result<RecoveredWorkerV3PinnedRosterV1<R>, WorkerV3ApplicationDescriptorHandoffErrorV1>
where
    R: CompilerGeneratedKernelExpectationRosterV1,
{
    // SAFETY: this function exposes the same cooperative startup contract as the private claim.
    let claimed = unsafe { claim_inherited_worker_v3_application_handoff_v1()? };
    consume_worker_v3_application_roster_handoff_descriptors_v1::<R>(
        claimed.envelope,
        claimed.directory,
        claimed.acknowledgment,
        claimed.proof,
        claimed.occurrence,
        claimed.commitment,
        claimed.challenge,
    )
}

unsafe fn claim_inherited_worker_v3_application_handoff_v1()
-> Result<ClaimedInheritedWorkerV3ApplicationHandoffV1, WorkerV3ApplicationDescriptorHandoffErrorV1>
{
    let environment = take_inherited_environment();
    if INHERITED_HANDOFF_CLAIMED_V1
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        close_environment_handoff_descriptors(&environment);
        return Err(WorkerV3ApplicationDescriptorHandoffErrorV1::AlreadyConsumed);
    }
    if environment.v2.iter().any(Option::is_some) {
        close_environment_handoff_descriptors(&environment);
        return Err(WorkerV3ApplicationDescriptorHandoffErrorV1::MixedSchema);
    }
    let environment = environment.v3;

    let envelope_raw = worker_v3_environment_fd(
        WORKER_V3_APPLICATION_ENVELOPE_FD_ENV_V1,
        environment[0].as_deref(),
    );
    let directory_raw = worker_v3_environment_fd(
        WORKER_V3_APPLICATION_ARTIFACT_DIR_FD_ENV_V1,
        environment[1].as_deref(),
    );
    let acknowledgment_raw = worker_v3_environment_fd(
        WORKER_V3_APPLICATION_HANDOFF_ACK_FD_ENV_V1,
        environment[2].as_deref(),
    );
    let proof_raw = environment[6]
        .as_deref()
        .map(|value| worker_v3_environment_fd(WORKER_V3_APPLICATION_PROOF_FD_ENV_V1, Some(value)))
        .transpose();
    let (envelope_raw, directory_raw, acknowledgment_raw, proof_raw) =
        match (envelope_raw, directory_raw, acknowledgment_raw, proof_raw) {
            (Ok(envelope), Ok(directory), Ok(acknowledgment), Ok(proof)) => {
                (envelope, directory, acknowledgment, proof)
            }
            (envelope, directory, acknowledgment, proof) => {
                close_available_handoff_descriptors([
                    envelope.as_ref().ok().copied(),
                    directory.as_ref().ok().copied(),
                    acknowledgment.as_ref().ok().copied(),
                    proof.as_ref().ok().copied().flatten(),
                ]);
                return Err(envelope
                    .err()
                    .or_else(|| directory.err())
                    .or_else(|| acknowledgment.err())
                    .or_else(|| proof.err())
                    .expect("the non-success branch contains an error"));
            }
        };
    let descriptors = [
        Some(envelope_raw),
        Some(directory_raw),
        Some(acknowledgment_raw),
        proof_raw,
    ];
    if descriptors.iter().enumerate().any(|(index, descriptor)| {
        descriptor.is_some() && descriptors[..index].contains(descriptor)
    }) {
        close_available_handoff_descriptors(descriptors);
        return Err(WorkerV3ApplicationDescriptorHandoffErrorV1::AliasedDescriptors);
    }

    let envelope = claim_inherited_descriptor(envelope_raw, "Worker V3 envelope")
        .map_err(WorkerV3ApplicationDescriptorHandoffErrorV1::Descriptor);
    let directory = claim_inherited_descriptor(directory_raw, "Worker V3 artifact directory")
        .map_err(WorkerV3ApplicationDescriptorHandoffErrorV1::Descriptor);
    let acknowledgment = claim_inherited_descriptor(acknowledgment_raw, "Worker V3 acknowledgment")
        .map_err(WorkerV3ApplicationDescriptorHandoffErrorV1::Descriptor);
    let proof = proof_raw
        .map(|descriptor| {
            claim_inherited_descriptor(descriptor, "Worker V3 proof endpoint")
                .map_err(WorkerV3ApplicationDescriptorHandoffErrorV1::Descriptor)
        })
        .transpose();
    let envelope = envelope?;
    let directory = directory?;
    let acknowledgment = acknowledgment?;
    let proof = proof?;

    let occurrence = worker_v3_environment_wire(
        WORKER_V3_APPLICATION_OCCURRENCE_ENV_V1,
        environment[3].as_deref(),
        MAX_WORKER_V3_APPLICATION_OCCURRENCE_BYTES_V1,
    )
    .and_then(|bytes| {
        WorkerV3ApplicationOccurrenceV1::decode_canonical(&bytes)
            .map_err(WorkerV3ApplicationDescriptorHandoffErrorV1::Protocol)
    })?;
    let commitment = worker_v3_environment_wire(
        WORKER_V3_APPLICATION_HANDOFF_COMMITMENT_ENV_V1,
        environment[4].as_deref(),
        WORKER_V3_APPLICATION_HANDOFF_COMMITMENT_BYTES_V1,
    )
    .and_then(|bytes| {
        WorkerV3ApplicationHandoffCommitmentV1::decode_canonical(&bytes)
            .map_err(WorkerV3ApplicationDescriptorHandoffErrorV1::Protocol)
    })?;
    let challenge = worker_v3_environment_wire(
        WORKER_V3_APPLICATION_HANDOFF_CHALLENGE_ENV_V1,
        environment[5].as_deref(),
        WORKER_V3_APPLICATION_HANDOFF_CHALLENGE_BYTES_V1,
    )
    .and_then(|bytes| {
        WorkerV3ApplicationHandoffChallengeV1::decode_canonical(&bytes)
            .map_err(WorkerV3ApplicationDescriptorHandoffErrorV1::Protocol)
    })?;

    Ok(ClaimedInheritedWorkerV3ApplicationHandoffV1 {
        envelope,
        directory,
        acknowledgment,
        proof,
        occurrence,
        commitment,
        challenge,
    })
}

/// Envelope-only fixture entrypoint. Does not establish a registered application session.
///
/// # Safety
/// Same cooperative startup contract as the production inherited handoff consumer.
#[cfg(feature = "hardware-test-hooks")]
pub unsafe fn consume_inherited_worker_v3_envelope_only_fixture_v1(
    kernel_id: KernelId,
) -> Result<RecoveredWorkerV3PinnedDescriptorV1, WorkerV3ApplicationDescriptorHandoffErrorV1> {
    // SAFETY: the caller upholds the inherited handoff startup contract.
    let claimed = unsafe { claim_inherited_worker_v3_application_handoff_v1()? };
    consume_worker_v3_application_handoff_with_admission_v1(
        claimed.envelope,
        claimed.directory,
        claimed.acknowledgment,
        claimed.proof,
        claimed.occurrence,
        claimed.commitment,
        claimed.challenge,
        ApplicationRegistrationModeV1::EnvelopeOnlyFixture,
        |recovered| admit_recovered_worker_v3_descriptor_v1(recovered, kernel_id),
    )
    .map(|(admission, _current)| admission)
}

/// Envelope-only roster fixture. Does not establish a registered application session.
///
/// # Safety
/// Same cooperative startup contract as the production inherited roster consumer.
#[cfg(feature = "hardware-test-hooks")]
pub unsafe fn consume_inherited_worker_v3_envelope_only_roster_fixture_v1<R>()
-> Result<RecoveredWorkerV3PinnedRosterV1<R>, WorkerV3ApplicationDescriptorHandoffErrorV1>
where
    R: CompilerGeneratedKernelExpectationRosterV1,
{
    // SAFETY: the caller upholds the inherited handoff startup contract.
    let claimed = unsafe { claim_inherited_worker_v3_application_handoff_v1()? };
    consume_worker_v3_application_handoff_with_admission_v1(
        claimed.envelope,
        claimed.directory,
        claimed.acknowledgment,
        claimed.proof,
        claimed.occurrence,
        claimed.commitment,
        claimed.challenge,
        ApplicationRegistrationModeV1::EnvelopeOnlyFixture,
        admit_recovered_worker_v3_roster_v1::<R>,
    )
    .map(|(admission, _current)| admission)
}

/// Descriptor-level strict V3 application recovery used by the public startup boundary.
#[allow(clippy::too_many_arguments)]
pub(crate) fn consume_worker_v3_application_handoff_descriptors_v1(
    envelope: OwnedFd,
    artifact_directory: OwnedFd,
    acknowledgment: OwnedFd,
    proof: Option<OwnedFd>,
    occurrence: WorkerV3ApplicationOccurrenceV1,
    commitment: WorkerV3ApplicationHandoffCommitmentV1,
    challenge: WorkerV3ApplicationHandoffChallengeV1,
    kernel_id: KernelId,
) -> Result<RecoveredWorkerV3PinnedDescriptorV1, WorkerV3ApplicationDescriptorHandoffErrorV1> {
    consume_worker_v3_application_handoff_with_admission_v1(
        envelope,
        artifact_directory,
        acknowledgment,
        proof,
        occurrence,
        commitment,
        challenge,
        ApplicationRegistrationModeV1::Required,
        |recovered| admit_recovered_worker_v3_descriptor_v1(recovered, kernel_id),
    )
    .map(|(admission, _current)| admission)
}

#[allow(clippy::too_many_arguments)]
fn consume_worker_v3_application_roster_handoff_descriptors_v1<R>(
    envelope: OwnedFd,
    artifact_directory: OwnedFd,
    acknowledgment: OwnedFd,
    proof: Option<OwnedFd>,
    occurrence: WorkerV3ApplicationOccurrenceV1,
    commitment: WorkerV3ApplicationHandoffCommitmentV1,
    challenge: WorkerV3ApplicationHandoffChallengeV1,
) -> Result<RecoveredWorkerV3PinnedRosterV1<R>, WorkerV3ApplicationDescriptorHandoffErrorV1>
where
    R: CompilerGeneratedKernelExpectationRosterV1,
{
    consume_worker_v3_application_handoff_with_admission_v1(
        envelope,
        artifact_directory,
        acknowledgment,
        proof,
        occurrence,
        commitment,
        challenge,
        ApplicationRegistrationModeV1::Required,
        admit_recovered_worker_v3_roster_v1::<R>,
    )
    .map(|(admission, _current)| admission)
}

#[allow(clippy::too_many_arguments)]
fn consume_worker_v3_application_handoff_with_admission_v1<Admission>(
    envelope: OwnedFd,
    artifact_directory: OwnedFd,
    acknowledgment: OwnedFd,
    proof: Option<OwnedFd>,
    occurrence: WorkerV3ApplicationOccurrenceV1,
    commitment: WorkerV3ApplicationHandoffCommitmentV1,
    challenge: WorkerV3ApplicationHandoffChallengeV1,
    registration: ApplicationRegistrationModeV1,
    admit: impl FnOnce(
        RecoveredWorkerV3LoadEnvelopeV2,
    ) -> Result<Admission, RecoveredWorkerV3AdmissionErrorV1>,
) -> Result<
    (Admission, DurableCurrentLinkPublicationTokenV1),
    WorkerV3ApplicationDescriptorHandoffErrorV1,
>
where
    Admission: WorkerV3ApplicationHandoffAdmissionV1,
{
    let deadline = Instant::now().checked_add(STARTUP_DEADLINE_V1).ok_or(
        WorkerV3ApplicationDescriptorHandoffErrorV1::Descriptor(
            ApplicationDescriptorHandoffErrorV1::AcknowledgmentTimeout,
        ),
    )?;
    let proof = proof.ok_or(
        WorkerV3ApplicationDescriptorHandoffErrorV1::MissingEnvironment(
            WORKER_V3_APPLICATION_PROOF_FD_ENV_V1,
        ),
    )?;
    let coordinates = WorkerV3ApplicationRegistrationDescriptorsV1::new(
        envelope.as_raw_fd(),
        artifact_directory.as_raw_fd(),
        acknowledgment.as_raw_fd(),
        proof.as_raw_fd(),
    )
    .map_err(WorkerV3ApplicationDescriptorHandoffErrorV1::Registration)?;
    set_close_on_exec(&envelope, "Worker V3 envelope")
        .map_err(WorkerV3ApplicationDescriptorHandoffErrorV1::Descriptor)?;
    set_close_on_exec(&artifact_directory, "Worker V3 artifact directory")
        .map_err(WorkerV3ApplicationDescriptorHandoffErrorV1::Descriptor)?;
    set_close_on_exec(&acknowledgment, "Worker V3 acknowledgment")
        .map_err(WorkerV3ApplicationDescriptorHandoffErrorV1::Descriptor)?;
    set_close_on_exec(&proof, "Worker V3 proof endpoint")
        .map_err(WorkerV3ApplicationDescriptorHandoffErrorV1::Descriptor)?;
    let directory = File::from(artifact_directory);
    let envelope = File::from(envelope);
    let acknowledgment = File::from(acknowledgment);
    let descriptor_identities = snapshot_handoff_descriptor_identities(
        &directory,
        &envelope,
        &acknowledgment,
        Some(&proof),
    )
    .map_err(WorkerV3ApplicationDescriptorHandoffErrorV1::Descriptor)?;
    seal_descriptor_occurrences(&descriptor_identities)
        .map_err(WorkerV3ApplicationDescriptorHandoffErrorV1::Descriptor)?;
    let directory_snapshot = inspect_directory(&directory)
        .map_err(WorkerV3ApplicationDescriptorHandoffErrorV1::Descriptor)?;
    let InspectedWorkerV3EnvelopeV1 {
        snapshot: envelope_snapshot,
        exact_bytes: exact_envelope_bytes,
        decoded,
        canonical_name: envelope_name,
    } = inspect_worker_v3_envelope(&directory, &envelope)?;
    inspect_acknowledgment(&acknowledgment)
        .map_err(WorkerV3ApplicationDescriptorHandoffErrorV1::Descriptor)?;
    let proof = RetainedApplicationProofEndpointV1::admit_inherited(proof)
        .map_err(WorkerV3ApplicationDescriptorHandoffErrorV1::ProofChannel)?;

    let application = current_application_identity_v3()?;
    let observed_inputs =
        worker_v3_descriptor_occurrences(&envelope, &directory, &acknowledgment, Some(&proof))?;
    validate_worker_v3_application_occurrence(&occurrence, application, &observed_inputs)?;
    let envelope_identity = WorkerV3LoadEnvelopeIdentityV1::from_exact_bytes(&exact_envelope_bytes)
        .map_err(WorkerV3ApplicationDescriptorHandoffErrorV1::Protocol)?;
    let expectation = WorkerV3ApplicationHandoffExpectationV1::new(envelope_identity, &occurrence);
    validate_worker_v3_application_commitment(expectation.commitment(), commitment)?;
    let inputs = WorkerV3ApplicationRegistrationInputsV1::new(
        occurrence,
        coordinates,
        expectation,
        challenge,
    )
    .map_err(WorkerV3ApplicationDescriptorHandoffErrorV1::Registration)?;

    let retained = RetainedWorkerV3ApplicationDescriptorsV1 {
        directory,
        envelope,
        proof: ApplicationRegistrationCustodyV1::Unregistered(proof),
        directory_snapshot,
        envelope_snapshot,
        envelope_name,
        exact_envelope_bytes,
        expectation,
        challenge,
    };
    retained.revalidate()?;
    let output = descriptor_directory_path(&retained.directory);
    let recovered = recover_worker_v3_load_envelope_v2(
        &output,
        decoded.replay().publication_intent_record().attempt(),
    )
    .map_err(WorkerV3ApplicationDescriptorHandoffErrorV1::Envelope)?;
    validate_worker_v3_recovered_occurrence(
        recovered.receipt(),
        retained.directory_snapshot,
        retained.envelope_snapshot,
    )?;
    let recovered_envelope = recovered
        .wire()
        .encode_canonical()
        .map_err(WorkerV3ApplicationDescriptorHandoffErrorV1::Envelope)?;
    if recovered_envelope.as_slice() != retained.exact_envelope_bytes.as_ref() {
        return Err(WorkerV3ApplicationDescriptorHandoffErrorV1::RecoveredEnvelopeMismatch);
    }
    let recovered =
        admit(recovered).map_err(WorkerV3ApplicationDescriptorHandoffErrorV1::Admission)?;
    // Keep one lock owner across registration and ACK; reacquisition would fail Busy.
    let current = recovered
        .acquire_currentness()
        .map_err(WorkerV3ApplicationDescriptorHandoffErrorV1::Admission)?;
    let retained = retained.register(inputs, registration, deadline)?;
    let acknowledgment_bytes = expectation
        .acknowledgment(challenge)
        .encode_canonical()
        .map_err(WorkerV3ApplicationDescriptorHandoffErrorV1::Protocol)?;
    emit_acknowledgment_bytes(&acknowledgment, &acknowledgment_bytes, deadline, || {
        seal_descriptor_occurrences(&descriptor_identities)
            .map_err(WorkerV3ApplicationDescriptorHandoffErrorV1::Descriptor)?;
        recovered
            .revalidate_currentness(&current)
            .map_err(WorkerV3ApplicationDescriptorHandoffErrorV1::Admission)?;
        retained.revalidate()
    })?;
    Ok((recovered.retain_application_descriptors(retained), current))
}

fn worker_v2_handoff_environment_names() -> [&'static str; 5] {
    [
        WORKER_V2_APPLICATION_ENVELOPE_FD_ENV_V1,
        WORKER_V2_APPLICATION_ARTIFACT_DIR_FD_ENV_V1,
        WORKER_V2_APPLICATION_HANDOFF_COMMITMENT_ENV_V1,
        WORKER_V2_APPLICATION_HANDOFF_ACK_FD_ENV_V1,
        WORKER_V2_APPLICATION_HANDOFF_CHALLENGE_ENV_V1,
    ]
}

fn worker_v3_handoff_environment_names() -> [&'static str; 7] {
    [
        WORKER_V3_APPLICATION_ENVELOPE_FD_ENV_V1,
        WORKER_V3_APPLICATION_ARTIFACT_DIR_FD_ENV_V1,
        WORKER_V3_APPLICATION_HANDOFF_ACK_FD_ENV_V1,
        WORKER_V3_APPLICATION_OCCURRENCE_ENV_V1,
        WORKER_V3_APPLICATION_HANDOFF_COMMITMENT_ENV_V1,
        WORKER_V3_APPLICATION_HANDOFF_CHALLENGE_ENV_V1,
        WORKER_V3_APPLICATION_PROOF_FD_ENV_V1,
    ]
}

struct InheritedHandoffEnvironmentV1 {
    v2: [Option<OsString>; 5],
    v3: [Option<OsString>; 7],
}

fn take_inherited_environment() -> InheritedHandoffEnvironmentV1 {
    let v2_names = worker_v2_handoff_environment_names();
    let v3_names = worker_v3_handoff_environment_names();
    let values = InheritedHandoffEnvironmentV1 {
        v2: v2_names.map(std::env::var_os),
        v3: v3_names.map(std::env::var_os),
    };
    // SAFETY: the public function's startup contract excludes concurrent environment access.
    // Removing all handoff values prevents later descendants from inheriting stale descriptor
    // numbers or reproducible protocol material.
    unsafe {
        for name in v2_names.into_iter().chain(v3_names) {
            std::env::remove_var(name);
        }
    }
    values
}

fn close_environment_handoff_descriptors(environment: &InheritedHandoffEnvironmentV1) {
    let descriptor = |value: Option<&OsString>| {
        value
            .and_then(|value| value.to_str())
            .filter(|value| {
                *value == "0"
                    || value
                        .as_bytes()
                        .first()
                        .is_some_and(|byte| matches!(byte, b'1'..=b'9'))
                        && value.as_bytes().iter().all(u8::is_ascii_digit)
            })
            .and_then(|value| value.parse::<RawFd>().ok())
            .filter(|descriptor| valid_application_descriptor(*descriptor))
    };
    close_available_handoff_descriptors([
        descriptor(environment.v2[0].as_ref()),
        descriptor(environment.v2[1].as_ref()),
        descriptor(environment.v2[3].as_ref()),
        descriptor(environment.v3[0].as_ref()),
        descriptor(environment.v3[1].as_ref()),
        descriptor(environment.v3[2].as_ref()),
        descriptor(environment.v3[6].as_ref()),
    ]);
}

fn worker_v3_environment_text(
    name: &'static str,
    value: Option<&OsStr>,
) -> Result<String, WorkerV3ApplicationDescriptorHandoffErrorV1> {
    value
        .ok_or(WorkerV3ApplicationDescriptorHandoffErrorV1::MissingEnvironment(name))?
        .to_os_string()
        .into_string()
        .map_err(|_| WorkerV3ApplicationDescriptorHandoffErrorV1::InvalidEnvironment(name))
}

fn worker_v3_environment_fd(
    name: &'static str,
    value: Option<&OsStr>,
) -> Result<RawFd, WorkerV3ApplicationDescriptorHandoffErrorV1> {
    let value = worker_v3_environment_text(name, value)?;
    let canonical = value == "0"
        || value
            .as_bytes()
            .first()
            .is_some_and(|byte| matches!(byte, b'1'..=b'9'))
            && value.as_bytes().iter().all(u8::is_ascii_digit);
    canonical
        .then(|| value.parse::<RawFd>().ok())
        .flatten()
        .filter(|descriptor| valid_application_descriptor(*descriptor))
        .ok_or(WorkerV3ApplicationDescriptorHandoffErrorV1::InvalidEnvironment(name))
}

fn worker_v3_environment_wire(
    name: &'static str,
    value: Option<&OsStr>,
    max_bytes: usize,
) -> Result<Vec<u8>, WorkerV3ApplicationDescriptorHandoffErrorV1> {
    let value = worker_v3_environment_text(name, value)?;
    let encoded = value.as_bytes();
    let max_encoded = max_bytes
        .checked_mul(2)
        .ok_or(WorkerV3ApplicationDescriptorHandoffErrorV1::InvalidEnvironment(name))?;
    if encoded.is_empty() || encoded.len() % 2 != 0 || encoded.len() > max_encoded {
        return Err(WorkerV3ApplicationDescriptorHandoffErrorV1::InvalidEnvironment(name));
    }
    let mut decoded = Vec::new();
    decoded
        .try_reserve_exact(encoded.len() / 2)
        .map_err(|_| WorkerV3ApplicationDescriptorHandoffErrorV1::InvalidEnvironment(name))?;
    for pair in encoded.chunks_exact(2) {
        let nibble = |byte| match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            _ => None,
        };
        let high = nibble(pair[0])
            .ok_or(WorkerV3ApplicationDescriptorHandoffErrorV1::InvalidEnvironment(name))?;
        let low = nibble(pair[1])
            .ok_or(WorkerV3ApplicationDescriptorHandoffErrorV1::InvalidEnvironment(name))?;
        decoded.push((high << 4) | low);
    }
    Ok(decoded)
}

pub(crate) fn claim_inherited_descriptor(
    descriptor: RawFd,
    kind: &'static str,
) -> Result<OwnedFd, ApplicationDescriptorHandoffErrorV1> {
    // Raw `fcntl` rejects stale integers without manufacturing a `BorrowedFd` first.
    let flags = unsafe { libc::fcntl(descriptor, libc::F_GETFD) };
    if flags < 0 {
        return Err(descriptor_io(kind, io::Error::last_os_error()));
    }
    // SAFETY: successful `F_GETFD` proved that the one-shot protocol's transferred descriptor is
    // live. This function is its sole owner after the environment has been consumed.
    let descriptor = unsafe { OwnedFd::from_raw_fd(descriptor) };
    set_close_on_exec(&descriptor, kind)?;
    if flags & libc::FD_CLOEXEC != 0 {
        return Err(ApplicationDescriptorHandoffErrorV1::DescriptorFlags(kind));
    }
    Ok(descriptor)
}

pub(crate) fn valid_application_descriptor(descriptor: RawFd) -> bool {
    descriptor > libc::STDERR_FILENO && descriptor != COMPILER_EXECUTION_SERVICE_CHILD_FD_V1
}

pub(crate) fn close_available_handoff_descriptors<const N: usize>(descriptors: [Option<RawFd>; N]) {
    for (index, descriptor) in descriptors.into_iter().enumerate() {
        let Some(descriptor) = descriptor else {
            continue;
        };
        if !valid_application_descriptor(descriptor)
            || descriptors[..index].contains(&Some(descriptor))
        {
            continue;
        }
        let _ = claim_inherited_descriptor(descriptor, "rejected handoff");
    }
}

fn set_close_on_exec(
    descriptor: &OwnedFd,
    kind: &'static str,
) -> Result<(), ApplicationDescriptorHandoffErrorV1> {
    let flags = unsafe { libc::fcntl(descriptor.as_raw_fd(), libc::F_GETFD) };
    if flags < 0 {
        return Err(descriptor_io(kind, io::Error::last_os_error()));
    }
    if unsafe {
        libc::fcntl(
            descriptor.as_raw_fd(),
            libc::F_SETFD,
            flags | libc::FD_CLOEXEC,
        )
    } < 0
    {
        return Err(descriptor_io(kind, io::Error::last_os_error()));
    }
    Ok(())
}

fn snapshot_handoff_descriptor_identities<D: AsFd, E: AsFd, A: AsFd>(
    directory: &D,
    envelope: &E,
    acknowledgment: &A,
    proof: Option<&OwnedFd>,
) -> Result<Vec<DescriptorIdentityV1>, ApplicationDescriptorHandoffErrorV1> {
    let directory = fstat(directory)
        .map(|stat| DescriptorIdentityV1::from_stat(&stat))
        .map_err(|error| descriptor_io("artifact directory identity", error));
    let envelope = fstat(envelope)
        .map(|stat| DescriptorIdentityV1::from_stat(&stat))
        .map_err(|error| descriptor_io("envelope identity", error));
    let acknowledgment = fstat(acknowledgment)
        .map(|stat| DescriptorIdentityV1::from_stat(&stat))
        .map_err(|error| descriptor_io("acknowledgment identity", error));
    let proof = proof
        .map(|fd| {
            fstat(fd)
                .map(|stat| DescriptorIdentityV1::from_stat(&stat))
                .map_err(|error| descriptor_io("proof endpoint identity", error))
        })
        .transpose();
    let available = [
        directory.as_ref().ok().copied(),
        envelope.as_ref().ok().copied(),
        acknowledgment.as_ref().ok().copied(),
        proof.as_ref().ok().copied().flatten(),
    ];
    seal_descriptor_occurrences(&available.into_iter().flatten().collect::<Vec<_>>())?;
    let mut identities = vec![directory?, envelope?, acknowledgment?];
    identities.extend(proof?);
    Ok(identities)
}

fn worker_v3_descriptor_occurrences<E: AsFd, D: AsFd, A: AsFd>(
    envelope: &E,
    directory: &D,
    acknowledgment: &A,
    proof: Option<&RetainedApplicationProofEndpointV1>,
) -> Result<Vec<WorkerV3ApplicationInputOccurrenceV1>, WorkerV3ApplicationDescriptorHandoffErrorV1>
{
    let occurrence = |slot, descriptor: &dyn AsFd, kind| {
        let stat = fstat(descriptor.as_fd()).map_err(|error| {
            WorkerV3ApplicationDescriptorHandoffErrorV1::Descriptor(descriptor_io(kind, error))
        })?;
        WorkerV3ApplicationInputOccurrenceV1::from_linux_descriptor_v1(
            slot,
            stat.st_dev,
            stat.st_ino,
            stat.st_mode,
        )
        .map_err(WorkerV3ApplicationDescriptorHandoffErrorV1::Protocol)
    };
    let mut inputs = vec![
        occurrence(1, envelope, "Worker V3 envelope occurrence")?,
        occurrence(2, directory, "Worker V3 artifact-directory occurrence")?,
        occurrence(3, acknowledgment, "Worker V3 acknowledgment occurrence")?,
    ];
    if let Some(proof) = proof {
        let (device, inode, mode) = proof.descriptor_identity();
        inputs.push(
            WorkerV3ApplicationInputOccurrenceV1::from_linux_descriptor_v1(4, device, inode, mode)
                .map_err(WorkerV3ApplicationDescriptorHandoffErrorV1::Protocol)?,
        );
    }
    Ok(inputs)
}

fn validate_worker_v3_application_occurrence(
    supplied: &WorkerV3ApplicationOccurrenceV1,
    application: WorkerV3ApplicationIdentityV1,
    inputs: &[WorkerV3ApplicationInputOccurrenceV1],
) -> Result<(), WorkerV3ApplicationDescriptorHandoffErrorV1> {
    if supplied.application() != application {
        return Err(WorkerV3ApplicationDescriptorHandoffErrorV1::ApplicationIdentityMismatch);
    }
    if supplied.inputs() != inputs {
        return Err(WorkerV3ApplicationDescriptorHandoffErrorV1::DescriptorOccurrenceMismatch);
    }
    Ok(())
}

fn validate_worker_v3_application_commitment(
    expected: WorkerV3ApplicationHandoffCommitmentV1,
    supplied: WorkerV3ApplicationHandoffCommitmentV1,
) -> Result<(), WorkerV3ApplicationDescriptorHandoffErrorV1> {
    if expected == supplied {
        Ok(())
    } else {
        Err(WorkerV3ApplicationDescriptorHandoffErrorV1::CommitmentMismatch)
    }
}

fn worker_v3_descriptor_error(
    error: ApplicationDescriptorHandoffErrorV1,
) -> WorkerV3ApplicationDescriptorHandoffErrorV1 {
    match error {
        ApplicationDescriptorHandoffErrorV1::UnsafeEnvelope => {
            WorkerV3ApplicationDescriptorHandoffErrorV1::UnsafeEnvelope
        }
        ApplicationDescriptorHandoffErrorV1::EnvelopeNotLinked => {
            WorkerV3ApplicationDescriptorHandoffErrorV1::EnvelopeNotLinked
        }
        ApplicationDescriptorHandoffErrorV1::EnvelopeChanged => {
            WorkerV3ApplicationDescriptorHandoffErrorV1::EnvelopeChanged
        }
        ApplicationDescriptorHandoffErrorV1::MixedEnvelopeSchema => {
            WorkerV3ApplicationDescriptorHandoffErrorV1::MixedEnvelopeSchema
        }
        error => WorkerV3ApplicationDescriptorHandoffErrorV1::Descriptor(error),
    }
}

fn validate_worker_v3_recovered_occurrence(
    receipt: WorkerV3LoadReadinessReceiptV1,
    directory: DirectorySnapshotV1,
    envelope: EnvelopeSnapshotV1,
) -> Result<(), WorkerV3ApplicationDescriptorHandoffErrorV1> {
    if receipt.output_directory_device() != directory.device
        || receipt.output_directory_inode() != directory.inode
        || receipt.envelope_file_device() != envelope.device
        || receipt.envelope_file_inode() != envelope.inode
        || receipt.envelope_file_mtime()
            != (envelope.modified_seconds, envelope.modified_nanoseconds)
        || receipt.envelope_file_ctime() != (envelope.changed_seconds, envelope.changed_nanoseconds)
    {
        return Err(
            WorkerV3ApplicationDescriptorHandoffErrorV1::RecoveredEnvelopeOccurrenceMismatch,
        );
    }
    Ok(())
}

fn seal_descriptor_occurrences(
    expected: &[DescriptorIdentityV1],
) -> Result<(), ApplicationDescriptorHandoffErrorV1> {
    for entry in std::fs::read_dir("/proc/self/fd")
        .map_err(|error| descriptor_io("process descriptor table", error))?
    {
        let entry = entry.map_err(|error| descriptor_io("process descriptor entry", error))?;
        let Some(descriptor) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<RawFd>().ok())
        else {
            continue;
        };
        let flags = unsafe { libc::fcntl(descriptor, libc::F_GETFD) };
        if flags < 0 {
            continue;
        }
        let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
        // SAFETY: `stat` points to writable storage and `F_GETFD` established that this descriptor
        // was live immediately before the non-mutating query. This startup protocol has no racing
        // descriptor owner.
        if unsafe { libc::fstat(descriptor, stat.as_mut_ptr()) } != 0 {
            continue;
        }
        // SAFETY: successful `fstat` initialized the complete record.
        let stat = unsafe { stat.assume_init() };
        if !expected.contains(&DescriptorIdentityV1 {
            device: stat.st_dev,
            inode: stat.st_ino,
        }) {
            continue;
        }
        if unsafe { libc::fcntl(descriptor, libc::F_SETFD, flags | libc::FD_CLOEXEC) } < 0 {
            return Err(descriptor_io(
                "retained handoff descriptor",
                io::Error::last_os_error(),
            ));
        }
    }
    Ok(())
}

fn inspect_directory(
    directory: &File,
) -> Result<DirectorySnapshotV1, ApplicationDescriptorHandoffErrorV1> {
    let flags =
        fcntl_getfl(directory).map_err(|error| descriptor_io("artifact directory", error))?;
    let stat = fstat(directory).map_err(|error| descriptor_io("artifact directory", error))?;
    if flags & OFlags::ACCMODE != OFlags::RDONLY
        || FileType::from_raw_mode(stat.st_mode) != FileType::Directory
        || stat.st_uid != rustix::process::geteuid().as_raw()
        || stat.st_mode & 0o077 != 0
    {
        return Err(ApplicationDescriptorHandoffErrorV1::UnsafeDirectory);
    }
    Ok(DirectorySnapshotV1::from_stat(&stat))
}

fn validate_directory(
    directory: &File,
    expected: DirectorySnapshotV1,
) -> Result<(), ApplicationDescriptorHandoffErrorV1> {
    if inspect_directory(directory)? != expected {
        return Err(ApplicationDescriptorHandoffErrorV1::DirectoryChanged);
    }
    Ok(())
}

fn inspect_worker_v3_envelope(
    directory: &File,
    envelope: &File,
) -> Result<InspectedWorkerV3EnvelopeV1, WorkerV3ApplicationDescriptorHandoffErrorV1> {
    let flags = fcntl_getfl(envelope).map_err(|error| {
        WorkerV3ApplicationDescriptorHandoffErrorV1::Descriptor(descriptor_io(
            "Worker V3 envelope",
            error,
        ))
    })?;
    let initial = fstat(envelope).map_err(|error| {
        WorkerV3ApplicationDescriptorHandoffErrorV1::Descriptor(descriptor_io(
            "Worker V3 envelope",
            error,
        ))
    })?;
    let snapshot = EnvelopeSnapshotV1::from_stat(&initial);
    if flags & OFlags::ACCMODE != OFlags::RDONLY
        || FileType::from_raw_mode(initial.st_mode) != FileType::RegularFile
        || initial.st_uid != rustix::process::geteuid().as_raw()
        || initial.st_nlink != 1
        || initial.st_mode & 0o077 != 0
    {
        return Err(WorkerV3ApplicationDescriptorHandoffErrorV1::UnsafeEnvelope);
    }
    let size = usize::try_from(initial.st_size)
        .ok()
        .filter(|size| (1..=MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V2).contains(size))
        .ok_or(WorkerV3ApplicationDescriptorHandoffErrorV1::EnvelopeSize {
            actual: initial.st_size,
        })?;
    let bytes = read_exact_at(envelope, size).map_err(worker_v3_descriptor_error)?;
    let final_stat = fstat(envelope).map_err(|error| {
        WorkerV3ApplicationDescriptorHandoffErrorV1::Descriptor(descriptor_io(
            "Worker V3 envelope",
            error,
        ))
    })?;
    if EnvelopeSnapshotV1::from_stat(&final_stat) != snapshot {
        return Err(WorkerV3ApplicationDescriptorHandoffErrorV1::EnvelopeChanged);
    }
    let decoded = WorkerV3LoadEnvelopeWireV2::decode_canonical(&bytes)
        .map_err(WorkerV3ApplicationDescriptorHandoffErrorV1::Envelope)?;
    if decoded
        .encode_canonical()
        .map_err(WorkerV3ApplicationDescriptorHandoffErrorV1::Envelope)?
        != bytes
    {
        return Err(WorkerV3ApplicationDescriptorHandoffErrorV1::NonCanonicalEnvelope);
    }
    let canonical_name =
        require_worker_v3_envelope_link(directory, snapshot).map_err(worker_v3_descriptor_error)?;
    Ok(InspectedWorkerV3EnvelopeV1 {
        snapshot,
        exact_bytes: bytes.into_boxed_slice(),
        decoded,
        canonical_name,
    })
}

fn validate_envelope(
    directory: &File,
    envelope: &File,
    expected: EnvelopeSnapshotV1,
    expected_bytes: &[u8],
    expected_name: &str,
) -> Result<(), ApplicationDescriptorHandoffErrorV1> {
    let stat = fstat(envelope).map_err(|error| descriptor_io("envelope", error))?;
    if EnvelopeSnapshotV1::from_stat(&stat) != expected {
        return Err(ApplicationDescriptorHandoffErrorV1::EnvelopeChanged);
    }
    require_canonical_envelope_link(directory, expected, expected_name)?;
    if read_exact_at(envelope, expected_bytes.len())? != expected_bytes {
        return Err(ApplicationDescriptorHandoffErrorV1::EnvelopeChanged);
    }
    Ok(())
}

fn require_canonical_envelope_link(
    directory: &File,
    expected: EnvelopeSnapshotV1,
    expected_name: &str,
) -> Result<(), ApplicationDescriptorHandoffErrorV1> {
    let mut entries = 0_usize;
    let mut links = 0_usize;
    let mut canonical_entries = 0_usize;
    let mut canonical_links = 0_usize;
    for entry in std::fs::read_dir(descriptor_directory_path(directory))
        .map_err(|error| descriptor_io("artifact directory", error))?
    {
        count_handoff_artifact_entry(&mut entries)?;
        let entry = entry.map_err(|error| descriptor_io("artifact directory entry", error))?;
        let metadata = entry
            .path()
            .symlink_metadata()
            .map_err(|error| descriptor_io("artifact directory entry", error))?;
        use std::os::unix::fs::MetadataExt;
        let has_canonical_name = entry.file_name() == OsStr::new(expected_name);
        if has_canonical_name {
            canonical_entries += 1;
        }
        if metadata.dev() == expected.device && metadata.ino() == expected.inode {
            links += 1;
            if has_canonical_name && metadata.file_type().is_file() && metadata.nlink() == 1 {
                canonical_links += 1;
            }
        }
    }
    if links != 1 || canonical_entries != 1 || canonical_links != 1 {
        return Err(ApplicationDescriptorHandoffErrorV1::EnvelopeNotLinked);
    }
    Ok(())
}

fn require_worker_v3_envelope_link(
    directory: &File,
    expected: EnvelopeSnapshotV1,
) -> Result<String, ApplicationDescriptorHandoffErrorV1> {
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::MetadataExt;

    let mut entries = 0_usize;
    let mut links = 0_usize;
    let mut canonical_link = None;
    for entry in std::fs::read_dir(descriptor_directory_path(directory))
        .map_err(|error| descriptor_io("artifact directory", error))?
    {
        count_handoff_artifact_entry(&mut entries)?;
        let entry = entry.map_err(|error| descriptor_io("artifact directory entry", error))?;
        let name = entry.file_name();
        let name_bytes = name.as_bytes();
        if name_bytes.starts_with(RETIRED_WORKER_V2_ENVELOPE_PREFIX_V1.as_bytes()) {
            return Err(ApplicationDescriptorHandoffErrorV1::MixedEnvelopeSchema);
        }
        let metadata = entry
            .path()
            .symlink_metadata()
            .map_err(|error| descriptor_io("artifact directory entry", error))?;
        if metadata.dev() != expected.device || metadata.ino() != expected.inode {
            continue;
        }
        links += 1;
        if is_canonical_worker_v3_envelope_name(name_bytes)
            && metadata.file_type().is_file()
            && metadata.nlink() == 1
        {
            if canonical_link.is_some() {
                return Err(ApplicationDescriptorHandoffErrorV1::EnvelopeNotLinked);
            }
            canonical_link = Some(
                name.into_string()
                    .expect("canonical Worker V3 envelope names are ASCII"),
            );
        }
    }
    if links != 1 {
        return Err(ApplicationDescriptorHandoffErrorV1::EnvelopeNotLinked);
    }
    canonical_link.ok_or(ApplicationDescriptorHandoffErrorV1::EnvelopeNotLinked)
}

fn is_canonical_worker_v3_envelope_name(name: &[u8]) -> bool {
    name.len() == WORKER_V3_ENVELOPE_NAME_BYTES_V1
        && name.starts_with(WORKER_V3_ENVELOPE_PREFIX_V1.as_bytes())
        && name.ends_with(WORKER_V3_ENVELOPE_SUFFIX_V1.as_bytes())
        && name[WORKER_V3_ENVELOPE_PREFIX_V1.len()..WORKER_V3_ENVELOPE_PREFIX_V1.len() + 64]
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn reject_worker_v2_envelope_coexistence(
    directory: &File,
) -> Result<(), ApplicationDescriptorHandoffErrorV1> {
    use std::os::unix::ffi::OsStrExt;

    let mut entries = 0_usize;
    for entry in std::fs::read_dir(descriptor_directory_path(directory))
        .map_err(|error| descriptor_io("artifact directory", error))?
    {
        count_handoff_artifact_entry(&mut entries)?;
        let entry = entry.map_err(|error| descriptor_io("artifact directory entry", error))?;
        if entry
            .file_name()
            .as_bytes()
            .starts_with(RETIRED_WORKER_V2_ENVELOPE_PREFIX_V1.as_bytes())
        {
            return Err(ApplicationDescriptorHandoffErrorV1::MixedEnvelopeSchema);
        }
    }
    Ok(())
}

fn count_handoff_artifact_entry(
    entries: &mut usize,
) -> Result<(), ApplicationDescriptorHandoffErrorV1> {
    *entries = entries
        .checked_add(1)
        .filter(|entries| *entries <= MAX_APPLICATION_ARTIFACT_DIRECTORY_ENTRIES_V1)
        .ok_or(ApplicationDescriptorHandoffErrorV1::DirectoryTooLarge)?;
    Ok(())
}

fn read_exact_at(file: &File, size: usize) -> Result<Vec<u8>, ApplicationDescriptorHandoffErrorV1> {
    let mut bytes = vec![0_u8; size];
    let mut offset = 0_usize;
    while offset < bytes.len() {
        let read = file
            .read_at(&mut bytes[offset..], offset as u64)
            .map_err(|error| descriptor_io("envelope", error))?;
        if read == 0 {
            return Err(ApplicationDescriptorHandoffErrorV1::EnvelopeChanged);
        }
        offset += read;
    }
    let mut trailing = [0_u8; 1];
    if file
        .read_at(&mut trailing, size as u64)
        .map_err(|error| descriptor_io("envelope", error))?
        != 0
    {
        return Err(ApplicationDescriptorHandoffErrorV1::EnvelopeChanged);
    }
    Ok(bytes)
}

fn current_application_identity_v3()
-> Result<WorkerV3ApplicationIdentityV1, WorkerV3ApplicationDescriptorHandoffErrorV1> {
    let exact = current_application_exact_bytes()
        .map_err(WorkerV3ApplicationDescriptorHandoffErrorV1::Descriptor)?;
    match WorkerV3ApplicationIdentityV1::from_sealed_static_elf_v1(&exact) {
        Ok(identity) => Ok(identity),
        #[cfg(test)]
        Err(_) => {
            WorkerV3ApplicationIdentityV1::from_sealed_static_elf_v1(&sealed_static_test_elf_v1())
                .map_err(WorkerV3ApplicationDescriptorHandoffErrorV1::Protocol)
        }
        #[cfg(not(test))]
        Err(error) => Err(WorkerV3ApplicationDescriptorHandoffErrorV1::Protocol(error)),
    }
}

fn current_application_exact_bytes() -> Result<Vec<u8>, ApplicationDescriptorHandoffErrorV1> {
    let mut executable = File::open("/proc/self/exe")
        .map_err(ApplicationDescriptorHandoffErrorV1::ApplicationExecutable)?;
    let initial = fstat(&executable).map_err(|error| {
        ApplicationDescriptorHandoffErrorV1::ApplicationExecutable(error.into())
    })?;
    if FileType::from_raw_mode(initial.st_mode) != FileType::RegularFile
        || initial.st_size <= 0
        || u64::try_from(initial.st_size)
            .ok()
            .is_none_or(|size| size == 0 || size > MAX_APPLICATION_EXECUTABLE_BYTES_V1)
    {
        return Err(ApplicationDescriptorHandoffErrorV1::UnsafeApplicationExecutable);
    }
    let expected = EnvelopeSnapshotV1::from_stat(&initial);
    let size = u64::try_from(initial.st_size).expect("validated application size");
    let mut remaining = size;
    let mut exact = Vec::new();
    exact
        .try_reserve_exact(usize::try_from(size).expect("bounded application size"))
        .map_err(|_| ApplicationDescriptorHandoffErrorV1::UnsafeApplicationExecutable)?;
    let mut buffer = [0_u8; 64 * 1024];
    while remaining != 0 {
        let limit = usize::try_from(remaining.min(buffer.len() as u64)).expect("bounded buffer");
        let read = executable
            .read(&mut buffer[..limit])
            .map_err(ApplicationDescriptorHandoffErrorV1::ApplicationExecutable)?;
        if read == 0 {
            return Err(ApplicationDescriptorHandoffErrorV1::ApplicationExecutableChanged);
        }
        exact.extend_from_slice(&buffer[..read]);
        remaining -= read as u64;
    }
    if executable
        .read(&mut buffer[..1])
        .map_err(ApplicationDescriptorHandoffErrorV1::ApplicationExecutable)?
        != 0
        || EnvelopeSnapshotV1::from_stat(&fstat(&executable).map_err(|error| {
            ApplicationDescriptorHandoffErrorV1::ApplicationExecutable(error.into())
        })?) != expected
    {
        return Err(ApplicationDescriptorHandoffErrorV1::ApplicationExecutableChanged);
    }
    Ok(exact)
}

#[cfg(test)]
pub(crate) fn sealed_static_test_elf_v1() -> Vec<u8> {
    const HEADER: usize = 64;
    const PROGRAM: usize = 56;
    const PROGRAMS: usize = 4;
    const CODE_OFFSET: usize = 0x1000;
    let mut bytes = vec![0_u8; CODE_OFFSET + 1];
    bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    bytes[16..18].copy_from_slice(&2_u16.to_le_bytes());
    bytes[18..20].copy_from_slice(&62_u16.to_le_bytes());
    bytes[20..24].copy_from_slice(&1_u32.to_le_bytes());
    bytes[24..32].copy_from_slice(&0x401000_u64.to_le_bytes());
    bytes[32..40].copy_from_slice(&(HEADER as u64).to_le_bytes());
    bytes[52..54].copy_from_slice(&(HEADER as u16).to_le_bytes());
    bytes[54..56].copy_from_slice(&(PROGRAM as u16).to_le_bytes());
    bytes[56..58].copy_from_slice(&(PROGRAMS as u16).to_le_bytes());

    let mut program = |index: usize,
                       kind: u32,
                       flags: u32,
                       offset: u64,
                       address: u64,
                       file_size: u64,
                       memory_size: u64,
                       alignment: u64| {
        let start = HEADER + index * PROGRAM;
        bytes[start..start + 4].copy_from_slice(&kind.to_le_bytes());
        bytes[start + 4..start + 8].copy_from_slice(&flags.to_le_bytes());
        bytes[start + 8..start + 16].copy_from_slice(&offset.to_le_bytes());
        bytes[start + 16..start + 24].copy_from_slice(&address.to_le_bytes());
        bytes[start + 32..start + 40].copy_from_slice(&file_size.to_le_bytes());
        bytes[start + 40..start + 48].copy_from_slice(&memory_size.to_le_bytes());
        bytes[start + 48..start + 56].copy_from_slice(&alignment.to_le_bytes());
    };
    let table_size = (PROGRAM * PROGRAMS) as u64;
    program(0, 6, 4, HEADER as u64, 0x400040, table_size, table_size, 8);
    program(
        1,
        1,
        4,
        0,
        0x400000,
        (HEADER as u64) + table_size,
        (HEADER as u64) + table_size,
        0x1000,
    );
    program(2, 1, 5, CODE_OFFSET as u64, 0x401000, 1, 1, 0x1000);
    program(3, 0x6474_e551, 6, 0, 0, 0, 0, 16);
    bytes[CODE_OFFSET] = 0xc3;
    bytes
}

#[cfg(test)]
mod static_identity_tests {
    use super::*;

    #[test]
    fn host_test_identity_uses_the_shared_static_application_domain() {
        let identity =
            WorkerV3ApplicationIdentityV1::from_sealed_static_elf_v1(&sealed_static_test_elf_v1())
                .unwrap();
        assert_eq!(
            identity.sha256(),
            [
                0xf5, 0xae, 0x74, 0xbd, 0xd1, 0x06, 0x49, 0x3c, 0x90, 0x14, 0x56, 0x97, 0x2a, 0x8d,
                0x15, 0x9d, 0x42, 0x7d, 0x11, 0xf9, 0xe8, 0x20, 0xa2, 0x62, 0xce, 0x3a, 0x1a, 0x0d,
                0xe5, 0xed, 0x15, 0xec,
            ]
        );
        assert_eq!(identity.byte_len(), 4_097);
    }

    #[test]
    fn child_handoff_scan_accepts_exact_entry_bound_and_rejects_limit_plus_one() {
        let mut entries = 0_usize;
        for _ in 0..MAX_APPLICATION_ARTIFACT_DIRECTORY_ENTRIES_V1 {
            count_handoff_artifact_entry(&mut entries).unwrap();
        }
        assert_eq!(entries, MAX_APPLICATION_ARTIFACT_DIRECTORY_ENTRIES_V1);
        assert!(matches!(
            count_handoff_artifact_entry(&mut entries),
            Err(ApplicationDescriptorHandoffErrorV1::DirectoryTooLarge)
        ));
    }
}

fn inspect_acknowledgment(
    acknowledgment: &File,
) -> Result<(), ApplicationDescriptorHandoffErrorV1> {
    let stat = fstat(acknowledgment).map_err(|error| descriptor_io("acknowledgment", error))?;
    let kind = stat.st_mode & libc::S_IFMT;
    if kind != libc::S_IFIFO && kind != libc::S_IFSOCK {
        return Err(ApplicationDescriptorHandoffErrorV1::UnsafeAcknowledgment);
    }
    let flags =
        fcntl_getfl(acknowledgment).map_err(|error| descriptor_io("acknowledgment", error))?;
    if flags & OFlags::ACCMODE != OFlags::WRONLY {
        return Err(ApplicationDescriptorHandoffErrorV1::UnsafeAcknowledgment);
    }
    Ok(())
}

fn emit_acknowledgment_bytes(
    acknowledgment: &File,
    bytes: &[u8],
    deadline: Instant,
    revalidate: impl FnOnce() -> Result<(), WorkerV3ApplicationDescriptorHandoffErrorV1>,
) -> Result<(), WorkerV3ApplicationDescriptorHandoffErrorV1> {
    let descriptor = WorkerV3ApplicationDescriptorHandoffErrorV1::Descriptor;
    inspect_acknowledgment(acknowledgment).map_err(descriptor)?;
    let flags = fcntl_getfl(acknowledgment)
        .map_err(|error| descriptor(descriptor_io("acknowledgment", error)))?;
    fcntl_setfl(acknowledgment, flags | OFlags::NONBLOCK)
        .map_err(|error| descriptor(descriptor_io("acknowledgment", error)))?;
    wait_writable(acknowledgment.as_raw_fd(), deadline).map_err(descriptor)?;
    revalidate()?;
    if Instant::now() >= deadline {
        return Err(descriptor(
            ApplicationDescriptorHandoffErrorV1::AcknowledgmentTimeout,
        ));
    }
    let written = unsafe {
        libc::write(
            acknowledgment.as_raw_fd(),
            bytes.as_ptr().cast(),
            bytes.len(),
        )
    };
    if written < 0 {
        return Err(descriptor(descriptor_io(
            "acknowledgment",
            io::Error::last_os_error(),
        )));
    }
    if usize::try_from(written).ok() != Some(bytes.len()) {
        return Err(descriptor(
            ApplicationDescriptorHandoffErrorV1::PartialAcknowledgment,
        ));
    }
    Ok(())
}

fn wait_writable(
    descriptor: RawFd,
    deadline: Instant,
) -> Result<(), ApplicationDescriptorHandoffErrorV1> {
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(ApplicationDescriptorHandoffErrorV1::AcknowledgmentTimeout);
        }
        let millis = remaining.as_millis().clamp(1, i32::MAX as u128) as i32;
        let mut poll = libc::pollfd {
            fd: descriptor,
            events: libc::POLLOUT,
            revents: 0,
        };
        let result = unsafe { libc::poll(&mut poll, 1, millis) };
        if result == 0 {
            return Err(ApplicationDescriptorHandoffErrorV1::AcknowledgmentTimeout);
        }
        if result < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(descriptor_io("acknowledgment", error));
        }
        if Instant::now() >= deadline {
            return Err(ApplicationDescriptorHandoffErrorV1::AcknowledgmentTimeout);
        }
        if poll.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
            return Err(ApplicationDescriptorHandoffErrorV1::AcknowledgmentClosed);
        }
        if poll.revents & libc::POLLOUT != 0 {
            return Ok(());
        }
        return Err(ApplicationDescriptorHandoffErrorV1::AcknowledgmentClosed);
    }
}

fn descriptor_directory_path(directory: &File) -> PathBuf {
    PathBuf::from(format!("/proc/self/fd/{}", directory.as_raw_fd()))
}

fn descriptor_io(
    kind: &'static str,
    error: impl Into<io::Error>,
) -> ApplicationDescriptorHandoffErrorV1 {
    ApplicationDescriptorHandoffErrorV1::DescriptorIo {
        kind,
        error: error.into(),
    }
}

/// Shared descriptor, executable, and acknowledgment failure at application handoff.
#[derive(Debug)]
#[non_exhaustive]
pub enum ApplicationDescriptorHandoffErrorV1 {
    AlreadyConsumed,
    MixedSchema,
    MissingEnvironment(&'static str),
    InvalidEnvironment(&'static str),
    AliasedDescriptors,
    DescriptorFlags(&'static str),
    DescriptorIo {
        kind: &'static str,
        error: io::Error,
    },
    UnsafeDirectory,
    DirectoryChanged,
    DirectoryTooLarge,
    UnsafeEnvelope,
    EnvelopeSize {
        actual: i64,
    },
    EnvelopeNotLinked,
    MixedEnvelopeSchema,
    EnvelopeChanged,
    NonCanonicalEnvelope,
    UnsafeApplicationExecutable,
    ApplicationExecutable(io::Error),
    ApplicationExecutableChanged,
    InvalidStaticApplication(fe2o3_runtime_protocol::SealedStaticApplicationErrorV1),
    UnsafeAcknowledgment,
    AcknowledgmentTimeout,
    AcknowledgmentClosed,
    PartialAcknowledgment,
}

impl fmt::Display for ApplicationDescriptorHandoffErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyConsumed => {
                formatter.write_str("application descriptor handoff was already consumed")
            }
            Self::MixedSchema => formatter
                .write_str("Worker V2 and Worker V3 application handoff environments coexist"),
            Self::MissingEnvironment(name) => {
                write!(formatter, "missing application handoff environment {name}")
            }
            Self::InvalidEnvironment(name) => {
                write!(formatter, "invalid application handoff environment {name}")
            }
            Self::AliasedDescriptors => {
                formatter.write_str("application handoff descriptors alias")
            }
            Self::DescriptorFlags(kind) => {
                write!(formatter, "inherited {kind} descriptor has invalid flags")
            }
            Self::DescriptorIo { kind, error } => {
                write!(formatter, "failed to access {kind} descriptor: {error}")
            }
            Self::UnsafeDirectory => {
                formatter.write_str("inherited artifact-directory descriptor is unsafe")
            }
            Self::DirectoryChanged => formatter.write_str("inherited artifact directory changed"),
            Self::DirectoryTooLarge => {
                formatter.write_str("inherited artifact directory exceeds its scan bound")
            }
            Self::UnsafeEnvelope => formatter.write_str("inherited envelope descriptor is unsafe"),
            Self::EnvelopeSize { actual } => {
                write!(formatter, "inherited envelope size {actual} is invalid")
            }
            Self::EnvelopeNotLinked => formatter.write_str(
                "inherited envelope is not linked exactly once in the artifact directory",
            ),
            Self::MixedEnvelopeSchema => {
                formatter.write_str("Worker V2 and Worker V3 application envelopes coexist")
            }
            Self::EnvelopeChanged => formatter.write_str("inherited envelope changed"),
            Self::NonCanonicalEnvelope => {
                formatter.write_str("inherited envelope is not canonical")
            }
            Self::UnsafeApplicationExecutable => formatter
                .write_str("current application executable is unsafe or outside its size bound"),
            Self::ApplicationExecutable(error) => write!(
                formatter,
                "failed to measure current application executable: {error}"
            ),
            Self::ApplicationExecutableChanged => {
                formatter.write_str("current application executable changed while measured")
            }
            Self::InvalidStaticApplication(error) => {
                write!(
                    formatter,
                    "current application image is not an admitted static ELF: {error}"
                )
            }
            Self::UnsafeAcknowledgment => {
                formatter.write_str("inherited application acknowledgment descriptor is unsafe")
            }
            Self::AcknowledgmentTimeout => {
                formatter.write_str("application handoff acknowledgment timed out")
            }
            Self::AcknowledgmentClosed => {
                formatter.write_str("application handoff acknowledgment channel closed")
            }
            Self::PartialAcknowledgment => {
                formatter.write_str("application handoff acknowledgment was not written atomically")
            }
        }
    }
}

impl Error for ApplicationDescriptorHandoffErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::DescriptorIo { error, .. } | Self::ApplicationExecutable(error) => Some(error),
            Self::InvalidStaticApplication(error) => Some(error),
            _ => None,
        }
    }
}

/// Failure while consuming Cargo's strict Worker V3 application descriptor handoff.
#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV3ApplicationDescriptorHandoffErrorV1 {
    AlreadyConsumed,
    MixedSchema,
    MissingEnvironment(&'static str),
    InvalidEnvironment(&'static str),
    AliasedDescriptors,
    Descriptor(ApplicationDescriptorHandoffErrorV1),
    ProofChannel(ApplicationProofChannelErrorV1),
    Registration(WorkerV3ApplicationRegistrationErrorV1),
    Protocol(WorkerV3ApplicationHandoffProtocolErrorV1),
    Envelope(WorkerV3LoadEnvelopeErrorV2),
    EnvelopeSize { actual: i64 },
    UnsafeEnvelope,
    EnvelopeNotLinked,
    EnvelopeChanged,
    MixedEnvelopeSchema,
    NonCanonicalEnvelope,
    ApplicationIdentityMismatch,
    DescriptorOccurrenceMismatch,
    CommitmentMismatch,
    RecoveredEnvelopeOccurrenceMismatch,
    RecoveredEnvelopeMismatch,
    Admission(RecoveredWorkerV3AdmissionErrorV1),
}

impl fmt::Display for WorkerV3ApplicationDescriptorHandoffErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyConsumed => {
                formatter.write_str("Worker V3 application descriptor handoff was already consumed")
            }
            Self::MixedSchema => formatter
                .write_str("Worker V2 and Worker V3 application handoff environments coexist"),
            Self::MissingEnvironment(name) => {
                write!(
                    formatter,
                    "missing Worker V3 application handoff environment {name}"
                )
            }
            Self::InvalidEnvironment(name) => {
                write!(
                    formatter,
                    "invalid Worker V3 application handoff environment {name}"
                )
            }
            Self::AliasedDescriptors => {
                formatter.write_str("Worker V3 application handoff descriptors alias")
            }
            Self::Descriptor(error) => {
                write!(
                    formatter,
                    "invalid Worker V3 application descriptor: {error}"
                )
            }
            Self::ProofChannel(error) => {
                write!(formatter, "invalid Worker V3 proof endpoint: {error}")
            }
            Self::Registration(error) => {
                write!(formatter, "invalid Worker V3 registration: {error}")
            }
            Self::Protocol(error) => {
                write!(
                    formatter,
                    "invalid Worker V3 application protocol value: {error}"
                )
            }
            Self::Envelope(error) => {
                write!(formatter, "invalid inherited Worker V3 envelope: {error}")
            }
            Self::EnvelopeSize { actual } => write!(
                formatter,
                "inherited Worker V3 envelope size {actual} is invalid"
            ),
            Self::UnsafeEnvelope => {
                formatter.write_str("inherited Worker V3 envelope descriptor is unsafe")
            }
            Self::EnvelopeNotLinked => formatter.write_str(
                "inherited Worker V3 envelope is not linked under its canonical durable name",
            ),
            Self::EnvelopeChanged => formatter.write_str("inherited Worker V3 envelope changed"),
            Self::MixedEnvelopeSchema => {
                formatter.write_str("Worker V2 and Worker V3 application envelopes coexist")
            }
            Self::NonCanonicalEnvelope => {
                formatter.write_str("inherited Worker V3 envelope is not canonical")
            }
            Self::ApplicationIdentityMismatch => formatter
                .write_str("Worker V3 application occurrence names a different executable image"),
            Self::DescriptorOccurrenceMismatch => formatter.write_str(
                "Worker V3 application occurrence differs from inherited descriptor objects",
            ),
            Self::CommitmentMismatch => formatter.write_str(
                "Worker V3 application commitment does not bind the envelope and occurrence",
            ),
            Self::RecoveredEnvelopeOccurrenceMismatch => formatter.write_str(
                "durable Worker V3 custody names a different directory or envelope occurrence",
            ),
            Self::RecoveredEnvelopeMismatch => formatter.write_str(
                "durably recovered Worker V3 envelope differs from the inherited envelope",
            ),
            Self::Admission(error) => {
                write!(
                    formatter,
                    "failed to admit inherited Worker V3 publication: {error}"
                )
            }
        }
    }
}

impl Error for WorkerV3ApplicationDescriptorHandoffErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Descriptor(error) => Some(error),
            Self::ProofChannel(error) => Some(error),
            Self::Registration(error) => Some(error),
            Self::Protocol(error) => Some(error),
            Self::Envelope(error) => Some(error),
            Self::Admission(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
