//! Typed native artifact owners; shared storage never supplies a public downgrade.
use super::{
    CanonicalCodeObjectDigest, ContentIdentityV1, InertNativeFirstBuildWorkerEvidenceV1, Result,
    SharedWorkerV3HsacoInspectionV1, WorkerV3HsacoPolicyV1, failure,
};
use std::mem::size_of;

/// Domain-separated binding of retained native source/Worker evidence, independent
/// raw inspection and the exact finalized descriptor/artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeWorkerFinalizationIdentityV1([u8; 32]);
impl NativeWorkerFinalizationIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Inert V4 binding; distinct from the legacy native finalization domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeWorkerFinalizationIdentityV4([u8; 32]);
impl NativeWorkerFinalizationIdentityV4 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Structural finalization retaining the complete native source/F owner and its
/// fresh-consumption or independently recovered transcript custody distinction.
/// This supplies no protected origin, currentness, proof, publication or launch
/// authority. No V3 outer owner is manufactured from the native carrier.
///
/// ```compile_fail
/// use fe2o3_hsaco_finalize::PreparedFinalizedNativeWorkerHsacoV1 as Finalized;
/// fn duplicate(value: Finalized) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::PreparedFinalizedNativeWorkerHsacoV1 as Finalized;
/// fn forge() -> Finalized { Finalized::default() }
/// ```
pub struct PreparedFinalizedNativeWorkerHsacoV1 {
    pub(super) core: NativeFinalizedCore,
}

/// Move-only descriptor V4 continuation retaining the complete native source/F
/// and its original custody kind. No conditional proof receipt is manufactured.
/// There is no conversion to a legacy native/Worker owner or publication gate.
/// This is a structural prerequisite, not #272 closure. Current upstream native
/// semantic recovery still requires a V1 descriptor; it is not bypassed here.
///
/// ```compile_fail
/// use fe2o3_hsaco_finalize::PreparedFinalizedNativeWorkerHsacoV4 as Finalized;
/// fn duplicate(value: Finalized) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::PreparedFinalizedNativeWorkerHsacoV4 as Finalized;
/// fn forge() -> Finalized { Finalized::default() }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::{PreparedFinalizedNativeWorkerHsacoV1, PreparedFinalizedNativeWorkerHsacoV4};
/// fn downgrade(value: PreparedFinalizedNativeWorkerHsacoV4) -> PreparedFinalizedNativeWorkerHsacoV1 { value.into() }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::{PreparedFinalizedNativeWorkerHsacoV1, PreparedFinalizedNativeWorkerHsacoV4};
/// fn rewrap(value: PreparedFinalizedNativeWorkerHsacoV4) -> PreparedFinalizedNativeWorkerHsacoV1 {
///     PreparedFinalizedNativeWorkerHsacoV1 { core: value.core }
/// }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::PreparedFinalizedNativeWorkerHsacoV4;
/// fn select(value: PreparedFinalizedNativeWorkerHsacoV4) {
///     use fe2o3_hsaco_finalize::native_worker_finalization::{NativeDescriptorMode, finalize_native_worker_core};
///     let _ = NativeDescriptorMode::Legacy;
/// }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::{PreparedFinalizedNativeWorkerHsacoV4, prepare_native_worker_hsaco_publication_v1};
/// use fe2o3_artifact_transaction::ProducerIdentity;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// fn publish(value: PreparedFinalizedNativeWorkerHsacoV4, producer: &ProducerIdentity, budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     prepare_native_worker_hsaco_publication_v1(producer, value, budget);
/// }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::{PreparedFinalizedNativeWorkerHsacoV4, prepare_nominal_worker_publication_v3};
/// use fe2o3_artifact_transaction::ProducerIdentity;
/// fn publish(value: PreparedFinalizedNativeWorkerHsacoV4, producer: &ProducerIdentity) {
///     prepare_nominal_worker_publication_v3(producer, value);
/// }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::{PreparedFinalizedNativeWorkerHsacoV4, prepare_protected_worker_v3_hsaco_publication_v1};
/// use fe2o3_artifact_transaction::ProducerIdentity;
/// fn publish(value: PreparedFinalizedNativeWorkerHsacoV4, producer: &ProducerIdentity) {
///     prepare_protected_worker_v3_hsaco_publication_v1(producer, value);
/// }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::{PreparedFinalizedNativeWorkerHsacoV4, PreparedFinalizedNominalWorkerHsacoV4};
/// fn change_source(value: PreparedFinalizedNativeWorkerHsacoV4) -> PreparedFinalizedNominalWorkerHsacoV4 { value.into() }
/// ```
pub struct PreparedFinalizedNativeWorkerHsacoV4 {
    pub(super) core: NativeFinalizedCore,
}

