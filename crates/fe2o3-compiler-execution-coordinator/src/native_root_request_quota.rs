//! Private closed schedule for the existing fixed-origin loaders and backing.
//! These are admission ceilings, not reservations or additional approval APIs.
use super::*;
use crate::compiler_invocation_staging::StagedRustcInvocationV1 as Staging;
use fe2o3_build_authority::{
    COMPILER_APPROVAL_POLICY_BYTES_V2 as POLICY_BYTES,
    COMPILER_APPROVAL_POLICY_WORK_V2 as POLICY_WORK,
    COMPILER_RUNTIME_MANIFEST_MAX_BYTES_V1 as MANIFEST_BYTES,
    COMPILER_RUNTIME_MANIFEST_MAX_ENTRIES_V1 as ENTRIES,
    COMPILER_RUNTIME_MANIFEST_MAX_TOTAL_BYTES_V1 as CODE_BYTES,
    COMPILER_RUNTIME_MANIFEST_STORAGE_V1 as CODEC_SCRATCH,
    COMPILER_RUNTIME_MANIFEST_WORK_V1 as CODEC_WORK,
};
use fe2o3_compiler_closure_capability::{
    CompilerApprovalStorageV2 as ApprovalCharge,
    CompilerExecutionClientProfileCapabilityV3 as Profile,
    RetainedCompilerRuntimeExecTransferChargeV1 as TransferCharge,
    RetainedCompilerRuntimeInventoryTransferV1 as Transfer,
    RetainedCompilerRuntimeStorageV1 as RuntimeCharge,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V3 as PROFILE_BYTES,
    COMPILER_EXECUTION_CLIENT_PROFILE_STORAGE_V3 as PROFILE_SCRATCH,
    COMPILER_EXECUTION_CLIENT_PROFILE_WORK_V3 as PROFILE_WORK,
};
use root::{CompilerExecutionRootAdmissionQuotaV2 as Quota, repeated, sum};

// Mirrors the closed local I/O allowance of compiler_approval_v2 and
// retained_compiler_runtime_v1. Nested codecs, hashing and scopes are separate.
const ORIGIN_WORK: usize = 256 * 1024;
const CHUNK: usize = 64 * 1024;
const APPROVAL_WORK: usize = 8
    + ORIGIN_WORK
    + POLICY_WORK
    + PROFILE_WORK
    + 2 * Profile::IO_WORK
    + POLICY_BYTES
    + PROFILE_BYTES
    + 1024;
const INVENTORY_WORK: usize = 8 + 32 * 1024 + 4 * MANIFEST_BYTES + ENTRIES * 1088;
const APPROVAL_SCRATCH: usize = 16 * 1024
    + PROFILE_SCRATCH
    + 2 * Profile::IO_STORAGE
    + size_of::<(Approval, ApprovalCharge)>()
    + POLICY_BYTES
    + PROFILE_BYTES;
const RUNTIME_SCRATCH: usize = 2 * CODEC_SCRATCH + 2 * size_of::<Runtime>() + 2 * CHUNK + 16 * 1024;
// Runtime includes the private inventory snapshots; using its size bounds each
// private transfer frame without exporting their layouts or inventing an API.
const TRANSFER_SCRATCH: usize = 2 * CHUNK
    + 12 * size_of::<Runtime>()
    + 8 * size_of::<std::fs::Metadata>()
    + 4 * size_of::<(Transfer, TransferCharge)>()
    + 8192;

fn file_pass() -> Result<usize> {
    sum(&[
        repeated(ENTRIES, ORIGIN_WORK)?,
        repeated(root::length(CODE_BYTES)?, 8)?,
    ])
}

// Covers either admission or revalidation: approval before/after, one complete
// code hash pass, both manifest decodes and the complete fixed-origin walk.
fn runtime_pass() -> Result<usize> {
    sum(&[
        3 * APPROVAL_WORK,
        ORIGIN_WORK,
        2048,
        2 * CODEC_WORK,
        MANIFEST_BYTES,
        repeated(ENTRIES, ORIGIN_WORK)?,
        file_pass()?,
    ])
}

pub(super) fn preparation() -> Result<Quota> {
    let code = root::length(CODE_BYTES)?;
    let runtime = sum(&[size_of::<(Runtime, RuntimeCharge)>(), MANIFEST_BYTES, code])?;
    let transfer = sum(&[size_of::<(Transfer, TransferCharge)>(), code])?;
    Ok(Quota {
        // Runtime load: one pass. Backing: require + clone's two validations
        // + final require + transfer's two validations, and five transfer checks.
        work: sum(&[
            RootCompilerRequest::LOCAL_WORK,
            APPROVAL_WORK,
            repeated(7, runtime_pass()?)?,
            repeated(5, file_pass()?)?,
            2 * INVENTORY_WORK,
            Backing::LOCAL_WORK,
            Staging::STAGING_WORK,
            2 * Invocation::NATIVE_REVALIDATION_WORK,
            2 * Output::WORK,
            4096,
        ])?,
        // Keep original input charges and all newly retained owners while nested
        // scopes overlap. Actual callers reserve returned deltas, never this quote.
        scratch: sum(&[
            APPROVAL_SCRATCH,
            runtime,
            transfer,
            RUNTIME_SCRATCH,
            TRANSFER_SCRATCH,
            Staging::STAGING_SCRATCH,
            Staging::STAGING_SCRATCH,
            Backing::FRAME_STORAGE,
            size_of::<Backing>(),
            Invocation::NATIVE_OPERATION_SCRATCH,
            Output::SCRATCH,
        ])?,
    })
}

pub(super) fn refusal() -> Result<Quota> {
    Ok(Quota {
        // Backing revalidation requires one closure check, two full runtime
        // validations, and two complete transfer-file checks before the V4 ACK.
        work: sum(&[
            RootCompilerRequest::LOCAL_WORK,
            8,
            EXCHANGE_WORK,
            Backing::LOCAL_WORK,
            8,
            Invocation::NATIVE_REVALIDATION_WORK,
            Output::WORK,
            repeated(3, runtime_pass()?)?,
            repeated(2, file_pass()?)?,
            INVENTORY_WORK,
            4096,
        ])?,
        scratch: sum(&[
            FRAME,
            io::packet_receive_scratch(N),
            Backing::FRAME_STORAGE,
            APPROVAL_SCRATCH,
            RUNTIME_SCRATCH,
            TRANSFER_SCRATCH,
            Invocation::NATIVE_OPERATION_SCRATCH,
            Output::SCRATCH,
        ])?,
    })
}
