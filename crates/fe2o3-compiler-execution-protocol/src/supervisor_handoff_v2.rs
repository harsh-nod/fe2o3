//! Metered native ownership over the shared, identity-only supervisor handoff.
use crate::{
    CompilerExecutionClientProcessIdentityV1 as Client,
    CompilerExecutionServiceLaunchManifestV2 as Launch,
    CompilerExecutionSupervisorHandoffErrorV1 as Framing,
    CompilerExecutionSupervisorHandoffIdentityV1 as Identity,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV2 as Storage},
    launch_manifest_codec, supervisor_handoff_codec as codec,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{error::Error, fmt, mem::size_of};

/// Native API size; the 184-byte identity-only wire and hash domain remain V1.
pub const COMPILER_EXECUTION_SUPERVISOR_HANDOFF_BYTES_V2: usize = codec::BYTES;
const RETAINED: usize = size_of::<(CompilerExecutionSupervisorHandoffV2, Storage)>();
/// Complete logical quota for both handoff and nested launch framing, hashing,
/// re-encoding and comparison: 9,480 units. One entry charge, no nested budget.
/// This is not a bound on generated instructions or elapsed time.
pub const COMPILER_EXECUTION_SUPERVISOR_HANDOFF_WORK_V2: usize =
    resources::ENTRY_WORK + 32 * (codec::BYTES + launch_manifest_codec::BYTES);
/// Additional fixed scratch above all prepaid live inputs. Covers owner/record
/// moves, both fixed wires, SHA state and guarded scalar/result controls. Not an
/// allocator, generated-stack or process-RSS bound.
pub const COMPILER_EXECUTION_SUPERVISOR_HANDOFF_STORAGE_V2: usize = 4 * RETAINED
    + 4 * (codec::BYTES + launch_manifest_codec::BYTES)
    + 2 * size_of::<sha2::Sha256>()
    + 4096;

const _: () = {
    type Output = (CompilerExecutionSupervisorHandoffV2, Storage);
    type Raw = (codec::Frame, launch_manifest_codec::Record);
    assert!(codec::BYTES == 184 && launch_manifest_codec::BYTES == 112);
    assert!(RETAINED >= size_of::<(Launch, Storage)>());
    assert!(size_of::<Raw>() <= RETAINED);
    assert!(size_of::<codec::Frame>() + size_of::<launch_manifest_codec::Record>() <= RETAINED);
    assert!(size_of::<Result<Output>>() >= RETAINED);
    assert!(size_of::<std::thread::Result<Result<Output>>>() >= RETAINED);
    assert!(size_of::<std::result::Result<Raw, Framing>>() >= size_of::<Raw>());
    assert!(size_of::<std::result::Result<codec::Frame, Framing>>() >= size_of::<codec::Frame>());
    assert!(
        8 * size_of::<CompilerExecutionSupervisorHandoffErrorV2>()
            + 64 * size_of::<usize>()
            + size_of::<Budget<'static>>()
            + 4 * size_of::<Storage>()
            + (size_of::<Result<Output>>() - RETAINED)
            + (size_of::<std::thread::Result<Result<Output>>>() - RETAINED)
            + (size_of::<std::result::Result<Raw, Framing>>() - size_of::<Raw>())
            + (size_of::<std::result::Result<codec::Frame, Framing>>() - size_of::<codec::Frame>())
            <= 4096
    );
};

