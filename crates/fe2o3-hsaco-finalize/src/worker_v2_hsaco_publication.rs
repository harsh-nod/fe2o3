//! Typed publication bridges for independently inspected Worker V2 HSACO.
//!
//! The compatibility bridge publishes exact inspected raw bytes. The finalized bridge consumes
//! canonical finalization evidence and publishes only those exact finalized bytes. Neither bridge
//! authenticates compiler origin, grants loading or launch authority, or proves Verus verification.

use std::{error::Error, fmt, path::Path};

use fe2o3_artifact_transaction::{
    AtomicPublicationIdentityV1, AttemptScopedHsacoPublicationErrorV1,
    AttemptScopedHsacoPublicationResultV1, BuildAttempt, CanonicalLinkRequestIdentityV1,
    CompilerModuleHandoffIdentityV2, CompilerModuleHandoffSlotV2, DurableLinkPublicationPlanV1,
    FinalizationIdentityV1, FinalizedOutputIdentityV1, KernelSetIdentityV1, LinkPublicationScopeV1,
    LinkedOutputIdentityV1, PackageIdentityV1, PinnedWorkerIdentityV1, ProducerIdentity,
    TargetIdentityV1, UpstreamCodeObjectEvidenceIdentityV1, ValidatedResponseIdentityV1,
    producer_package_identity_v1, publish_exact_hsaco_evidence_for_attempt_v1,
};
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_kernel_descriptor::CodeObjectVersion;
use sha2::{Digest, Sha256};

use crate::{
    ContentIdentityV1, InspectedProtectedRawWorkerV2HsacoIdentityV1,
    InspectedProtectedRawWorkerV2HsacoV1, InspectedRawWorkerV2HsacoV1,
    PreparedFinalizedProtectedWorkerV2HsacoV2, PreparedFinalizedWorkerV2HsacoV1,
    WorkerMeasurementV1, WorkerV2RawHsacoPolicyV1,
};

const KERNEL_SET_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/WORKER-V2-PUBLICATION-KERNEL-SET/V1\0";
const TARGET_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/WORKER-V2-PUBLICATION-TARGET/V1\0";
const REQUEST_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/WORKER-V2-PUBLICATION-REQUEST/V1\0";
const WORKER_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/WORKER-V2-PUBLICATION-WORKER/V1\0";
const RESPONSE_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/WORKER-V2-PUBLICATION-RESPONSE/V1\0";
const RAW_INSPECTION_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/WORKER-V2-PUBLICATION-RAW-INSPECTION/V1\0";
const ATOMIC_PUBLICATION_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/WORKER-V2-ATOMIC-PUBLICATION/V1\0";
const FINALIZED_KERNEL_SET_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/WORKER-V2-FINALIZED-PUBLICATION-KERNEL-SET/V1\0";
const FINALIZED_TARGET_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/WORKER-V2-FINALIZED-PUBLICATION-TARGET/V1\0";
const FINALIZED_REQUEST_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/WORKER-V2-FINALIZED-PUBLICATION-REQUEST/V1\0";
const FINALIZED_WORKER_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/WORKER-V2-FINALIZED-PUBLICATION-WORKER/V1\0";
const FINALIZED_RESPONSE_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/WORKER-V2-FINALIZED-PUBLICATION-RESPONSE/V1\0";
const CANONICAL_FINALIZATION_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/WORKER-V2-FINALIZED-PUBLICATION-FINALIZATION/V1\0";
const FINALIZED_ATOMIC_PUBLICATION_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/WORKER-V2-FINALIZED-ATOMIC-PUBLICATION/V1\0";
const FINALIZED_UPSTREAM_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/WORKER-V2-FINALIZED-PUBLICATION-UPSTREAM/V1\0";

const PROTECTED_RAW_KERNEL_SET_IDENTITY_DOMAIN_V2: &[u8] =
    b"FE2O3/CLOSURE-PROTECTED-WORKER-V2-PUBLICATION-KERNEL-SET/V2\0";
const PROTECTED_RAW_TARGET_IDENTITY_DOMAIN_V2: &[u8] =
    b"FE2O3/CLOSURE-PROTECTED-WORKER-V2-PUBLICATION-TARGET/V2\0";
const PROTECTED_RAW_REQUEST_IDENTITY_DOMAIN_V2: &[u8] =
    b"FE2O3/CLOSURE-PROTECTED-WORKER-V2-PUBLICATION-REQUEST/V2\0";
const PROTECTED_RAW_WORKER_IDENTITY_DOMAIN_V2: &[u8] =
    b"FE2O3/CLOSURE-PROTECTED-WORKER-V2-PUBLICATION-WORKER/V2\0";
const PROTECTED_RAW_RESPONSE_IDENTITY_DOMAIN_V2: &[u8] =
    b"FE2O3/CLOSURE-PROTECTED-WORKER-V2-PUBLICATION-RESPONSE/V2\0";
const PROTECTED_RAW_FINALIZATION_IDENTITY_DOMAIN_V2: &[u8] =
    b"FE2O3/CLOSURE-PROTECTED-WORKER-V2-PUBLICATION-RAW-INSPECTION/V2\0";
const PROTECTED_RAW_ATOMIC_PUBLICATION_IDENTITY_DOMAIN_V2: &[u8] =
    b"FE2O3/CLOSURE-PROTECTED-WORKER-V2-ATOMIC-PUBLICATION/V2\0";
const PROTECTED_RAW_UPSTREAM_IDENTITY_DOMAIN_V2: &[u8] =
    b"FE2O3/CLOSURE-PROTECTED-WORKER-V2-PUBLICATION-UPSTREAM/V2\0";
const PROTECTED_FINALIZED_KERNEL_SET_IDENTITY_DOMAIN_V2: &[u8] =
    b"FE2O3/CLOSURE-PROTECTED-WORKER-V2-FINALIZED-PUBLICATION-KERNEL-SET/V2\0";
const PROTECTED_FINALIZED_TARGET_IDENTITY_DOMAIN_V2: &[u8] =
    b"FE2O3/CLOSURE-PROTECTED-WORKER-V2-FINALIZED-PUBLICATION-TARGET/V2\0";
