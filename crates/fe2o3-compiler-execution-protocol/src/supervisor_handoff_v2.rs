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

crate::supervisor_handoff_adapter::supervisor_handoff_adapter!(
    COMPILER_EXECUTION_SUPERVISOR_HANDOFF_BYTES_V2,
    COMPILER_EXECUTION_SUPERVISOR_HANDOFF_WORK_V2,
    COMPILER_EXECUTION_SUPERVISOR_HANDOFF_STORAGE_V2,
    CompilerExecutionSupervisorHandoffV2,
    CompilerExecutionSupervisorHandoffErrorV2
);
