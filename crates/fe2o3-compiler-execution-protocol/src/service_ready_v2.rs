//! Native ownership and metering over the existing inert readiness wire.
use crate::{
    CompilerExecutionIssuerPolicyIdentityV2 as PolicyIdentity,
    CompilerExecutionIssuerPolicyV2 as Policy,
    CompilerExecutionServiceLaunchManifestIdentityV1 as ManifestIdentity,
    CompilerExecutionServiceLaunchManifestV2 as Manifest,
    CompilerExecutionServiceReadyErrorV1 as Framing,
    CompilerExecutionServiceReadyIdentityV1 as Identity,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV2 as Storage},
    service_ready_codec as codec,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{error::Error, fmt, mem::size_of};

/// Native API size; the identity-only readiness wire remains version 1.
pub const COMPILER_EXECUTION_SERVICE_READY_BYTES_V2: usize = codec::BYTES;
const RETAINED: usize = size_of::<(CompilerExecutionServiceReadyV2, Storage)>();
/// Logical work for entry, fixed framing, hashing, reencoding and comparison.
/// This is not an instruction or wall-time bound.
pub const COMPILER_EXECUTION_SERVICE_READY_WORK_V2: usize =
    resources::ENTRY_WORK + 32 * codec::BYTES;
/// Additional logical scratch. Borrowed inputs remain separately prepaid.
/// This is not an allocator, generated-stack or RSS bound.
pub const COMPILER_EXECUTION_SERVICE_READY_STORAGE_V2: usize =
    4 * RETAINED + 4 * codec::BYTES + 2 * size_of::<sha2::Sha256>() + 4096;

const _: () = {
    assert!(
        8 * size_of::<CompilerExecutionServiceReadyErrorV2>() + 64 * size_of::<usize>() <= 4096
    );
};

/// Move-only inert readiness framing for native consumers. Construction,
/// decoding and matching grant no process, signing, compiler, launch,
/// publication, loading or execution authority, and prove no admission or
/// recovery. Private-channel provenance and child liveness remain supervisor
/// obligations.
///
/// The unchanged 120-byte wire contains opaque identities, so structural
/// decoding also accepts legacy-bound frames. Consumers must match the exact
/// native manifest and independently pinned PolicyV2 before use. There is no
/// conversion from an admitted V1 owner and no fallback policy admission.
///
/// Keep complete borrowed owners prepaid on the original caller ledger.
/// Operations restore entry storage on success, error and unwind. Reserve the
/// returned additional storage before retaining the owner, then release its
/// `retained_storage()` when it is retired.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::CompilerExecutionServiceReadyV2;
/// fn duplicate(value: CompilerExecutionServiceReadyV2) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::CompilerExecutionServiceReadyV2;
/// fn requires_copy<T: Copy>() {}
/// requires_copy::<CompilerExecutionServiceReadyV2>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::{CompilerExecutionServiceReadyV1,
///     CompilerExecutionServiceReadyV2};
/// fn legacy(_: &CompilerExecutionServiceReadyV1) {}
/// fn mix(value: &CompilerExecutionServiceReadyV2) { legacy(value); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::{CompilerExecutionServiceReadyV1,
///     CompilerExecutionServiceReadyV2};
/// fn upgrade(value: CompilerExecutionServiceReadyV1) -> CompilerExecutionServiceReadyV2 {
///     value.into()
/// }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::{CompilerExecutionServiceReadyV1,
///     CompilerExecutionServiceReadyV2};
/// fn downgrade(value: CompilerExecutionServiceReadyV2) -> CompilerExecutionServiceReadyV1 {
///     value.into()
/// }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::{CompilerExecutionIssuerPolicyV1,
///     CompilerExecutionServiceLaunchManifestV2, CompilerExecutionServiceReadyV2};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn mix(m: &CompilerExecutionServiceLaunchManifestV2,
///        p: &CompilerExecutionIssuerPolicyV1, b: &mut Budget<'_>) {
///     let _ = CompilerExecutionServiceReadyV2::new(1, m, p, b);
/// }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::{CompilerExecutionIssuerPolicyV2,
///     CompilerExecutionServiceLaunchManifestV1, CompilerExecutionServiceReadyV2};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn mix(m: &CompilerExecutionServiceLaunchManifestV1,
///        p: &CompilerExecutionIssuerPolicyV2, b: &mut Budget<'_>) {
///     let _ = CompilerExecutionServiceReadyV2::new(1, m, p, b);
/// }
/// ```
#[derive(Eq, PartialEq)]
pub struct CompilerExecutionServiceReadyV2 {
    record: codec::Record,
}

