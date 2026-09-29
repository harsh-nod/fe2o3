//! Inert root-gate payloads over the existing root-control record. No helper
//! admits an issuer, authenticates a peer, or proves readiness or freshness.
//! The caller supplies the retained root epoch and an independently random
//! connection generation created after readiness; these helpers generate none.
use crate::{
    COMPILER_EXECUTION_ROOT_CONTROL_STORAGE_V3, COMPILER_EXECUTION_ROOT_CONTROL_WORK_V3,
    CompilerExecutionIssuerPolicyV3 as Policy, CompilerExecutionRootControlBindingV3 as Binding,
    CompilerExecutionRootControlErrorV3 as Error, CompilerExecutionRootControlKindV3 as Kind,
    CompilerExecutionRootControlRecordV3 as Record,
    CompilerExecutionServiceLaunchManifestV3 as Manifest,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV3 as Storage},
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use std::mem::size_of;

const REQUEST_PAYLOAD: &[u8] = b"FE2O3/ROOT-GATE/V3\0";
const REPLY_PAYLOAD: &[u8] = b"FE2O3/ISSUER-ADMITTED/V3\0";
const FRAME: usize = 4 * size_of::<(Record, Storage)>() + 4 * size_of::<Error>() + 4096;

/// Full successful work quote for any gate helper, including one nested codec
/// operation on the same ledger. Early shape refusal spends only the outer
/// ROOT_CONTROL_WORK quote; neither scope refunds accepted work or denial history.
pub const COMPILER_EXECUTION_ROOT_GATE_WORK_V3: usize = 2 * COMPILER_EXECUTION_ROOT_CONTROL_WORK_V3;
/// Additional peak scratch above all borrowed owners, including the fixed magic
/// payloads, outer result/error frames and the nested codec scratch. Logical
/// accounting only, not generated stack or RSS. Every return restores entry storage.
pub const COMPILER_EXECUTION_ROOT_GATE_STORAGE_V3: usize =
    FRAME + COMPILER_EXECUTION_ROOT_CONTROL_STORAGE_V3;

type Result<T> = std::result::Result<T, Error>;

/// Builds the exact sequence-one Reconcile request with the fixed gate payload.
/// The binding must be prepaid. Returns the full UNRESERVED record charge;
/// reserve it before retention. This neither generates nor verifies freshness.
pub fn compiler_execution_root_gate_request_v3(
    binding: &Binding,
    b: &mut Budget<'_>,
) -> Result<(Record, Storage)> {
    metered(b, binding.retained_storage(), |b| {
        Record::request(binding, 1, Kind::Reconcile, REQUEST_PAYLOAD, b)
    })
}

/// Checks exact request shape and its association with the supplied actual
/// policy and manifest. All three owners must be prepaid. Epoch/generation
/// matching, peer authentication, readiness and admission remain caller duties.
pub fn validate_compiler_execution_root_gate_request_v3(
    request: &Record,
    policy: &Policy,
    manifest: &Manifest,
    b: &mut Budget<'_>,
) -> Result<()> {
    metered(
        b,
        request.retained_storage() + policy.retained_storage() + manifest.retained_storage(),
        |b| {
            request_shape(request)?;
            if !request.matches_launch(policy, manifest, b)? {
                return Err(Error::Framing("root gate launch association"));
            }
            Ok(())
        },
    )
}

/// Builds the exact fixed reply joined to the original request digest. Checks
/// request shape first; the payload marker itself grants no issuer admission.
/// Prepay the request and reserve the full UNRESERVED returned record charge.
pub fn compiler_execution_root_gate_reply_v3(
    request: &Record,
    b: &mut Budget<'_>,
) -> Result<(Record, Storage)> {
    metered(b, request.retained_storage(), |b| {
        request_shape(request)?;
        Record::reply(request, REPLY_PAYLOAD, b)
    })
}

/// Checks both the original gate request and the exact reply payload/digest join.
/// Both records must be prepaid; successful framing is not peer authentication,
/// readiness, freshness, replay acceptance or issuer admission.
pub fn validate_compiler_execution_root_gate_reply_v3(
    reply: &Record,
    request: &Record,
    b: &mut Budget<'_>,
) -> Result<()> {
    metered(
        b,
        reply.retained_storage() + request.retained_storage(),
        |b| {
            request_shape(request)?;
            if reply.payload() != REPLY_PAYLOAD || !reply.matches_reply(request, b)? {
                return Err(Error::Framing("root gate reply"));
            }
            Ok(())
        },
    )
}

fn request_shape(request: &Record) -> Result<()> {
    if request.is_reply()
        || request.kind() != Kind::Reconcile
        || request.sequence() != 1
        || request.payload() != REQUEST_PAYLOAD
    {
        return Err(Error::Framing("root gate request"));
    }
    Ok(())
}

fn metered<'work, T>(
    b: &mut Budget<'work>,
    floor: usize,
    operation: impl FnOnce(&mut Budget<'work>) -> Result<T>,
) -> Result<T> {
    resources::nested_fixed(
        b,
        floor,
        COMPILER_EXECUTION_ROOT_CONTROL_WORK_V3,
        FRAME,
        operation,
    )
}

#[cfg(test)]
#[path = "root_gate_v3_tests.rs"]
mod tests;