const PROTECTED_FINALIZED_REQUEST_IDENTITY_DOMAIN_V2: &[u8] =
    b"FE2O3/CLOSURE-PROTECTED-WORKER-V2-FINALIZED-PUBLICATION-REQUEST/V2\0";
const PROTECTED_FINALIZED_WORKER_IDENTITY_DOMAIN_V2: &[u8] =
    b"FE2O3/CLOSURE-PROTECTED-WORKER-V2-FINALIZED-PUBLICATION-WORKER/V2\0";
const PROTECTED_FINALIZED_RESPONSE_IDENTITY_DOMAIN_V2: &[u8] =
    b"FE2O3/CLOSURE-PROTECTED-WORKER-V2-FINALIZED-PUBLICATION-RESPONSE/V2\0";
const PROTECTED_FINALIZED_FINALIZATION_IDENTITY_DOMAIN_V2: &[u8] =
    b"FE2O3/CLOSURE-PROTECTED-WORKER-V2-FINALIZED-PUBLICATION-FINALIZATION/V2\0";
const PROTECTED_FINALIZED_ATOMIC_PUBLICATION_IDENTITY_DOMAIN_V2: &[u8] =
    b"FE2O3/CLOSURE-PROTECTED-WORKER-V2-FINALIZED-ATOMIC-PUBLICATION/V2\0";
const PROTECTED_FINALIZED_UPSTREAM_IDENTITY_DOMAIN_V2: &[u8] =
    b"FE2O3/CLOSURE-PROTECTED-WORKER-V2-FINALIZED-PUBLICATION-UPSTREAM/V2\0";

const PROTECTED_INSPECTION_BINDING_DOMAIN_V2: &[u8] =
    b"FE2O3/CLOSURE-PROTECTED-PUBLICATION/RAW-INSPECTION/V2\0";
const PROTECTED_SOURCE_BINDING_DOMAIN_V2: &[u8] =
    b"FE2O3/CLOSURE-PROTECTED-PUBLICATION/SOURCE-EVIDENCE/V2\0";
const PROTECTED_HANDOFF_SLOT_BINDING_DOMAIN_V2: &[u8] =
    b"FE2O3/CLOSURE-PROTECTED-PUBLICATION/HANDOFF-SLOT/V2\0";
const PROTECTED_HANDOFF_IDENTITY_BINDING_DOMAIN_V2: &[u8] =
    b"FE2O3/CLOSURE-PROTECTED-PUBLICATION/HANDOFF-IDENTITY/V2\0";
const PROTECTED_COMPILER_CLOSURE_BINDING_DOMAIN_V2: &[u8] =
    b"FE2O3/CLOSURE-PROTECTED-PUBLICATION/COMPILER-CLOSURE/V2\0";
const PROTECTED_CANONICAL_FINALIZATION_BINDING_DOMAIN_V2: &[u8] =
    b"FE2O3/CLOSURE-PROTECTED-PUBLICATION/CANONICAL-FINALIZATION/V2\0";

/// Canonical Worker V2 publication route sealed by the finalizer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerV2HsacoPublicationRouteV1 {
    /// Publish the exact independently inspected raw COV5 compatibility snapshot.
    InspectedRaw,
    /// Publish the exact COV6 snapshot produced by canonical descriptor finalization.
    CanonicallyFinalized,
}

/// Inert, canonical publication intent derived from sealed Worker V2 evidence.
///
/// This is the only public view of the finalizer's private publication-plan derivation. It binds
/// the raw linked snapshot and inspection identity even when the retained output is a distinct
/// canonically finalized snapshot. Callers may persist the returned plan and upstream identity for
/// restart recovery, but cannot construct or modify this value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SealedWorkerV2HsacoPublicationIntentV1 {
    route: WorkerV2HsacoPublicationRouteV1,
    plan: DurableLinkPublicationPlanV1,
    upstream: UpstreamCodeObjectEvidenceIdentityV1,
    raw_inspection: crate::InspectedRawWorkerV2HsacoIdentityV1,
    canonical_finalization: Option<crate::FinalizedWorkerV2HsacoIdentityV1>,
    raw_snapshot: crate::ContentIdentityV1,
    finalized_snapshot: crate::ContentIdentityV1,
}

impl SealedWorkerV2HsacoPublicationIntentV1 {
    /// Returns whether the intent retains raw inspected bytes or canonical finalized bytes.
    pub const fn route(self) -> WorkerV2HsacoPublicationRouteV1 {
        self.route
    }

    /// Returns the complete canonical durable plan for restart persistence.
    pub const fn durable_plan(self) -> DurableLinkPublicationPlanV1 {
        self.plan
    }

    /// Returns the canonical upstream evidence identity paired with the durable plan.
    pub const fn upstream_evidence(self) -> UpstreamCodeObjectEvidenceIdentityV1 {
        self.upstream
    }

    /// Returns the independently derived identity of the sealed raw Worker V2 inspection.
    pub const fn raw_inspection_identity(self) -> crate::InspectedRawWorkerV2HsacoIdentityV1 {
        self.raw_inspection
    }

    /// Returns the canonical finalization identity when this is a finalized publication route.
    pub const fn canonical_finalization_identity(
        self,
    ) -> Option<crate::FinalizedWorkerV2HsacoIdentityV1> {
        self.canonical_finalization
    }

    /// Returns the digest and length of the exact raw linked snapshot.
    pub const fn raw_linked_snapshot_identity(self) -> crate::ContentIdentityV1 {
        self.raw_snapshot
    }

    /// Returns the digest and length of the exact snapshot retained for publication.
    pub const fn finalized_snapshot_identity(self) -> crate::ContentIdentityV1 {
        self.finalized_snapshot
    }

    /// Checks exact retained output bytes against both their sealed digest and length.
    pub fn matches_exact_retained_output(self, bytes: &[u8]) -> bool {
        self.finalized_snapshot.matches(bytes)
    }

    /// A restartable intent remains inert without attempt-scoped publication authority.
    pub const fn grants_publication_authority(self) -> bool {
        false
    }

    /// Publication identities do not authenticate compiler origin.
    pub const fn authenticates_compiler_origin(self) -> bool {
        false
    }

    /// Publication intent is not HSA loading authority.
    pub const fn grants_load_authority(self) -> bool {
        false
    }

