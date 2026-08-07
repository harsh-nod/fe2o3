use crate::api::{ApiError, DispatchApi, QueueHandle};
use crate::environment::{AdapterCore, HsaRuntimeAdapterError, ReviewedHsaRuntimeAdapterV1};
use crate::lifecycle::{ReviewedHsaExecutableV1, ReviewedHsaKernelV1};
use fe2o3_host::{
    HsaDispatchObservationV1, HsaImplicitKernargInitializationObservationV1, HsaLaunchGeometryV1,
    ReviewedHsaImplicitKernargAdapterV1,
};
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};

const EXPLICIT_BYTES: usize = 48;
const IMPLICIT_OFFSET: usize = 48;
const IMPLICIT_BYTES: usize = 256;
const TOTAL_BYTES: usize = EXPLICIT_BYTES + IMPLICIT_BYTES;
const BLOCK_COUNT_X: usize = 0;
const BLOCK_COUNT_Y: usize = 4;
const BLOCK_COUNT_Z: usize = 8;
const GROUP_SIZE_X: usize = 12;
const GROUP_SIZE_Y: usize = 14;
const GROUP_SIZE_Z: usize = 16;
const REMAINDER_X: usize = 18;
const REMAINDER_Y: usize = 20;
const REMAINDER_Z: usize = 22;
const GLOBAL_OFFSET_X: usize = 40;
const GLOBAL_OFFSET_Y: usize = 48;
const GLOBAL_OFFSET_Z: usize = 56;
const GRID_DIMS: usize = 64;
const HOSTCALL_PTR: usize = 80;
const MULTIGRID_SYNC_ARG: usize = 88;
const HEAP_V1_PTR: usize = 96;
const DEFAULT_QUEUE_PTR: usize = 104;
const COMPLETION_ACTION: usize = 112;
const QUEUE_PTR: usize = 200;
pub(crate) const COMPLETION_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) struct PendingDispatch {
    queue: QueueHandle,
    executable_identity: fe2o3_host::HsaExecutableObjectIdentityV1,
    kernel_identity: fe2o3_host::HsaKernelObjectIdentityV1,
    geometry: HsaLaunchGeometryV1,
    kernarg_digest: [u8; 32],
}

struct PreSubmitDispatch {
    queue: QueueHandle,
    kernarg_address: usize,
    completion_signal: u64,
}

struct SubmittedDispatch {
    resources: PreSubmitDispatch,
    packet_id: u64,
}

struct QuiescedDispatch(SubmittedDispatch);

