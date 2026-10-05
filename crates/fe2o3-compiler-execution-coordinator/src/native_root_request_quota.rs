//! Private closed schedule for the existing fixed-origin loaders and backing.
//! These are admission ceilings, not reservations or additional approval APIs.
use super::*;
use crate::compiler_invocation_staging::StagedRustcInvocationV1 as Staging;
use crate::native_launch::{self as native, CompilerExecutionLaunchErrorV2 as NativeError};
use crate::native_runtime_inventory::NativeCompilerExecutableInventory as Executables;
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
use fe2o3_protected_service_spawn::{
    RetainedResourcesV2 as Resources, launch_io,
    native_spawn::{
        RootOwnedProtectedServiceChildV2 as PlainChild, RootOwnedRetainedServiceChildV2 as Child,
        RootTaskTraceV2 as TaskTrace, StagedProtectedServiceExecV2 as Stage,
    },
};
use fe2o3_protected_static_executable::{
    ProtectedStaticExecutableOperationV2 as ImageOperation, ProtectedStaticExecutableV2 as Image,
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
const SINGLE_TRANSFER_WORK: usize = 16 + 32 * 1024 + 4 * MANIFEST_BYTES;
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

fn backing_check_work() -> Result<usize> {
    sum(&[
        Backing::LOCAL_WORK,
        8,
        Invocation::NATIVE_REVALIDATION_WORK,
        Output::WORK,
        repeated(3, runtime_pass()?)?,
        repeated(2, file_pass()?)?,
        INVENTORY_WORK,
    ])
}

pub(super) fn refusal() -> Result<Quota> {
    let image = image_quota(ImageOperation::Revalidate)?;
    Ok(Quota {
        // Helper check validates its compiler twice; final stage validation
        // validates it once more and then checks both actual image transfers.
        work: sum(&[
            RootCompilerRequest::LOCAL_WORK,
            8,
            EXCHANGE_WORK,
            repeated(4, backing_check_work()?)?,
            Executables::WORK,
            3 * crate::native_runtime_descriptors::WORK,
            proof_helper_launch::LOCAL_WORK,
            ProofHelperBacking::LOCAL_WORK,
            compiler_attempt::LOCAL_WORK,
            compiler_attempt::INPUT_COMPARE_WORK,
            crate::compiler_child_channel::CompilerTrace::<ManagedProofHelper>::OBSERVATION_WORK,
            Resources::<ManagedProofHelper>::ACCESS_WORK,
            Resources::<proof_helper_launch::Payload>::ACCESS_WORK,
            PlainChild::OPERATION_WORK,
            fe2o3_protected_service_profile::observations::PROCESS_VALIDATE_WORK,
            2 * SINGLE_TRANSFER_WORK,
            repeated(4, runtime_pass()?)?,
            repeated(4, file_pass()?)?,
            image.work(),
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
            proof_helper_launch::FRAME,
            ProofHelperBacking::FRAME_STORAGE,
            compiler_attempt::FRAME,
            Executables::FRAME,
            crate::native_runtime_descriptors::FRAME,
            crate::compiler_child_channel::CompilerTrace::<ManagedProofHelper>::OBSERVATION_SCRATCH,
            Resources::<ManagedProofHelper>::ACCESS_SCRATCH,
            Resources::<proof_helper_launch::Payload>::ACCESS_SCRATCH,
            PlainChild::OPERATION_SCRATCH,
            fe2o3_protected_service_profile::observations::PROCESS_VALIDATE_SCRATCH,
            image.scratch(),
        ])?,
    })
}

/// Full original-stage revalidation followed by one gated interrupt. The
/// existing refusal schedule conservatively includes the same backing/transfer
/// closure; retaining its extra intake checks does not renew any account.
pub(super) fn runtime_interrupt() -> Result<Quota> {
    let validation = refusal()?;
    let interrupt =
        crate::compiler_child_channel::CompilerTrace::<ManagedProofHelper>::gated_operation_quota(
        )?;
    Ok(Quota {
        work: sum(&[validation.work(), interrupt.work()])?,
        scratch: sum(&[validation.scratch(), interrupt.scratch()])?,
    })
}

/// First-exec transition: ready helper content validation, original inventory
/// revalidation and the non-resuming image/census check. The fixed refusal
/// schedule already funds four full compiler checks (this path needs three)
/// and its helper image/profile/access closure; checkpoint turns use the narrow
/// non-hashing accessor instead. No larger deployment budget is selected here.
pub(super) fn runtime_capture() -> Result<Quota> {
    let validation = refusal()?;
    let trace =
        crate::compiler_child_channel::CompilerTrace::<ManagedProofHelper>::runtime_backing_quota(
        )?;
    use crate::native_runtime_controller::NativeRuntimeController as Controller;
    Ok(Quota {
        work: sum(&[
            validation.work(),
            trace.work(),
            Controller::INITIAL_IMAGE_WORK,
        ])?,
        scratch: sum(&[
            validation.scratch(),
            trace.scratch(),
            Controller::INITIAL_IMAGE_SCRATCH,
        ])?,
    })
}

fn image_quota(
    operation: ImageOperation,
) -> Result<fe2o3_protected_static_executable::ProtectedStaticExecutableQuotaV2> {
    Image::quota_for_length(
        fe2o3_build_authority::COMPILER_RUNTIME_MANIFEST_MAX_FILE_BYTES_V1,
        operation,
    )
    .map_err(|e| helper_backing_error(ProofHelperBackingError::Image(e)))
}

// Ceilings only: actual consumption always reserves returned growth. These
// maxima overlap the complete original capture, inventory and staging owners.
fn payloads() -> Result<(usize, usize)> {
    let image = root::length(fe2o3_build_authority::COMPILER_RUNTIME_MANIFEST_MAX_FILE_BYTES_V1)?;
    let backing = sum(&[
        Receiver::STORAGE,
        preparation()?.scratch(),
        image,
        size_of::<ProofHelperBacking>(),
        Image::file_storage_for_length(image as u64)
            .map_err(|e| helper_backing_error(ProofHelperBackingError::Image(e)))?,
    ])?;
    let helper = proof_helper_launch::Payload::storage(backing).map_err(helper_error)?;
    let compiler = sum(&[
        Child::<proof_helper_launch::Payload>::storage_for(helper)
            .map_err(|e| Error::from(NativeError::from(e)))?,
        native::FILE_STORAGE,
        ManagedProofHelper::ENVELOPE,
    ])?;
    Ok((helper, compiler))
}

pub(super) fn launch() -> Result<Quota> {
    let (helper, compiler) = payloads()?;
    let guard = Prepared::maximum_cleanup_guard_quota()?;
    let image = image_quota(ImageOperation::Admit)?;
    let image_check = image_quota(ImageOperation::Revalidate)?;
    let image_transfer = image_quota(ImageOperation::Transfer)?;
    let polling = sum(&[
        launch_io::MAX_WORK,
        launch_io::MAX_PIPE_WORK,
        repeated(
            sum(&[
                launch_io::MAX_LIVENESS_CHECKS,
                launch_io::MAX_PIPE_LIVENESS_CHECKS,
            ])?,
            PlainChild::OPERATION_WORK,
        )?,
    ])?;
    let sources = sum(&[compiler, native::Channels::STORAGE, compiler_attempt::FRAME])?;
    let staging = Stage::compiler_staging_scratch_for_sources(sources)
        .map_err(|e| Error::from(NativeError::from(e)))?;
    Ok(Quota {
        work: sum(&[
            2 * RootCompilerRequest::LOCAL_WORK,
            repeated(2, guard.work())?,
            // Compiler checks: helper preparation 3, helper launch 20,
            // compiler staging/channel/final validation 11. Each helper check
            // includes TWO compiler checks, not one detached runtime check.
            repeated(34, backing_check_work()?)?,
            // ELF inventory capture: helper access checks compiler twice and
            // capture checks it twice; final attempt revalidation adds one.
            repeated(5, backing_check_work()?)?,
            repeated(2, Executables::WORK)?,
            // Stage validation, post-profile validation and final trace-backed
            // validation each inspect the three actual selected stdio sources.
            9 * crate::native_runtime_descriptors::WORK,
            proof_helper_launch::LOCAL_WORK,
            ProofHelperBacking::LOCAL_WORK,
            image_check.work(),
            Resources::<proof_helper_launch::Payload>::ACCESS_WORK,
            PlainChild::OPERATION_WORK,
            fe2o3_protected_service_profile::observations::PROCESS_VALIDATE_WORK,
            // Helper source clone + validation, then three validations of
            // the compiler's two actual staged image transfers.
            8 * SINGLE_TRANSFER_WORK,
            repeated(16, runtime_pass()?)?,
            repeated(17, file_pass()?)?,
            image.work(),
            repeated(14, image_check.work())?,
            repeated(9, image_transfer.work())?,
            repeated(10, ProofHelperBacking::LOCAL_WORK)?,
            repeated(5, proof_helper_launch::LOCAL_WORK)?,
            repeated(3, compiler_attempt::LOCAL_WORK)?,
            repeated(3, compiler_attempt::INPUT_COMPARE_WORK)?,
            Output::WORK,
            Stage::STAGING_WORK,
            Stage::COMPILER_CHILD_CHANNEL_STAGING_WORK,
            Stage::RUNTIME_CHECKPOINT_STAGING_WORK,
            Stage::RUNTIME_CHECKPOINT_CHILD_WORK,
            Stage::OUTPUT_CONFINEMENT_STAGING_WORK,
            Stage::OUTPUT_CONFINEMENT_CHILD_WORK,
            // Also bounds compiler-only child setup above generic spawn work.
            Stage::COMPILER_CHILD_CHANNEL_STAGING_WORK,
            2 * Stage::spawn_work_for(
                fe2o3_protected_service_spawn::MAX_PROTECTED_SERVICE_DESCRIPTOR_BINDINGS_V1,
                63,
            )
            .map_err(|e| Error::from(NativeError::from(e)))?,
            Cleanup::retained_launch_work::<proof_helper_launch::Payload>(helper)?,
            Cleanup::retained_launch_work::<ManagedProofHelper>(compiler)?,
            Stage::FRESH_NAMESPACE_WORK,
            Stage::FRESH_DOMAIN_WORK,
            // Helper: profile, gate, exec EOF, Initial, READY. Compiler:
            // profile and original child-created channel receipt.
            repeated(7, polling)?,
            repeated(8, PlainChild::OPERATION_WORK)?,
            TaskTrace::OPERATION_WORK,
            repeated(3, crate::compiler_child_channel::CompilerTrace::<ManagedProofHelper>::OBSERVATION_WORK)?,
            repeated(5, Resources::<proof_helper_launch::Payload>::ACCESS_WORK)?,
            repeated(2, Resources::<ManagedProofHelper>::ACCESS_WORK)?,
            repeated(6, fe2o3_protected_service_profile::observations::PROCESS_VALIDATE_WORK)?,
            // Bootstrap record codecs and fixed descriptor/scalar bookkeeping.
            proof_helper_launch::LOCAL_WORK,
        ])?,
        scratch: sum(&[
            guard.scratch(),
            Executables::MAX_STORAGE,
            Executables::FRAME,
            helper,
            compiler,
            2 * staging,
            Stage::RUNTIME_CHECKPOINT_STAGING_SCRATCH,
            Stage::OUTPUT_CONFINEMENT_STAGING_SCRATCH,
            image.scratch(),
            refusal()?.scratch(),
            Stage::spawn_retaining_scratch::<proof_helper_launch::Payload>(helper)
                .map_err(|e| Error::from(NativeError::from(e)))?,
            Stage::spawn_retaining_scratch::<ManagedProofHelper>(compiler)
                .map_err(|e| Error::from(NativeError::from(e)))?,
            Stage::FRESH_NAMESPACE_SCRATCH,
            Stage::FRESH_DOMAIN_SCRATCH,
            TaskTrace::OPERATION_SCRATCH,
            PlainChild::ROOT_TRACE_GROWTH,
            fe2o3_protected_service_profile::observations::PROCESS_VALIDATE_SCRATCH,
        ])?,
    })
}

pub(super) fn cleanup_growth() -> Result<(usize, usize)> {
    let (helper, compiler) = payloads()?;
    Ok((
        sum(&[
            2 * Cleanup::GUARD_CLONE_WORK,
            Cleanup::retained_launch_work::<proof_helper_launch::Payload>(helper)?,
            Cleanup::retained_launch_work::<ManagedProofHelper>(compiler)?,
        ])?,
        sum(&[
            Resources::<proof_helper_launch::Payload>::payload_storage(helper)?,
            Resources::<ManagedProofHelper>::payload_storage(compiler)?,
        ])?,
    ))
}