    /// Publication intent is not kernel-launch authority.
    pub const fn grants_launch_authority(self) -> bool {
        false
    }
}

/// Protected Worker V2 preparation route bound under V2 identity domains.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtectedWorkerV2HsacoPublicationRouteV2 {
    InspectedRaw,
    CanonicallyFinalized,
}

/// Inert protected restart input derived from one consumed protected inspection.
///
/// The complete compiler closure and exact V2 handoff lineage remain inspectable. This value is
/// not publication authority; `#203` supplies the V2 attempt-scoped publication operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SealedProtectedWorkerV2HsacoPublicationIntentV2 {
    route: ProtectedWorkerV2HsacoPublicationRouteV2,
    plan: DurableLinkPublicationPlanV1,
    upstream: UpstreamCodeObjectEvidenceIdentityV1,
    raw_inspection: InspectedProtectedRawWorkerV2HsacoIdentityV1,
    canonical_finalization: Option<crate::FinalizedProtectedWorkerV2HsacoIdentityV2>,
    raw_snapshot: ContentIdentityV1,
    retained_snapshot: ContentIdentityV1,
    handoff_slot: CompilerModuleHandoffSlotV2,
    handoff_identity: CompilerModuleHandoffIdentityV2,
    compiler_closure: CompilerClosureV2,
}

impl SealedProtectedWorkerV2HsacoPublicationIntentV2 {
    pub const fn route(self) -> ProtectedWorkerV2HsacoPublicationRouteV2 {
        self.route
    }

    pub const fn durable_plan(self) -> DurableLinkPublicationPlanV1 {
        self.plan
    }

    pub const fn upstream_evidence(self) -> UpstreamCodeObjectEvidenceIdentityV1 {
        self.upstream
    }

    pub const fn raw_inspection_identity(self) -> InspectedProtectedRawWorkerV2HsacoIdentityV1 {
        self.raw_inspection
    }

    pub const fn canonical_finalization_identity(
        self,
    ) -> Option<crate::FinalizedProtectedWorkerV2HsacoIdentityV2> {
        self.canonical_finalization
    }

    pub const fn raw_linked_snapshot_identity(self) -> ContentIdentityV1 {
        self.raw_snapshot
    }

    pub const fn retained_snapshot_identity(self) -> ContentIdentityV1 {
        self.retained_snapshot
    }

    pub const fn handoff_slot(self) -> CompilerModuleHandoffSlotV2 {
        self.handoff_slot
    }

    pub const fn handoff_identity(self) -> CompilerModuleHandoffIdentityV2 {
        self.handoff_identity
    }

    pub const fn compiler_closure(self) -> CompilerClosureV2 {
        self.compiler_closure
    }

    pub fn matches_exact_retained_output(self, bytes: &[u8]) -> bool {
        self.retained_snapshot.matches(bytes)
    }

    pub const fn grants_compiler_authority(self) -> bool {
        false
    }

    pub const fn grants_publication_authority(self) -> bool {
        false
    }

    pub const fn grants_load_authority(self) -> bool {
        false
    }

    pub const fn grants_launch_authority(self) -> bool {
        false
    }
}

/// Complete inert preparation for exact protected raw Worker V2 bytes.
#[derive(Debug)]
pub struct PreparedProtectedWorkerV2HsacoPublicationV2 {
    inspected: InspectedProtectedRawWorkerV2HsacoV1,
    plan: DurableLinkPublicationPlanV1,
    upstream: UpstreamCodeObjectEvidenceIdentityV1,
}

impl PreparedProtectedWorkerV2HsacoPublicationV2 {
    pub const fn attempt(&self) -> BuildAttempt {
        self.inspected.attempt()
    }

    pub const fn handoff_slot(&self) -> CompilerModuleHandoffSlotV2 {
        self.inspected.handoff_slot()
    }

    pub const fn handoff_identity(&self) -> CompilerModuleHandoffIdentityV2 {
        self.inspected.handoff_identity()
    }

    pub const fn compiler_closure(&self) -> CompilerClosureV2 {
        self.inspected.compiler_closure()
    }

    pub fn exact_retained_output(&self) -> &[u8] {
        self.inspected.exact_bytes()
    }

    pub const fn publication_intent(&self) -> SealedProtectedWorkerV2HsacoPublicationIntentV2 {
        let raw_snapshot = self.inspected.linked_output_identity();
        SealedProtectedWorkerV2HsacoPublicationIntentV2 {
            route: ProtectedWorkerV2HsacoPublicationRouteV2::InspectedRaw,
            plan: self.plan,
            upstream: self.upstream,
            raw_inspection: self.inspected.identity(),
            canonical_finalization: None,
            raw_snapshot,
            retained_snapshot: raw_snapshot,
            handoff_slot: self.inspected.handoff_slot(),
            handoff_identity: self.inspected.handoff_identity(),
            compiler_closure: self.inspected.compiler_closure(),
        }
    }

    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Complete inert preparation for exact protected canonically finalized Worker V2 bytes.
#[derive(Debug)]
pub struct PreparedFinalizedProtectedWorkerV2HsacoPublicationV2 {
    finalized: PreparedFinalizedProtectedWorkerV2HsacoV2,
    plan: DurableLinkPublicationPlanV1,
    upstream: UpstreamCodeObjectEvidenceIdentityV1,
}

impl PreparedFinalizedProtectedWorkerV2HsacoPublicationV2 {
    pub const fn attempt(&self) -> BuildAttempt {
        self.finalized.attempt()
    }

    pub const fn handoff_slot(&self) -> CompilerModuleHandoffSlotV2 {
        self.finalized.handoff_slot()
    }

    pub const fn handoff_identity(&self) -> CompilerModuleHandoffIdentityV2 {
        self.finalized.handoff_identity()
    }

    pub const fn compiler_closure(&self) -> CompilerClosureV2 {
        self.finalized.compiler_closure()
    }

    pub fn exact_retained_output(&self) -> &[u8] {
        self.finalized.exact_finalized_bytes()
    }

