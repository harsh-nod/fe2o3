//! Native ownership and metering over the existing identity-only launch wire.
use crate::{
    CompilerExecutionClientProcessIdentityV1 as Client,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerPolicyIdentityV2 as PolicyIdentity,
    CompilerExecutionIssuerPolicyV2 as Policy,
    CompilerExecutionServiceLaunchManifestErrorV1 as Framing,
    CompilerExecutionServiceLaunchManifestIdentityV1 as Identity,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV2 as Storage},
    launch_manifest_codec as codec,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{error::Error, fmt, mem::size_of};

/// Native API size; the identity-only wire remains version 1.
pub const COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_BYTES_V2: usize = codec::BYTES;
const RETAINED: usize = size_of::<(CompilerExecutionServiceLaunchManifestV2, Storage)>();
/// Logical work for fixed framing, encoding, hashing and comparison, not instructions.
pub const COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_WORK_V2: usize =
    resources::ENTRY_WORK + 32 * codec::BYTES;
/// Additional logical scratch. Inputs remain separately prepaid. Not an RSS,
/// allocator, generated-stack or wall-time bound.
pub const COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_STORAGE_V2: usize =
    4 * RETAINED + 4 * codec::BYTES + 2 * size_of::<sha2::Sha256>() + 4096;

const _: () = {
    assert!(
        8 * size_of::<CompilerExecutionServiceLaunchManifestErrorV2>() + 64 * size_of::<usize>()
            <= 4096
    );
};

/// Move-only inert launch frame for native consumers. No process, signing,
/// protected-readiness, publication, load or launch authority.
///
/// The unchanged 112-byte wire contains opaque identities. Structural decoding
/// therefore also accepts a legacy-bound frame; it does NOT admit its policy as
/// V2. Consumers must match an independently pinned PolicyV2 before use. There
/// is no conversion from an admitted V1 policy or launch owner.
///
/// All borrowed owners stay prepaid on the same ledger. Operations restore
/// entry storage; reserve the returned additional storage before retention.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::CompilerExecutionServiceLaunchManifestV2;
/// fn duplicate(value: CompilerExecutionServiceLaunchManifestV2) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::{CompilerExecutionServiceLaunchManifestV1,
///     CompilerExecutionServiceLaunchManifestV2};
/// fn legacy(_: &CompilerExecutionServiceLaunchManifestV1) {}
/// fn mix(value: &CompilerExecutionServiceLaunchManifestV2) { legacy(value); }
/// ```
#[derive(Eq, PartialEq)]
pub struct CompilerExecutionServiceLaunchManifestV2 {
    record: codec::Record,
}

impl CompilerExecutionServiceLaunchManifestV2 {
    pub fn new(
        client: Client,
        service: Service,
        policy: &Policy,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        metered(budget, policy.retained_storage(), || {
            Ok((
                Self {
                    record: codec::encode(client, service, *policy.identity().as_bytes()),
                },
                Storage(RETAINED),
            ))
        })
    }

    /// Strict shared-wire framing only. A native contextual policy match is still required.
    pub fn decode(bytes: &[u8], budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        metered(
            budget,
            resources::fixed_input_floor(bytes, codec::BYTES),
            || {
                Ok((
                    Self {
                        record: codec::decode(bytes)?,
                    },
                    Storage(RETAINED),
                ))
            },
        )
    }

    pub fn matches_policy(&self, policy: &Policy, budget: &mut Budget<'_>) -> Result<bool> {
        metered(budget, RETAINED + policy.retained_storage(), || {
            Ok(&self.record.policy == policy.identity().as_bytes())
        })
    }

    pub const fn client(&self) -> Client {
        self.record.client
    }
    pub const fn external_anchor_service(&self) -> Service {
        self.record.service
    }
    /// Opaque identity interpreted by this native API; not PolicyV2 admission.
    pub const fn policy_identity(&self) -> PolicyIdentity {
        PolicyIdentity::from_bytes_for_protocol(self.record.policy)
    }
    /// Wire identity keeps the existing framing domain and identity type.
    pub const fn identity(&self) -> Identity {
        Identity::from_bytes_for_protocol(self.record.identity)
    }
    pub const fn canonical_bytes(&self) -> &[u8; codec::BYTES] {
        &self.record.bytes
    }
    pub const fn retained_storage(&self) -> usize {
        RETAINED
    }
}

impl fmt::Debug for CompilerExecutionServiceLaunchManifestV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompilerExecutionServiceLaunchManifestV2")
            .field("authority", &"none")
            .field("client", &self.client())
            .field("identity", &self.identity())
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub enum CompilerExecutionServiceLaunchManifestErrorV2 {
    Framing(Framing),
    Resource(Resource),
}
type Result<T> = std::result::Result<T, CompilerExecutionServiceLaunchManifestErrorV2>;
impl From<Framing> for CompilerExecutionServiceLaunchManifestErrorV2 {
    fn from(value: Framing) -> Self {
        Self::Framing(value)
    }
}
impl From<Resource> for CompilerExecutionServiceLaunchManifestErrorV2 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for CompilerExecutionServiceLaunchManifestErrorV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Framing(e) => e.fmt(f),
            Self::Resource(e) => e.fmt(f),
        }
    }
}
impl Error for CompilerExecutionServiceLaunchManifestErrorV2 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(match self {
            Self::Framing(e) => e,
            Self::Resource(e) => e,
        })
    }
}

fn metered<T>(
    budget: &mut Budget<'_>,
    floor: usize,
    operation: impl FnOnce() -> Result<T>,
) -> Result<T> {
    resources::fixed(
        budget,
        floor,
        COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_WORK_V2,
        COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_STORAGE_V2,
        operation,
    )
}
