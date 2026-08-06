//! Typed publication bridge for independently inspected raw Worker V2 HSACO.
//!
//! This bridge derives a complete durable publication plan from retained inspection evidence. It
//! publishes the exact inspected raw HSACO bytes; canonical `.fe2o3.kd.v1` finalization does not
//! run here. Neither preparation nor publication authenticates compiler origin, grants loading or
//! launch authority, or proves Verus verification. The prepared object supports exact in-process
//! retries only. Persisting enough evidence for process-restart recovery is a separate protocol.

use std::{error::Error, fmt, path::Path};

use fe2o3_artifact_transaction::{
    AtomicPublicationIdentityV1, AttemptScopedHsacoPublicationErrorV1,
    AttemptScopedHsacoPublicationResultV1, BuildAttempt, CanonicalLinkRequestIdentityV1,
    DurableLinkPublicationPlanV1, FinalizationIdentityV1, FinalizedOutputIdentityV1,
    KernelSetIdentityV1, LinkPublicationScopeV1, LinkedOutputIdentityV1, PackageIdentityV1,
    PinnedWorkerIdentityV1, ProducerIdentity, TargetIdentityV1,
    UpstreamCodeObjectEvidenceIdentityV1, ValidatedResponseIdentityV1,
    producer_package_identity_v1, publish_exact_hsaco_evidence_for_attempt_v1,
};
use fe2o3_kernel_descriptor::CodeObjectVersion;
use sha2::{Digest, Sha256};

use crate::InspectedRawWorkerV2HsacoV1;

const KERNEL_SET_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/WORKER-V2-PUBLICATION-KERNEL-SET/V1\0";
const TARGET_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/WORKER-V2-PUBLICATION-TARGET/V1\0";
const REQUEST_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/WORKER-V2-PUBLICATION-REQUEST/V1\0";
const WORKER_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/WORKER-V2-PUBLICATION-WORKER/V1\0";
const RESPONSE_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/WORKER-V2-PUBLICATION-RESPONSE/V1\0";
const RAW_INSPECTION_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/WORKER-V2-PUBLICATION-RAW-INSPECTION/V1\0";
const ATOMIC_PUBLICATION_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/WORKER-V2-ATOMIC-PUBLICATION/V1\0";

/// A complete, internally derived publication intent for exact inspected Worker V2 bytes.
///
/// The durable plan and upstream evidence identity remain private so callers cannot replace any
/// retained manifest, target, request, worker, response, output, or inspection identity. This
/// object is inert without the matching producer and live build-attempt registry authority.
#[derive(Debug)]
pub struct PreparedWorkerV2HsacoPublicationV1 {
    inspected: InspectedRawWorkerV2HsacoV1,
    producer_package: PackageIdentityV1,
    plan: DurableLinkPublicationPlanV1,
    upstream: UpstreamCodeObjectEvidenceIdentityV1,
}

impl PreparedWorkerV2HsacoPublicationV1 {
    /// Returns the exact managed attempt retained by the inspected evidence.
    pub const fn attempt(&self) -> BuildAttempt {
        self.inspected.attempt()
    }

    /// Returns the exact raw HSACO bytes retained for publication and exact retry.
    pub fn exact_bytes(&self) -> &[u8] {
        self.inspected.exact_bytes()
    }

    /// Preparation does not authenticate compiler origin.
    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    /// The prepared value alone is not publication authority.
    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    /// Publication evidence is not HSA loading authority.
    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    /// Publication evidence is not kernel-launch authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Failure while deriving or using a typed raw-HSACO publication intent.
#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV2HsacoPublicationError {
    /// Retained bytes no longer match the output identity admitted upstream.
    OutputIdentityMismatch,
    /// Publication supplied a different producer from the one bound during preparation.
    ProducerIdentityMismatch,
    /// The attempt-scoped durable publication protocol rejected the operation.
    Publication(AttemptScopedHsacoPublicationErrorV1),
}

impl fmt::Display for WorkerV2HsacoPublicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutputIdentityMismatch => formatter.write_str(
                "retained raw HSACO bytes do not match the admitted linked-output identity",
            ),
            Self::ProducerIdentityMismatch => formatter.write_str(
                "publication producer does not match the producer bound during preparation",
            ),
            Self::Publication(error) => write!(formatter, "raw HSACO publication failed: {error}"),
        }
    }
}