    pub const fn publication_intent(&self) -> SealedProtectedWorkerV2HsacoPublicationIntentV2 {
        SealedProtectedWorkerV2HsacoPublicationIntentV2 {
            route: ProtectedWorkerV2HsacoPublicationRouteV2::CanonicallyFinalized,
            plan: self.plan,
            upstream: self.upstream,
            raw_inspection: self.finalized.raw_inspection_identity(),
            canonical_finalization: Some(self.finalized.identity()),
            raw_snapshot: self.finalized.raw_output_identity(),
            retained_snapshot: self.finalized.finalized_output_identity(),
            handoff_slot: self.finalized.handoff_slot(),
            handoff_identity: self.finalized.handoff_identity(),
            compiler_closure: self.finalized.compiler_closure(),
        }
    }

    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// A complete, internally derived publication intent for exact inspected Worker V2 bytes.
///
/// Construction of the durable plan and upstream evidence identity remains private so callers
/// cannot replace any retained manifest, target, request, worker, response, output, or inspection
/// identity inside this object. It is inert without the matching producer and live build-attempt
/// registry authority.
#[derive(Debug)]
pub struct PreparedWorkerV2HsacoPublicationV1 {
    inspected: InspectedRawWorkerV2HsacoV1,
    producer_package: PackageIdentityV1,
    plan: DurableLinkPublicationPlanV1,
    upstream: UpstreamCodeObjectEvidenceIdentityV1,
}

/// Complete durable intent for exact canonically finalized Worker V2 bytes.
///
/// This value owns the finalization evidence, including its raw inspection lineage. It is not
/// cloneable and exposes the durable plan only through an opaque inert intent; byte fields remain
/// private and non-replaceable.
#[derive(Debug)]
pub struct PreparedFinalizedWorkerV2HsacoPublicationV1 {
    finalized: PreparedFinalizedWorkerV2HsacoV1,
    producer_package: PackageIdentityV1,
    plan: DurableLinkPublicationPlanV1,
    upstream: UpstreamCodeObjectEvidenceIdentityV1,
}

impl PreparedFinalizedWorkerV2HsacoPublicationV1 {
    pub const fn attempt(&self) -> BuildAttempt {
        self.finalized.attempt()
    }

    pub fn exact_finalized_bytes(&self) -> &[u8] {
        self.finalized.exact_finalized_bytes()
    }

    pub const fn raw_inspection_identity(&self) -> crate::InspectedRawWorkerV2HsacoIdentityV1 {
        self.finalized.raw_inspection_identity()
    }

    pub const fn canonical_finalization_identity(&self) -> crate::FinalizedWorkerV2HsacoIdentityV1 {
        self.finalized.identity()
    }

    pub const fn raw_output_identity(&self) -> crate::ContentIdentityV1 {
        self.finalized.raw_output_identity()
    }

    pub const fn finalized_output_identity(&self) -> crate::ContentIdentityV1 {
        self.finalized.finalized_output_identity()
    }