/// Move-only, inert direct-parent/rustc binding with native launch ownership.
///
/// This grants no process, signing, protected-readiness, publication, load or
/// execution authority. The unchanged wire contains opaque policy identities:
/// structural decoding of a legacy-bound frame is NOT native policy admission.
/// Consumers must independently match a caller-pinned native policy and actual
/// process identities before use. There is no admitted V1 owner conversion.
///
/// Inputs stay prepaid on the caller's ledger. Every operation restores entry
/// storage while preserving work, peak and denial history. Reserve the returned
/// additional storage before retaining the owner. On a consuming error, the
/// launch is dropped but its reservation remains for the caller to release.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::CompilerExecutionSupervisorHandoffV2;
/// fn duplicate(value: CompilerExecutionSupervisorHandoffV2) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::{CompilerExecutionSupervisorHandoffV1,
///     CompilerExecutionSupervisorHandoffV2};
/// fn legacy(_: &CompilerExecutionSupervisorHandoffV1) {}
/// fn mix(value: &CompilerExecutionSupervisorHandoffV2) { legacy(value); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::{CompilerExecutionSupervisorHandoffV2 as Handoff,
///     CompilerExecutionServiceLaunchManifestV1 as Launch,
///     CompilerExecutionClientProcessIdentityV1 as Client};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn mix(client: Client, launch: Launch, budget: &mut Budget<'_>) {
///     let _ = Handoff::new(client, launch, budget);
/// }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::{CompilerExecutionSupervisorHandoffV2 as Handoff,
///     CompilerExecutionServiceLaunchManifestV2 as Launch,
///     CompilerExecutionClientProcessIdentityV1 as Client};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn reuse(client: Client, launch: Launch, budget: &mut Budget<'_>) {
///     let _ = Handoff::new(client, launch, budget);
///     let _ = launch.canonical_bytes();
/// }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::CompilerExecutionSupervisorHandoffV2 as Handoff;
/// fn unmetered(bytes: &[u8]) { let _ = Handoff::decode(bytes); }
/// ```
#[derive(Eq, PartialEq)]
pub struct CompilerExecutionSupervisorHandoffV2 {
    frame: codec::Frame,
    launch_manifest: Launch,
}

impl CompilerExecutionSupervisorHandoffV2 {
    /// Consumes a fully prepaid native launch owner. Keep its reservation and
    /// add only the returned growth; eventual release uses `retained_storage()`.
    pub fn new(
        submitter: Client,
        launch: Launch,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        let inherited = launch.retained_storage();
        metered(budget, inherited, || {
            let growth = RETAINED
                .checked_sub(inherited)
                .ok_or(Resource::Accounting)?;
            let frame = codec::encode(submitter, launch.client(), launch.canonical_bytes())?;
            Ok((
                Self {
                    frame,
                    launch_manifest: launch,
                },
                Storage(growth),
            ))
        })
    }

    /// Strict shared framing with full retained storage returned unreserved.
    /// Exactly sized input needs all 184 bytes prepaid; wrong lengths need no
    /// input floor. The complete fixed quota includes the private launch codec.
    pub fn decode(bytes: &[u8], budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        metered(
            budget,
            resources::fixed_input_floor(bytes, codec::BYTES),
            || {
                let (frame, launch) = codec::decode(bytes)?;
                Ok((
                    Self {
                        frame,
                        launch_manifest: Launch::from_record(launch),
                    },
                    Storage(RETAINED),
                ))
            },
        )
    }

    pub const fn submitter(&self) -> Client {
        self.frame.submitter
    }
    pub const fn launch_manifest(&self) -> &Launch {
        &self.launch_manifest
    }
    /// Existing wire identity, not evidence of native policy admission.
    pub const fn identity(&self) -> Identity {
        Identity::from_bytes_for_protocol(self.frame.identity)
    }
    pub const fn canonical_bytes(&self) -> &[u8; codec::BYTES] {
        &self.frame.bytes
    }
    /// Full owner and padded storage-receipt header, including the nested launch.
    pub const fn retained_storage(&self) -> usize {
        RETAINED
    }
}

impl fmt::Debug for CompilerExecutionSupervisorHandoffV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompilerExecutionSupervisorHandoffV2")
            .field("authority", &"none")
            .field("submitter", &self.submitter())
            .field("identity", &self.identity())
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub enum CompilerExecutionSupervisorHandoffErrorV2 {
    Framing(Framing),
    Resource(Resource),
}
type Result<T> = std::result::Result<T, CompilerExecutionSupervisorHandoffErrorV2>;
impl From<Framing> for CompilerExecutionSupervisorHandoffErrorV2 {
    fn from(value: Framing) -> Self {
        Self::Framing(value)
    }
}
impl From<Resource> for CompilerExecutionSupervisorHandoffErrorV2 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for CompilerExecutionSupervisorHandoffErrorV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Framing(e) => e.fmt(f),
            Self::Resource(e) => e.fmt(f),
        }
    }
}
impl Error for CompilerExecutionSupervisorHandoffErrorV2 {
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
        COMPILER_EXECUTION_SUPERVISOR_HANDOFF_WORK_V2,
        COMPILER_EXECUTION_SUPERVISOR_HANDOFF_STORAGE_V2,
        operation,
    )
}
