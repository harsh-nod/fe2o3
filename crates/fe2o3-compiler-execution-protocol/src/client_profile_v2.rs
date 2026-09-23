//! Native trust inputs; no service activation or fallback to a V1 profile.
use crate::{
    CompilerExecutionAttestationErrorV2 as PolicyError,
    CompilerExecutionClientProfileErrorV1 as FramingError,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerPolicyV2 as Policy,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV2 as Storage},
    client_profile_codec as codec, issuer_policy_codec, issuer_policy_v2,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{error::Error, fmt, mem::size_of};

pub const COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V2: usize = codec::BYTES;
const RETAINED: usize = size_of::<CompilerExecutionClientProfileV2>() + size_of::<Storage>();
/// Prepaid logical work includes the complete nested native policy decoder.
pub const COMPILER_EXECUTION_CLIENT_PROFILE_WORK_V2: usize =
    issuer_policy_v2::COMPILER_EXECUTION_ISSUER_POLICY_WORK_V2 + 32 * codec::BYTES;
/// Fixed additional logical peak, including nested policy staging. Not RSS,
/// heap-allocation or generated stack accounting. Inputs stay separately paid.
pub const COMPILER_EXECUTION_CLIENT_PROFILE_STORAGE_V2: usize =
    issuer_policy_v2::COMPILER_EXECUTION_ISSUER_POLICY_STORAGE_V2
        + 4 * RETAINED
        + 4 * codec::BYTES
        + 2 * size_of::<sha2::Sha256>()
        + 4096;
const _: () = {
    assert!(RETAINED >= issuer_policy_v2::RETAINED);
    assert!(size_of::<(CompilerExecutionClientProfileV2, Storage)>() <= RETAINED);
    assert!(
        8 * size_of::<CompilerExecutionClientProfileErrorV2>() + 64 * size_of::<usize>() <= 4096
    );
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerExecutionClientProfileIdentityV2([u8; 32]);
impl CompilerExecutionClientProfileIdentityV2 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
    /// Fixed-wire identity comparison, not provisioning or policy pinning.
    pub fn matches_canonical_bytes(self, bytes: &[u8], budget: &mut Budget<'_>) -> Result<bool> {
        metered(
            budget,
            resources::fixed_input_floor(bytes, codec::BYTES),
            || Ok(codec::V2.matches(self.0, bytes)),
        )
    }
}

/// Move-only supervisor/anchor credentials and a nominal native issuer policy.
/// This public configuration grants no signing, compiler, load or launch rights.
/// All operations restore the caller's entry storage, including on failure.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::CompilerExecutionClientProfileV2;
/// fn duplicate(profile: CompilerExecutionClientProfileV2) { let _ = profile.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::{CompilerExecutionClientProfileV2, CompilerExecutionIssuerPolicyV1,
///     CompilerExecutionExternalAnchorServiceIdentityV1};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// fn mix(policy: CompilerExecutionIssuerPolicyV1, service: CompilerExecutionExternalAnchorServiceIdentityV1,
///        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let _ = CompilerExecutionClientProfileV2::new(1000, 1000, service, policy, budget);
/// }
/// ```
#[derive(Eq, PartialEq)]
pub struct CompilerExecutionClientProfileV2 {
    supervisor_uid: u32,
    supervisor_gid: u32,
    external_anchor_service: Service,
    policy: Policy,
    identity: CompilerExecutionClientProfileIdentityV2,
    canonical_bytes: [u8; codec::BYTES],
}
impl CompilerExecutionClientProfileV2 {
    /// Consumes a prepaid policy. Retain its reservation and reserve only the
    /// returned delta. On failure the policy drops; retire its reservation then.
    pub fn new(
        supervisor_uid: u32,
        supervisor_gid: u32,
        external_anchor_service: Service,
        policy: Policy,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        metered(budget, policy.retained_storage(), || {
            codec::validate_credentials(supervisor_uid, supervisor_gid)?;
            Ok((
                Self::from_policy(
                    supervisor_uid,
                    supervisor_gid,
                    external_anchor_service,
                    policy,
                ),
                Storage(RETAINED - issuer_policy_v2::RETAINED),
            ))
        })
    }

    /// Borrows an already prepaid exact wire owner. Unlike `new`, this returns
    /// the FULL retained charge: the caller transferred no nested reservation.
    /// The fixed aggregate quota prepays nested decoding on this same ledger.
    pub fn decode(bytes: &[u8], budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        metered(
            budget,
            resources::fixed_input_floor(bytes, codec::BYTES),
            || {
                let parsed = codec::V2.parse(bytes)?;
                let policy = Policy {
                    record: issuer_policy_codec::V2
                        .decode(parsed.policy)
                        .map_err(PolicyError::from)?,
                };
                codec::V2.check_identity(bytes)?;
                let decoded = Self::from_policy(parsed.uid, parsed.gid, parsed.service, policy);
                if decoded.canonical_bytes.as_slice() != bytes {
                    return Err(FramingError::Canonical.into());
                }
                Ok((decoded, Storage(RETAINED)))
            },
        )
    }

    fn from_policy(uid: u32, gid: u32, service: Service, policy: Policy) -> Self {
        let (canonical_bytes, identity) =
            codec::V2.encode(uid, gid, service, policy.canonical_bytes());
        Self {
            supervisor_uid: uid,
            supervisor_gid: gid,
            external_anchor_service: service,
            policy,
            identity: CompilerExecutionClientProfileIdentityV2(identity),
            canonical_bytes,
        }
    }
    pub const fn supervisor_uid(&self) -> u32 {
        self.supervisor_uid
    }
    pub const fn supervisor_gid(&self) -> u32 {
        self.supervisor_gid
    }
    pub const fn external_anchor_service(&self) -> Service {
        self.external_anchor_service
    }
    pub const fn policy(&self) -> &Policy {
        &self.policy
    }
    pub const fn identity(&self) -> CompilerExecutionClientProfileIdentityV2 {
        self.identity
    }
    pub const fn canonical_bytes(&self) -> &[u8; codec::BYTES] {
        &self.canonical_bytes
    }
    /// Total fixed reservation, including the nested policy and descriptor.
    pub const fn retained_storage(&self) -> usize {
        RETAINED
    }
}
impl fmt::Debug for CompilerExecutionClientProfileV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompilerExecutionClientProfileV2")
            .field("supervisor_uid", &self.supervisor_uid)
            .field("supervisor_gid", &self.supervisor_gid)
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub enum CompilerExecutionClientProfileErrorV2 {
    Framing(FramingError),
    Policy(PolicyError),
    Resource(Resource),
}
type Result<T> = std::result::Result<T, CompilerExecutionClientProfileErrorV2>;
impl From<FramingError> for CompilerExecutionClientProfileErrorV2 {
    fn from(value: FramingError) -> Self {
        Self::Framing(value)
    }
}
impl From<PolicyError> for CompilerExecutionClientProfileErrorV2 {
    fn from(value: PolicyError) -> Self {
        Self::Policy(value)
    }
}
impl From<Resource> for CompilerExecutionClientProfileErrorV2 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for CompilerExecutionClientProfileErrorV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Framing(e) => e.fmt(f),
            Self::Policy(e) => e.fmt(f),
            Self::Resource(e) => e.fmt(f),
        }
    }
}
impl Error for CompilerExecutionClientProfileErrorV2 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(match self {
            Self::Framing(e) => e,
            Self::Policy(e) => e,
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
        COMPILER_EXECUTION_CLIENT_PROFILE_WORK_V2,
        COMPILER_EXECUTION_CLIENT_PROFILE_STORAGE_V2,
        operation,
    )
}