impl Error for WorkerV2HsacoPublicationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Publication(error) => Some(error),
            _ => None,
        }
    }
}

impl From<AttemptScopedHsacoPublicationErrorV1> for WorkerV2HsacoPublicationError {
    fn from(value: AttemptScopedHsacoPublicationErrorV1) -> Self {
        Self::Publication(value)
    }
}

/// Consumes independently inspected Worker V2 evidence and derives its complete publication plan.
///
/// The producer contributes only a non-authoritative cooperating-writer package namespace. Every
/// artifact, symbol, target, request, worker, response, output, finalization, and upstream evidence
/// identity is derived from the retained inspection and its sealed source evidence.
pub fn prepare_worker_v2_hsaco_publication_v1(
    producer: &ProducerIdentity,
    inspected: InspectedRawWorkerV2HsacoV1,
) -> Result<PreparedWorkerV2HsacoPublicationV1, WorkerV2HsacoPublicationError> {
    let exact_bytes = inspected.exact_bytes();
    let linked_source = inspected.linked_output_identity();
    if !linked_source.matches(exact_bytes) {
        return Err(WorkerV2HsacoPublicationError::OutputIdentityMismatch);
    }

    let producer_package = producer_package_identity_v1(producer);
    let manifest = inspected.policy().symbol_manifest().identity();
    let compiler_envelope = inspected.compiler_envelope_identity();

    let kernel_set = hash_identity(KERNEL_SET_IDENTITY_DOMAIN_V1, |digest| {
        digest.update(manifest.sha256());
        digest.update(manifest.byte_len().to_le_bytes());
        digest.update(compiler_envelope.as_bytes());
    });
    let kernel_set = KernelSetIdentityV1::from_bytes(kernel_set);

    let launch = inspected.policy().launch();
    let target_text = inspected.target().to_string();
    let target = hash_identity(TARGET_IDENTITY_DOMAIN_V1, |digest| {
        update_length_prefixed(digest, target_text.as_bytes());
        digest.update([code_object_version_tag(inspected.code_object_version())]);
        for axis in launch.required_workgroup_size() {
            digest.update(axis.to_le_bytes());
        }
        digest.update(launch.max_flat_workgroup_size().to_le_bytes());
        digest.update(launch.wavefront_size().to_le_bytes());
    });
    let target = TargetIdentityV1::from_bytes(target);
    let scope = LinkPublicationScopeV1::new(producer_package, kernel_set, target);

    let source = inspected.source_evidence_identity();
    let request = hash_identity(REQUEST_IDENTITY_DOMAIN_V1, |digest| {
        digest.update(inspected.sealed_request_id());
        digest.update(inspected.sealed_request_identity());
        digest.update(inspected.handoff_identity().as_bytes());
        digest.update(manifest.sha256());
        digest.update(manifest.byte_len().to_le_bytes());
        digest.update(inspected.link_plan_identity().as_bytes());
        digest.update(inspected.policy().compiler_envelope_identity().as_bytes());
        digest.update(inspected.policy().identity().as_bytes());
        digest.update(source.as_bytes());
        digest.update(inspected.worker_measurement().executable().sha256());
        digest.update(
            inspected
                .worker_measurement()
                .executable()
                .byte_len()
                .to_le_bytes(),
        );
    });
    let request = CanonicalLinkRequestIdentityV1::from_bytes(request);

    let worker_measurement = inspected.worker_measurement();
    let executable = worker_measurement.executable();
    let worker = hash_identity(WORKER_IDENTITY_DOMAIN_V1, |digest| {
        digest.update(executable.sha256());
        digest.update(executable.byte_len().to_le_bytes());
        update_length_prefixed(
            digest,
            worker_measurement.worker_build_identity().as_bytes(),
        );
        update_length_prefixed(digest, worker_measurement.llvm_build_identity().as_bytes());
    });
    let worker = PinnedWorkerIdentityV1::from_bytes(worker);

    let response = hash_identity(RESPONSE_IDENTITY_DOMAIN_V1, |digest| {
        digest.update(inspected.response_identity().as_bytes());
    });
    let response = ValidatedResponseIdentityV1::from_bytes(response);

    let output_digest: [u8; 32] = Sha256::digest(exact_bytes).into();
    if output_digest != *linked_source.sha256() {
        return Err(WorkerV2HsacoPublicationError::OutputIdentityMismatch);
    }
    let linked_output = LinkedOutputIdentityV1::from_bytes(output_digest);

    // This identity records raw-HSACO inspection. It does not claim canonical descriptor
    // finalization, which intentionally has not run on this path.
    let finalization = hash_identity(RAW_INSPECTION_IDENTITY_DOMAIN_V1, |digest| {
        digest.update(inspected.identity().as_bytes());
    });
    let finalization = FinalizationIdentityV1::from_bytes(finalization);
    let finalized_output = FinalizedOutputIdentityV1::from_bytes(output_digest);

    let attempt = inspected.attempt();
    let publication = hash_identity(ATOMIC_PUBLICATION_IDENTITY_DOMAIN_V1, |digest| {
        digest.update(attempt.generation().to_le_bytes());
        digest.update(attempt.session().as_bytes());
        digest.update(attempt.invocation().as_bytes());
        digest.update(producer_package.as_bytes());
        digest.update(kernel_set.as_bytes());
        digest.update(target.as_bytes());
        digest.update(request.as_bytes());
        digest.update(worker.as_bytes());
        digest.update(response.as_bytes());
        digest.update(linked_output.as_bytes());
        digest.update(finalization.as_bytes());
        digest.update(finalized_output.as_bytes());
        digest.update(inspected.identity().as_bytes());
    });
    let publication = AtomicPublicationIdentityV1::from_bytes(publication);
    let plan = DurableLinkPublicationPlanV1::new(
        attempt,
        scope,
        request,
        worker,
        response,
        linked_output,
        finalization,
        finalized_output,
        publication,
    );
    let upstream =
        UpstreamCodeObjectEvidenceIdentityV1::from_bytes(*inspected.identity().as_bytes());

    Ok(PreparedWorkerV2HsacoPublicationV1 {
        inspected,
        producer_package,
        plan,
        upstream,
    })
}