struct UnquiescedDispatch {
    submitted: SubmittedDispatch,
    reason: UnquiescedReason,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UnquiescedReason {
    QueueError(ApiError),
    CompletionDeadline {
        last_observation: i64,
    },
    #[cfg(feature = "hardware-test-hooks")]
    TestPhaseEvidence,
}

enum CompletionTransition {
    Quiesced {
        dispatch: QuiescedDispatch,
        queue_error: Option<ApiError>,
    },
    Unquiesced(UnquiescedDispatch),
}

// SAFETY: the exact 256-byte COV6 hidden span is initialized from reviewed
// geometry and the exact private HSA queue retained for the following launch.
// This profile rejects every other layout and every launch substitution.
unsafe impl ReviewedHsaImplicitKernargAdapterV1 for ReviewedHsaRuntimeAdapterV1 {
    unsafe fn initialize_implicit_kernarg(
        &mut self,
        executable: &Self::Executable,
        kernel: &Self::Kernel,
        geometry: HsaLaunchGeometryV1,
        explicit_byte_len: usize,
        implicit_byte_offset: usize,
        implicit_byte_len: usize,
        kernarg: &mut [u8],
    ) -> Result<HsaImplicitKernargInitializationObservationV1, Self::Error> {
        prepare_implicit_kernarg(
            &mut self.core,
            &mut self.pending_dispatch,
            executable,
            kernel,
            geometry,
            explicit_byte_len,
            implicit_byte_offset,
            implicit_byte_len,
            kernarg,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_implicit_kernarg(
    executable: &ReviewedHsaExecutableV1,
    kernel: &ReviewedHsaKernelV1,
    geometry: HsaLaunchGeometryV1,
    explicit_byte_len: usize,
    implicit_byte_offset: usize,
    implicit_byte_len: usize,
    kernarg: &[u8],
) -> Result<(), HsaRuntimeAdapterError> {
    let executable =
        executable
            .state
            .as_ref()
            .ok_or(HsaRuntimeAdapterError::InvalidImplicitKernarg(
                "consumed executable",
            ))?;
    if kernel.executable_identity != executable.identity {
        return Err(HsaRuntimeAdapterError::InvalidImplicitKernarg(
            "kernel/executable identity",
        ));
    }
    if explicit_byte_len != EXPLICIT_BYTES
        || implicit_byte_offset != IMPLICIT_OFFSET
        || implicit_byte_len != IMPLICIT_BYTES
        || kernarg.len() != TOTAL_BYTES
        || usize::try_from(kernel.kernarg_segment_size).ok() != Some(kernarg.len())
        || kernel.kernarg_segment_alignment == 0
        || !kernel.kernarg_segment_alignment.is_power_of_two()
    {
        return Err(HsaRuntimeAdapterError::InvalidImplicitKernarg(
            "exact 48+256 byte layout",
        ));
    }
    let grid = geometry.grid();
    let workgroup = geometry.workgroup();
    if grid.contains(&0)
        || workgroup.contains(&0)
        || workgroup.iter().any(|value| u16::try_from(*value).is_err())
    {
        return Err(HsaRuntimeAdapterError::InvalidImplicitKernarg(
            "launch geometry",
        ));
    }
    if geometry.dynamic_shared_memory_bytes() != 0 {
        return Err(HsaRuntimeAdapterError::InvalidImplicitKernarg(
            "the reviewed COV6 vecadd profile requires zero dynamic LDS",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn prepare_implicit_kernarg<A: DispatchApi>(
    core: &mut AdapterCore<A>,
    pending: &mut Option<PendingDispatch>,
    executable: &ReviewedHsaExecutableV1,
    kernel: &ReviewedHsaKernelV1,
    geometry: HsaLaunchGeometryV1,
    explicit_byte_len: usize,
    implicit_byte_offset: usize,
    implicit_byte_len: usize,
    kernarg: &mut [u8],
) -> Result<HsaImplicitKernargInitializationObservationV1, HsaRuntimeAdapterError> {
    if pending.is_some() {
        return Err(HsaRuntimeAdapterError::InvalidImplicitKernarg(
            "one unconsumed queue binding already exists",
        ));
    }
    validate_implicit_kernarg(
        executable,
        kernel,
        geometry,
        explicit_byte_len,
        implicit_byte_offset,
        implicit_byte_len,
        kernarg,
    )?;
    let state = executable
        .state
        .as_ref()
        .expect("validated executable remains live");
    let queue_size = reviewed_queue_size(core.queue_min_size, core.queue_max_size)?;
    let mut queue = core
        .api
        .queue_create(core.agent, queue_size)
        .map_err(HsaRuntimeAdapterError::api)?;
    if let Err(primary) = core.api.queue_async_error(&queue) {
        if core.api.queue_destroy(&mut queue).is_err() {
            std::process::abort();
        }
        return Err(HsaRuntimeAdapterError::api(primary));
    }
    let queue_pointer = match u64::try_from(queue.pointer()) {
        Ok(pointer) if pointer != 0 => pointer,
        _ => {
            let primary = ApiError {
                operation: "validate HSA queue pointer for COV6 kernarg",
                status: -1,
            };
            if core.api.queue_destroy(&mut queue).is_err() {
                std::process::abort();
            }
            return Err(HsaRuntimeAdapterError::api(primary));
        }
    };

    let grid = geometry.grid();
    let workgroup = geometry.workgroup();
    let explicit = kernarg[..EXPLICIT_BYTES].to_vec();
    kernarg[IMPLICIT_OFFSET..].fill(0);
    put_u32(kernarg, IMPLICIT_OFFSET + BLOCK_COUNT_X, grid[0]);
    put_u32(kernarg, IMPLICIT_OFFSET + BLOCK_COUNT_Y, grid[1]);
    put_u32(kernarg, IMPLICIT_OFFSET + BLOCK_COUNT_Z, grid[2]);
    put_u16(kernarg, IMPLICIT_OFFSET + GROUP_SIZE_X, workgroup[0] as u16);
    put_u16(kernarg, IMPLICIT_OFFSET + GROUP_SIZE_Y, workgroup[1] as u16);
    put_u16(kernarg, IMPLICIT_OFFSET + GROUP_SIZE_Z, workgroup[2] as u16);
    put_u16(kernarg, IMPLICIT_OFFSET + REMAINDER_X, 0);
    put_u16(kernarg, IMPLICIT_OFFSET + REMAINDER_Y, 0);
    put_u16(kernarg, IMPLICIT_OFFSET + REMAINDER_Z, 0);
    put_u64(kernarg, IMPLICIT_OFFSET + GLOBAL_OFFSET_X, 0);
    put_u64(kernarg, IMPLICIT_OFFSET + GLOBAL_OFFSET_Y, 0);
    put_u64(kernarg, IMPLICIT_OFFSET + GLOBAL_OFFSET_Z, 0);
    let dimensions = if grid[2]
        .checked_mul(workgroup[2])
        .is_some_and(|size| size > 1)
    {
        3
    } else if grid[1]
        .checked_mul(workgroup[1])
        .is_some_and(|size| size > 1)
    {
        2
    } else {
        1
    };
    put_u16(kernarg, IMPLICIT_OFFSET + GRID_DIMS, dimensions);
    put_u64(kernarg, IMPLICIT_OFFSET + HOSTCALL_PTR, 0);
    put_u64(kernarg, IMPLICIT_OFFSET + MULTIGRID_SYNC_ARG, 0);
    put_u64(kernarg, IMPLICIT_OFFSET + HEAP_V1_PTR, 0);
    put_u64(kernarg, IMPLICIT_OFFSET + DEFAULT_QUEUE_PTR, 0);
    put_u64(kernarg, IMPLICIT_OFFSET + COMPLETION_ACTION, 0);
    put_u64(kernarg, IMPLICIT_OFFSET + QUEUE_PTR, queue_pointer);
    if kernarg[..EXPLICIT_BYTES] != explicit {
        std::process::abort();
    }
    let mut digest = Sha256::new();
    digest.update(&*kernarg);
    let kernarg_digest = digest.finalize().into();
    *pending = Some(PendingDispatch {
        queue,
        executable_identity: state.identity,
        kernel_identity: kernel.identity,
        geometry,
        kernarg_digest,
    });
    Ok(HsaImplicitKernargInitializationObservationV1::new(
        state.identity,
        kernel.identity,
        geometry,
        EXPLICIT_BYTES as u64,
        IMPLICIT_OFFSET as u64,
        IMPLICIT_BYTES as u64,
        true,
    ))
}

pub(crate) fn launch_and_wait<A: DispatchApi>(
    core: &mut AdapterCore<A>,
    pending: &mut Option<PendingDispatch>,
    executable: &ReviewedHsaExecutableV1,
    kernel: &ReviewedHsaKernelV1,
    geometry: HsaLaunchGeometryV1,
    kernarg: &mut [u8],
) -> Result<HsaDispatchObservationV1, HsaRuntimeAdapterError> {
    let mut prepared =
        pending
            .take()
            .ok_or(HsaRuntimeAdapterError::InvalidExecutableObservation(
                "missing reviewed COV6 queue binding",
            ))?;
    let executable = match executable.state.as_ref() {
        Some(executable) => executable,
        None => {
            return Err(reject_pending_dispatch(
                &mut core.api,
                &mut prepared,
                HsaRuntimeAdapterError::InvalidExecutableObservation("consumed executable"),
            ));
        }
    };
    let mut digest = Sha256::new();
    digest.update(&*kernarg);
    let kernarg_digest: [u8; 32] = digest.finalize().into();
    if kernel.executable_identity != executable.identity
        || kernarg.len() != TOTAL_BYTES
        || usize::try_from(kernel.kernarg_segment_size).ok() != Some(kernarg.len())
        || kernel.kernarg_segment_alignment == 0
        || !kernel.kernarg_segment_alignment.is_power_of_two()
        || kernel.kernel_object == 0
        || kernel.symbol == 0
        || prepared.executable_identity != executable.identity
        || prepared.kernel_identity != kernel.identity
        || prepared.geometry != geometry
        || prepared.kernarg_digest != kernarg_digest
    {
        return Err(reject_pending_dispatch(
            &mut core.api,
            &mut prepared,
            HsaRuntimeAdapterError::InvalidExecutableObservation(
                "dispatch handle, geometry, or kernarg binding",
            ),
        ));
    }
    let aql_grid = match checked_aql_grid(geometry) {
        Ok(grid) => grid,
        Err(error) => {
            return Err(reject_pending_dispatch(&mut core.api, &mut prepared, error));
        }
    };
    let group_segment_size = match kernel
        .group_segment_size
        .checked_add(geometry.dynamic_shared_memory_bytes())
    {
        Some(size) => size,
        None => {
            return Err(reject_pending_dispatch(
                &mut core.api,
                &mut prepared,
                HsaRuntimeAdapterError::InvalidExecutableObservation("group segment size overflow"),
            ));
        }
    };
    let generation = core.next_identity;
    let next_generation = match generation.checked_add(1) {
        Some(next) => next,
        None => {
            return Err(reject_pending_dispatch(
                &mut core.api,
                &mut prepared,
                HsaRuntimeAdapterError::InvalidExecutableObservation(
                    "dispatch generation overflow",
                ),
            ));
        }
    };

    let address = match core.api.memory_allocate(core.kernarg_pool, kernarg.len()) {
        Ok(address) => address,
        Err(primary) => {
            return Err(cleanup_dispatch(
                &mut core.api,
                None,
                Some(prepared.queue),
                None,
                primary,
            ));
        }
    };
    let required_alignment = kernel.kernarg_segment_alignment as usize;
    if !address.is_multiple_of(required_alignment) {
        return Err(cleanup_dispatch(
            &mut core.api,
            Some(address),
            Some(prepared.queue),
            None,
            ApiError {
                operation: "validate HSA kernarg allocation alignment",
                status: -1,
            },
        ));
    }
    if let Err(primary) = core.api.allow_access(core.agent, address) {
        return Err(cleanup_dispatch(
            &mut core.api,
            Some(address),
            Some(prepared.queue),
            None,
            primary,
        ));
    }
    core.api.write_memory(address, kernarg);
    let queue = prepared.queue;
    let signal = match core.api.signal_create(1) {
        Ok(signal) => signal,
        Err(primary) => {
            return Err(cleanup_dispatch(
                &mut core.api,
                Some(address),
                Some(queue),
                None,
                primary,
            ));
        }
    };
    if let Err(primary) = core.api.queue_async_error(&queue) {
        return Err(cleanup_dispatch(
            &mut core.api,
            Some(address),
            Some(queue),
            Some(signal),
            primary,
        ));
    }
    let pre_submit = PreSubmitDispatch {
        queue,
        kernarg_address: address,
        completion_signal: signal,
    };
    let packet_id = match core.api.publish_dispatch(
        &pre_submit.queue,
        aql_grid,
        geometry.workgroup(),
        kernel.private_segment_size,
        group_segment_size,
        kernel.kernel_object,
        pre_submit.kernarg_address,
        pre_submit.completion_signal,
    ) {
        Ok(packet_id) => packet_id,
        Err(primary) => {
            return Err(cleanup_dispatch(
                &mut core.api,
                Some(pre_submit.kernarg_address),
                Some(pre_submit.queue),
                Some(pre_submit.completion_signal),
                primary,
            ));
        }
    };
    core.next_identity = next_generation;
    let submitted = SubmittedDispatch {
        resources: pre_submit,
        packet_id,
    };
    let (quiesced, queue_error) =
        match await_quiescence(&mut core.api, submitted, core.completion_timeout) {
            CompletionTransition::Quiesced {
                dispatch,
                queue_error,
            } => (dispatch, queue_error),
            CompletionTransition::Unquiesced(unquiesced) => terminate_unquiesced(unquiesced),
        };
    let SubmittedDispatch {
        resources,
        packet_id,
    } = quiesced.0;
    let queue_id = resources.queue.id();
    let signal = resources.completion_signal;
    match queue_error {
        Some(primary) => {
            return Err(cleanup_dispatch(
                &mut core.api,
                Some(resources.kernarg_address),
                Some(resources.queue),
                Some(resources.completion_signal),
                primary,
            ));
        }
        None => cleanup_completed(
            &mut core.api,
            resources.kernarg_address,
            resources.queue,
            resources.completion_signal,
        ),
    }
    let dispatch_identity = derive_dispatch_identity(
        core.environment.runtime().instance(),
        queue_id,
        packet_id,
        signal,
        generation,
    );
    HsaDispatchObservationV1::new(
        dispatch_identity,
        executable.identity,
        kernel.identity,
        geometry,
        true,
    )
    .map_err(|_| HsaRuntimeAdapterError::InvalidExecutableObservation("dispatch identity"))
}

fn await_quiescence<A: DispatchApi>(
    api: &mut A,
    submitted: SubmittedDispatch,
    timeout: Duration,
) -> CompletionTransition {
    #[cfg(feature = "hardware-test-hooks")]
    if record_post_submit_wait_phase().is_err() {
        return CompletionTransition::Unquiesced(UnquiescedDispatch {
            submitted,
            reason: UnquiescedReason::TestPhaseEvidence,
        });
    }
    let started = Instant::now();
    let deadline = started.checked_add(timeout).unwrap_or(started);
    let last_observation = loop {
        let observation = api.signal_load_acquire(submitted.resources.completion_signal);
        if observation == 0 {
            let queue_error = api.queue_async_error(&submitted.resources.queue).err();
            return CompletionTransition::Quiesced {
                dispatch: QuiescedDispatch(submitted),
                queue_error,
            };
        }
        if let Err(error) = api.queue_async_error(&submitted.resources.queue) {
            return CompletionTransition::Unquiesced(UnquiescedDispatch {
                submitted,
                reason: UnquiescedReason::QueueError(error),
            });
        }
        if Instant::now() >= deadline {
            break observation;
        }
        std::thread::yield_now();
    };
    CompletionTransition::Unquiesced(UnquiescedDispatch {
        submitted,
        reason: UnquiescedReason::CompletionDeadline { last_observation },
    })
}

#[cfg(feature = "hardware-test-hooks")]
fn record_post_submit_wait_phase() -> std::io::Result<()> {
    use std::io::Write;

    const VARIABLE: &str = "FE2O3_HSA_TEST_POST_SUBMIT_PHASE";
    const RECORD: &[u8] = b"fe2o3-hsa-post-submit-wait-v1\n";
    let Some(path) = std::env::var_os(VARIABLE) else {
        return Ok(());
    };
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(RECORD)?;
    file.sync_all()
}

fn terminate_unquiesced(unquiesced: UnquiescedDispatch) -> ! {
    let _reason = unquiesced.reason;
    let _packet_id = unquiesced.submitted.packet_id;
    #[cfg(feature = "hardware-test-hooks")]
    let _ = record_test_terminal_reason(unquiesced.reason);
    // Returning would release caller-side allocations while the GPU may still
    // reference them. Process termination is the production terminal policy.
    let _retained_authority = std::mem::ManuallyDrop::new(unquiesced);
    std::process::abort()
}

#[cfg(feature = "hardware-test-hooks")]
fn record_test_terminal_reason(reason: UnquiescedReason) -> std::io::Result<()> {
    use std::io::Write;

    const VARIABLE: &str = "FE2O3_HSA_TEST_POST_SUBMIT_PHASE";
    let Some(path) = std::env::var_os(VARIABLE) else {
        return Ok(());
    };
    let record: &[u8] = match reason {
        UnquiescedReason::QueueError(_) => b"fe2o3-hsa-unquiesced-queue-error-v1\n",
        UnquiescedReason::CompletionDeadline { .. } => b"fe2o3-hsa-unquiesced-deadline-v1\n",
        UnquiescedReason::TestPhaseEvidence => b"fe2o3-hsa-test-evidence-failure-v1\n",
    };
    let mut file = std::fs::OpenOptions::new().append(true).open(path)?;
    file.write_all(record)?;
    file.sync_all()
}

fn checked_aql_grid(geometry: HsaLaunchGeometryV1) -> Result<[u32; 3], HsaRuntimeAdapterError> {
    let blocks = geometry.grid();
    let workgroup = geometry.workgroup();
    let mut result = [0; 3];
    for index in 0..3 {
        result[index] = blocks[index].checked_mul(workgroup[index]).ok_or(
            HsaRuntimeAdapterError::InvalidExecutableObservation("AQL grid size overflow"),
        )?;
        if result[index] == 0 {
            return Err(HsaRuntimeAdapterError::InvalidExecutableObservation(
                "zero AQL grid size",
            ));
        }
    }
    Ok(result)
}

fn reviewed_queue_size(minimum: u32, maximum: u32) -> Result<u32, HsaRuntimeAdapterError> {
    if minimum == 0 || maximum < minimum || !minimum.is_power_of_two() || !maximum.is_power_of_two()
    {
        return Err(HsaRuntimeAdapterError::InvalidExecutableObservation(
            "HSA queue limits",
        ));
    }
    Ok(64_u32.clamp(minimum, maximum))
}

pub(crate) fn destroy_pending_dispatch<A: DispatchApi>(
    api: &mut A,
    pending: &mut Option<PendingDispatch>,
) {
    if let Some(mut pending) = pending.take()
        && api.queue_destroy(&mut pending.queue).is_err()
    {
        std::process::abort();
    }
}

fn reject_pending_dispatch<A: DispatchApi>(
    api: &mut A,
    pending: &mut PendingDispatch,
    primary: HsaRuntimeAdapterError,
) -> HsaRuntimeAdapterError {
    if api.queue_destroy(&mut pending.queue).is_err() {
        std::process::abort();
    }
    primary
}

fn cleanup_dispatch<A: DispatchApi>(
    api: &mut A,
    address: Option<usize>,
    mut queue: Option<QueueHandle>,
    signal: Option<u64>,
    primary: ApiError,
) -> HsaRuntimeAdapterError {
    if let Some(signal) = signal
        && api.signal_destroy(signal).is_err()
    {
        std::process::abort();
    }
    if let Some(address) = address
        && api.memory_free(address).is_err()
    {
        std::process::abort();
    }
    if let Some(queue) = queue.as_mut()
        && api.queue_destroy(queue).is_err()
    {
        std::process::abort();
    }
    HsaRuntimeAdapterError::api(primary)
}

fn cleanup_completed<A: DispatchApi>(
    api: &mut A,
    address: usize,
    mut queue: QueueHandle,
    signal: u64,
) {
    if api.signal_destroy(signal).is_err() {
        std::process::abort();
    }
    if api.memory_free(address).is_err() {
        std::process::abort();
    }
    if api.queue_destroy(&mut queue).is_err() {
        std::process::abort();
    }
}

fn derive_dispatch_identity(
    runtime: [u8; 16],
    queue: u64,
    packet: u64,
    signal: u64,
    generation: u64,
) -> [u8; 16] {
    let mut hasher = Sha256::new();
    hasher.update(b"fe2o3-hsa-aql-dispatch-v1\0");
    hasher.update(runtime);
    hasher.update(queue.to_le_bytes());
    hasher.update(packet.to_le_bytes());
    hasher.update(signal.to_le_bytes());
    hasher.update(generation.to_le_bytes());
    let digest = hasher.finalize();
    let mut result = [0; 16];
    result.copy_from_slice(&digest[..16]);
    result
}

fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{
        AgentFacts, EnvironmentApi, ExecutableApi, HipFacts, PoolFacts, RuntimeFacts, SymbolFacts,
    };
    use crate::lifecycle::ExecutableState;
    use fe2o3_amd_target::AmdTargetId;
    use fe2o3_artifacts::{DigestAlgorithm, PayloadDigest};
    use fe2o3_host::{
        HsaAgentIdentityV1, HsaEnvironmentObservationV1, HsaExecutableObjectIdentityV1,
        HsaKernelObjectIdentityV1, HsaPhysicalDeviceIdentityV1, HsaRuntimeIdentityV1,
    };
    use std::collections::{BTreeMap, VecDeque};

    #[derive(Default)]
    struct MockApi {
        log: Vec<&'static str>,
        failures: BTreeMap<&'static str, i32>,
        async_calls: usize,
        fail_async_call: Option<usize>,
        completion: i64,
        completion_sequence: VecDeque<i64>,
        written: Vec<u8>,
        published_grid: Option<[u32; 3]>,
    }

    impl MockApi {
        fn call(&mut self, operation: &'static str) -> Result<(), ApiError> {
            self.log.push(operation);
            match self.failures.get(operation) {
                Some(status) => Err(ApiError {
                    operation,
                    status: *status,
                }),
                None => Ok(()),
            }
        }
    }

    impl EnvironmentApi for MockApi {
        fn initialize(&mut self) -> Result<RuntimeFacts, ApiError> {
            unreachable!()
        }

        fn shut_down(&mut self) -> Result<(), ApiError> {
            self.call("shutdown")
        }

        fn observe_hip_device(&mut self, _ordinal: i32) -> Result<HipFacts, ApiError> {
            unreachable!()
        }

        fn collect_agents(&mut self) -> Result<Vec<AgentFacts>, ApiError> {
            unreachable!()
        }

        fn collect_kernarg_pools(&mut self) -> Result<Vec<PoolFacts>, ApiError> {
            unreachable!()
        }
    }

    impl ExecutableApi for MockApi {
        fn reader_create(&mut self, _bytes: &[u8]) -> Result<u64, ApiError> {
            unreachable!()
        }

        fn reader_destroy(&mut self, _reader: u64) -> Result<(), ApiError> {
            unreachable!()
        }

        fn executable_create(&mut self, _profile: u32) -> Result<u64, ApiError> {
            unreachable!()
        }

        fn executable_load(
            &mut self,
            _executable: u64,
            _agent: u64,
            _reader: u64,
        ) -> Result<u64, ApiError> {
            unreachable!()
        }

        fn executable_freeze(&mut self, _executable: u64) -> Result<(), ApiError> {
            unreachable!()
        }

        fn executable_destroy(&mut self, _executable: u64) -> Result<(), ApiError> {
            unreachable!()
        }

        fn resolve_symbol(
            &mut self,
            _executable: u64,
            _agent: u64,
            _name: &str,
        ) -> Result<SymbolFacts, ApiError> {
            unreachable!()
        }
    }

    impl DispatchApi for MockApi {
        fn memory_allocate(&mut self, _pool: u64, _len: usize) -> Result<usize, ApiError> {
            self.call("memory_allocate")?;
            Ok(0x1000)
        }

        fn allow_access(&mut self, _agent: u64, _address: usize) -> Result<(), ApiError> {
            self.call("allow_access")
        }

        fn write_memory(&mut self, _address: usize, bytes: &[u8]) {
            self.log.push("write_memory");
            self.written = bytes.to_vec();
        }

        fn memory_free(&mut self, _address: usize) -> Result<(), ApiError> {
            self.call("memory_free")
        }

        fn queue_create(&mut self, _agent: u64, size: u32) -> Result<QueueHandle, ApiError> {
            self.call("queue_create")?;
            Ok(QueueHandle::for_test(0xabc0, 41, size))
        }

        fn queue_async_error(&mut self, _queue: &QueueHandle) -> Result<(), ApiError> {
            self.log.push("queue_async_error");
            self.async_calls += 1;
            if self.fail_async_call == Some(self.async_calls) {
                Err(ApiError {
                    operation: "queue_async_error",
                    status: 82,
                })
            } else {
                self.failures
                    .get("queue_async_error")
                    .map_or(Ok(()), |status| {
                        Err(ApiError {
                            operation: "queue_async_error",
                            status: *status,
                        })
                    })
            }
        }

        fn queue_destroy(&mut self, _queue: &mut QueueHandle) -> Result<(), ApiError> {
            self.call("queue_destroy")
        }

        fn signal_create(&mut self, _initial_value: i64) -> Result<u64, ApiError> {
            self.call("signal_create")?;
            Ok(51)
        }

        fn signal_destroy(&mut self, _signal: u64) -> Result<(), ApiError> {
            self.call("signal_destroy")
        }

        fn signal_load_acquire(&mut self, _signal: u64) -> i64 {
            self.log.push("signal_load");
            self.completion_sequence
                .pop_front()
                .unwrap_or(self.completion)
        }

        fn publish_dispatch(
            &mut self,
            _queue: &QueueHandle,
            grid: [u32; 3],
            _workgroup: [u32; 3],
            _private_segment_size: u32,
            _group_segment_size: u32,
            _kernel_object: u64,
            _kernarg: usize,
            _completion_signal: u64,
        ) -> Result<u64, ApiError> {
            self.call("publish")?;
            self.published_grid = Some(grid);
            Ok(61)
        }
    }

    fn environment() -> HsaEnvironmentObservationV1 {
        let target = AmdTargetId::parse("gfx942").unwrap();
        let runtime = HsaRuntimeIdentityV1::new(
            "ROCr",
            "1.18",
            DigestAlgorithm::Sha256.calculate(b"runtime"),
            [1; 16],
        )
        .unwrap();
        let physical = HsaPhysicalDeviceIdentityV1::new([2; 16], 2, 0, target).unwrap();
        let agent = HsaAgentIdentityV1::new([1; 16], 20, [2; 16], target).unwrap();
        HsaEnvironmentObservationV1::new(runtime, physical, agent).unwrap()
    }

    fn make_core(api: MockApi) -> AdapterCore<MockApi> {
        AdapterCore {
            api,
            environment: environment(),
            agent: 20,
            profile: 0,
            queue_min_size: 64,
            queue_max_size: 1024,
            kernarg_pool: 30,
            completion_timeout: COMPLETION_TIMEOUT,
            next_identity: 1,
            runtime_live: true,
            _context: None,
        }
    }

    fn handles() -> (ReviewedHsaExecutableV1, ReviewedHsaKernelV1) {
        let executable_identity = HsaExecutableObjectIdentityV1::new([3; 32]).unwrap();
        let kernel_identity = HsaKernelObjectIdentityV1::new([4; 32]).unwrap();
        (
            ReviewedHsaExecutableV1 {
                state: Some(ExecutableState {
                    bytes: b"code".to_vec().into_boxed_slice(),
                    reader: 11,
                    executable: 12,
                    _loaded_code_object: 13,
                    identity: executable_identity,
                }),
            },
            ReviewedHsaKernelV1 {
                symbol: 14,
                kernel_object: 15,
                executable_identity,
                identity: kernel_identity,
                kernarg_segment_size: TOTAL_BYTES as u32,
                kernarg_segment_alignment: 8,
                group_segment_size: 32,
                private_segment_size: 64,
            },
        )
    }

    fn geometry() -> HsaLaunchGeometryV1 {
        HsaLaunchGeometryV1::new([2, 1, 1], [256, 1, 1], 0)
    }

    fn kernarg() -> [u8; TOTAL_BYTES] {
        let mut bytes = [0; TOTAL_BYTES];
        for (index, byte) in bytes[..EXPLICIT_BYTES].iter_mut().enumerate() {
            *byte = index as u8;
        }
        bytes
    }

    #[test]
    fn cov6_hidden_layout_is_exact_and_preserves_the_explicit_prefix() {
        let (executable, kernel) = handles();
        let mut core = make_core(MockApi::default());
        let mut pending = None;
        let mut bytes = kernarg();
        let explicit = bytes[..EXPLICIT_BYTES].to_vec();
        let observation = prepare_implicit_kernarg(
            &mut core,
            &mut pending,
            &executable,
            &kernel,
            geometry(),
            EXPLICIT_BYTES,
            IMPLICIT_OFFSET,
            IMPLICIT_BYTES,
            &mut bytes,
        )
        .unwrap();
        assert!(observation.initialized());
        assert_eq!(&bytes[..EXPLICIT_BYTES], explicit);
        let mut expected = [0_u8; IMPLICIT_BYTES];
        expected[0..4].copy_from_slice(&2_u32.to_le_bytes());
        expected[4..8].copy_from_slice(&1_u32.to_le_bytes());
        expected[8..12].copy_from_slice(&1_u32.to_le_bytes());
        expected[12..14].copy_from_slice(&256_u16.to_le_bytes());
        expected[14..16].copy_from_slice(&1_u16.to_le_bytes());
        expected[16..18].copy_from_slice(&1_u16.to_le_bytes());
        expected[64..66].copy_from_slice(&1_u16.to_le_bytes());
        expected[200..208].copy_from_slice(&0xabc0_u64.to_le_bytes());
        assert_eq!(&bytes[IMPLICIT_OFFSET..], expected);
        destroy_pending_dispatch(&mut core.api, &mut pending);
    }

    #[test]
    fn implicit_initialization_rejects_layout_and_handle_substitution() {
        let (executable, mut kernel) = handles();
        for (explicit, offset, implicit) in [(47, 48, 256), (48, 49, 255), (48, 48, 255)] {
            let mut core = make_core(MockApi::default());
            let mut pending = None;
            let mut bytes = kernarg();
            assert!(matches!(
                prepare_implicit_kernarg(
                    &mut core,
                    &mut pending,
                    &executable,
                    &kernel,
                    geometry(),
                    explicit,
                    offset,
                    implicit,
                    &mut bytes,
                ),
                Err(HsaRuntimeAdapterError::InvalidImplicitKernarg(_))
            ));
            assert!(pending.is_none());
            assert!(core.api.log.is_empty());
        }
        let mut core = make_core(MockApi::default());
        let mut pending = None;
        let mut bytes = kernarg();
        assert!(matches!(
            prepare_implicit_kernarg(
                &mut core,
                &mut pending,
                &executable,
                &kernel,
                HsaLaunchGeometryV1::new([2, 1, 1], [256, 1, 1], 1),
                48,
                48,
                256,
                &mut bytes,
            ),
            Err(HsaRuntimeAdapterError::InvalidImplicitKernarg(_))
        ));
        assert!(core.api.log.is_empty());

        kernel.executable_identity = HsaExecutableObjectIdentityV1::new([9; 32]).unwrap();
        let mut core = make_core(MockApi::default());
        let mut pending = None;
        let mut bytes = kernarg();
        assert!(matches!(
            prepare_implicit_kernarg(
                &mut core,
                &mut pending,
                &executable,
                &kernel,
                geometry(),
                48,
                48,
                256,
                &mut bytes,
            ),
            Err(HsaRuntimeAdapterError::InvalidImplicitKernarg(_))
        ));

        let (_, mut wrong_abi) = handles();
        wrong_abi.kernarg_segment_size = (TOTAL_BYTES - 1) as u32;
        let mut core = make_core(MockApi::default());
        let mut pending = None;
        assert!(matches!(
            prepare_implicit_kernarg(
                &mut core,
                &mut pending,
                &executable,
                &wrong_abi,
                geometry(),
                48,
                48,
                256,
                &mut bytes,
            ),
            Err(HsaRuntimeAdapterError::InvalidImplicitKernarg(_))
        ));
        assert!(core.api.log.is_empty());
    }

    #[test]
    fn implicit_initialization_owns_one_exact_queue_binding() {
        let (executable, kernel) = handles();
        let mut core = make_core(MockApi::default());
        let mut pending = None;
        let mut bytes = kernarg();
        prepare_implicit_kernarg(
            &mut core,
            &mut pending,
            &executable,
            &kernel,
            geometry(),
            48,
            48,
            256,
            &mut bytes,
        )
        .unwrap();
        assert!(matches!(
            prepare_implicit_kernarg(
                &mut core,
                &mut pending,
                &executable,
                &kernel,
                geometry(),
                48,
                48,
                256,
                &mut bytes,
            ),
            Err(HsaRuntimeAdapterError::InvalidImplicitKernarg(_))
        ));
        assert_eq!(core.api.log, ["queue_create", "queue_async_error"]);
        destroy_pending_dispatch(&mut core.api, &mut pending);
        assert!(pending.is_none());
        assert!(core.api.log.ends_with(&["queue_destroy"]));
    }

    #[test]
    fn prepared_queue_and_kernarg_cannot_cross_kernel_identities() {
        let (executable, first) = handles();
        let mut second = ReviewedHsaKernelV1 {
            symbol: 16,
            kernel_object: 17,
            executable_identity: first.executable_identity,
            identity: HsaKernelObjectIdentityV1::new([5; 32]).unwrap(),
            kernarg_segment_size: first.kernarg_segment_size,
            kernarg_segment_alignment: first.kernarg_segment_alignment,
            group_segment_size: first.group_segment_size,
            private_segment_size: first.private_segment_size,
        };
        let mut core = make_core(MockApi::default());
        let mut pending = None;
        let mut bytes = kernarg();
        prepare_implicit_kernarg(
            &mut core,
            &mut pending,
            &executable,
            &first,
            geometry(),
            EXPLICIT_BYTES,
            IMPLICIT_OFFSET,
            IMPLICIT_BYTES,
            &mut bytes,
        )
        .unwrap();

        assert!(matches!(
            launch_and_wait(
                &mut core,
                &mut pending,
                &executable,
                &second,
                geometry(),
                &mut bytes,
            ),
            Err(HsaRuntimeAdapterError::InvalidExecutableObservation(
                "dispatch handle, geometry, or kernarg binding"
            ))
        ));
        assert!(pending.is_none());
        assert_eq!(core.api.log.last(), Some(&"queue_destroy"));
        assert!(!core.api.log.contains(&"memory_allocate"));

        second.executable_identity = HsaExecutableObjectIdentityV1::new([9; 32]).unwrap();
        let mut fresh = kernarg();
        assert!(matches!(
            prepare_implicit_kernarg(
                &mut core,
                &mut pending,
                &executable,
                &second,
                geometry(),
                EXPLICIT_BYTES,
                IMPLICIT_OFFSET,
                IMPLICIT_BYTES,
                &mut fresh,
            ),
            Err(HsaRuntimeAdapterError::InvalidImplicitKernarg(
                "kernel/executable identity"
            ))
        ));
        assert!(pending.is_none());
    }

    #[test]
    fn implicit_queue_creation_and_observation_fail_closed() {
        let (executable, kernel) = handles();
        let mut api = MockApi::default();
        api.failures.insert("queue_create", 71);
        let mut core = make_core(api);
        let mut pending = None;
        let mut bytes = kernarg();
        assert!(
            prepare_implicit_kernarg(
                &mut core,
                &mut pending,
                &executable,
                &kernel,
                geometry(),
                48,
                48,
                256,
                &mut bytes,
            )
            .is_err()
        );
        assert_eq!(core.api.log, ["queue_create"]);
    }

    #[test]
    fn synchronous_dispatch_retains_resources_until_zero_completion() {
        let (executable, kernel) = handles();
        let mut core = make_core(MockApi::default());
        let mut pending = None;
        let mut bytes = kernarg();
        prepare_implicit_kernarg(
            &mut core,
            &mut pending,
            &executable,
            &kernel,
            geometry(),
            48,
            48,
            256,
            &mut bytes,
        )
        .unwrap();
        let observation = launch_and_wait(
            &mut core,
            &mut pending,
            &executable,
            &kernel,
            geometry(),
            &mut bytes,
        )
        .unwrap();
        assert!(observation.completed());
        assert!(pending.is_none());
        assert_eq!(core.api.published_grid, Some([512, 1, 1]));
        assert_eq!(core.api.written, bytes);
        assert_eq!(
            core.api.log,
            [
                "queue_create",
                "queue_async_error",
                "memory_allocate",
                "allow_access",
                "write_memory",
                "signal_create",
                "queue_async_error",
                "publish",
                "signal_load",
                "queue_async_error",
                "signal_destroy",
                "memory_free",
                "queue_destroy",
            ]
        );
    }

    #[test]
    fn prepublication_failures_clean_every_live_resource_in_reverse_order() {
        let cases = [
            (
                "memory_allocate",
                vec![
                    "queue_create",
                    "queue_async_error",
                    "memory_allocate",
                    "queue_destroy",
                ],
            ),
            (
                "allow_access",
                vec![
                    "queue_create",
                    "queue_async_error",
                    "memory_allocate",
                    "allow_access",
                    "memory_free",
                    "queue_destroy",
                ],
            ),
            (
                "signal_create",
                vec![
                    "queue_create",
                    "queue_async_error",
                    "memory_allocate",
                    "allow_access",
                    "write_memory",
                    "signal_create",
                    "memory_free",
                    "queue_destroy",
                ],
            ),
        ];
        for (failure, expected) in cases {
            let (executable, kernel) = handles();
            let mut api = MockApi::default();
            api.failures.insert(failure, 77);
            let mut core = make_core(api);
            let mut pending = None;
            let mut bytes = kernarg();
            prepare_implicit_kernarg(
                &mut core,
                &mut pending,
                &executable,
                &kernel,
                geometry(),
                48,
                48,
                256,
                &mut bytes,
            )
            .unwrap();
            assert!(
                launch_and_wait(
                    &mut core,
                    &mut pending,
                    &executable,
                    &kernel,
                    geometry(),
                    &mut bytes,
                )
                .is_err()
            );
            assert_eq!(core.api.log, expected, "failure edge {failure}");
        }

        let (executable, kernel) = handles();
        let api = MockApi {
            fail_async_call: Some(2),
            ..MockApi::default()
        };
        let mut core = make_core(api);
        let mut pending = None;
        let mut bytes = kernarg();
        prepare_implicit_kernarg(
            &mut core,
            &mut pending,
            &executable,
            &kernel,
            geometry(),
            48,
            48,
            256,
            &mut bytes,
        )
        .unwrap();
        assert!(
            launch_and_wait(
                &mut core,
                &mut pending,
                &executable,
                &kernel,
                geometry(),
                &mut bytes,
            )
            .is_err()
        );
        assert!(core.api.log.ends_with(&[
            "signal_create",
            "queue_async_error",
            "signal_destroy",
            "memory_free",
            "queue_destroy",
        ]));
    }

    #[test]
    fn dispatch_allocation_must_meet_the_resolved_kernel_alignment() {
        let (executable, mut kernel) = handles();
        kernel.kernarg_segment_alignment = 8192;
        let mut core = make_core(MockApi::default());
        let mut pending = None;
        let mut bytes = kernarg();
        prepare_implicit_kernarg(
            &mut core,
            &mut pending,
            &executable,
            &kernel,
            geometry(),
            48,
            48,
            256,
            &mut bytes,
        )
        .unwrap();
        assert!(
            launch_and_wait(
                &mut core,
                &mut pending,
                &executable,
                &kernel,
                geometry(),
                &mut bytes,
            )
            .is_err()
        );
        assert!(!core.api.log.contains(&"publish"));
        assert!(core.api.log.ends_with(&["memory_free", "queue_destroy"]));
    }

    #[test]
    fn launch_rejects_kernarg_or_geometry_substitution_before_publication() {
        let (executable, kernel) = handles();
        let mut core = make_core(MockApi::default());
        let mut pending = None;
        let mut bytes = kernarg();
        prepare_implicit_kernarg(
            &mut core,
            &mut pending,
            &executable,
            &kernel,
            geometry(),
            48,
            48,
            256,
            &mut bytes,
        )
        .unwrap();
        bytes[0] ^= 1;
        assert!(
            launch_and_wait(
                &mut core,
                &mut pending,
                &executable,
                &kernel,
                geometry(),
                &mut bytes,
            )
            .is_err()
        );
        assert!(pending.is_none());
        assert_eq!(core.api.log.last(), Some(&"queue_destroy"));
        assert!(!core.api.log.contains(&"memory_allocate"));
    }

    #[test]
    fn publication_failure_is_definitely_presubmit_and_cleans_resources() {
        let (executable, kernel) = handles();
        let mut api = MockApi::default();
        api.failures.insert("publish", 79);
        let mut core = make_core(api);
        let mut pending = None;
        let mut bytes = kernarg();
        prepare_implicit_kernarg(
            &mut core,
            &mut pending,
            &executable,
            &kernel,
            geometry(),
            48,
            48,
            256,
            &mut bytes,
        )
        .unwrap();
        assert!(matches!(
            launch_and_wait(
                &mut core,
                &mut pending,
                &executable,
                &kernel,
                geometry(),
                &mut bytes,
            ),
            Err(HsaRuntimeAdapterError::RuntimeCall { status: 79, .. })
        ));
        assert!(
            core.api
                .log
                .ends_with(&["signal_destroy", "memory_free", "queue_destroy"])
        );
    }

    fn submitted_dispatch() -> SubmittedDispatch {
        SubmittedDispatch {
            resources: PreSubmitDispatch {
                queue: QueueHandle::for_test(0xabc0, 0, 64),
                kernarg_address: 0x1000,
                completion_signal: 51,
            },
            packet_id: 61,
        }
    }

    #[test]
    fn spurious_wakeups_repeat_until_exact_zero_completion() {
        let mut api = MockApi {
            completion_sequence: VecDeque::from([1, -1, 0]),
            ..MockApi::default()
        };
        assert!(matches!(
            await_quiescence(&mut api, submitted_dispatch(), COMPLETION_TIMEOUT),
            CompletionTransition::Quiesced {
                queue_error: None,
                ..
            }
        ));
        assert_eq!(
            api.log,
            [
                "signal_load",
                "queue_async_error",
                "signal_load",
                "queue_async_error",
                "signal_load",
                "queue_async_error",
            ]
        );
    }

    #[test]
    fn queue_fault_and_completion_deadline_remain_submitted_without_cleanup() {
        let mut faulted = MockApi {
            completion: 1,
            fail_async_call: Some(1),
            ..MockApi::default()
        };
        match await_quiescence(&mut faulted, submitted_dispatch(), COMPLETION_TIMEOUT) {
            CompletionTransition::Unquiesced(unquiesced) => assert!(matches!(
                unquiesced.reason,
                UnquiescedReason::QueueError(ApiError { status: 82, .. })
            )),
            CompletionTransition::Quiesced { .. } => panic!("faulted queue reported quiescence"),
        }
        assert!(!faulted.log.contains(&"signal_destroy"));
        assert!(!faulted.log.contains(&"memory_free"));
        assert!(!faulted.log.contains(&"queue_destroy"));

        let mut timed_out = MockApi {
            completion: 1,
            ..MockApi::default()
        };
        match await_quiescence(&mut timed_out, submitted_dispatch(), Duration::ZERO) {
            CompletionTransition::Unquiesced(unquiesced) => assert_eq!(
                unquiesced.reason,
                UnquiescedReason::CompletionDeadline {
                    last_observation: 1
                }
            ),
            CompletionTransition::Quiesced { .. } => {
                panic!("expired completion deadline reported quiescence")
            }
        }
        assert_eq!(timed_out.log, ["signal_load", "queue_async_error"]);
        assert!(!timed_out.log.contains(&"memory_free"));
    }

    #[test]
    fn completed_async_error_is_reported_after_conclusive_cleanup() {
        let (executable, kernel) = handles();
        let api = MockApi {
            fail_async_call: Some(3),
            ..MockApi::default()
        };
        let mut core = make_core(api);
        let mut pending = None;
        let mut bytes = kernarg();
        prepare_implicit_kernarg(
            &mut core,
            &mut pending,
            &executable,
            &kernel,
            geometry(),
            48,
            48,
            256,
            &mut bytes,
        )
        .unwrap();
        assert!(matches!(
            launch_and_wait(
                &mut core,
                &mut pending,
                &executable,
                &kernel,
                geometry(),
                &mut bytes,
            ),
            Err(HsaRuntimeAdapterError::RuntimeCall { status: 82, .. })
        ));
        assert!(
            core.api
                .log
                .ends_with(&["signal_destroy", "memory_free", "queue_destroy"])
        );
    }

    #[test]
    #[cfg(unix)]
    fn ambiguous_dispatch_cleanup_is_terminal() {
        const CHILD: &str = "FE2O3_HSA_AMBIGUOUS_DISPATCH_CLEANUP_CHILD";
        if let Ok(case) = std::env::var(CHILD) {
            let (executable, kernel) = handles();
            let mut api = MockApi::default();
            match case.as_str() {
                "implicit-queue" => {
                    api.fail_async_call = Some(1);
                    api.failures.insert("queue_destroy", 73);
                    let mut core = make_core(api);
                    let mut pending = None;
                    let mut bytes = kernarg();
                    let _ = prepare_implicit_kernarg(
                        &mut core,
                        &mut pending,
                        &executable,
                        &kernel,
                        geometry(),
                        48,
                        48,
                        256,
                        &mut bytes,
                    );
                }
                "presubmit-signal" | "quiesced-signal" => {
                    if case == "quiesced-signal" {
                        api.fail_async_call = Some(3);
                    } else {
                        api.failures.insert("publish", 74);
                    }
                    api.failures.insert("signal_destroy", 75);
                    let mut core = make_core(api);
                    let mut pending = None;
                    let mut bytes = kernarg();
                    prepare_implicit_kernarg(
                        &mut core,
                        &mut pending,
                        &executable,
                        &kernel,
                        geometry(),
                        48,
                        48,
                        256,
                        &mut bytes,
                    )
                    .unwrap();
                    let _ = launch_and_wait(
                        &mut core,
                        &mut pending,
                        &executable,
                        &kernel,
                        geometry(),
                        &mut bytes,
                    );
                }
                _ => panic!("unknown dispatch cleanup case"),
            }
            std::process::exit(91);
        }

        use std::os::unix::process::ExitStatusExt;
        for case in ["implicit-queue", "presubmit-signal", "quiesced-signal"] {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .arg("--exact")
                .arg("dispatch::tests::ambiguous_dispatch_cleanup_is_terminal")
                .arg("--nocapture")
                .env(CHILD, case)
                .status()
                .unwrap();
            assert_eq!(status.signal(), Some(6), "cleanup case {case}: {status}");
        }
    }

    #[allow(dead_code)]
    fn _payload_type_is_part_of_the_reviewed_surface(_: PayloadDigest) {}
}