// Only the typed finalization/replay entrypoints wrap this shared inert storage.
pub(super) struct NativeFinalizedCore {
    pub(super) source: InertNativeFirstBuildWorkerEvidenceV1,
    pub(super) inspection: SharedWorkerV3HsacoInspectionV1,
    pub(super) bytes: Vec<u8>,
    pub(super) descriptor: Vec<u8>,
    pub(super) digest: CanonicalCodeObjectDigest,
    pub(super) identity: [u8; 32],
    pub(super) output_identity: ContentIdentityV1,
    pub(super) descriptor_identity: ContentIdentityV1,
    pub(super) retained_storage: usize,
}
// Identical read-only observations do not provide conversion between owners.
macro_rules! native_owner_accessors {
    ($owner:ident, $identity:ident) => {
        impl $owner {
            pub fn source_evidence(&self) -> &InertNativeFirstBuildWorkerEvidenceV1 {
                &self.core.source
            }
            pub fn exact_finalized_bytes(&self) -> &[u8] {
                &self.core.bytes
            }
            pub fn descriptor_bytes(&self) -> &[u8] {
                &self.core.descriptor
            }
            pub const fn canonical_digest(&self) -> CanonicalCodeObjectDigest {
                self.core.digest
            }
            pub const fn identity(&self) -> $identity {
                $identity(self.core.identity)
            }
            pub const fn output_identity(&self) -> ContentIdentityV1 {
                self.core.output_identity
            }
            pub const fn descriptor_identity(&self) -> ContentIdentityV1 {
                self.core.descriptor_identity
            }
            pub fn raw_policy(&self) -> &WorkerV3HsacoPolicyV1 {
                &self.core.inspection.policy
            }
            /// Complete native ledger floor, excluding the separately bounded artifact domain.
            pub const fn required_retained_storage(&self) -> usize {
                self.core.retained_storage
            }
            pub const fn is_structural_only(&self) -> bool {
                true
            }
            pub const fn grants_compiler_authority(&self) -> bool {
                false
            }
            pub const fn grants_proof_authority(&self) -> bool {
                false
            }
            pub const fn authenticates_compiler_origin(&self) -> bool {
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
    };
}
native_owner_accessors!(
    PreparedFinalizedNativeWorkerHsacoV1,
    NativeWorkerFinalizationIdentityV1
);
native_owner_accessors!(
    PreparedFinalizedNativeWorkerHsacoV4,
    NativeWorkerFinalizationIdentityV4
);

// Both wrappers retain exactly the same header; the existing resource schedule
// must cover either typed result without adding an uncharged allocation.
const _: () = assert!(
    size_of::<PreparedFinalizedNativeWorkerHsacoV1>()
        == size_of::<PreparedFinalizedNativeWorkerHsacoV4>()
);

pub(crate) enum NativeFinalizedOwner {
    Legacy(PreparedFinalizedNativeWorkerHsacoV1),
    V4(PreparedFinalizedNativeWorkerHsacoV4),
}
impl NativeFinalizedOwner {
    pub(crate) fn view(&self) -> NativeFinalizedRef<'_> {
        match self {
            Self::Legacy(value) => NativeFinalizedRef::Legacy(value),
            Self::V4(value) => NativeFinalizedRef::V4(value),
        }
    }
    pub(crate) fn into_legacy(self) -> Result<PreparedFinalizedNativeWorkerHsacoV1> {
        match self {
            Self::Legacy(value) => Ok(value),
            Self::V4(_) => Err(failure(
                "descriptor schema",
                "V4 cannot enter legacy native custody",
            )),
        }
    }
    pub(crate) fn into_v4(self) -> Result<PreparedFinalizedNativeWorkerHsacoV4> {
        match self {
            Self::V4(value) => Ok(value),
            Self::Legacy(_) => Err(failure(
                "descriptor schema",
                "legacy native custody is not V4",
            )),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum NativeFinalizedRef<'a> {
    Legacy(&'a PreparedFinalizedNativeWorkerHsacoV1),
    V4(&'a PreparedFinalizedNativeWorkerHsacoV4),
}
impl<'a> NativeFinalizedRef<'a> {
    fn core(self) -> &'a NativeFinalizedCore {
        match self {
            Self::Legacy(value) => &value.core,
            Self::V4(value) => &value.core,
        }
    }
    pub(crate) fn source_evidence(self) -> &'a InertNativeFirstBuildWorkerEvidenceV1 {
        &self.core().source
    }
    pub(crate) fn exact_finalized_bytes(self) -> &'a [u8] {
        &self.core().bytes
    }
    pub(crate) fn identity(self) -> &'a [u8; 32] {
        &self.core().identity
    }
    pub(crate) fn required_retained_storage(self) -> usize {
        self.core().retained_storage
    }
}