    /// Reconstructs the canonical inert intent from the retained sealed raw/finalized lineage.
    pub const fn publication_intent(&self) -> SealedWorkerV2HsacoPublicationIntentV1 {
        SealedWorkerV2HsacoPublicationIntentV1 {
            route: WorkerV2HsacoPublicationRouteV1::CanonicallyFinalized,
            plan: self.plan,
            upstream: self.upstream,
            raw_inspection: self.finalized.raw_inspection_identity(),
            canonical_finalization: Some(self.finalized.identity()),
            raw_snapshot: self.finalized.raw_output_identity(),
            finalized_snapshot: self.finalized.finalized_output_identity(),
        }
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    pub const fn proves_verus_verification(&self) -> bool {
        false
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
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

    /// Reconstructs the canonical inert intent from the retained sealed raw inspection.
    pub const fn publication_intent(&self) -> SealedWorkerV2HsacoPublicationIntentV1 {
        let raw_snapshot = self.inspected.linked_output_identity();
        SealedWorkerV2HsacoPublicationIntentV1 {
            route: WorkerV2HsacoPublicationRouteV1::InspectedRaw,
            plan: self.plan,
            upstream: self.upstream,
            raw_inspection: self.inspected.identity(),
            canonical_finalization: None,
            raw_snapshot,
            finalized_snapshot: raw_snapshot,
        }
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
    /// Retained raw lineage no longer matches the linked worker output.
    RawOutputIdentityMismatch,
    /// Retained finalized bytes no longer match canonical finalization evidence.
    FinalizedOutputIdentityMismatch,
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
            Self::RawOutputIdentityMismatch => formatter
                .write_str("retained raw HSACO lineage does not match the linked-output identity"),
            Self::FinalizedOutputIdentityMismatch => formatter.write_str(
                "retained finalized HSACO bytes do not match canonical finalization evidence",
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

enum PublicationSchemaBindingV2 {
    OrdinaryV1 {
        handoff: [u8; 32],
        source: [u8; 32],
        inspection: [u8; 32],
    },
    ProtectedV2 {
        handoff_slot: CompilerModuleHandoffSlotV2,
        handoff: [u8; 32],
        source: [u8; 32],
        inspection: [u8; 32],
        compiler_closure: Box<CompilerClosureV2>,
    },
}

struct PublicationInspectionViewV2<'a> {
    exact_bytes: &'a [u8],
    linked_output: ContentIdentityV1,
    attempt: BuildAttempt,
    target: fe2o3_kernel_descriptor::DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    policy: &'a WorkerV2RawHsacoPolicyV1,
    compiler_envelope: crate::CompilerFfiEnvelopeIdentityV1,
    sealed_request_id: &'a [u8; 32],
    sealed_request_identity: &'a [u8; 32],
    link_plan_identity: crate::LinkPlanIdentityV1,
    worker_measurement: &'a WorkerMeasurementV1,
    response_identity: crate::SealedWorkerV2ResponseIdentityV1,
    schema: PublicationSchemaBindingV2,
}

impl<'a> PublicationInspectionViewV2<'a> {
    fn ordinary(raw: &'a InspectedRawWorkerV2HsacoV1) -> Self {
        Self {
            exact_bytes: raw.exact_bytes(),
            linked_output: raw.linked_output_identity(),
            attempt: raw.attempt(),
            target: raw.target(),
            code_object_version: raw.code_object_version(),
            policy: raw.policy(),
            compiler_envelope: raw.compiler_envelope_identity(),
            sealed_request_id: raw.sealed_request_id(),
            sealed_request_identity: raw.sealed_request_identity(),
            link_plan_identity: raw.link_plan_identity(),
            worker_measurement: raw.worker_measurement(),
            response_identity: raw.response_identity(),
            schema: PublicationSchemaBindingV2::OrdinaryV1 {
                handoff: *raw.handoff_identity().as_bytes(),
                source: *raw.source_evidence_identity().as_bytes(),
                inspection: *raw.identity().as_bytes(),
            },
        }
    }

    fn protected(raw: &'a InspectedProtectedRawWorkerV2HsacoV1) -> Self {
        Self {
            exact_bytes: raw.exact_bytes(),
            linked_output: raw.linked_output_identity(),
            attempt: raw.attempt(),
            target: raw.target(),
            code_object_version: raw.code_object_version(),
            policy: raw.policy(),
            compiler_envelope: raw.compiler_envelope_identity(),
            sealed_request_id: raw.sealed_request_id(),
            sealed_request_identity: raw.sealed_request_identity(),
            link_plan_identity: raw.link_plan_identity(),
            worker_measurement: raw.worker_measurement(),
            response_identity: raw.response_identity(),
            schema: PublicationSchemaBindingV2::ProtectedV2 {
                handoff_slot: raw.handoff_slot(),
                handoff: *raw.handoff_identity().as_bytes(),
                source: *raw.source_evidence_identity().as_bytes(),
                inspection: *raw.identity().as_bytes(),
                compiler_closure: Box::new(raw.compiler_closure()),
            },
        }
    }
}

#[derive(Clone, Copy)]
struct PublicationIdentityDomainsV2 {
    kernel_set: &'static [u8],
    target: &'static [u8],
    request: &'static [u8],
    worker: &'static [u8],
    response: &'static [u8],
    publication: &'static [u8],
}

const RAW_DOMAINS_V1: PublicationIdentityDomainsV2 = PublicationIdentityDomainsV2 {
    kernel_set: KERNEL_SET_IDENTITY_DOMAIN_V1,
    target: TARGET_IDENTITY_DOMAIN_V1,
    request: REQUEST_IDENTITY_DOMAIN_V1,
    worker: WORKER_IDENTITY_DOMAIN_V1,
    response: RESPONSE_IDENTITY_DOMAIN_V1,
    publication: ATOMIC_PUBLICATION_IDENTITY_DOMAIN_V1,
};
const FINALIZED_DOMAINS_V1: PublicationIdentityDomainsV2 = PublicationIdentityDomainsV2 {
    kernel_set: FINALIZED_KERNEL_SET_IDENTITY_DOMAIN_V1,
    target: FINALIZED_TARGET_IDENTITY_DOMAIN_V1,
    request: FINALIZED_REQUEST_IDENTITY_DOMAIN_V1,
    worker: FINALIZED_WORKER_IDENTITY_DOMAIN_V1,
    response: FINALIZED_RESPONSE_IDENTITY_DOMAIN_V1,
    publication: FINALIZED_ATOMIC_PUBLICATION_IDENTITY_DOMAIN_V1,
};
const PROTECTED_RAW_DOMAINS_V2: PublicationIdentityDomainsV2 = PublicationIdentityDomainsV2 {
    kernel_set: PROTECTED_RAW_KERNEL_SET_IDENTITY_DOMAIN_V2,
    target: PROTECTED_RAW_TARGET_IDENTITY_DOMAIN_V2,
    request: PROTECTED_RAW_REQUEST_IDENTITY_DOMAIN_V2,
    worker: PROTECTED_RAW_WORKER_IDENTITY_DOMAIN_V2,
    response: PROTECTED_RAW_RESPONSE_IDENTITY_DOMAIN_V2,
    publication: PROTECTED_RAW_ATOMIC_PUBLICATION_IDENTITY_DOMAIN_V2,
};
const PROTECTED_FINALIZED_DOMAINS_V2: PublicationIdentityDomainsV2 = PublicationIdentityDomainsV2 {
    kernel_set: PROTECTED_FINALIZED_KERNEL_SET_IDENTITY_DOMAIN_V2,
    target: PROTECTED_FINALIZED_TARGET_IDENTITY_DOMAIN_V2,
    request: PROTECTED_FINALIZED_REQUEST_IDENTITY_DOMAIN_V2,
    worker: PROTECTED_FINALIZED_WORKER_IDENTITY_DOMAIN_V2,
    response: PROTECTED_FINALIZED_RESPONSE_IDENTITY_DOMAIN_V2,
    publication: PROTECTED_FINALIZED_ATOMIC_PUBLICATION_IDENTITY_DOMAIN_V2,
};

struct PublicationOutputBindingV2<'a> {
    exact_output: &'a [u8],
    finalized_output: ContentIdentityV1,
    finalization: FinalizationIdentityV1,
    canonical_finalization: Option<[u8; 32]>,
}

fn derive_publication_plan_shared_v2(
    producer_package: PackageIdentityV1,
    raw: &PublicationInspectionViewV2<'_>,
    output: &PublicationOutputBindingV2<'_>,
    domains: PublicationIdentityDomainsV2,
) -> Result<DurableLinkPublicationPlanV1, WorkerV2HsacoPublicationError> {
    if !raw.linked_output.matches(raw.exact_bytes) {
        return Err(if output.canonical_finalization.is_some() {
            WorkerV2HsacoPublicationError::RawOutputIdentityMismatch
        } else {
            WorkerV2HsacoPublicationError::OutputIdentityMismatch
        });
    }
    if !output.finalized_output.matches(output.exact_output) {
        return Err(if output.canonical_finalization.is_some() {
            WorkerV2HsacoPublicationError::FinalizedOutputIdentityMismatch
        } else {
            WorkerV2HsacoPublicationError::OutputIdentityMismatch
        });
    }

    let manifest = raw.policy.symbol_manifest().identity();
    let kernel_set = hash_identity(domains.kernel_set, |digest| {
        digest.update(manifest.sha256());
        digest.update(manifest.byte_len().to_le_bytes());
        digest.update(raw.compiler_envelope.as_bytes());
    });
    let kernel_set = KernelSetIdentityV1::from_bytes(kernel_set);

    let launch = raw.policy.launch();
    let target_text = raw.target.to_string();
    let target = hash_identity(domains.target, |digest| {
        update_length_prefixed(digest, target_text.as_bytes());
        digest.update([code_object_version_tag(raw.code_object_version)]);
        for axis in launch.required_workgroup_size() {
            digest.update(axis.to_le_bytes());
        }
        digest.update(launch.max_flat_workgroup_size().to_le_bytes());
        digest.update(launch.wavefront_size().to_le_bytes());
    });
    let target = TargetIdentityV1::from_bytes(target);
    let scope = LinkPublicationScopeV1::new(producer_package, kernel_set, target);

    let request = hash_identity(domains.request, |digest| {
        digest.update(raw.sealed_request_id);
        digest.update(raw.sealed_request_identity);
        update_request_schema_binding(digest, &raw.schema);
        digest.update(manifest.sha256());
        digest.update(manifest.byte_len().to_le_bytes());
        digest.update(raw.link_plan_identity.as_bytes());
        digest.update(raw.policy.compiler_envelope_identity().as_bytes());
        digest.update(raw.policy.identity().as_bytes());
        update_source_schema_binding(digest, &raw.schema);
        digest.update(raw.worker_measurement.executable().sha256());
        digest.update(raw.worker_measurement.executable().byte_len().to_le_bytes());
        if let PublicationSchemaBindingV2::ProtectedV2 { .. } = &raw.schema {
            update_protected_closure_and_inspection_binding(digest, &raw.schema);
        }
    });
    let request = CanonicalLinkRequestIdentityV1::from_bytes(request);

    let executable = raw.worker_measurement.executable();
    let worker = hash_identity(domains.worker, |digest| {
        digest.update(executable.sha256());
        digest.update(executable.byte_len().to_le_bytes());
        update_length_prefixed(
            digest,
            raw.worker_measurement.worker_build_identity().as_bytes(),
        );
        update_length_prefixed(
            digest,
            raw.worker_measurement.llvm_build_identity().as_bytes(),
        );
    });
    let worker = PinnedWorkerIdentityV1::from_bytes(worker);

    let response = hash_identity(domains.response, |digest| {
        digest.update(raw.response_identity.as_bytes());
    });
    let response = ValidatedResponseIdentityV1::from_bytes(response);
    let linked_output = LinkedOutputIdentityV1::from_bytes(*raw.linked_output.sha256());
    let finalized_output = FinalizedOutputIdentityV1::from_bytes(*output.finalized_output.sha256());

    let publication = hash_identity(domains.publication, |digest| {
        digest.update(raw.attempt.generation().to_le_bytes());
        digest.update(raw.attempt.session().as_bytes());
        digest.update(raw.attempt.invocation().as_bytes());
        digest.update(producer_package.as_bytes());
        digest.update(kernel_set.as_bytes());
        digest.update(target.as_bytes());
        digest.update(request.as_bytes());
        digest.update(worker.as_bytes());
        digest.update(response.as_bytes());
        digest.update(linked_output.as_bytes());
        digest.update(output.finalization.as_bytes());
        digest.update(finalized_output.as_bytes());
        update_atomic_schema_binding(digest, &raw.schema, output.canonical_finalization);
    });
    Ok(DurableLinkPublicationPlanV1::new(
        raw.attempt,
        scope,
        request,
        worker,
        response,
        linked_output,
        output.finalization,
        finalized_output,
        AtomicPublicationIdentityV1::from_bytes(publication),
    ))
}

fn update_request_schema_binding(digest: &mut Sha256, schema: &PublicationSchemaBindingV2) {
    match schema {
        PublicationSchemaBindingV2::OrdinaryV1 { handoff, .. } => digest.update(handoff),
        PublicationSchemaBindingV2::ProtectedV2 {
            handoff_slot,
            handoff,
            ..
        } => {
            digest.update(hash_identity(
                PROTECTED_HANDOFF_SLOT_BINDING_DOMAIN_V2,
                |component| component.update([*handoff_slot as u8]),
            ));
            digest.update(hash_identity(
                PROTECTED_HANDOFF_IDENTITY_BINDING_DOMAIN_V2,
                |component| component.update(handoff),
            ));
        }
    }
}

fn update_source_schema_binding(digest: &mut Sha256, schema: &PublicationSchemaBindingV2) {
    match schema {
        PublicationSchemaBindingV2::OrdinaryV1 { source, .. } => digest.update(source),
        PublicationSchemaBindingV2::ProtectedV2 { source, .. } => digest.update(hash_identity(
            PROTECTED_SOURCE_BINDING_DOMAIN_V2,
            |component| component.update(source),
        )),
    }
}

fn update_inspection_schema_binding(digest: &mut Sha256, schema: &PublicationSchemaBindingV2) {
    match schema {
        PublicationSchemaBindingV2::OrdinaryV1 { inspection, .. } => digest.update(inspection),
        PublicationSchemaBindingV2::ProtectedV2 { inspection, .. } => digest.update(hash_identity(
            PROTECTED_INSPECTION_BINDING_DOMAIN_V2,
            |component| component.update(inspection),
        )),
    }
}

fn update_protected_closure_and_inspection_binding(
    digest: &mut Sha256,
    schema: &PublicationSchemaBindingV2,
) {
    if let PublicationSchemaBindingV2::ProtectedV2 {
        inspection,
        compiler_closure,
        ..
    } = schema
    {
        digest.update(hash_identity(
            PROTECTED_COMPILER_CLOSURE_BINDING_DOMAIN_V2,
            |component| hash_compiler_closure_v2(component, **compiler_closure),
        ));
        digest.update(hash_identity(
            PROTECTED_INSPECTION_BINDING_DOMAIN_V2,
            |component| component.update(inspection),
        ));
    }
}

fn update_all_protected_schema_bindings(digest: &mut Sha256, schema: &PublicationSchemaBindingV2) {
    if let PublicationSchemaBindingV2::ProtectedV2 { .. } = schema {
        update_inspection_schema_binding(digest, schema);
        update_source_schema_binding(digest, schema);
        update_request_schema_binding(digest, schema);
        if let PublicationSchemaBindingV2::ProtectedV2 {
            compiler_closure, ..
        } = schema
        {
            digest.update(hash_identity(
                PROTECTED_COMPILER_CLOSURE_BINDING_DOMAIN_V2,
                |component| hash_compiler_closure_v2(component, **compiler_closure),
            ));
        }
    }
}

fn update_atomic_schema_binding(
    digest: &mut Sha256,
    schema: &PublicationSchemaBindingV2,
    canonical_finalization: Option<[u8; 32]>,
) {
    match schema {
        PublicationSchemaBindingV2::OrdinaryV1 { inspection, .. } => {
            digest.update(inspection);
            if let Some(finalization) = canonical_finalization {
                digest.update(finalization);
            }
        }
        PublicationSchemaBindingV2::ProtectedV2 { .. } => {
            update_all_protected_schema_bindings(digest, schema);
            if let Some(finalization) = canonical_finalization {
                digest.update(hash_identity(
                    PROTECTED_CANONICAL_FINALIZATION_BINDING_DOMAIN_V2,
                    |component| component.update(finalization),
                ));
            }
        }
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
    let output_digest: [u8; 32] = Sha256::digest(exact_bytes).into();
    if output_digest != *linked_source.sha256() {
        return Err(WorkerV2HsacoPublicationError::OutputIdentityMismatch);
    }
    let view = PublicationInspectionViewV2::ordinary(&inspected);
    let finalization = hash_identity(RAW_INSPECTION_IDENTITY_DOMAIN_V1, |digest| {
        update_inspection_schema_binding(digest, &view.schema);
    });
    let upstream = UpstreamCodeObjectEvidenceIdentityV1::from_bytes(match &view.schema {
        PublicationSchemaBindingV2::OrdinaryV1 { inspection, .. } => *inspection,
        PublicationSchemaBindingV2::ProtectedV2 { .. } => unreachable!(),
    });
    let output = PublicationOutputBindingV2 {
        exact_output: exact_bytes,
        finalized_output: linked_source,
        finalization: FinalizationIdentityV1::from_bytes(finalization),
        canonical_finalization: None,
    };
    let plan = derive_publication_plan_shared_v2(producer_package, &view, &output, RAW_DOMAINS_V1)?;

    Ok(PreparedWorkerV2HsacoPublicationV1 {
        inspected,
        producer_package,
        plan,
        upstream,
    })
}

/// Consumes canonical finalization evidence and derives a durable exact-byte publication plan.
///
/// The raw worker output remains the linked-output identity. The canonical bytes have a distinct
/// finalized-output identity, while the finalization and upstream identities bind both stages.
pub fn prepare_finalized_worker_v2_hsaco_publication_v1(
    producer: &ProducerIdentity,
    finalized: PreparedFinalizedWorkerV2HsacoV1,
) -> Result<PreparedFinalizedWorkerV2HsacoPublicationV1, WorkerV2HsacoPublicationError> {
    let raw = finalized.raw_inspection();
    if !finalized.raw_output_identity().matches(raw.exact_bytes()) {
        return Err(WorkerV2HsacoPublicationError::RawOutputIdentityMismatch);
    }
    let exact_bytes = finalized.exact_finalized_bytes();
    if !finalized.finalized_output_identity().matches(exact_bytes) {
        return Err(WorkerV2HsacoPublicationError::FinalizedOutputIdentityMismatch);
    }

    let producer_package = producer_package_identity_v1(producer);
    let view = PublicationInspectionViewV2::ordinary(raw);
    let finalization = hash_identity(CANONICAL_FINALIZATION_IDENTITY_DOMAIN_V1, |digest| {
        update_inspection_schema_binding(digest, &view.schema);
        digest.update(finalized.identity().as_bytes());
        digest.update(finalized.canonical_digest().as_bytes());
        hash_content_identity(digest, finalized.raw_output_identity());
        hash_content_identity(digest, finalized.finalized_output_identity());
    });
    let finalization = FinalizationIdentityV1::from_bytes(finalization);
    let upstream = hash_identity(FINALIZED_UPSTREAM_IDENTITY_DOMAIN_V1, |digest| {
        update_inspection_schema_binding(digest, &view.schema);
        digest.update(finalized.identity().as_bytes());
        digest.update(finalization.as_bytes());
    });
    let upstream = UpstreamCodeObjectEvidenceIdentityV1::from_bytes(upstream);
    let output = PublicationOutputBindingV2 {
        exact_output: exact_bytes,
        finalized_output: finalized.finalized_output_identity(),
        finalization,
        canonical_finalization: Some(*finalized.identity().as_bytes()),
    };
    let plan =
        derive_publication_plan_shared_v2(producer_package, &view, &output, FINALIZED_DOMAINS_V1)?;

    Ok(PreparedFinalizedWorkerV2HsacoPublicationV1 {
        finalized,
        producer_package,
        plan,
        upstream,
    })
}

/// Consumes protected raw inspection and derives inert V2-domain restart inputs.
///
/// No V1 publication API is called. The returned values are intended for V2 intent persistence
/// and the V2 attempt-scoped publication protocol supplied by `#203`.
pub fn prepare_protected_worker_v2_hsaco_publication_v2(
    producer: &ProducerIdentity,
    inspected: InspectedProtectedRawWorkerV2HsacoV1,
) -> Result<PreparedProtectedWorkerV2HsacoPublicationV2, WorkerV2HsacoPublicationError> {
    if !inspected
        .linked_output_identity()
        .matches(inspected.exact_bytes())
    {
        return Err(WorkerV2HsacoPublicationError::OutputIdentityMismatch);
    }

    let producer_package = producer_package_identity_v1(producer);
    let view = PublicationInspectionViewV2::protected(&inspected);
    let finalization = hash_identity(PROTECTED_RAW_FINALIZATION_IDENTITY_DOMAIN_V2, |digest| {
        update_all_protected_schema_bindings(digest, &view.schema);
    });
    let finalization = FinalizationIdentityV1::from_bytes(finalization);
    let upstream = hash_identity(PROTECTED_RAW_UPSTREAM_IDENTITY_DOMAIN_V2, |digest| {
        update_all_protected_schema_bindings(digest, &view.schema);
        digest.update(finalization.as_bytes());
    });
    let upstream = UpstreamCodeObjectEvidenceIdentityV1::from_bytes(upstream);
    let output = PublicationOutputBindingV2 {
        exact_output: inspected.exact_bytes(),
        finalized_output: inspected.linked_output_identity(),
        finalization,
        canonical_finalization: None,
    };
    let plan = derive_publication_plan_shared_v2(
        producer_package,
        &view,
        &output,
        PROTECTED_RAW_DOMAINS_V2,
    )?;
    Ok(PreparedProtectedWorkerV2HsacoPublicationV2 {
        inspected,
        plan,
        upstream,
    })
}

/// Consumes protected canonical finalization and derives exact finalized restart inputs.
///
/// The linked-output field continues to identify the raw worker result while finalized-output and
/// exact retained bytes identify the canonical snapshot. This function performs no publication.
pub fn prepare_finalized_protected_worker_v2_hsaco_publication_v2(
    producer: &ProducerIdentity,
    finalized: PreparedFinalizedProtectedWorkerV2HsacoV2,
) -> Result<PreparedFinalizedProtectedWorkerV2HsacoPublicationV2, WorkerV2HsacoPublicationError> {
    let raw = finalized.raw_inspection();
    if !finalized.raw_output_identity().matches(raw.exact_bytes()) {
        return Err(WorkerV2HsacoPublicationError::RawOutputIdentityMismatch);
    }
    if !finalized
        .finalized_output_identity()
        .matches(finalized.exact_finalized_bytes())
    {
        return Err(WorkerV2HsacoPublicationError::FinalizedOutputIdentityMismatch);
    }

    let producer_package = producer_package_identity_v1(producer);
    let view = PublicationInspectionViewV2::protected(raw);
    let protected_finalization = *finalized.identity().as_bytes();
    let finalization = hash_identity(
        PROTECTED_FINALIZED_FINALIZATION_IDENTITY_DOMAIN_V2,
        |digest| {
            update_all_protected_schema_bindings(digest, &view.schema);
            digest.update(hash_identity(
                PROTECTED_CANONICAL_FINALIZATION_BINDING_DOMAIN_V2,
                |component| component.update(protected_finalization),
            ));
            digest.update(finalized.canonical_digest().as_bytes());
            hash_content_identity(digest, finalized.raw_output_identity());
            hash_content_identity(digest, finalized.finalized_output_identity());
        },
    );
    let finalization = FinalizationIdentityV1::from_bytes(finalization);
    let upstream = hash_identity(PROTECTED_FINALIZED_UPSTREAM_IDENTITY_DOMAIN_V2, |digest| {
        update_all_protected_schema_bindings(digest, &view.schema);
        digest.update(hash_identity(
            PROTECTED_CANONICAL_FINALIZATION_BINDING_DOMAIN_V2,
            |component| component.update(protected_finalization),
        ));
        digest.update(finalization.as_bytes());
    });
    let upstream = UpstreamCodeObjectEvidenceIdentityV1::from_bytes(upstream);
    let output = PublicationOutputBindingV2 {
        exact_output: finalized.exact_finalized_bytes(),
        finalized_output: finalized.finalized_output_identity(),
        finalization,
        canonical_finalization: Some(protected_finalization),
    };
    let plan = derive_publication_plan_shared_v2(
        producer_package,
        &view,
        &output,
        PROTECTED_FINALIZED_DOMAINS_V2,
    )?;
    Ok(PreparedFinalizedProtectedWorkerV2HsacoPublicationV2 {
        finalized,
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

/// Publishes only the exact canonical bytes retained by the finalized prepared intent.
pub fn publish_prepared_finalized_worker_v2_hsaco_v1(
    output_dir: &Path,
    producer: &ProducerIdentity,
    prepared: &PreparedFinalizedWorkerV2HsacoPublicationV1,
) -> Result<AttemptScopedHsacoPublicationResultV1, WorkerV2HsacoPublicationError> {
    if producer_package_identity_v1(producer) != prepared.producer_package {
        return Err(WorkerV2HsacoPublicationError::ProducerIdentityMismatch);
    }
    let raw = prepared.finalized.raw_inspection();
    if !prepared
        .finalized
        .raw_output_identity()
        .matches(raw.exact_bytes())
        || prepared.plan.linked_output().as_bytes()
            != prepared.finalized.raw_output_identity().sha256()
    {
        return Err(WorkerV2HsacoPublicationError::RawOutputIdentityMismatch);
    }
    let exact_bytes = prepared.finalized.exact_finalized_bytes();
    let output_digest: [u8; 32] = Sha256::digest(exact_bytes).into();
    if !prepared
        .finalized
        .finalized_output_identity()
        .matches(exact_bytes)
        || &output_digest != prepared.plan.finalized_output().as_bytes()
    {
        return Err(WorkerV2HsacoPublicationError::FinalizedOutputIdentityMismatch);
    }

    publish_exact_hsaco_evidence_for_attempt_v1(
        output_dir,
        producer,
        prepared.finalized.attempt(),
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

fn hash_content_identity(digest: &mut Sha256, identity: crate::ContentIdentityV1) {
    digest.update(identity.sha256());
    digest.update(identity.byte_len().to_le_bytes());
}

fn hash_compiler_closure_v2(digest: &mut Sha256, closure: CompilerClosureV2) {
    digest.update(
        closure
            .cargo_binding_transition_protocol_version()
            .to_le_bytes(),
    );
    digest.update(closure.cargo_executable_sha256());
    digest.update(closure.cargo_binding_trampoline_sha256());
    digest.update(closure.cargo_fe2o3_binding_wrapper_sha256());
    digest.update(closure.rustc_executable_sha256());
    digest.update(closure.rustc_runtime_tree_sha256());
    digest.update(closure.codegen_backend_sha256());
    digest.update(closure.identity_sha256());
}

const fn code_object_version_tag(version: CodeObjectVersion) -> u8 {
    match version {
        CodeObjectVersion::V4 => 4,
        CodeObjectVersion::V5 => 5,
        CodeObjectVersion::V6 => 6,
    }
}