/// Publishes the exact inspected raw HSACO bytes for the prepared managed attempt.
///
/// The prepared object is borrowed so callers can retry the exact in-memory intent after a
/// retryable interruption. This does not provide process-restart recovery, compiler
/// authentication, HSA loading authority, or kernel-launch authority.
pub fn publish_prepared_worker_v2_hsaco_v1(
    output_dir: &Path,
    producer: &ProducerIdentity,
    prepared: &PreparedWorkerV2HsacoPublicationV1,
) -> Result<AttemptScopedHsacoPublicationResultV1, WorkerV2HsacoPublicationError> {
    if producer_package_identity_v1(producer) != prepared.producer_package {
        return Err(WorkerV2HsacoPublicationError::ProducerIdentityMismatch);
    }
    let exact_bytes = prepared.inspected.exact_bytes();
    let output_digest: [u8; 32] = Sha256::digest(exact_bytes).into();
    if !prepared
        .inspected
        .linked_output_identity()
        .matches(exact_bytes)
        || &output_digest != prepared.plan.finalized_output().as_bytes()
    {
        return Err(WorkerV2HsacoPublicationError::OutputIdentityMismatch);
    }

    publish_exact_hsaco_evidence_for_attempt_v1(
        output_dir,
        producer,
        prepared.inspected.attempt(),
        prepared.plan,
        prepared.upstream,
        exact_bytes,
    )
    .map_err(Into::into)
}

fn hash_identity(domain: &[u8], update: impl FnOnce(&mut Sha256)) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    update(&mut digest);
    digest.finalize().into()
}

fn update_length_prefixed(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

const fn code_object_version_tag(version: CodeObjectVersion) -> u8 {
    match version {
        CodeObjectVersion::V4 => 4,
        CodeObjectVersion::V5 => 5,
        CodeObjectVersion::V6 => 6,
    }
}
