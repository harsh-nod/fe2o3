//! Native ownership and metering over the existing identity-only launch wire.
use crate::{
    CompilerExecutionClientProcessIdentityV1 as Client,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerPolicyIdentityV3 as PolicyIdentity,
    CompilerExecutionIssuerPolicyV3 as Policy,
    CompilerExecutionServiceLaunchManifestErrorV1 as Framing,
    CompilerExecutionServiceLaunchManifestIdentityV1 as Identity,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV3 as Storage},
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
/// V3. Consumers must match an independently pinned PolicyV3 before use. There
/// is no conversion from an admitted V1 policy or launch owner.
///
/// All borrowed owners stay prepaid on the same ledger. Operations restore
/// entry storage; reserve the returned additional storage before retention.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::CompilerExecutionServiceLaunchManifestV3;
/// fn duplicate(value: CompilerExecutionServiceLaunchManifestV3) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::{CompilerExecutionServiceLaunchManifestV1,
///     CompilerExecutionServiceLaunchManifestV3};
/// fn legacy(_: &CompilerExecutionServiceLaunchManifestV1) {}
/// fn mix(value: &CompilerExecutionServiceLaunchManifestV3) { legacy(value); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::{CompilerExecutionServiceLaunchManifestV3 as Launch,
///     CompilerExecutionIssuerPolicyV2 as Policy, CompilerExecutionClientProcessIdentityV1 as Client,
///     CompilerExecutionExternalAnchorServiceIdentityV1 as Service};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn mix(c: Client, s: Service, p: &Policy, b: &mut Budget<'_>) {
///     let _ = Launch::new(c, s, p, b);
/// }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::{CompilerExecutionServiceLaunchManifestV2 as V2,
///     CompilerExecutionServiceLaunchManifestV3 as V3};
/// fn downgrade(owner: V3) -> V2 { owner.into() }
/// ```
#[derive(Eq, PartialEq)]
pub struct CompilerExecutionServiceLaunchManifestV3 {
    record: codec::Record,
}

crate::launch_manifest_adapter::launch_manifest_adapter!(
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_BYTES_V3,
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_WORK_V3,
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_STORAGE_V3,
    CompilerExecutionServiceLaunchManifestV3,
    CompilerExecutionServiceLaunchManifestErrorV3
);
