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

crate::launch_manifest_adapter::launch_manifest_adapter!(
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_BYTES_V2,
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_WORK_V2,
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_STORAGE_V2,
    CompilerExecutionServiceLaunchManifestV2,
    CompilerExecutionServiceLaunchManifestErrorV2
);