impl CompilerExecutionServiceReadyV2 {
    pub fn new(
        issuer_pid: u32,
        launch: &Manifest,
        policy: &Policy,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        metered(
            budget,
            launch.retained_storage() + policy.retained_storage(),
            || {
                if issuer_pid == 0 {
                    return Err(Framing::IssuerPid.into());
                }
                if launch.policy_identity() != policy.identity() {
                    return Err(Framing::PolicyMismatch.into());
                }
                Ok((
                    Self {
                        record: codec::encode(
                            issuer_pid,
                            *launch.identity().as_bytes(),
                            *policy.identity().as_bytes(),
                        ),
                    },
                    Storage(RETAINED),
                ))
            },
        )
    }

    /// Strict shared-wire decode and independent reencode. Native contextual
    /// matching and protected readiness admission remain caller obligations.
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

    /// Requires this PID, both exact identities and the native manifest's
    /// binding to the supplied policy. Even a mismatch is prepaid in full.
    pub fn matches_launch(
        &self,
        issuer_pid: u32,
        launch: &Manifest,
        policy: &Policy,
        budget: &mut Budget<'_>,
    ) -> Result<bool> {
        metered(
            budget,
            RETAINED + launch.retained_storage() + policy.retained_storage(),
            || {
                Ok(self.record.issuer_pid == issuer_pid
                    && &self.record.launch_manifest == launch.identity().as_bytes()
                    && &self.record.policy == policy.identity().as_bytes()
                    && launch.policy_identity() == policy.identity())
            },
        )
    }

    pub const fn issuer_pid(&self) -> u32 {
        self.record.issuer_pid
    }
    pub const fn launch_manifest_identity(&self) -> ManifestIdentity {
        ManifestIdentity::from_bytes_for_protocol(self.record.launch_manifest)
    }
    /// Opaque identity interpreted by this native API; not PolicyV2 admission.
    pub const fn policy_identity(&self) -> PolicyIdentity {
        PolicyIdentity::from_bytes_for_protocol(self.record.policy)
    }
    /// Wire identity retains the existing framing domain and identity type.
    pub const fn identity(&self) -> Identity {
        Identity::from_bytes_for_protocol(self.record.identity)
    }
    pub const fn canonical_bytes(&self) -> &[u8; COMPILER_EXECUTION_SERVICE_READY_BYTES_V2] {
        &self.record.bytes
    }
    pub const fn retained_storage(&self) -> usize {
        RETAINED
    }
}

impl fmt::Debug for CompilerExecutionServiceReadyV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompilerExecutionServiceReadyV2")
            .field("authority", &"none")
            .field("issuer_pid", &self.issuer_pid())
            .field("identity", &self.identity())
            .finish_non_exhaustive()
    }
}

/// Bounded framing and resource diagnostics; no V1 owner admission.
#[derive(Debug)]
pub enum CompilerExecutionServiceReadyErrorV2 {
    Framing(Framing),
    Resource(Resource),
}
type Result<T> = std::result::Result<T, CompilerExecutionServiceReadyErrorV2>;
impl From<Framing> for CompilerExecutionServiceReadyErrorV2 {
    fn from(value: Framing) -> Self {
        Self::Framing(value)
    }
}
impl From<Resource> for CompilerExecutionServiceReadyErrorV2 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for CompilerExecutionServiceReadyErrorV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Framing(e) => e.fmt(f),
            Self::Resource(e) => e.fmt(f),
        }
    }
}
impl Error for CompilerExecutionServiceReadyErrorV2 {
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
        COMPILER_EXECUTION_SERVICE_READY_WORK_V2,
        COMPILER_EXECUTION_SERVICE_READY_STORAGE_V2,
        operation,
    )
}

#[cfg(test)]
#[path = "service_ready_v2_tests.rs"]
mod tests;
