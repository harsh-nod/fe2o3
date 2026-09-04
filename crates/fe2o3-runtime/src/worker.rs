//! Bounded subprocess transport for terminal native runtime backends.

use core::fmt;
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::{
    BackendBindingV1, BackendCancellationV1, BackendDeviceDescriptionV1, BackendLaunchV1,
    BackendMemoryRegionV1, BackendPollV1, MAX_RUNTIME_MODULE_IMAGE_BYTES_V1,
    RuntimeAsyncCopyBackendV1, RuntimeBackendFailureV1, RuntimeBackendV1,
    RuntimeCancellationBackendV1, RuntimeExecutionCapabilitiesV1, RuntimeFlushBackendV1,
    RuntimeLaunchGeometryV1, RuntimeMemoryKindV1,
};

/// Space reserved above the largest facade payload for canonical codec fields.
pub const MAX_RUNTIME_WORKER_CODEC_OVERHEAD_BYTES_V1: usize = 1024 * 1024;
/// Maximum request or response frame, including bounded codec overhead.
pub const MAX_RUNTIME_WORKER_FRAME_BYTES_V1: usize =
    MAX_RUNTIME_MODULE_IMAGE_BYTES_V1 + MAX_RUNTIME_WORKER_CODEC_OVERHEAD_BYTES_V1;
/// Maximum bytes carried by a successful byte-response frame.
pub const MAX_RUNTIME_WORKER_BYTE_RESPONSE_BYTES_V1: usize =
    MAX_RUNTIME_WORKER_FRAME_BYTES_V1 - 1 - size_of::<u32>();
/// Exact first frame emitted by a conforming runtime worker.
pub const RUNTIME_WORKER_HANDSHAKE_V1: &[u8] = b"fe2o3-runtime-worker-v1";
/// Exact first frame emitted by a worker supporting the complete V4 extension set.
///
/// A distinct handshake prevents either endpoint from silently treating the
/// additive operations as part of the frozen Runtime Worker V1 wire surface.
/// This transport version is separate from the compiler's Worker V3 proof and
/// application protocol.
pub const RUNTIME_WORKER_HANDSHAKE_V4: &[u8] = b"fe2o3-runtime-worker-v4;extensions=flush-v1,async-copy-v1,cancellation-v1,execution-capabilities-v1";
/// Parent-side allowance reserved inside a caller deadline for a wait response.
pub const RUNTIME_WORKER_RESPONSE_GRACE_V1: Duration = Duration::from_millis(100);

/// Immutable command used to start a dedicated runtime worker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeWorkerCommandV1 {
    program: PathBuf,
    arguments: Vec<String>,
}

impl RuntimeWorkerCommandV1 {
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            arguments: Vec::new(),
        }
    }

    pub fn argument(mut self, argument: impl Into<String>) -> Self {
        self.arguments.push(argument.into());
        self
    }

    pub fn program(&self) -> &Path {
        &self.program
    }

    pub fn arguments(&self) -> &[String] {
        &self.arguments
    }
}

/// Backend operation presented to a worker protocol codec.
pub enum RuntimeWorkerOperationV1<'a> {
    EnumerateDevices,
    CreateStream {
        device: u64,
    },
    DestroyStream {
        stream: u64,
    },
    Allocate {
        device: u64,
        kind: RuntimeMemoryKindV1,
        byte_len: u64,
        alignment: u64,
    },
    ReleaseAllocation {
        allocation: u64,
    },
    WriteAllocation {
        allocation: u64,
        byte_offset: u64,
        bytes: &'a [u8],
    },
    ReadAllocation {
        allocation: u64,
        byte_offset: u64,
        byte_len: usize,
    },
    LoadModule {
        device: u64,
        image: &'a [u8],
    },
    UnloadModule {
        module: u64,
    },
    ResolveKernel {
        module: u64,
        name: &'a str,
        signature: [u8; 32],
    },
    Submit {
        stream: u64,
        kernel: u64,
        explicit_kernarg: &'a [u8],
        bindings: &'a [BackendBindingV1],
        dependencies: &'a [u64],
        geometry: RuntimeLaunchGeometryV1,
    },
    Poll {
        submission: u64,
    },
    Wait {
        submission: u64,
        timeout: Duration,
    },
    ReleaseSubmission {
        submission: u64,
    },
    RecordEvent {
        stream: u64,
        submission: u64,
    },
    ReleaseEvent {
        event: u64,
    },
    PeerCopy {
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
        dependencies: &'a [u64],
    },
}

/// Expected semantic class of one worker operation response.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeWorkerOperationKindV1 {
    EnumerateDevices,
    Handle,
    Unit,
    Bytes,
    Poll,
}

/// Decoded backend response returned by a worker protocol codec.
#[derive(Debug)]
pub enum RuntimeWorkerResponseV1 {
    Devices(Vec<BackendDeviceDescriptionV1>),
    Handle(u64),
    Unit,
    Bytes(Vec<u8>),
    Poll(BackendPollV1),
}

/// Codec implemented by a concrete KFD or HSA worker protocol.
pub trait RuntimeWorkerCodecV1 {
    type Error: std::error::Error + Send + Sync + 'static;

    fn encode_request_v1(
        &mut self,
        operation: RuntimeWorkerOperationV1<'_>,
    ) -> Result<Vec<u8>, Self::Error>;

    fn decode_response_v1(
        &mut self,
        expected: RuntimeWorkerOperationKindV1,
        response: &[u8],
    ) -> Result<RuntimeWorkerResponseV1, RuntimeBackendFailureV1<Self::Error>>;
}

/// Complete extension set negotiated by the Worker V4 handshake.
///
/// Implementing the V1 codec alone enables none of these operations. A V4
/// backend requires this trait and the exact V4 handshake before it publishes
/// capability, flush, asynchronous-copy, cancellation, or drain requests.
pub trait RuntimeWorkerCodecV4: RuntimeWorkerCodecV1 {
    fn encode_execution_capabilities_request_v4(
        &mut self,
        device: u64,
    ) -> Result<Vec<u8>, Self::Error>;

    fn decode_execution_capabilities_response_v4(
        &mut self,
        response: &[u8],
    ) -> Result<RuntimeExecutionCapabilitiesV1, RuntimeBackendFailureV1<Self::Error>>;

    fn encode_flush_stream_request_v4(&mut self, stream: u64) -> Result<Vec<u8>, Self::Error>;

    fn decode_flush_stream_response_v4(
        &mut self,
        response: &[u8],
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>>;

    fn encode_async_copy_request_v4(
        &mut self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
        dependencies: &[u64],
    ) -> Result<Vec<u8>, Self::Error>;

    fn decode_async_copy_response_v4(
        &mut self,
        response: &[u8],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>>;

    fn encode_cancel_request_v4(&mut self, submission: u64) -> Result<Vec<u8>, Self::Error>;

    fn decode_cancel_response_v4(
        &mut self,
        response: &[u8],
    ) -> Result<BackendCancellationV1, RuntimeBackendFailureV1<Self::Error>>;

    fn encode_drain_request_v4(
        &mut self,
        submission: u64,
        timeout: Duration,
    ) -> Result<Vec<u8>, Self::Error>;

    fn decode_drain_response_v4(
        &mut self,
        response: &[u8],
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>>;
}

const MAX_RUNTIME_WORKER_ERROR_BYTES_V1: usize = 4096;

/// Canonical bounded binary worker protocol failure.
#[derive(Debug)]
pub enum RuntimeBinaryCodecErrorV1 {
    Malformed(&'static str),
    Limit(&'static str),
    Remote(String),
}

impl fmt::Display for RuntimeBinaryCodecErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(detail) => {
                write!(formatter, "malformed runtime worker message: {detail}")
            }
            Self::Limit(detail) => write!(formatter, "runtime worker limit exceeded: {detail}"),
            Self::Remote(detail) => write!(formatter, "remote runtime backend failed: {detail}"),
        }
    }
}

impl std::error::Error for RuntimeBinaryCodecErrorV1 {}

impl From<RuntimeBinaryCodecErrorV1> for RuntimeBackendFailureV1<RuntimeBinaryCodecErrorV1> {
    fn from(error: RuntimeBinaryCodecErrorV1) -> Self {
        Self::Terminal(error)
    }
}

/// Canonical address-free V1 codec usable by every runtime worker backend.
#[derive(Clone, Copy, Debug, Default)]
pub struct RuntimeBinaryCodecV1;

impl RuntimeWorkerCodecV1 for RuntimeBinaryCodecV1 {
    type Error = RuntimeBinaryCodecErrorV1;

    fn encode_request_v1(
        &mut self,
        operation: RuntimeWorkerOperationV1<'_>,
    ) -> Result<Vec<u8>, Self::Error> {
        encode_binary_request_v1(operation)
    }

    fn decode_response_v1(
        &mut self,
        expected: RuntimeWorkerOperationKindV1,
        response: &[u8],
    ) -> Result<RuntimeWorkerResponseV1, RuntimeBackendFailureV1<Self::Error>> {
        decode_binary_response_v1(expected, response)
    }
}

/// Canonical address-free Worker V4 codec.
#[derive(Clone, Copy, Debug, Default)]
pub struct RuntimeBinaryCodecV4;

impl RuntimeWorkerCodecV1 for RuntimeBinaryCodecV4 {
    type Error = RuntimeBinaryCodecErrorV1;

    fn encode_request_v1(
        &mut self,
        operation: RuntimeWorkerOperationV1<'_>,
    ) -> Result<Vec<u8>, Self::Error> {
        encode_binary_request_v1(operation)
    }

    fn decode_response_v1(
        &mut self,
        expected: RuntimeWorkerOperationKindV1,
        response: &[u8],
    ) -> Result<RuntimeWorkerResponseV1, RuntimeBackendFailureV1<Self::Error>> {
        decode_binary_response_v1(expected, response)
    }
}

const OP_ENUMERATE_DEVICES_V1: u8 = 1;
const OP_CREATE_STREAM_V1: u8 = 2;
const OP_DESTROY_STREAM_V1: u8 = 3;
const OP_ALLOCATE_V1: u8 = 4;
const OP_RELEASE_ALLOCATION_V1: u8 = 5;
const OP_WRITE_ALLOCATION_V1: u8 = 6;
const OP_READ_ALLOCATION_V1: u8 = 7;
const OP_LOAD_MODULE_V1: u8 = 8;
const OP_UNLOAD_MODULE_V1: u8 = 9;
const OP_RESOLVE_KERNEL_V1: u8 = 10;
const OP_SUBMIT_V1: u8 = 11;
const OP_POLL_V1: u8 = 12;
const OP_WAIT_V1: u8 = 13;
const OP_RECORD_EVENT_V1: u8 = 14;
const OP_RELEASE_EVENT_V1: u8 = 15;
const OP_PEER_COPY_V1: u8 = 16;
const OP_RELEASE_SUBMISSION_V1: u8 = 17;
const OP_FLUSH_STREAM_V4: u8 = 18;
const OP_EXECUTION_CAPABILITIES_V4: u8 = 19;
const OP_ASYNC_COPY_V4: u8 = 20;
const OP_CANCEL_V4: u8 = 21;
const OP_DRAIN_V4: u8 = 22;

const RESPONSE_OK_V1: u8 = 0;
const RESPONSE_REJECTED_V1: u8 = 1;
const RESPONSE_QUIESCENT_V1: u8 = 2;
const RESPONSE_TERMINAL_V1: u8 = 3;

fn encode_binary_request_v1(
    operation: RuntimeWorkerOperationV1<'_>,
) -> Result<Vec<u8>, RuntimeBinaryCodecErrorV1> {
    let mut output = Vec::new();
    match operation {
        RuntimeWorkerOperationV1::EnumerateDevices => output.push(OP_ENUMERATE_DEVICES_V1),
        RuntimeWorkerOperationV1::CreateStream { device } => {
            output.push(OP_CREATE_STREAM_V1);
            put_u64_v1(&mut output, device);
        }
        RuntimeWorkerOperationV1::DestroyStream { stream } => {
            output.push(OP_DESTROY_STREAM_V1);
            put_u64_v1(&mut output, stream);
        }
        RuntimeWorkerOperationV1::Allocate {
            device,
            kind,
            byte_len,
            alignment,
        } => {
            output.push(OP_ALLOCATE_V1);
            put_u64_v1(&mut output, device);
            output.push(memory_kind_tag_v1(kind));
            put_u64_v1(&mut output, byte_len);
            put_u64_v1(&mut output, alignment);
        }
        RuntimeWorkerOperationV1::ReleaseAllocation { allocation } => {
            output.push(OP_RELEASE_ALLOCATION_V1);
            put_u64_v1(&mut output, allocation);
        }
        RuntimeWorkerOperationV1::WriteAllocation {
            allocation,
            byte_offset,
            bytes,
        } => {
            output.push(OP_WRITE_ALLOCATION_V1);
            put_u64_v1(&mut output, allocation);
            put_u64_v1(&mut output, byte_offset);
            put_blob_v1(&mut output, bytes, MAX_RUNTIME_WORKER_FRAME_BYTES_V1)?;
        }
        RuntimeWorkerOperationV1::ReadAllocation {
            allocation,
            byte_offset,
            byte_len,
        } => {
            if byte_len > MAX_RUNTIME_WORKER_BYTE_RESPONSE_BYTES_V1 {
                return Err(RuntimeBinaryCodecErrorV1::Limit("allocation read"));
            }
            output.push(OP_READ_ALLOCATION_V1);
            put_u64_v1(&mut output, allocation);
            put_u64_v1(&mut output, byte_offset);
            put_u32_v1(
                &mut output,
                u32::try_from(byte_len)
                    .map_err(|_| RuntimeBinaryCodecErrorV1::Limit("allocation read"))?,
            );
        }
        RuntimeWorkerOperationV1::LoadModule { device, image } => {
            output.push(OP_LOAD_MODULE_V1);
            put_u64_v1(&mut output, device);
            put_blob_v1(&mut output, image, MAX_RUNTIME_MODULE_IMAGE_BYTES_V1)?;
        }
        RuntimeWorkerOperationV1::UnloadModule { module } => {
            output.push(OP_UNLOAD_MODULE_V1);
            put_u64_v1(&mut output, module);
        }
        RuntimeWorkerOperationV1::ResolveKernel {
            module,
            name,
            signature,
        } => {
            output.push(OP_RESOLVE_KERNEL_V1);
            put_u64_v1(&mut output, module);
            put_blob_v1(
                &mut output,
                name.as_bytes(),
                crate::MAX_RUNTIME_KERNEL_NAME_BYTES_V1,
            )?;
            output.extend_from_slice(&signature);
        }
        RuntimeWorkerOperationV1::Submit {
            stream,
            kernel,
            explicit_kernarg,
            bindings,
            dependencies,
            geometry,
        } => {
            if bindings.len() > fe2o3_host_api::MAX_DISPATCH_BINDINGS_V1 {
                return Err(RuntimeBinaryCodecErrorV1::Limit("launch bindings"));
            }
            if dependencies.len() > crate::MAX_RUNTIME_DEPENDENCIES_V1 {
                return Err(RuntimeBinaryCodecErrorV1::Limit("launch dependencies"));
            }
            output.push(OP_SUBMIT_V1);
            put_u64_v1(&mut output, stream);
            put_u64_v1(&mut output, kernel);
            put_blob_v1(
                &mut output,
                explicit_kernarg,
                crate::MAX_RUNTIME_EXPLICIT_KERNARG_BYTES_V1,
            )?;
            put_count_v1(&mut output, bindings.len(), "launch bindings")?;
            for binding in bindings {
                put_backend_region_v1(&mut output, binding.region);
                put_u32_v1(&mut output, binding.kernarg_byte_offset);
            }
            put_dependencies_v1(&mut output, dependencies)?;
            for axis in geometry.grid {
                put_u32_v1(&mut output, axis);
            }
            for axis in geometry.workgroup {
                put_u32_v1(&mut output, axis);
            }
            put_u32_v1(&mut output, geometry.dynamic_shared_bytes);
        }
        RuntimeWorkerOperationV1::Poll { submission } => {
            output.push(OP_POLL_V1);
            put_u64_v1(&mut output, submission);
        }
        RuntimeWorkerOperationV1::Wait {
            submission,
            timeout,
        } => {
            output.push(OP_WAIT_V1);
            put_u64_v1(&mut output, submission);
            put_u64_v1(&mut output, timeout.as_secs());
            put_u32_v1(&mut output, timeout.subsec_nanos());
        }
        RuntimeWorkerOperationV1::ReleaseSubmission { submission } => {
            output.push(OP_RELEASE_SUBMISSION_V1);
            put_u64_v1(&mut output, submission);
        }
        RuntimeWorkerOperationV1::RecordEvent { stream, submission } => {
            output.push(OP_RECORD_EVENT_V1);
            put_u64_v1(&mut output, stream);
            put_u64_v1(&mut output, submission);
        }
        RuntimeWorkerOperationV1::ReleaseEvent { event } => {
            output.push(OP_RELEASE_EVENT_V1);
            put_u64_v1(&mut output, event);
        }
        RuntimeWorkerOperationV1::PeerCopy {
            stream,
            source,
            destination,
            dependencies,
        } => {
            if dependencies.len() > crate::MAX_RUNTIME_DEPENDENCIES_V1 {
                return Err(RuntimeBinaryCodecErrorV1::Limit("peer-copy dependencies"));
            }
            output.push(OP_PEER_COPY_V1);
            put_u64_v1(&mut output, stream);
            put_backend_region_v1(&mut output, source);
            put_backend_region_v1(&mut output, destination);
            put_dependencies_v1(&mut output, dependencies)?;
        }
    }
    if output.len() > MAX_RUNTIME_WORKER_FRAME_BYTES_V1 {
        return Err(RuntimeBinaryCodecErrorV1::Limit("encoded request frame"));
    }
    Ok(output)
}

fn decode_binary_response_v1(
    expected: RuntimeWorkerOperationKindV1,
    response: &[u8],
) -> Result<RuntimeWorkerResponseV1, RuntimeBackendFailureV1<RuntimeBinaryCodecErrorV1>> {
    let mut input = decode_binary_success_payload_v1(response)?;
    let decoded = match expected {
        RuntimeWorkerOperationKindV1::EnumerateDevices => {
            let count = input.count(crate::MAX_RUNTIME_DEVICES_V1, "device count")?;
            let mut devices = Vec::with_capacity(count);
            for _ in 0..count {
                let backend_device = input.u64()?;
                let name = input
                    .string(crate::MAX_RUNTIME_DEVICE_NAME_BYTES_V1)?
                    .to_owned();
                let target = input
                    .string(crate::MAX_RUNTIME_DEVICE_TARGET_BYTES_V1)?
                    .to_owned();
                let global_memory_bytes = input.u64()?;
                let capabilities = decode_capabilities_v1(input.u16()?)?;
                devices.push(BackendDeviceDescriptionV1 {
                    backend_device,
                    name,
                    target,
                    global_memory_bytes,
                    capabilities,
                });
            }
            RuntimeWorkerResponseV1::Devices(devices)
        }
        RuntimeWorkerOperationKindV1::Handle => RuntimeWorkerResponseV1::Handle(input.u64()?),
        RuntimeWorkerOperationKindV1::Unit => RuntimeWorkerResponseV1::Unit,
        RuntimeWorkerOperationKindV1::Bytes => {
            RuntimeWorkerResponseV1::Bytes(input.blob(MAX_RUNTIME_WORKER_FRAME_BYTES_V1)?.to_vec())
        }
        RuntimeWorkerOperationKindV1::Poll => {
            RuntimeWorkerResponseV1::Poll(decode_poll_v1(&mut input)?)
        }
    };
    input.finish().map_err(RuntimeBackendFailureV1::Terminal)?;
    Ok(decoded)
}

fn decode_binary_success_payload_v1(
    response: &[u8],
) -> Result<BinaryCursorV1<'_>, RuntimeBackendFailureV1<RuntimeBinaryCodecErrorV1>> {
    let mut input = BinaryCursorV1::new(response);
    let status = input.u8().map_err(RuntimeBackendFailureV1::Terminal)?;
    if status != RESPONSE_OK_V1 {
        let message = input
            .string(MAX_RUNTIME_WORKER_ERROR_BYTES_V1)
            .and_then(|message| {
                input.finish()?;
                Ok(message.to_owned())
            })
            .map_err(RuntimeBackendFailureV1::Terminal)?;
        let error = RuntimeBinaryCodecErrorV1::Remote(message);
        return Err(match status {
            RESPONSE_REJECTED_V1 => RuntimeBackendFailureV1::Rejected(error),
            RESPONSE_QUIESCENT_V1 => RuntimeBackendFailureV1::Quiescent(error),
            RESPONSE_TERMINAL_V1 => RuntimeBackendFailureV1::Terminal(error),
            _ => RuntimeBackendFailureV1::Terminal(RuntimeBinaryCodecErrorV1::Malformed(
                "response status",
            )),
        });
    }
    Ok(input)
}

fn decode_fixed_blob_response_v4<const N: usize>(
    response: &[u8],
    detail: &'static str,
) -> Result<[u8; N], RuntimeBackendFailureV1<RuntimeBinaryCodecErrorV1>> {
    let mut input = decode_binary_success_payload_v1(response)?;
    let bytes = input.blob(N).map_err(RuntimeBackendFailureV1::Terminal)?;
    let bytes: [u8; N] = bytes.try_into().map_err(|_| {
        RuntimeBackendFailureV1::Terminal(RuntimeBinaryCodecErrorV1::Malformed(detail))
    })?;
    input.finish().map_err(RuntimeBackendFailureV1::Terminal)?;
    Ok(bytes)
}

impl RuntimeWorkerCodecV4 for RuntimeBinaryCodecV4 {
    fn encode_execution_capabilities_request_v4(
        &mut self,
        device: u64,
    ) -> Result<Vec<u8>, Self::Error> {
        let mut output = vec![OP_EXECUTION_CAPABILITIES_V4];
        put_u64_v1(&mut output, device);
        Ok(output)
    }

    fn decode_execution_capabilities_response_v4(
        &mut self,
        response: &[u8],
    ) -> Result<RuntimeExecutionCapabilitiesV1, RuntimeBackendFailureV1<Self::Error>> {
        let bits = u16::from_le_bytes(decode_fixed_blob_response_v4(
            response,
            "execution-capabilities response",
        )?);
        decode_execution_capabilities_v4(bits).map_err(RuntimeBackendFailureV1::Terminal)
    }

    fn encode_flush_stream_request_v4(&mut self, stream: u64) -> Result<Vec<u8>, Self::Error> {
        let mut output = vec![OP_FLUSH_STREAM_V4];
        put_u64_v1(&mut output, stream);
        Ok(output)
    }

    fn decode_flush_stream_response_v4(
        &mut self,
        response: &[u8],
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        match decode_binary_response_v1(RuntimeWorkerOperationKindV1::Unit, response)? {
            RuntimeWorkerResponseV1::Unit => Ok(()),
            _ => Err(RuntimeBackendFailureV1::Terminal(
                RuntimeBinaryCodecErrorV1::Malformed("flush response"),
            )),
        }
    }

    fn encode_async_copy_request_v4(
        &mut self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
        dependencies: &[u64],
    ) -> Result<Vec<u8>, Self::Error> {
        if dependencies.len() > crate::MAX_RUNTIME_DEPENDENCIES_V1 {
            return Err(RuntimeBinaryCodecErrorV1::Limit("async-copy dependencies"));
        }
        let mut output = vec![OP_ASYNC_COPY_V4];
        put_u64_v1(&mut output, stream);
        put_backend_region_v1(&mut output, source);
        put_backend_region_v1(&mut output, destination);
        put_dependencies_v1(&mut output, dependencies)?;
        debug_assert!(output.len() <= MAX_RUNTIME_WORKER_FRAME_BYTES_V1);
        Ok(output)
    }

    fn decode_async_copy_response_v4(
        &mut self,
        response: &[u8],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        match decode_binary_response_v1(RuntimeWorkerOperationKindV1::Handle, response)? {
            RuntimeWorkerResponseV1::Handle(handle) => Ok(handle),
            _ => Err(RuntimeBackendFailureV1::Terminal(
                RuntimeBinaryCodecErrorV1::Malformed("async-copy response"),
            )),
        }
    }

    fn encode_cancel_request_v4(&mut self, submission: u64) -> Result<Vec<u8>, Self::Error> {
        let mut output = vec![OP_CANCEL_V4];
        put_u64_v1(&mut output, submission);
        Ok(output)
    }

    fn decode_cancel_response_v4(
        &mut self,
        response: &[u8],
    ) -> Result<BackendCancellationV1, RuntimeBackendFailureV1<Self::Error>> {
        match decode_fixed_blob_response_v4(response, "cancel response")? {
            [0] => Ok(BackendCancellationV1::Cancelled),
            [1] => Ok(BackendCancellationV1::TooLate),
            _ => Err(RuntimeBackendFailureV1::Terminal(
                RuntimeBinaryCodecErrorV1::Malformed("cancel response"),
            )),
        }
    }

    fn encode_drain_request_v4(
        &mut self,
        submission: u64,
        timeout: Duration,
    ) -> Result<Vec<u8>, Self::Error> {
        let mut output = vec![OP_DRAIN_V4];
        put_u64_v1(&mut output, submission);
        put_u64_v1(&mut output, timeout.as_secs());
        put_u32_v1(&mut output, timeout.subsec_nanos());
        Ok(output)
    }

    fn decode_drain_response_v4(
        &mut self,
        response: &[u8],
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
        match decode_binary_response_v1(RuntimeWorkerOperationKindV1::Poll, response)? {
            RuntimeWorkerResponseV1::Poll(poll) => Ok(poll),
            _ => Err(RuntimeBackendFailureV1::Terminal(
                RuntimeBinaryCodecErrorV1::Malformed("drain response"),
            )),
        }
    }
}

fn put_u16_v1(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn put_u32_v1(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn put_u64_v1(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn put_i64_v1(output: &mut Vec<u8>, value: i64) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn put_count_v1(
    output: &mut Vec<u8>,
    count: usize,
    detail: &'static str,
) -> Result<(), RuntimeBinaryCodecErrorV1> {
    put_u32_v1(
        output,
        u32::try_from(count).map_err(|_| RuntimeBinaryCodecErrorV1::Limit(detail))?,
    );
    Ok(())
}

fn put_blob_v1(
    output: &mut Vec<u8>,
    bytes: &[u8],
    maximum: usize,
) -> Result<(), RuntimeBinaryCodecErrorV1> {
    if bytes.len() > maximum {
        return Err(RuntimeBinaryCodecErrorV1::Limit("byte string"));
    }
    put_count_v1(output, bytes.len(), "byte string")?;
    output.extend_from_slice(bytes);
    Ok(())
}

fn put_dependencies_v1(
    output: &mut Vec<u8>,
    dependencies: &[u64],
) -> Result<(), RuntimeBinaryCodecErrorV1> {
    put_count_v1(output, dependencies.len(), "dependencies")?;
    for dependency in dependencies {
        put_u64_v1(output, *dependency);
    }
    Ok(())
}

fn memory_kind_tag_v1(kind: RuntimeMemoryKindV1) -> u8 {
    match kind {
        RuntimeMemoryKindV1::DeviceLocal => 1,
        RuntimeMemoryKindV1::HostVisible => 2,
    }
}

fn access_tag_v1(access: crate::RuntimeAccessV1) -> u8 {
    match access {
        crate::RuntimeAccessV1::Read => 1,
        crate::RuntimeAccessV1::Write => 2,
        crate::RuntimeAccessV1::ReadWrite => 3,
    }
}

fn put_backend_region_v1(output: &mut Vec<u8>, region: BackendMemoryRegionV1) {
    put_u64_v1(output, region.allocation);
    output.push(access_tag_v1(region.access));
    put_u64_v1(output, region.byte_offset);
    put_u64_v1(output, region.byte_len);
}

struct BinaryCursorV1<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> BinaryCursorV1<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, byte_len: usize) -> Result<&'a [u8], RuntimeBinaryCodecErrorV1> {
        let end = self
            .offset
            .checked_add(byte_len)
            .ok_or(RuntimeBinaryCodecErrorV1::Malformed("length overflow"))?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(RuntimeBinaryCodecErrorV1::Malformed("truncated message"))?;
        self.offset = end;
        Ok(bytes)
    }

    fn u8(&mut self) -> Result<u8, RuntimeBinaryCodecErrorV1> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, RuntimeBinaryCodecErrorV1> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }

    fn u32(&mut self) -> Result<u32, RuntimeBinaryCodecErrorV1> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn u64(&mut self) -> Result<u64, RuntimeBinaryCodecErrorV1> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn count(
        &mut self,
        maximum: usize,
        detail: &'static str,
    ) -> Result<usize, RuntimeBinaryCodecErrorV1> {
        let count = self.u32()? as usize;
        if count > maximum {
            return Err(RuntimeBinaryCodecErrorV1::Limit(detail));
        }
        Ok(count)
    }

    fn blob(&mut self, maximum: usize) -> Result<&'a [u8], RuntimeBinaryCodecErrorV1> {
        let byte_len = self.count(maximum, "byte string")?;
        self.take(byte_len)
    }

    fn string(&mut self, maximum: usize) -> Result<&'a str, RuntimeBinaryCodecErrorV1> {
        std::str::from_utf8(self.blob(maximum)?)
            .map_err(|_| RuntimeBinaryCodecErrorV1::Malformed("invalid UTF-8"))
    }

    fn finish(&self) -> Result<(), RuntimeBinaryCodecErrorV1> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(RuntimeBinaryCodecErrorV1::Malformed("trailing bytes"))
        }
    }
}

fn decode_memory_kind_v1(tag: u8) -> Result<RuntimeMemoryKindV1, RuntimeBinaryCodecErrorV1> {
    match tag {
        1 => Ok(RuntimeMemoryKindV1::DeviceLocal),
        2 => Ok(RuntimeMemoryKindV1::HostVisible),
        _ => Err(RuntimeBinaryCodecErrorV1::Malformed("memory kind")),
    }
}

fn decode_access_v1(tag: u8) -> Result<crate::RuntimeAccessV1, RuntimeBinaryCodecErrorV1> {
    match tag {
        1 => Ok(crate::RuntimeAccessV1::Read),
        2 => Ok(crate::RuntimeAccessV1::Write),
        3 => Ok(crate::RuntimeAccessV1::ReadWrite),
        _ => Err(RuntimeBinaryCodecErrorV1::Malformed("memory access")),
    }
}

fn decode_backend_region_v1(
    input: &mut BinaryCursorV1<'_>,
) -> Result<BackendMemoryRegionV1, RuntimeBinaryCodecErrorV1> {
    Ok(BackendMemoryRegionV1 {
        allocation: input.u64()?,
        access: decode_access_v1(input.u8()?)?,
        byte_offset: input.u64()?,
        byte_len: input.u64()?,
    })
}

fn capabilities_bits_v1(capabilities: crate::RuntimeCapabilitiesV1) -> u16 {
    u16::from(capabilities.typed_async_launch)
        | (u16::from(capabilities.streams) << 1)
        | (u16::from(capabilities.events) << 2)
        | (u16::from(capabilities.device_memory) << 3)
        | (u16::from(capabilities.host_visible_memory) << 4)
        | (u16::from(capabilities.peer_copy) << 5)
        | (u16::from(capabilities.multi_device) << 6)
        | (u16::from(capabilities.atomics) << 7)
        | (u16::from(capabilities.collectives) << 8)
}

fn decode_capabilities_v1(
    bits: u16,
) -> Result<crate::RuntimeCapabilitiesV1, RuntimeBinaryCodecErrorV1> {
    if bits & !0x01ff != 0 {
        return Err(RuntimeBinaryCodecErrorV1::Malformed("capability bits"));
    }
    Ok(crate::RuntimeCapabilitiesV1 {
        typed_async_launch: bits & 1 != 0,
        streams: bits & 2 != 0,
        events: bits & 4 != 0,
        device_memory: bits & 8 != 0,
        host_visible_memory: bits & 16 != 0,
        peer_copy: bits & 32 != 0,
        multi_device: bits & 64 != 0,
        atomics: bits & 128 != 0,
        collectives: bits & 256 != 0,
    })
}

fn execution_capabilities_bits_v4(capabilities: RuntimeExecutionCapabilitiesV1) -> u16 {
    u16::from(capabilities.native_async_copy)
        | (u16::from(capabilities.native_peer_copy) << 1)
        | (u16::from(capabilities.concurrent_compute) << 2)
        | (u16::from(capabilities.compute_copy_overlap) << 3)
        | (u16::from(capabilities.memory_pool) << 4)
        | (u16::from(capabilities.profiling) << 5)
        | (u16::from(capabilities.cancellation) << 6)
        | (u16::from(capabilities.atomics) << 7)
        | (u16::from(capabilities.collectives) << 8)
}

fn decode_execution_capabilities_v4(
    bits: u16,
) -> Result<RuntimeExecutionCapabilitiesV1, RuntimeBinaryCodecErrorV1> {
    if bits & !0x01ff != 0 {
        return Err(RuntimeBinaryCodecErrorV1::Malformed(
            "execution-capability bits",
        ));
    }
    Ok(RuntimeExecutionCapabilitiesV1 {
        native_async_copy: bits & 1 != 0,
        native_peer_copy: bits & 2 != 0,
        concurrent_compute: bits & 4 != 0,
        compute_copy_overlap: bits & 8 != 0,
        memory_pool: bits & 16 != 0,
        profiling: bits & 32 != 0,
        cancellation: bits & 64 != 0,
        atomics: bits & 128 != 0,
        collectives: bits & 256 != 0,
    })
}

fn encode_poll_v1(output: &mut Vec<u8>, poll: BackendPollV1) {
    match poll {
        BackendPollV1::Pending => output.push(0),
        BackendPollV1::Succeeded => output.push(1),
        BackendPollV1::Failed { code } => {
            output.push(2);
            put_i64_v1(output, code);
        }
    }
}

fn decode_poll_v1(
    input: &mut BinaryCursorV1<'_>,
) -> Result<BackendPollV1, RuntimeBinaryCodecErrorV1> {
    match input.u8()? {
        0 => Ok(BackendPollV1::Pending),
        1 => Ok(BackendPollV1::Succeeded),
        2 => Ok(BackendPollV1::Failed {
            code: i64::from_le_bytes(input.take(8)?.try_into().unwrap()),
        }),
        _ => Err(RuntimeBinaryCodecErrorV1::Malformed("poll state")),
    }
}

/// Explicit admission marker for canonical Runtime Worker V1 backends.
///
/// An implementation certifies that every successfully accepted operation can
/// reach publication and completion without a later explicit progress call.
/// This marker is intentionally not implemented as a blanket: downstream
/// backends may opt in only after reviewing that progress invariant. Backends
/// that defer publication must use Worker V4 instead.
///
/// Direct, multi-device, and native-XGMI KFD backends require explicit flush
/// progress and therefore cannot satisfy this bound:
///
/// ```compile_fail
/// use fe2o3_runtime::{KfdRuntimeBackendV1, RuntimeWorkerV1ImmediateProgressBackendV1};
/// fn require_v1<T: RuntimeWorkerV1ImmediateProgressBackendV1>() {}
/// require_v1::<KfdRuntimeBackendV1>();
/// ```
///
/// ```compile_fail
/// use fe2o3_runtime::{KfdMultiDeviceRuntimeBackendV1, RuntimeWorkerV1ImmediateProgressBackendV1};
/// fn require_v1<T: RuntimeWorkerV1ImmediateProgressBackendV1>() {}
/// require_v1::<KfdMultiDeviceRuntimeBackendV1>();
/// ```
///
/// ```compile_fail
/// use fe2o3_runtime::{KfdNativeXgmiRuntimeBackendV1, RuntimeWorkerV1ImmediateProgressBackendV1};
/// fn require_v1<T: RuntimeWorkerV1ImmediateProgressBackendV1>() {}
/// require_v1::<KfdNativeXgmiRuntimeBackendV1>();
/// ```
pub trait RuntimeWorkerV1ImmediateProgressBackendV1: RuntimeBackendV1 {}

/// Serves the canonical bounded Runtime Worker V1 protocol.
///
/// The hosted backend must never require deferred publication or cooperative
/// progress after a successful submission. A flush-dependent deployment must
/// use [`serve_runtime_backend_worker_v4`]; V1 cannot express that operation.
pub fn serve_runtime_backend_worker_v1<B, R, W>(
    mut backend: B,
    input: R,
    output: W,
) -> Result<(), RuntimeWorkerErrorV1>
where
    B: RuntimeWorkerV1ImmediateProgressBackendV1,
    R: Read,
    W: Write,
{
    serve_runtime_worker_v1(input, output, |request| {
        dispatch_binary_request_v1(&mut backend, request)
    })
}

/// Serves the canonical bounded Worker V4 protocol over a backend implementing
/// the complete negotiated extension set.
pub fn serve_runtime_backend_worker_v4<B, R, W>(
    mut backend: B,
    input: R,
    output: W,
) -> Result<(), RuntimeWorkerErrorV1>
where
    B: RuntimeFlushBackendV1 + RuntimeAsyncCopyBackendV1 + RuntimeCancellationBackendV1,
    R: Read,
    W: Write,
{
    serve_runtime_worker_v4(input, output, |request| {
        dispatch_binary_request_v4(&mut backend, request)
    })
}

fn dispatch_binary_request_v4<B>(
    backend: &mut B,
    request: &[u8],
) -> Result<Vec<u8>, RuntimeWorkerErrorV1>
where
    B: RuntimeFlushBackendV1 + RuntimeAsyncCopyBackendV1 + RuntimeCancellationBackendV1,
{
    let Some(operation) = request.first().copied() else {
        return dispatch_binary_request_v1(backend, request);
    };
    if !matches!(
        operation,
        OP_FLUSH_STREAM_V4
            | OP_EXECUTION_CAPABILITIES_V4
            | OP_ASYNC_COPY_V4
            | OP_CANCEL_V4
            | OP_DRAIN_V4
    ) {
        return dispatch_binary_request_v1(backend, request);
    }
    let mut input = BinaryCursorV1::new(request);
    let _operation = binary_u8_v1(&mut input)?;
    let response = match operation {
        OP_EXECUTION_CAPABILITIES_V4 => {
            let device = binary_u64_v1(&mut input)?;
            require_binary_end_v1(&input)?;
            let bits = execution_capabilities_bits_v4(backend.execution_capabilities_v1(device));
            encode_success_response_v1(|output| put_blob_v1(output, &bits.to_le_bytes(), 2))?
        }
        OP_FLUSH_STREAM_V4 => {
            let stream = binary_u64_v1(&mut input)?;
            require_binary_end_v1(&input)?;
            encode_unit_response_v1(backend.flush_stream_v1(stream))?
        }
        OP_ASYNC_COPY_V4 => {
            let stream = binary_u64_v1(&mut input)?;
            let source = binary_region_v1(&mut input)?;
            let destination = binary_region_v1(&mut input)?;
            let dependencies = binary_dependencies_v1(&mut input)?;
            require_binary_end_v1(&input)?;
            encode_handle_response_v1(backend.copy_async_v1(
                stream,
                source,
                destination,
                &dependencies,
            ))?
        }
        OP_CANCEL_V4 => {
            let submission = binary_u64_v1(&mut input)?;
            require_binary_end_v1(&input)?;
            encode_backend_response_v1(backend.cancel_v1(submission), |output, disposition| {
                let tag = match disposition {
                    BackendCancellationV1::Cancelled => 0,
                    BackendCancellationV1::TooLate => 1,
                };
                put_blob_v1(output, &[tag], 1)
            })?
        }
        OP_DRAIN_V4 => {
            let submission = binary_u64_v1(&mut input)?;
            let seconds = binary_u64_v1(&mut input)?;
            let nanoseconds = binary_u32_v1(&mut input)?;
            if nanoseconds >= 1_000_000_000 {
                return Err(RuntimeWorkerErrorV1::Protocol("invalid drain duration"));
            }
            let timeout = Duration::new(seconds, nanoseconds);
            let deadline = Instant::now()
                .checked_add(timeout)
                .ok_or(RuntimeWorkerErrorV1::InvalidDeadline)?;
            require_binary_end_v1(&input)?;
            encode_poll_response_v1(backend.drain_v1(submission, deadline))?
        }
        _ => unreachable!("V4 extension opcode was filtered above"),
    };
    if response.len() > MAX_RUNTIME_WORKER_FRAME_BYTES_V1 {
        return Err(RuntimeWorkerErrorV1::FrameTooLarge {
            actual: response.len(),
            maximum: MAX_RUNTIME_WORKER_FRAME_BYTES_V1,
        });
    }
    Ok(response)
}

fn dispatch_binary_request_v1<B: RuntimeBackendV1>(
    backend: &mut B,
    request: &[u8],
) -> Result<Vec<u8>, RuntimeWorkerErrorV1> {
    let mut input = BinaryCursorV1::new(request);
    let operation = input
        .u8()
        .map_err(|_| RuntimeWorkerErrorV1::Protocol("malformed canonical request"))?;
    let response = match operation {
        OP_ENUMERATE_DEVICES_V1 => {
            require_binary_end_v1(&input)?;
            encode_backend_response_v1(backend.enumerate_devices_v1(), |output, devices| {
                if devices.len() > crate::MAX_RUNTIME_DEVICES_V1 {
                    return Err(RuntimeBinaryCodecErrorV1::Limit("device count"));
                }
                put_count_v1(output, devices.len(), "device count")?;
                for device in devices {
                    put_u64_v1(output, device.backend_device);
                    put_blob_v1(
                        output,
                        device.name.as_bytes(),
                        crate::MAX_RUNTIME_DEVICE_NAME_BYTES_V1,
                    )?;
                    put_blob_v1(
                        output,
                        device.target.as_bytes(),
                        crate::MAX_RUNTIME_DEVICE_TARGET_BYTES_V1,
                    )?;
                    put_u64_v1(output, device.global_memory_bytes);
                    put_u16_v1(output, capabilities_bits_v1(device.capabilities));
                }
                Ok(())
            })?
        }
        OP_CREATE_STREAM_V1 => {
            let device = binary_u64_v1(&mut input)?;
            require_binary_end_v1(&input)?;
            encode_handle_response_v1(backend.create_stream_v1(device))?
        }
        OP_DESTROY_STREAM_V1 => {
            let stream = binary_u64_v1(&mut input)?;
            require_binary_end_v1(&input)?;
            encode_unit_response_v1(backend.destroy_stream_v1(stream))?
        }
        OP_ALLOCATE_V1 => {
            let device = binary_u64_v1(&mut input)?;
            let kind = decode_memory_kind_v1(binary_u8_v1(&mut input)?)
                .map_err(|_| RuntimeWorkerErrorV1::Protocol("invalid memory kind"))?;
            let byte_len = binary_u64_v1(&mut input)?;
            let alignment = binary_u64_v1(&mut input)?;
            require_binary_end_v1(&input)?;
            encode_handle_response_v1(backend.allocate_v1(device, kind, byte_len, alignment))?
        }
        OP_RELEASE_ALLOCATION_V1 => {
            let allocation = binary_u64_v1(&mut input)?;
            require_binary_end_v1(&input)?;
            encode_unit_response_v1(backend.release_allocation_v1(allocation))?
        }
        OP_WRITE_ALLOCATION_V1 => {
            let allocation = binary_u64_v1(&mut input)?;
            let byte_offset = binary_u64_v1(&mut input)?;
            let bytes = binary_blob_v1(&mut input, MAX_RUNTIME_WORKER_FRAME_BYTES_V1)?;
            require_binary_end_v1(&input)?;
            encode_unit_response_v1(backend.write_allocation_v1(allocation, byte_offset, bytes))?
        }
        OP_READ_ALLOCATION_V1 => {
            let allocation = binary_u64_v1(&mut input)?;
            let byte_offset = binary_u64_v1(&mut input)?;
            let byte_len = binary_u32_v1(&mut input)? as usize;
            if byte_len > MAX_RUNTIME_WORKER_BYTE_RESPONSE_BYTES_V1 {
                return Err(RuntimeWorkerErrorV1::Protocol("allocation read too large"));
            }
            require_binary_end_v1(&input)?;
            let mut bytes = vec![0; byte_len];
            match backend.read_allocation_v1(allocation, byte_offset, &mut bytes) {
                Ok(()) => encode_success_response_v1(|output| {
                    put_blob_v1(output, &bytes, MAX_RUNTIME_WORKER_BYTE_RESPONSE_BYTES_V1)
                })?,
                Err(failure) => encode_backend_failure_v1(failure),
            }
        }
        OP_LOAD_MODULE_V1 => {
            let device = binary_u64_v1(&mut input)?;
            let image = binary_blob_v1(&mut input, MAX_RUNTIME_MODULE_IMAGE_BYTES_V1)?;
            require_binary_end_v1(&input)?;
            encode_handle_response_v1(backend.load_module_v1(device, image))?
        }
        OP_UNLOAD_MODULE_V1 => {
            let module = binary_u64_v1(&mut input)?;
            require_binary_end_v1(&input)?;
            encode_unit_response_v1(backend.unload_module_v1(module))?
        }
        OP_RESOLVE_KERNEL_V1 => {
            let module = binary_u64_v1(&mut input)?;
            let name = binary_string_v1(&mut input, crate::MAX_RUNTIME_KERNEL_NAME_BYTES_V1)?;
            if name.is_empty() || name.as_bytes().contains(&0) {
                return Err(RuntimeWorkerErrorV1::Protocol("invalid kernel symbol"));
            }
            let signature: [u8; 32] = binary_take_v1(&mut input, 32)?
                .try_into()
                .map_err(|_| RuntimeWorkerErrorV1::Protocol("invalid kernel signature"))?;
            require_binary_end_v1(&input)?;
            encode_handle_response_v1(backend.resolve_kernel_v1(module, name, signature))?
        }
        OP_SUBMIT_V1 => {
            let stream = binary_u64_v1(&mut input)?;
            let kernel = binary_u64_v1(&mut input)?;
            let explicit_kernarg =
                binary_blob_v1(&mut input, crate::MAX_RUNTIME_EXPLICIT_KERNARG_BYTES_V1)?;
            let binding_count =
                binary_count_v1(&mut input, fe2o3_host_api::MAX_DISPATCH_BINDINGS_V1)?;
            let mut bindings = Vec::with_capacity(binding_count);
            for _ in 0..binding_count {
                let region = binary_region_v1(&mut input)?;
                let kernarg_byte_offset = binary_u32_v1(&mut input)?;
                bindings.push(BackendBindingV1 {
                    region,
                    kernarg_byte_offset,
                });
            }
            let dependencies = binary_dependencies_v1(&mut input)?;
            let geometry = RuntimeLaunchGeometryV1 {
                grid: [
                    binary_u32_v1(&mut input)?,
                    binary_u32_v1(&mut input)?,
                    binary_u32_v1(&mut input)?,
                ],
                workgroup: [
                    binary_u32_v1(&mut input)?,
                    binary_u32_v1(&mut input)?,
                    binary_u32_v1(&mut input)?,
                ],
                dynamic_shared_bytes: binary_u32_v1(&mut input)?,
            }
            .validate()
            .map_err(|_| RuntimeWorkerErrorV1::Protocol("invalid launch geometry"))?;
            require_binary_end_v1(&input)?;
            validate_binary_bindings_v1(explicit_kernarg, &bindings)?;
            encode_handle_response_v1(backend.submit_v1(BackendLaunchV1 {
                stream,
                kernel,
                explicit_kernarg,
                bindings: &bindings,
                dependencies: &dependencies,
                geometry,
                semantic_launch: crate::BackendSemanticLaunchV1::Ordinary,
            }))?
        }
        OP_POLL_V1 => {
            let submission = binary_u64_v1(&mut input)?;
            require_binary_end_v1(&input)?;
            encode_poll_response_v1(backend.poll_v1(submission))?
        }
        OP_WAIT_V1 => {
            let submission = binary_u64_v1(&mut input)?;
            let seconds = binary_u64_v1(&mut input)?;
            let nanoseconds = binary_u32_v1(&mut input)?;
            if nanoseconds >= 1_000_000_000 {
                return Err(RuntimeWorkerErrorV1::Protocol("invalid wait duration"));
            }
            let timeout = Duration::new(seconds, nanoseconds);
            let deadline = Instant::now()
                .checked_add(timeout)
                .ok_or(RuntimeWorkerErrorV1::InvalidDeadline)?;
            require_binary_end_v1(&input)?;
            encode_poll_response_v1(backend.wait_v1(submission, deadline))?
        }
        OP_RELEASE_SUBMISSION_V1 => {
            let submission = binary_u64_v1(&mut input)?;
            require_binary_end_v1(&input)?;
            encode_unit_response_v1(backend.release_submission_v1(submission))?
        }
        OP_RECORD_EVENT_V1 => {
            let stream = binary_u64_v1(&mut input)?;
            let submission = binary_u64_v1(&mut input)?;
            require_binary_end_v1(&input)?;
            encode_handle_response_v1(backend.record_event_v1(stream, submission))?
        }
        OP_RELEASE_EVENT_V1 => {
            let event = binary_u64_v1(&mut input)?;
            require_binary_end_v1(&input)?;
            encode_unit_response_v1(backend.release_event_v1(event))?
        }
        OP_PEER_COPY_V1 => {
            let stream = binary_u64_v1(&mut input)?;
            let source = binary_region_v1(&mut input)?;
            let destination = binary_region_v1(&mut input)?;
            let dependencies = binary_dependencies_v1(&mut input)?;
            require_binary_end_v1(&input)?;
            encode_handle_response_v1(backend.peer_copy_v1(
                stream,
                source,
                destination,
                &dependencies,
            ))?
        }
        _ => {
            return Err(RuntimeWorkerErrorV1::Protocol(
                "unknown canonical operation",
            ));
        }
    };
    if response.len() > MAX_RUNTIME_WORKER_FRAME_BYTES_V1 {
        return Err(RuntimeWorkerErrorV1::FrameTooLarge {
            actual: response.len(),
            maximum: MAX_RUNTIME_WORKER_FRAME_BYTES_V1,
        });
    }
    Ok(response)
}

fn require_binary_end_v1(input: &BinaryCursorV1<'_>) -> Result<(), RuntimeWorkerErrorV1> {
    input
        .finish()
        .map_err(|_| RuntimeWorkerErrorV1::Protocol("trailing canonical request bytes"))
}

fn binary_take_v1<'a>(
    input: &mut BinaryCursorV1<'a>,
    byte_len: usize,
) -> Result<&'a [u8], RuntimeWorkerErrorV1> {
    input
        .take(byte_len)
        .map_err(|_| RuntimeWorkerErrorV1::Protocol("truncated canonical request"))
}

fn binary_u8_v1(input: &mut BinaryCursorV1<'_>) -> Result<u8, RuntimeWorkerErrorV1> {
    input
        .u8()
        .map_err(|_| RuntimeWorkerErrorV1::Protocol("truncated canonical request"))
}

fn binary_u32_v1(input: &mut BinaryCursorV1<'_>) -> Result<u32, RuntimeWorkerErrorV1> {
    input
        .u32()
        .map_err(|_| RuntimeWorkerErrorV1::Protocol("truncated canonical request"))
}

fn binary_u64_v1(input: &mut BinaryCursorV1<'_>) -> Result<u64, RuntimeWorkerErrorV1> {
    input
        .u64()
        .map_err(|_| RuntimeWorkerErrorV1::Protocol("truncated canonical request"))
}

fn binary_count_v1(
    input: &mut BinaryCursorV1<'_>,
    maximum: usize,
) -> Result<usize, RuntimeWorkerErrorV1> {
    input
        .count(maximum, "canonical request count")
        .map_err(|_| RuntimeWorkerErrorV1::Protocol("invalid canonical request count"))
}

fn binary_blob_v1<'a>(
    input: &mut BinaryCursorV1<'a>,
    maximum: usize,
) -> Result<&'a [u8], RuntimeWorkerErrorV1> {
    input
        .blob(maximum)
        .map_err(|_| RuntimeWorkerErrorV1::Protocol("invalid canonical byte string"))
}

fn binary_string_v1<'a>(
    input: &mut BinaryCursorV1<'a>,
    maximum: usize,
) -> Result<&'a str, RuntimeWorkerErrorV1> {
    input
        .string(maximum)
        .map_err(|_| RuntimeWorkerErrorV1::Protocol("invalid canonical string"))
}

fn binary_region_v1(
    input: &mut BinaryCursorV1<'_>,
) -> Result<BackendMemoryRegionV1, RuntimeWorkerErrorV1> {
    decode_backend_region_v1(input)
        .map_err(|_| RuntimeWorkerErrorV1::Protocol("invalid canonical memory region"))
}

fn binary_dependencies_v1(
    input: &mut BinaryCursorV1<'_>,
) -> Result<Vec<u64>, RuntimeWorkerErrorV1> {
    let count = binary_count_v1(input, crate::MAX_RUNTIME_DEPENDENCIES_V1)?;
    let mut dependencies = Vec::with_capacity(count);
    for _ in 0..count {
        let dependency = binary_u64_v1(input)?;
        if dependencies.contains(&dependency) {
            return Err(RuntimeWorkerErrorV1::Protocol("duplicate dependency"));
        }
        dependencies.push(dependency);
    }
    Ok(dependencies)
}

fn validate_binary_bindings_v1(
    explicit_kernarg: &[u8],
    bindings: &[BackendBindingV1],
) -> Result<(), RuntimeWorkerErrorV1> {
    for (index, binding) in bindings.iter().enumerate() {
        let start = binding.kernarg_byte_offset as usize;
        let end = start
            .checked_add(crate::RUNTIME_DEVICE_POINTER_BYTES_V1 as usize)
            .ok_or(RuntimeWorkerErrorV1::Protocol("kernarg patch overflow"))?;
        if !binding
            .kernarg_byte_offset
            .is_multiple_of(crate::RUNTIME_DEVICE_POINTER_BYTES_V1)
            || end > explicit_kernarg.len()
            || explicit_kernarg[start..end].iter().any(|byte| *byte != 0)
            || bindings[..index].iter().any(|prior| {
                let prior_start = prior.kernarg_byte_offset as usize;
                let prior_end = prior_start + crate::RUNTIME_DEVICE_POINTER_BYTES_V1 as usize;
                start < prior_end && prior_start < end
            })
        {
            return Err(RuntimeWorkerErrorV1::Protocol("invalid kernarg patch"));
        }
        if binding.region.byte_len == 0
            || binding
                .region
                .byte_offset
                .checked_add(binding.region.byte_len)
                .is_none()
        {
            return Err(RuntimeWorkerErrorV1::Protocol("invalid binding range"));
        }
    }
    Ok(())
}

fn encode_success_response_v1(
    encode: impl FnOnce(&mut Vec<u8>) -> Result<(), RuntimeBinaryCodecErrorV1>,
) -> Result<Vec<u8>, RuntimeWorkerErrorV1> {
    let mut output = vec![RESPONSE_OK_V1];
    encode(&mut output)
        .map_err(|_| RuntimeWorkerErrorV1::Protocol("canonical response exceeds limits"))?;
    Ok(output)
}

fn encode_backend_response_v1<T, E>(
    result: Result<T, RuntimeBackendFailureV1<E>>,
    encode: impl FnOnce(&mut Vec<u8>, T) -> Result<(), RuntimeBinaryCodecErrorV1>,
) -> Result<Vec<u8>, RuntimeWorkerErrorV1> {
    match result {
        Ok(value) => encode_success_response_v1(|output| encode(output, value)),
        Err(failure) => Ok(encode_backend_failure_v1(failure)),
    }
}

fn encode_handle_response_v1<E>(
    result: Result<u64, RuntimeBackendFailureV1<E>>,
) -> Result<Vec<u8>, RuntimeWorkerErrorV1> {
    encode_backend_response_v1(result, |output, handle| {
        put_u64_v1(output, handle);
        Ok(())
    })
}

fn encode_unit_response_v1<E>(
    result: Result<(), RuntimeBackendFailureV1<E>>,
) -> Result<Vec<u8>, RuntimeWorkerErrorV1> {
    encode_backend_response_v1(result, |_, ()| Ok(()))
}

fn encode_poll_response_v1<E>(
    result: Result<BackendPollV1, RuntimeBackendFailureV1<E>>,
) -> Result<Vec<u8>, RuntimeWorkerErrorV1> {
    encode_backend_response_v1(result, |output, poll| {
        encode_poll_v1(output, poll);
        Ok(())
    })
}

fn encode_backend_failure_v1<E>(failure: RuntimeBackendFailureV1<E>) -> Vec<u8> {
    let (status, message): (u8, &[u8]) = match failure {
        RuntimeBackendFailureV1::Rejected(_) => (RESPONSE_REJECTED_V1, b"backend rejected"),
        RuntimeBackendFailureV1::Quiescent(_) => (RESPONSE_QUIESCENT_V1, b"backend quiescent"),
        RuntimeBackendFailureV1::Terminal(_) => (RESPONSE_TERMINAL_V1, b"backend terminal"),
    };
    let mut output = vec![status];
    put_blob_v1(&mut output, message, MAX_RUNTIME_WORKER_ERROR_BYTES_V1).unwrap();
    output
}

/// Error exposed by a runtime backend hosted in a supervised worker process.
#[derive(Debug)]
pub enum RuntimeWorkerBackendErrorV1<E> {
    Codec(E),
    Transport(RuntimeWorkerErrorV1),
    Protocol(&'static str),
}

impl<E: fmt::Display> fmt::Display for RuntimeWorkerBackendErrorV1<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Codec(error) => write!(formatter, "runtime worker codec failed: {error}"),
            Self::Transport(error) => write!(formatter, "{error}"),
            Self::Protocol(detail) => {
                write!(formatter, "runtime worker response mismatch: {detail}")
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for RuntimeWorkerBackendErrorV1<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Codec(error) => Some(error),
            Self::Transport(error) => Some(error),
            Self::Protocol(_) => None,
        }
    }
}

/// `RuntimeBackendV1` implementation for the frozen Runtime Worker V1 protocol.
///
/// This type is suitable only when accepted operations do not require a later
/// explicit progress call. Backends requiring any negotiated V4 extension must
/// be hosted and opened through [`RuntimeWorkerBackendV4`]. V1 intentionally
/// does not implement [`RuntimeFlushBackendV1`]:
///
/// ```compile_fail
/// use fe2o3_runtime::{
///     RuntimeBinaryCodecV1, RuntimeFlushBackendV1, RuntimeWorkerBackendV1,
/// };
///
/// fn require_flush<T: RuntimeFlushBackendV1>() {}
/// require_flush::<RuntimeWorkerBackendV1<RuntimeBinaryCodecV1>>();
/// ```
pub struct RuntimeWorkerBackendV1<C: RuntimeWorkerCodecV1> {
    transport: RuntimeWorkerTransportV1,
    codec: C,
    request_timeout: Duration,
}

impl<C: RuntimeWorkerCodecV1> RuntimeWorkerBackendV1<C> {
    pub fn spawn(
        command: &RuntimeWorkerCommandV1,
        codec: C,
        startup_timeout: Duration,
        request_timeout: Duration,
    ) -> Result<Self, RuntimeWorkerErrorV1> {
        Ok(Self {
            transport: RuntimeWorkerTransportV1::spawn(command, startup_timeout)?,
            codec,
            request_timeout,
        })
    }

    pub const fn is_terminal(&self) -> bool {
        self.transport.is_terminal()
    }

    pub fn shutdown(self, timeout: Duration) -> Result<(), RuntimeWorkerErrorV1> {
        self.transport.shutdown(timeout)
    }

    fn call(
        &mut self,
        operation: RuntimeWorkerOperationV1<'_>,
        expected: RuntimeWorkerOperationKindV1,
        timeout: Duration,
    ) -> Result<
        RuntimeWorkerResponseV1,
        RuntimeBackendFailureV1<RuntimeWorkerBackendErrorV1<C::Error>>,
    > {
        let request = self.codec.encode_request_v1(operation).map_err(|error| {
            RuntimeBackendFailureV1::Rejected(RuntimeWorkerBackendErrorV1::Codec(error))
        })?;
        let response = self
            .transport
            .request_owned(request, timeout)
            .map_err(|error| {
                RuntimeBackendFailureV1::Terminal(RuntimeWorkerBackendErrorV1::Transport(error))
            })?;
        self.decode_call_response(expected, &response)
    }

    fn call_until(
        &mut self,
        operation: RuntimeWorkerOperationV1<'_>,
        expected: RuntimeWorkerOperationKindV1,
        deadline: Instant,
    ) -> Result<
        Option<RuntimeWorkerResponseV1>,
        RuntimeBackendFailureV1<RuntimeWorkerBackendErrorV1<C::Error>>,
    > {
        let request = self.codec.encode_request_v1(operation).map_err(|error| {
            RuntimeBackendFailureV1::Rejected(RuntimeWorkerBackendErrorV1::Codec(error))
        })?;
        if Instant::now() >= deadline {
            return Ok(None);
        }
        let response = self
            .transport
            .request_owned_until(request, deadline)
            .map_err(|error| {
                RuntimeBackendFailureV1::Terminal(RuntimeWorkerBackendErrorV1::Transport(error))
            })?;
        self.decode_call_response(expected, &response).map(Some)
    }

    fn decode_call_response(
        &mut self,
        expected: RuntimeWorkerOperationKindV1,
        response: &[u8],
    ) -> Result<
        RuntimeWorkerResponseV1,
        RuntimeBackendFailureV1<RuntimeWorkerBackendErrorV1<C::Error>>,
    > {
        let decoded = self
            .codec
            .decode_response_v1(expected, response)
            .map_err(map_codec_failure_v1);
        if matches!(&decoded, Err(RuntimeBackendFailureV1::Terminal(_))) {
            self.transport.terminate();
        }
        decoded
    }

    fn response_mismatch<T>(
        &mut self,
        detail: &'static str,
    ) -> Result<T, RuntimeBackendFailureV1<RuntimeWorkerBackendErrorV1<C::Error>>> {
        self.transport.terminate();
        Err(response_mismatch_v1(detail))
    }
}

/// Worker backend with the explicitly negotiated V4 extension set.
///
/// Standard backend calls retain their V1 codec representation, but the
/// process must advertise the exact V4 handshake before this value can be
/// constructed. This type, unlike [`RuntimeWorkerBackendV1`], implements
/// [`RuntimeFlushBackendV1`], [`RuntimeAsyncCopyBackendV1`], and
/// [`RuntimeCancellationBackendV1`], and caches exact execution capabilities
/// for the device roster returned by each successful enumeration. Before
/// enumeration, after roster substitution, and for unknown handles, capability
/// queries fail closed to the all-false record. A received replacement roster
/// clears the prior cache before its capability records are queried, so a
/// recoverable query failure cannot expose stale or partial records. A flush or
/// other ordinary call may synchronously block for up to the `request_timeout` supplied to
/// [`RuntimeWorkerBackendV4::spawn`]; drain instead obeys its caller deadline.
/// A transport timeout or terminal response seals and reaps the worker.
/// Runtime Worker V1 remains intentionally incapable of these extensions.
pub struct RuntimeWorkerBackendV4<C: RuntimeWorkerCodecV4> {
    inner: RuntimeWorkerBackendV1<C>,
    execution_capabilities: HashMap<u64, RuntimeExecutionCapabilitiesV1>,
}

impl<C: RuntimeWorkerCodecV4> RuntimeWorkerBackendV4<C> {
    pub fn spawn(
        command: &RuntimeWorkerCommandV1,
        codec: C,
        startup_timeout: Duration,
        request_timeout: Duration,
    ) -> Result<Self, RuntimeWorkerErrorV1> {
        Ok(Self {
            inner: RuntimeWorkerBackendV1 {
                transport: RuntimeWorkerTransportV1::spawn_with_handshake_v1(
                    command,
                    startup_timeout,
                    RUNTIME_WORKER_HANDSHAKE_V4,
                )?,
                codec,
                request_timeout,
            },
            execution_capabilities: HashMap::new(),
        })
    }

    pub const fn is_terminal(&self) -> bool {
        self.inner.is_terminal()
    }

    pub fn shutdown(self, timeout: Duration) -> Result<(), RuntimeWorkerErrorV1> {
        self.inner.shutdown(timeout)
    }
}

impl<C: RuntimeWorkerCodecV4> RuntimeBackendV1 for RuntimeWorkerBackendV4<C> {
    type Error = RuntimeWorkerBackendErrorV1<C::Error>;

    fn execution_capabilities_v1(&self, device: u64) -> RuntimeExecutionCapabilitiesV1 {
        if self.inner.is_terminal() {
            return RuntimeExecutionCapabilitiesV1::default();
        }
        self.execution_capabilities
            .get(&device)
            .copied()
            .unwrap_or_default()
    }

    fn enumerate_devices_v1(
        &mut self,
    ) -> Result<Vec<BackendDeviceDescriptionV1>, RuntimeBackendFailureV1<Self::Error>> {
        let devices = self.inner.enumerate_devices_v1()?;
        self.execution_capabilities.clear();
        let mut capabilities = HashMap::new();
        capabilities.try_reserve(devices.len()).map_err(|_| {
            RuntimeBackendFailureV1::Rejected(RuntimeWorkerBackendErrorV1::Protocol(
                "execution-capability cache allocation",
            ))
        })?;
        for device in &devices {
            let request = self
                .inner
                .codec
                .encode_execution_capabilities_request_v4(device.backend_device)
                .map_err(|error| {
                    RuntimeBackendFailureV1::Rejected(RuntimeWorkerBackendErrorV1::Codec(error))
                })?;
            let response = self
                .inner
                .transport
                .request_owned(request, self.inner.request_timeout)
                .map_err(|error| {
                    RuntimeBackendFailureV1::Terminal(RuntimeWorkerBackendErrorV1::Transport(error))
                })?;
            let decoded = self
                .inner
                .codec
                .decode_execution_capabilities_response_v4(&response)
                .map_err(map_codec_failure_v1);
            if matches!(&decoded, Err(RuntimeBackendFailureV1::Terminal(_))) {
                self.inner.transport.terminate();
            }
            capabilities.insert(device.backend_device, decoded?);
        }
        self.execution_capabilities = capabilities;
        Ok(devices)
    }

    fn create_stream_v1(
        &mut self,
        device: u64,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.inner.create_stream_v1(device)
    }

    fn destroy_stream_v1(
        &mut self,
        stream: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.inner.destroy_stream_v1(stream)
    }

    fn allocate_v1(
        &mut self,
        device: u64,
        kind: RuntimeMemoryKindV1,
        byte_len: u64,
        alignment: u64,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.inner.allocate_v1(device, kind, byte_len, alignment)
    }

    fn release_allocation_v1(
        &mut self,
        allocation: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.inner.release_allocation_v1(allocation)
    }

    fn write_allocation_v1(
        &mut self,
        allocation: u64,
        byte_offset: u64,
        bytes: &[u8],
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.inner
            .write_allocation_v1(allocation, byte_offset, bytes)
    }

    fn read_allocation_v1(
        &mut self,
        allocation: u64,
        byte_offset: u64,
        destination: &mut [u8],
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.inner
            .read_allocation_v1(allocation, byte_offset, destination)
    }

    fn load_module_v1(
        &mut self,
        device: u64,
        image: &[u8],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.inner.load_module_v1(device, image)
    }

    fn unload_module_v1(
        &mut self,
        module: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.inner.unload_module_v1(module)
    }

    fn resolve_kernel_v1(
        &mut self,
        module: u64,
        name: &str,
        signature: [u8; 32],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.inner.resolve_kernel_v1(module, name, signature)
    }

    fn submit_v1(
        &mut self,
        launch: BackendLaunchV1<'_>,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.inner.submit_v1(launch)
    }

    fn poll_v1(
        &mut self,
        submission: u64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
        self.inner.poll_v1(submission)
    }

    fn wait_v1(
        &mut self,
        submission: u64,
        deadline: Instant,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
        self.inner.wait_v1(submission, deadline)
    }

    fn release_submission_v1(
        &mut self,
        submission: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.inner.release_submission_v1(submission)
    }

    fn record_event_v1(
        &mut self,
        stream: u64,
        submission: u64,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.inner.record_event_v1(stream, submission)
    }

    fn release_event_v1(&mut self, event: u64) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.inner.release_event_v1(event)
    }

    fn peer_copy_v1(
        &mut self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
        dependencies: &[u64],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.inner
            .peer_copy_v1(stream, source, destination, dependencies)
    }
}

impl<C: RuntimeWorkerCodecV4> RuntimeFlushBackendV1 for RuntimeWorkerBackendV4<C> {
    fn flush_stream_v1(&mut self, stream: u64) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        let request = self
            .inner
            .codec
            .encode_flush_stream_request_v4(stream)
            .map_err(|error| {
                RuntimeBackendFailureV1::Rejected(RuntimeWorkerBackendErrorV1::Codec(error))
            })?;
        let response = self
            .inner
            .transport
            .request_owned(request, self.inner.request_timeout)
            .map_err(|error| {
                RuntimeBackendFailureV1::Terminal(RuntimeWorkerBackendErrorV1::Transport(error))
            })?;
        let decoded = self
            .inner
            .codec
            .decode_flush_stream_response_v4(&response)
            .map_err(map_codec_failure_v1);
        if matches!(&decoded, Err(RuntimeBackendFailureV1::Terminal(_))) {
            self.inner.transport.terminate();
        }
        decoded
    }
}

impl<C: RuntimeWorkerCodecV4> RuntimeAsyncCopyBackendV1 for RuntimeWorkerBackendV4<C> {
    fn copy_async_v1(
        &mut self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
        dependencies: &[u64],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        let request = self
            .inner
            .codec
            .encode_async_copy_request_v4(stream, source, destination, dependencies)
            .map_err(|error| {
                RuntimeBackendFailureV1::Rejected(RuntimeWorkerBackendErrorV1::Codec(error))
            })?;
        let response = self
            .inner
            .transport
            .request_owned(request, self.inner.request_timeout)
            .map_err(|error| {
                RuntimeBackendFailureV1::Terminal(RuntimeWorkerBackendErrorV1::Transport(error))
            })?;
        let decoded = self
            .inner
            .codec
            .decode_async_copy_response_v4(&response)
            .map_err(map_codec_failure_v1);
        if matches!(&decoded, Err(RuntimeBackendFailureV1::Terminal(_))) {
            self.inner.transport.terminate();
        }
        match decoded? {
            0 => self.inner.response_mismatch("nonzero async-copy handle"),
            handle => Ok(handle),
        }
    }
}

impl<C: RuntimeWorkerCodecV4> RuntimeCancellationBackendV1 for RuntimeWorkerBackendV4<C> {
    fn cancel_v1(
        &mut self,
        submission: u64,
    ) -> Result<BackendCancellationV1, RuntimeBackendFailureV1<Self::Error>> {
        let request = self
            .inner
            .codec
            .encode_cancel_request_v4(submission)
            .map_err(|error| {
                RuntimeBackendFailureV1::Rejected(RuntimeWorkerBackendErrorV1::Codec(error))
            })?;
        let response = self
            .inner
            .transport
            .request_owned(request, self.inner.request_timeout)
            .map_err(|error| {
                RuntimeBackendFailureV1::Terminal(RuntimeWorkerBackendErrorV1::Transport(error))
            })?;
        let decoded = self
            .inner
            .codec
            .decode_cancel_response_v4(&response)
            .map_err(map_codec_failure_v1);
        if matches!(&decoded, Err(RuntimeBackendFailureV1::Terminal(_))) {
            self.inner.transport.terminate();
        }
        decoded
    }

    fn drain_v1(
        &mut self,
        submission: u64,
        deadline: Instant,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
        if self.inner.transport.is_terminal() {
            return Err(RuntimeBackendFailureV1::Terminal(
                RuntimeWorkerBackendErrorV1::Transport(RuntimeWorkerErrorV1::WorkerExited),
            ));
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Ok(BackendPollV1::Pending);
        }
        let worker_timeout = remaining.saturating_sub(RUNTIME_WORKER_RESPONSE_GRACE_V1);
        let request = self
            .inner
            .codec
            .encode_drain_request_v4(submission, worker_timeout)
            .map_err(|error| {
                RuntimeBackendFailureV1::Rejected(RuntimeWorkerBackendErrorV1::Codec(error))
            })?;
        if Instant::now() >= deadline {
            return Ok(BackendPollV1::Pending);
        }
        let response = self
            .inner
            .transport
            .request_owned_until(request, deadline)
            .map_err(|error| {
                RuntimeBackendFailureV1::Terminal(RuntimeWorkerBackendErrorV1::Transport(error))
            })?;
        let decoded = self
            .inner
            .codec
            .decode_drain_response_v4(&response)
            .map_err(map_codec_failure_v1);
        if matches!(&decoded, Err(RuntimeBackendFailureV1::Terminal(_))) {
            self.inner.transport.terminate();
        }
        decoded
    }
}

fn map_codec_failure_v1<E>(
    failure: RuntimeBackendFailureV1<E>,
) -> RuntimeBackendFailureV1<RuntimeWorkerBackendErrorV1<E>> {
    match failure {
        RuntimeBackendFailureV1::Rejected(error) => {
            RuntimeBackendFailureV1::Rejected(RuntimeWorkerBackendErrorV1::Codec(error))
        }
        RuntimeBackendFailureV1::Quiescent(error) => {
            RuntimeBackendFailureV1::Quiescent(RuntimeWorkerBackendErrorV1::Codec(error))
        }
        RuntimeBackendFailureV1::Terminal(error) => {
            RuntimeBackendFailureV1::Terminal(RuntimeWorkerBackendErrorV1::Codec(error))
        }
    }
}

fn response_mismatch_v1<E>(
    detail: &'static str,
) -> RuntimeBackendFailureV1<RuntimeWorkerBackendErrorV1<E>> {
    RuntimeBackendFailureV1::Terminal(RuntimeWorkerBackendErrorV1::Protocol(detail))
}

impl<C: RuntimeWorkerCodecV1> RuntimeBackendV1 for RuntimeWorkerBackendV1<C> {
    type Error = RuntimeWorkerBackendErrorV1<C::Error>;

    fn enumerate_devices_v1(
        &mut self,
    ) -> Result<Vec<BackendDeviceDescriptionV1>, RuntimeBackendFailureV1<Self::Error>> {
        match self.call(
            RuntimeWorkerOperationV1::EnumerateDevices,
            RuntimeWorkerOperationKindV1::EnumerateDevices,
            self.request_timeout,
        )? {
            RuntimeWorkerResponseV1::Devices(devices) => Ok(devices),
            _ => self.response_mismatch("device inventory"),
        }
    }

    fn create_stream_v1(
        &mut self,
        device: u64,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.handle_call(RuntimeWorkerOperationV1::CreateStream { device })
    }

    fn destroy_stream_v1(
        &mut self,
        stream: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.unit_call(RuntimeWorkerOperationV1::DestroyStream { stream })
    }

    fn allocate_v1(
        &mut self,
        device: u64,
        kind: RuntimeMemoryKindV1,
        byte_len: u64,
        alignment: u64,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.handle_call(RuntimeWorkerOperationV1::Allocate {
            device,
            kind,
            byte_len,
            alignment,
        })
    }

    fn release_allocation_v1(
        &mut self,
        allocation: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.unit_call(RuntimeWorkerOperationV1::ReleaseAllocation { allocation })
    }

    fn write_allocation_v1(
        &mut self,
        allocation: u64,
        byte_offset: u64,
        bytes: &[u8],
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.unit_call(RuntimeWorkerOperationV1::WriteAllocation {
            allocation,
            byte_offset,
            bytes,
        })
    }

    fn read_allocation_v1(
        &mut self,
        allocation: u64,
        byte_offset: u64,
        destination: &mut [u8],
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        match self.call(
            RuntimeWorkerOperationV1::ReadAllocation {
                allocation,
                byte_offset,
                byte_len: destination.len(),
            },
            RuntimeWorkerOperationKindV1::Bytes,
            self.request_timeout,
        )? {
            RuntimeWorkerResponseV1::Bytes(bytes) if bytes.len() == destination.len() => {
                destination.copy_from_slice(&bytes);
                Ok(())
            }
            _ => self.response_mismatch("allocation read"),
        }
    }

    fn load_module_v1(
        &mut self,
        device: u64,
        image: &[u8],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.handle_call(RuntimeWorkerOperationV1::LoadModule { device, image })
    }

    fn unload_module_v1(
        &mut self,
        module: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.unit_call(RuntimeWorkerOperationV1::UnloadModule { module })
    }

    fn resolve_kernel_v1(
        &mut self,
        module: u64,
        name: &str,
        signature: [u8; 32],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.handle_call(RuntimeWorkerOperationV1::ResolveKernel {
            module,
            name,
            signature,
        })
    }

    fn submit_v1(
        &mut self,
        launch: BackendLaunchV1<'_>,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        if launch.semantic_launch != crate::BackendSemanticLaunchV1::Ordinary {
            return Err(RuntimeBackendFailureV1::Rejected(
                RuntimeWorkerBackendErrorV1::Protocol(
                    "semantic launch requires an exact negotiated worker operation",
                ),
            ));
        }
        self.handle_call(RuntimeWorkerOperationV1::Submit {
            stream: launch.stream,
            kernel: launch.kernel,
            explicit_kernarg: launch.explicit_kernarg,
            bindings: launch.bindings,
            dependencies: launch.dependencies,
            geometry: launch.geometry,
        })
    }

    fn poll_v1(
        &mut self,
        submission: u64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
        self.poll_call(
            RuntimeWorkerOperationV1::Poll { submission },
            self.request_timeout,
        )
    }

    fn wait_v1(
        &mut self,
        submission: u64,
        deadline: Instant,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
        if self.transport.is_terminal() {
            return Err(RuntimeBackendFailureV1::Terminal(
                RuntimeWorkerBackendErrorV1::Transport(RuntimeWorkerErrorV1::WorkerExited),
            ));
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Ok(BackendPollV1::Pending);
        }
        let worker_timeout = remaining.saturating_sub(RUNTIME_WORKER_RESPONSE_GRACE_V1);
        match self.call_until(
            RuntimeWorkerOperationV1::Wait {
                submission,
                timeout: worker_timeout,
            },
            RuntimeWorkerOperationKindV1::Poll,
            deadline,
        )? {
            Some(RuntimeWorkerResponseV1::Poll(observation)) => Ok(observation),
            Some(_) => self.response_mismatch("completion observation"),
            None => Ok(BackendPollV1::Pending),
        }
    }

    fn release_submission_v1(
        &mut self,
        submission: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.unit_call(RuntimeWorkerOperationV1::ReleaseSubmission { submission })
    }

    fn record_event_v1(
        &mut self,
        stream: u64,
        submission: u64,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.handle_call(RuntimeWorkerOperationV1::RecordEvent { stream, submission })
    }

    fn release_event_v1(&mut self, event: u64) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.unit_call(RuntimeWorkerOperationV1::ReleaseEvent { event })
    }

    fn peer_copy_v1(
        &mut self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
        dependencies: &[u64],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.handle_call(RuntimeWorkerOperationV1::PeerCopy {
            stream,
            source,
            destination,
            dependencies,
        })
    }
}

impl<C: RuntimeWorkerCodecV1> RuntimeWorkerBackendV1<C> {
    fn handle_call(
        &mut self,
        operation: RuntimeWorkerOperationV1<'_>,
    ) -> Result<u64, RuntimeBackendFailureV1<RuntimeWorkerBackendErrorV1<C::Error>>> {
        match self.call(
            operation,
            RuntimeWorkerOperationKindV1::Handle,
            self.request_timeout,
        )? {
            RuntimeWorkerResponseV1::Handle(handle) if handle != 0 => Ok(handle),
            _ => self.response_mismatch("nonzero handle"),
        }
    }

    fn unit_call(
        &mut self,
        operation: RuntimeWorkerOperationV1<'_>,
    ) -> Result<(), RuntimeBackendFailureV1<RuntimeWorkerBackendErrorV1<C::Error>>> {
        match self.call(
            operation,
            RuntimeWorkerOperationKindV1::Unit,
            self.request_timeout,
        )? {
            RuntimeWorkerResponseV1::Unit => Ok(()),
            _ => self.response_mismatch("unit response"),
        }
    }

    fn poll_call(
        &mut self,
        operation: RuntimeWorkerOperationV1<'_>,
        timeout: Duration,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<RuntimeWorkerBackendErrorV1<C::Error>>> {
        match self.call(operation, RuntimeWorkerOperationKindV1::Poll, timeout)? {
            RuntimeWorkerResponseV1::Poll(observation) => Ok(observation),
            _ => self.response_mismatch("completion observation"),
        }
    }
}

/// Transport or lifecycle failure of an isolated runtime worker.
#[derive(Debug)]
pub enum RuntimeWorkerErrorV1 {
    Spawn(io::Error),
    MissingPipe(&'static str),
    Io(io::Error),
    Protocol(&'static str),
    FrameTooLarge { actual: usize, maximum: usize },
    StartupTimeout,
    RequestWriteTimeout,
    ResponseTimeout,
    WorkerExited,
    InvalidDeadline,
    ShutdownTimeout,
}

impl fmt::Display for RuntimeWorkerErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn(error) => write!(formatter, "failed to spawn runtime worker: {error}"),
            Self::MissingPipe(pipe) => write!(formatter, "runtime worker has no {pipe} pipe"),
            Self::Io(error) => write!(formatter, "runtime worker transport failed: {error}"),
            Self::Protocol(detail) => write!(formatter, "runtime worker protocol failed: {detail}"),
            Self::FrameTooLarge { actual, maximum } => {
                write!(
                    formatter,
                    "runtime worker frame is {actual} bytes; maximum is {maximum}"
                )
            }
            Self::StartupTimeout => formatter.write_str("runtime worker startup timed out"),
            Self::RequestWriteTimeout => {
                formatter.write_str("runtime worker request write timed out")
            }
            Self::ResponseTimeout => formatter.write_str("runtime worker response timed out"),
            Self::WorkerExited => formatter.write_str("runtime worker exited"),
            Self::InvalidDeadline => formatter.write_str("runtime worker deadline overflowed"),
            Self::ShutdownTimeout => formatter.write_str("runtime worker shutdown timed out"),
        }
    }
}

impl std::error::Error for RuntimeWorkerErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Spawn(error) | Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

/// One supervised child process carrying backend-specific bounded messages.
///
/// The worker may abort on an ambiguous native transition. The parent observes
/// a closed response channel and retains application-side resource ownership.
pub struct RuntimeWorkerTransportV1 {
    child: Child,
    requests: Option<SyncSender<RuntimeWorkerWriteV1>>,
    responses: Receiver<Result<Vec<u8>, RuntimeWorkerErrorV1>>,
    writer: Option<JoinHandle<()>>,
    reader: Option<JoinHandle<()>>,
    terminal: bool,
}

struct RuntimeWorkerWriteV1 {
    payload: Vec<u8>,
    completion: SyncSender<Result<(), RuntimeWorkerErrorV1>>,
}

impl RuntimeWorkerTransportV1 {
    pub fn spawn(
        command: &RuntimeWorkerCommandV1,
        startup_timeout: Duration,
    ) -> Result<Self, RuntimeWorkerErrorV1> {
        Self::spawn_with_handshake_v1(command, startup_timeout, RUNTIME_WORKER_HANDSHAKE_V1)
    }

    fn spawn_with_handshake_v1(
        command: &RuntimeWorkerCommandV1,
        startup_timeout: Duration,
        expected_handshake: &[u8],
    ) -> Result<Self, RuntimeWorkerErrorV1> {
        let mut child = Command::new(command.program())
            .args(command.arguments())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(RuntimeWorkerErrorV1::Spawn)?;
        let input = match child.stdin.take() {
            Some(input) => input,
            None => {
                terminate_child_v1(&mut child);
                return Err(RuntimeWorkerErrorV1::MissingPipe("stdin"));
            }
        };
        let output = match child.stdout.take() {
            Some(output) => output,
            None => {
                drop(input);
                terminate_child_v1(&mut child);
                return Err(RuntimeWorkerErrorV1::MissingPipe("stdout"));
            }
        };
        let (request_sender, request_receiver) = mpsc::sync_channel::<RuntimeWorkerWriteV1>(1);
        let writer = match thread::Builder::new()
            .name("fe2o3-runtime-worker-writer".into())
            .spawn(move || {
                let mut input = input;
                while let Ok(request) = request_receiver.recv() {
                    let result = write_frame_v1(&mut input, &request.payload)
                        .and_then(|()| input.flush().map_err(RuntimeWorkerErrorV1::Io));
                    let terminal = result.is_err();
                    let _ = request.completion.send(result);
                    if terminal {
                        break;
                    }
                }
            }) {
            Ok(writer) => writer,
            Err(error) => {
                terminate_child_v1(&mut child);
                return Err(RuntimeWorkerErrorV1::Spawn(error));
            }
        };
        let (sender, responses) = mpsc::sync_channel(1);
        let reader = match thread::Builder::new()
            .name("fe2o3-runtime-worker-reader".into())
            .spawn(move || {
                let mut output = output;
                loop {
                    let response = read_frame_v1(&mut output);
                    let terminal = response.is_err();
                    if sender.send(response).is_err() || terminal {
                        break;
                    }
                }
            }) {
            Ok(reader) => reader,
            Err(error) => {
                drop(request_sender);
                terminate_child_v1(&mut child);
                let _ = writer.join();
                return Err(RuntimeWorkerErrorV1::Spawn(error));
            }
        };
        let mut transport = Self {
            child,
            requests: Some(request_sender),
            responses,
            writer: Some(writer),
            reader: Some(reader),
            terminal: false,
        };
        let handshake = match transport.responses.recv_timeout(startup_timeout) {
            Ok(Ok(handshake)) => handshake,
            Ok(Err(error)) => {
                transport.terminate();
                return Err(error);
            }
            Err(RecvTimeoutError::Timeout) => {
                transport.terminate();
                return Err(RuntimeWorkerErrorV1::StartupTimeout);
            }
            Err(RecvTimeoutError::Disconnected) => {
                transport.terminate();
                return Err(RuntimeWorkerErrorV1::WorkerExited);
            }
        };
        if handshake != expected_handshake {
            transport.terminate();
            return Err(RuntimeWorkerErrorV1::Protocol("handshake mismatch"));
        }
        Ok(transport)
    }

    pub const fn is_terminal(&self) -> bool {
        self.terminal
    }

    pub fn request(
        &mut self,
        request: &[u8],
        timeout: Duration,
    ) -> Result<Vec<u8>, RuntimeWorkerErrorV1> {
        self.request_owned(request.to_vec(), timeout)
    }

    fn request_owned(
        &mut self,
        request: Vec<u8>,
        timeout: Duration,
    ) -> Result<Vec<u8>, RuntimeWorkerErrorV1> {
        if self.terminal {
            return Err(RuntimeWorkerErrorV1::WorkerExited);
        }
        let Some(deadline) = Instant::now().checked_add(timeout) else {
            self.terminate();
            return Err(RuntimeWorkerErrorV1::InvalidDeadline);
        };
        self.request_owned_until(request, deadline)
    }

    fn request_owned_until(
        &mut self,
        request: Vec<u8>,
        deadline: Instant,
    ) -> Result<Vec<u8>, RuntimeWorkerErrorV1> {
        if self.terminal {
            return Err(RuntimeWorkerErrorV1::WorkerExited);
        }
        if request.is_empty() {
            return Err(RuntimeWorkerErrorV1::Protocol(
                "empty request is reserved for shutdown",
            ));
        }
        self.write_request_until(request, deadline)?;
        match self
            .responses
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(error)) => {
                self.terminate();
                Err(error)
            }
            Err(RecvTimeoutError::Timeout) => {
                self.terminate();
                Err(RuntimeWorkerErrorV1::ResponseTimeout)
            }
            Err(RecvTimeoutError::Disconnected) => {
                self.terminate();
                Err(RuntimeWorkerErrorV1::WorkerExited)
            }
        }
    }

    fn write_request_until(
        &mut self,
        payload: Vec<u8>,
        deadline: Instant,
    ) -> Result<(), RuntimeWorkerErrorV1> {
        let requests = self
            .requests
            .as_ref()
            .ok_or(RuntimeWorkerErrorV1::WorkerExited)?;
        let (completion, completed) = mpsc::sync_channel(1);
        match requests.try_send(RuntimeWorkerWriteV1 {
            payload,
            completion,
        }) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {
                self.terminate();
                return Err(RuntimeWorkerErrorV1::WorkerExited);
            }
        }
        match completed.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => {
                self.terminate();
                Err(error)
            }
            Err(RecvTimeoutError::Timeout) => {
                self.terminate();
                Err(RuntimeWorkerErrorV1::RequestWriteTimeout)
            }
            Err(RecvTimeoutError::Disconnected) => {
                self.terminate();
                Err(RuntimeWorkerErrorV1::WorkerExited)
            }
        }
    }

    pub fn shutdown(mut self, timeout: Duration) -> Result<(), RuntimeWorkerErrorV1> {
        if self.terminal {
            self.join_reader();
            return Ok(());
        }
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(RuntimeWorkerErrorV1::InvalidDeadline)?;
        self.write_request_until(Vec::new(), deadline)?;
        self.requests.take();
        loop {
            match self.child.try_wait().map_err(RuntimeWorkerErrorV1::Io)? {
                Some(status) if status.success() => {
                    self.terminal = true;
                    self.join_reader();
                    return Ok(());
                }
                Some(_) => {
                    self.terminal = true;
                    self.join_reader();
                    return Err(RuntimeWorkerErrorV1::WorkerExited);
                }
                None if Instant::now() < deadline => thread::sleep(Duration::from_millis(1)),
                None => {
                    self.terminate();
                    return Err(RuntimeWorkerErrorV1::ShutdownTimeout);
                }
            }
        }
    }

    fn terminate(&mut self) {
        self.terminal = true;
        self.requests.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.join_reader();
        self.join_writer();
    }

    fn join_reader(&mut self) {
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }

    fn join_writer(&mut self) {
        if let Some(writer) = self.writer.take() {
            let _ = writer.join();
        }
    }
}

fn terminate_child_v1(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

impl Drop for RuntimeWorkerTransportV1 {
    fn drop(&mut self) {
        if !self.terminal {
            self.terminate();
        } else {
            self.join_reader();
            self.join_writer();
        }
    }
}

/// Runs a raw handler over the bounded Runtime Worker V1 framing protocol.
///
/// This primitive does not grant canonical backend compatibility or certify
/// immediate progress. Use [`serve_runtime_backend_worker_v1`] only for a
/// backend admitted by [`RuntimeWorkerV1ImmediateProgressBackendV1`], or use
/// [`serve_runtime_backend_worker_v4`] for a backend implementing the complete
/// negotiated V4 extension set.
pub fn serve_runtime_worker_v1<R, W, F>(
    input: R,
    output: W,
    handler: F,
) -> Result<(), RuntimeWorkerErrorV1>
where
    R: Read,
    W: Write,
    F: FnMut(&[u8]) -> Result<Vec<u8>, RuntimeWorkerErrorV1>,
{
    serve_runtime_worker_with_handshake_v1(input, output, RUNTIME_WORKER_HANDSHAKE_V1, handler)
}

/// Runs the worker side of the bounded, exact V4 request/response protocol.
/// The advertised handshake commits the handler to capability, flush,
/// asynchronous-copy, cancellation, and drain opcodes; use
/// [`serve_runtime_backend_worker_v4`] for typed backend dispatch.
pub fn serve_runtime_worker_v4<R, W, F>(
    input: R,
    output: W,
    handler: F,
) -> Result<(), RuntimeWorkerErrorV1>
where
    R: Read,
    W: Write,
    F: FnMut(&[u8]) -> Result<Vec<u8>, RuntimeWorkerErrorV1>,
{
    serve_runtime_worker_with_handshake_v1(input, output, RUNTIME_WORKER_HANDSHAKE_V4, handler)
}

fn serve_runtime_worker_with_handshake_v1<R, W, F>(
    mut input: R,
    mut output: W,
    handshake: &[u8],
    mut handler: F,
) -> Result<(), RuntimeWorkerErrorV1>
where
    R: Read,
    W: Write,
    F: FnMut(&[u8]) -> Result<Vec<u8>, RuntimeWorkerErrorV1>,
{
    write_frame_v1(&mut output, handshake)?;
    output.flush().map_err(RuntimeWorkerErrorV1::Io)?;
    loop {
        let request = read_frame_v1(&mut input)?;
        if request.is_empty() {
            return Ok(());
        }
        let response = handler(&request)?;
        write_frame_v1(&mut output, &response)?;
        output.flush().map_err(RuntimeWorkerErrorV1::Io)?;
    }
}

fn write_frame_v1(output: &mut impl Write, payload: &[u8]) -> Result<(), RuntimeWorkerErrorV1> {
    if payload.len() > MAX_RUNTIME_WORKER_FRAME_BYTES_V1 {
        return Err(RuntimeWorkerErrorV1::FrameTooLarge {
            actual: payload.len(),
            maximum: MAX_RUNTIME_WORKER_FRAME_BYTES_V1,
        });
    }
    let byte_len =
        u32::try_from(payload.len()).map_err(|_| RuntimeWorkerErrorV1::FrameTooLarge {
            actual: payload.len(),
            maximum: MAX_RUNTIME_WORKER_FRAME_BYTES_V1,
        })?;
    output
        .write_all(&byte_len.to_le_bytes())
        .and_then(|()| output.write_all(payload))
        .map_err(RuntimeWorkerErrorV1::Io)
}

fn read_frame_v1(input: &mut impl Read) -> Result<Vec<u8>, RuntimeWorkerErrorV1> {
    let mut header = [0_u8; 4];
    input
        .read_exact(&mut header)
        .map_err(RuntimeWorkerErrorV1::Io)?;
    let byte_len = u32::from_le_bytes(header) as usize;
    if byte_len > MAX_RUNTIME_WORKER_FRAME_BYTES_V1 {
        return Err(RuntimeWorkerErrorV1::FrameTooLarge {
            actual: byte_len,
            maximum: MAX_RUNTIME_WORKER_FRAME_BYTES_V1,
        });
    }
    let mut payload = vec![0; byte_len];
    input
        .read_exact(&mut payload)
        .map_err(RuntimeWorkerErrorV1::Io)?;
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        RuntimeAccessV1, RuntimeAllocationIdV1, RuntimeArgumentsV1, RuntimeBindingV1,
        RuntimeCapabilitiesV1, RuntimeContextV1, RuntimeLaunchGeometryV1, RuntimeMemoryKindV1,
        RuntimeMemoryRegionV1,
    };
    use std::io::Cursor;

    #[derive(Debug)]
    struct TestCodecError(&'static str);

    impl fmt::Display for TestCodecError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str(self.0)
        }
    }

    impl std::error::Error for TestCodecError {}

    #[derive(Default)]
    struct TestWorkerCodecV1;

    impl RuntimeWorkerCodecV1 for TestWorkerCodecV1 {
        type Error = TestCodecError;

        fn encode_request_v1(
            &mut self,
            operation: RuntimeWorkerOperationV1<'_>,
        ) -> Result<Vec<u8>, Self::Error> {
            let mut request = Vec::new();
            match operation {
                RuntimeWorkerOperationV1::EnumerateDevices => request.push(1),
                RuntimeWorkerOperationV1::CreateStream { device } => {
                    request.push(2);
                    request.extend_from_slice(&device.to_le_bytes());
                }
                RuntimeWorkerOperationV1::DestroyStream { stream } => {
                    request.push(3);
                    request.extend_from_slice(&stream.to_le_bytes());
                }
                RuntimeWorkerOperationV1::Allocate {
                    device,
                    byte_len,
                    alignment,
                    ..
                } => {
                    request.push(4);
                    request.extend_from_slice(&device.to_le_bytes());
                    request.extend_from_slice(&byte_len.to_le_bytes());
                    request.extend_from_slice(&alignment.to_le_bytes());
                }
                RuntimeWorkerOperationV1::ReleaseAllocation { allocation } => {
                    request.push(5);
                    request.extend_from_slice(&allocation.to_le_bytes());
                }
                RuntimeWorkerOperationV1::LoadModule { device, image } => {
                    request.push(6);
                    request.extend_from_slice(&device.to_le_bytes());
                    request.extend_from_slice(image);
                }
                RuntimeWorkerOperationV1::UnloadModule { module } => {
                    request.push(7);
                    request.extend_from_slice(&module.to_le_bytes());
                }
                RuntimeWorkerOperationV1::ResolveKernel {
                    module, signature, ..
                } => {
                    request.push(8);
                    request.extend_from_slice(&module.to_le_bytes());
                    request.extend_from_slice(&signature);
                }
                RuntimeWorkerOperationV1::Submit { bindings, .. } => {
                    request.push(9);
                    request.extend_from_slice(&(bindings.len() as u32).to_le_bytes());
                    for binding in bindings {
                        request.extend_from_slice(&binding.kernarg_byte_offset.to_le_bytes());
                    }
                }
                RuntimeWorkerOperationV1::Poll { submission } => {
                    request.push(10);
                    request.extend_from_slice(&submission.to_le_bytes());
                }
                RuntimeWorkerOperationV1::Wait { submission, .. } => {
                    request.push(11);
                    request.extend_from_slice(&submission.to_le_bytes());
                }
                RuntimeWorkerOperationV1::ReleaseSubmission { submission } => {
                    request.push(17);
                    request.extend_from_slice(&submission.to_le_bytes());
                }
                RuntimeWorkerOperationV1::RecordEvent { stream, submission } => {
                    request.push(12);
                    request.extend_from_slice(&stream.to_le_bytes());
                    request.extend_from_slice(&submission.to_le_bytes());
                }
                RuntimeWorkerOperationV1::ReleaseEvent { event } => {
                    request.push(13);
                    request.extend_from_slice(&event.to_le_bytes());
                }
                RuntimeWorkerOperationV1::WriteAllocation { .. } => request.push(14),
                RuntimeWorkerOperationV1::ReadAllocation { byte_len, .. } => {
                    request.push(15);
                    request.extend_from_slice(&(byte_len as u64).to_le_bytes());
                }
                RuntimeWorkerOperationV1::PeerCopy { .. } => request.push(16),
            }
            Ok(request)
        }

        fn decode_response_v1(
            &mut self,
            expected: RuntimeWorkerOperationKindV1,
            response: &[u8],
        ) -> Result<RuntimeWorkerResponseV1, RuntimeBackendFailureV1<Self::Error>> {
            let malformed =
                || RuntimeBackendFailureV1::Terminal(TestCodecError("malformed worker response"));
            match (expected, response.first().copied()) {
                (RuntimeWorkerOperationKindV1::EnumerateDevices, Some(b'D'))
                    if response.len() == 1 =>
                {
                    Ok(RuntimeWorkerResponseV1::Devices(vec![
                        BackendDeviceDescriptionV1 {
                            backend_device: 1,
                            name: "worker-device".into(),
                            target: "gfx942".into(),
                            global_memory_bytes: 1 << 30,
                            capabilities: RuntimeCapabilitiesV1 {
                                typed_async_launch: true,
                                streams: true,
                                events: true,
                                device_memory: true,
                                host_visible_memory: true,
                                peer_copy: false,
                                multi_device: false,
                                atomics: true,
                                collectives: true,
                            },
                        },
                    ]))
                }
                (RuntimeWorkerOperationKindV1::Handle, Some(b'H')) if response.len() == 9 => {
                    let handle =
                        u64::from_le_bytes(response[1..].try_into().map_err(|_| malformed())?);
                    Ok(RuntimeWorkerResponseV1::Handle(handle))
                }
                (RuntimeWorkerOperationKindV1::Unit, Some(b'U')) if response.len() == 1 => {
                    Ok(RuntimeWorkerResponseV1::Unit)
                }
                (RuntimeWorkerOperationKindV1::Poll, Some(b'P')) if response == b"P\x01" => {
                    Ok(RuntimeWorkerResponseV1::Poll(BackendPollV1::Succeeded))
                }
                (RuntimeWorkerOperationKindV1::Bytes, Some(b'B')) => {
                    Ok(RuntimeWorkerResponseV1::Bytes(response[1..].to_vec()))
                }
                _ => Err(malformed()),
            }
        }
    }

    struct MismatchedCodecV1;

    impl RuntimeWorkerCodecV1 for MismatchedCodecV1 {
        type Error = TestCodecError;

        fn encode_request_v1(
            &mut self,
            _operation: RuntimeWorkerOperationV1<'_>,
        ) -> Result<Vec<u8>, Self::Error> {
            Ok(vec![1])
        }

        fn decode_response_v1(
            &mut self,
            _expected: RuntimeWorkerOperationKindV1,
            _response: &[u8],
        ) -> Result<RuntimeWorkerResponseV1, RuntimeBackendFailureV1<Self::Error>> {
            Ok(RuntimeWorkerResponseV1::Unit)
        }
    }

    struct WorkerArgumentsV1 {
        allocation: RuntimeAllocationIdV1,
    }

    impl RuntimeArgumentsV1 for WorkerArgumentsV1 {
        const SIGNATURE_V1: [u8; 32] = [19; 32];

        fn encode_explicit_kernarg_v1(&self) -> Vec<u8> {
            vec![0; 8]
        }

        fn bindings_v1(&self) -> Vec<RuntimeBindingV1> {
            vec![RuntimeBindingV1 {
                region: RuntimeMemoryRegionV1 {
                    allocation: self.allocation,
                    access: RuntimeAccessV1::ReadWrite,
                    byte_offset: 0,
                    byte_len: 16,
                },
                kernarg_byte_offset: 0,
            }]
        }
    }

    #[derive(Default)]
    struct ProtocolBackendV1 {
        next: u64,
        calls: Vec<&'static str>,
    }

    impl ProtocolBackendV1 {
        fn handle(&mut self, call: &'static str) -> u64 {
            self.calls.push(call);
            self.next += 1;
            self.next
        }
    }

    impl RuntimeBackendV1 for ProtocolBackendV1 {
        type Error = TestCodecError;

        fn execution_capabilities_v1(&self, device: u64) -> RuntimeExecutionCapabilitiesV1 {
            if device != 1 {
                return RuntimeExecutionCapabilitiesV1::default();
            }
            RuntimeExecutionCapabilitiesV1 {
                native_async_copy: true,
                native_peer_copy: true,
                concurrent_compute: true,
                compute_copy_overlap: true,
                memory_pool: true,
                profiling: true,
                cancellation: true,
                atomics: true,
                collectives: true,
            }
        }

        fn enumerate_devices_v1(
            &mut self,
        ) -> Result<Vec<BackendDeviceDescriptionV1>, RuntimeBackendFailureV1<Self::Error>> {
            self.calls.push("enumerate");
            Ok(vec![BackendDeviceDescriptionV1 {
                backend_device: 1,
                name: "canonical".into(),
                target: "gfx942".into(),
                global_memory_bytes: 1024,
                capabilities: RuntimeCapabilitiesV1::default(),
            }])
        }

        fn create_stream_v1(
            &mut self,
            _device: u64,
        ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
            Ok(self.handle("create_stream"))
        }

        fn destroy_stream_v1(
            &mut self,
            _stream: u64,
        ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
            self.calls.push("destroy_stream");
            Ok(())
        }

        fn allocate_v1(
            &mut self,
            _device: u64,
            _kind: RuntimeMemoryKindV1,
            _byte_len: u64,
            _alignment: u64,
        ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
            Ok(self.handle("allocate"))
        }

        fn release_allocation_v1(
            &mut self,
            _allocation: u64,
        ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
            self.calls.push("release_allocation");
            Ok(())
        }

        fn write_allocation_v1(
            &mut self,
            _allocation: u64,
            _byte_offset: u64,
            _bytes: &[u8],
        ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
            self.calls.push("write_allocation");
            Ok(())
        }

        fn read_allocation_v1(
            &mut self,
            _allocation: u64,
            _byte_offset: u64,
            destination: &mut [u8],
        ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
            self.calls.push("read_allocation");
            destination.fill(7);
            Ok(())
        }

        fn load_module_v1(
            &mut self,
            _device: u64,
            _image: &[u8],
        ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
            Ok(self.handle("load_module"))
        }

        fn unload_module_v1(
            &mut self,
            _module: u64,
        ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
            self.calls.push("unload_module");
            Ok(())
        }

        fn resolve_kernel_v1(
            &mut self,
            _module: u64,
            _name: &str,
            _signature: [u8; 32],
        ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
            Ok(self.handle("resolve_kernel"))
        }

        fn submit_v1(
            &mut self,
            _launch: BackendLaunchV1<'_>,
        ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
            Ok(self.handle("submit"))
        }

        fn poll_v1(
            &mut self,
            submission: u64,
        ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
            self.calls.push("poll");
            if submission == u64::MAX {
                return Err(RuntimeBackendFailureV1::Terminal(TestCodecError(
                    "terminal",
                )));
            }
            Ok(BackendPollV1::Pending)
        }

        fn wait_v1(
            &mut self,
            _submission: u64,
            _deadline: Instant,
        ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
            self.calls.push("wait");
            Ok(BackendPollV1::Succeeded)
        }

        fn release_submission_v1(
            &mut self,
            _submission: u64,
        ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
            self.calls.push("release_submission");
            Ok(())
        }

        fn record_event_v1(
            &mut self,
            _stream: u64,
            _submission: u64,
        ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
            Ok(self.handle("record_event"))
        }

        fn release_event_v1(
            &mut self,
            event: u64,
        ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
            self.calls.push("release_event");
            match event {
                1 => Err(RuntimeBackendFailureV1::Rejected(TestCodecError(
                    "rejected",
                ))),
                2 => Err(RuntimeBackendFailureV1::Quiescent(TestCodecError(
                    "quiescent",
                ))),
                _ => Ok(()),
            }
        }

        fn peer_copy_v1(
            &mut self,
            _stream: u64,
            _source: BackendMemoryRegionV1,
            _destination: BackendMemoryRegionV1,
            _dependencies: &[u64],
        ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
            Ok(self.handle("peer_copy"))
        }
    }

    impl RuntimeFlushBackendV1 for ProtocolBackendV1 {
        fn flush_stream_v1(
            &mut self,
            stream: u64,
        ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
            self.calls.push("flush_stream");
            match stream {
                2 => Err(RuntimeBackendFailureV1::Rejected(TestCodecError(
                    "rejected",
                ))),
                3 => Err(RuntimeBackendFailureV1::Quiescent(TestCodecError(
                    "quiescent",
                ))),
                u64::MAX => Err(RuntimeBackendFailureV1::Terminal(TestCodecError(
                    "terminal",
                ))),
                _ => Ok(()),
            }
        }
    }

    impl RuntimeAsyncCopyBackendV1 for ProtocolBackendV1 {
        fn copy_async_v1(
            &mut self,
            stream: u64,
            _source: BackendMemoryRegionV1,
            _destination: BackendMemoryRegionV1,
            _dependencies: &[u64],
        ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
            match stream {
                2 => Err(RuntimeBackendFailureV1::Rejected(TestCodecError(
                    "rejected",
                ))),
                3 => Err(RuntimeBackendFailureV1::Quiescent(TestCodecError(
                    "quiescent",
                ))),
                u64::MAX => Err(RuntimeBackendFailureV1::Terminal(TestCodecError(
                    "terminal",
                ))),
                _ => Ok(self.handle("async_copy")),
            }
        }
    }

    impl RuntimeCancellationBackendV1 for ProtocolBackendV1 {
        fn cancel_v1(
            &mut self,
            submission: u64,
        ) -> Result<BackendCancellationV1, RuntimeBackendFailureV1<Self::Error>> {
            self.calls.push("cancel");
            match submission {
                1 => Ok(BackendCancellationV1::Cancelled),
                2 => Err(RuntimeBackendFailureV1::Rejected(TestCodecError(
                    "rejected",
                ))),
                3 => Err(RuntimeBackendFailureV1::Quiescent(TestCodecError(
                    "quiescent",
                ))),
                u64::MAX => Err(RuntimeBackendFailureV1::Terminal(TestCodecError(
                    "terminal",
                ))),
                _ => Ok(BackendCancellationV1::TooLate),
            }
        }

        fn drain_v1(
            &mut self,
            submission: u64,
            _deadline: Instant,
        ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
            self.calls.push("drain");
            match submission {
                2 => Err(RuntimeBackendFailureV1::Rejected(TestCodecError(
                    "rejected",
                ))),
                3 => Err(RuntimeBackendFailureV1::Quiescent(TestCodecError(
                    "quiescent",
                ))),
                u64::MAX => Err(RuntimeBackendFailureV1::Terminal(TestCodecError(
                    "terminal",
                ))),
                _ => Ok(BackendPollV1::Succeeded),
            }
        }
    }

    impl RuntimeWorkerV1ImmediateProgressBackendV1 for ProtocolBackendV1 {}

    fn canonical_call_v1(
        backend: &mut ProtocolBackendV1,
        operation: RuntimeWorkerOperationV1<'_>,
        expected: RuntimeWorkerOperationKindV1,
    ) -> Result<RuntimeWorkerResponseV1, RuntimeBackendFailureV1<RuntimeBinaryCodecErrorV1>> {
        let mut codec = RuntimeBinaryCodecV1;
        let request = codec.encode_request_v1(operation).unwrap();
        let response = dispatch_binary_request_v1(backend, &request).unwrap();
        codec.decode_response_v1(expected, &response)
    }

    fn canonical_flush_call_v4(
        backend: &mut ProtocolBackendV1,
        stream: u64,
    ) -> Result<(), RuntimeBackendFailureV1<RuntimeBinaryCodecErrorV1>> {
        let mut codec = RuntimeBinaryCodecV4;
        let request = codec.encode_flush_stream_request_v4(stream).unwrap();
        let response = dispatch_binary_request_v4(backend, &request).unwrap();
        codec.decode_flush_stream_response_v4(&response)
    }

    fn canonical_capabilities_call_v4(
        backend: &mut ProtocolBackendV1,
        device: u64,
    ) -> Result<RuntimeExecutionCapabilitiesV1, RuntimeBackendFailureV1<RuntimeBinaryCodecErrorV1>>
    {
        let mut codec = RuntimeBinaryCodecV4;
        let request = codec
            .encode_execution_capabilities_request_v4(device)
            .unwrap();
        let response = dispatch_binary_request_v4(backend, &request).unwrap();
        codec.decode_execution_capabilities_response_v4(&response)
    }

    fn canonical_async_copy_call_v4(
        backend: &mut ProtocolBackendV1,
        stream: u64,
        dependencies: &[u64],
    ) -> Result<u64, RuntimeBackendFailureV1<RuntimeBinaryCodecErrorV1>> {
        let mut codec = RuntimeBinaryCodecV4;
        let region = BackendMemoryRegionV1 {
            allocation: 1,
            access: RuntimeAccessV1::ReadWrite,
            byte_offset: 0,
            byte_len: 8,
        };
        let request = codec
            .encode_async_copy_request_v4(stream, region, region, dependencies)
            .unwrap();
        let response = dispatch_binary_request_v4(backend, &request).unwrap();
        codec.decode_async_copy_response_v4(&response)
    }

    fn canonical_cancel_call_v4(
        backend: &mut ProtocolBackendV1,
        submission: u64,
    ) -> Result<BackendCancellationV1, RuntimeBackendFailureV1<RuntimeBinaryCodecErrorV1>> {
        let mut codec = RuntimeBinaryCodecV4;
        let request = codec.encode_cancel_request_v4(submission).unwrap();
        let response = dispatch_binary_request_v4(backend, &request).unwrap();
        codec.decode_cancel_response_v4(&response)
    }

    fn canonical_drain_call_v4(
        backend: &mut ProtocolBackendV1,
        submission: u64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<RuntimeBinaryCodecErrorV1>> {
        let mut codec = RuntimeBinaryCodecV4;
        let request = codec
            .encode_drain_request_v4(submission, Duration::from_millis(1))
            .unwrap();
        let response = dispatch_binary_request_v4(backend, &request).unwrap();
        codec.decode_drain_response_v4(&response)
    }

    const TEST_WORKER_SERVER: &str = r#"
import struct
import sys

stdin = sys.stdin.buffer
stdout = sys.stdout.buffer
cleanup = []
next_handle = 100

def read_exact(size):
    data = b''
    while len(data) < size:
        part = stdin.read(size - len(data))
        if not part:
            raise EOFError()
        data += part
    return data

def write_frame(payload):
    stdout.write(struct.pack('<I', len(payload)) + payload)
    stdout.flush()

write_frame(b'fe2o3-runtime-worker-v1')
while True:
    size = struct.unpack('<I', read_exact(4))[0]
    request = read_exact(size)
    if not request:
        sys.exit(0 if cleanup == [3, 13, 17, 7, 5] else 9)
    operation = request[0]
    if operation == 1:
        response = b'D'
    elif operation in (2, 4, 6, 8, 9, 12, 16):
        if operation == 9 and request[1:] != struct.pack('<II', 1, 0):
            sys.exit(8)
        next_handle += 1
        response = b'H' + struct.pack('<Q', next_handle)
    elif operation in (3, 5, 7, 13, 14, 17):
        required_prefix = {3: [], 13: [3], 17: [3, 13], 7: [3, 13, 17], 5: [3, 13, 17, 7]}.get(operation)
        if required_prefix is not None:
            if cleanup != required_prefix:
                sys.exit(7)
            cleanup.append(operation)
        response = b'U'
    elif operation in (10, 11):
        response = b'P\x01'
    elif operation == 15:
        response = b'B' + bytes(struct.unpack('<Q', request[1:9])[0])
    else:
        sys.exit(6)
    write_frame(response)
"#;

    const V4_FLUSH_WORKER_SERVER: &str = r#"
import struct
import sys

stdin = sys.stdin.buffer
stdout = sys.stdout.buffer

def read_exact(size):
    data = b''
    while len(data) < size:
        part = stdin.read(size - len(data))
        if not part:
            raise EOFError()
        data += part
    return data

def write_frame(payload):
    stdout.write(struct.pack('<I', len(payload)) + payload)
    stdout.flush()

def read_frame():
    size = struct.unpack('<I', read_exact(4))[0]
    return read_exact(size)

write_frame(b'fe2o3-runtime-worker-v4;extensions=flush-v1,async-copy-v1,cancellation-v1,execution-capabilities-v1')
request = read_frame()
if request == bytes((18,)) + struct.pack('<Q', 41):
    write_frame(bytes((0,)))
elif request == bytes((1,)):
    name = b'v4-device'
    target = b'gfx942'
    response = bytes((0,)) + struct.pack('<I', 1) + struct.pack('<Q', 1)
    response += struct.pack('<I', len(name)) + name
    response += struct.pack('<I', len(target)) + target
    response += struct.pack('<QH', 1 << 30, 2)
    write_frame(response)
    if read_frame() != bytes((19,)) + struct.pack('<Q', 1):
        sys.exit(7)
    write_frame(bytes((0,)) + struct.pack('<I', 2) + struct.pack('<H', 0x01ff))
    if read_frame() != bytes((2,)) + struct.pack('<Q', 1):
        sys.exit(7)
    write_frame(bytes((0,)) + struct.pack('<Q', 41))
    if read_frame() != bytes((18,)) + struct.pack('<Q', 41):
        sys.exit(8)
    write_frame(bytes((0,)))
    if read_frame() != bytes((3,)) + struct.pack('<Q', 41):
        sys.exit(9)
    write_frame(bytes((0,)))
else:
    sys.exit(8)
request = read_frame()
sys.exit(0 if not request else 9)
"#;

    const V4_ADDITIVE_WORKER_SERVER: &str = r#"
import struct
import sys

stdin = sys.stdin.buffer
stdout = sys.stdout.buffer
enumerations = 0

def read_exact(size):
    data = b''
    while len(data) < size:
        part = stdin.read(size - len(data))
        if not part:
            raise EOFError()
        data += part
    return data

def read_frame():
    size = struct.unpack('<I', read_exact(4))[0]
    return read_exact(size)

def write_frame(payload):
    stdout.write(struct.pack('<I', len(payload)) + payload)
    stdout.flush()

write_frame(b'fe2o3-runtime-worker-v4;extensions=flush-v1,async-copy-v1,cancellation-v1,execution-capabilities-v1')
while True:
    request = read_frame()
    if not request:
        sys.exit(0)
    operation = request[0]
    if operation == 1:
        enumerations += 1
        device = enumerations
        name = ('v4-device-%d' % device).encode()
        target = b'gfx942'
        response = bytes((0,)) + struct.pack('<I', 1) + struct.pack('<Q', device)
        response += struct.pack('<I', len(name)) + name
        response += struct.pack('<I', len(target)) + target
        response += struct.pack('<QH', 1 << 30, 2)
    elif operation == 19:
        if len(request) != 9:
            sys.exit(10)
        device = struct.unpack('<Q', request[1:9])[0]
        if device == 3:
            message = b'backend rejected'
            response = bytes((1,)) + struct.pack('<I', len(message)) + message
        else:
            bits = 0x49 if device == 1 else 0x06 if device == 2 else 0
            response = bytes((0,)) + struct.pack('<I', 2) + struct.pack('<H', bits)
    elif operation == 20:
        if len(request) != 63:
            sys.exit(11)
        response = bytes((0,)) + struct.pack('<Q', 101)
    elif operation == 21:
        if len(request) != 9:
            sys.exit(12)
        submission = struct.unpack('<Q', request[1:9])[0]
        disposition = 0 if submission == 101 else 1
        response = bytes((0,)) + struct.pack('<I', 1) + bytes((disposition,))
    elif operation == 22:
        if len(request) != 21:
            sys.exit(13)
        seconds = struct.unpack('<Q', request[9:17])[0]
        nanoseconds = struct.unpack('<I', request[17:21])[0]
        if seconds != 0 or nanoseconds > 350000000:
            sys.exit(14)
        response = bytes((0, 1))
    elif operation == 18:
        response = bytes((0,))
    else:
        sys.exit(15)
    write_frame(response)
"#;

    const V4_ZERO_ASYNC_COPY_WORKER_SERVER: &str = r#"
import struct
import sys
import time

stdin = sys.stdin.buffer
stdout = sys.stdout.buffer
handshake = b'fe2o3-runtime-worker-v4;extensions=flush-v1,async-copy-v1,cancellation-v1,execution-capabilities-v1'
stdout.write(struct.pack('<I', len(handshake)) + handshake)
stdout.flush()
size = struct.unpack('<I', stdin.read(4))[0]
request = stdin.read(size)
if not request or request[0] != 20:
    sys.exit(16)
stdout.write(struct.pack('<I', 9) + bytes((0,)) + struct.pack('<Q', 0))
stdout.flush()
time.sleep(60)
"#;

    const V4_DELAYED_FLUSH_WORKER_SERVER: &str = r#"
import struct
import sys
import time

stdin = sys.stdin.buffer
stdout = sys.stdout.buffer
payload = b'fe2o3-runtime-worker-v4;extensions=flush-v1,async-copy-v1,cancellation-v1,execution-capabilities-v1'
stdout.write(struct.pack('<I', len(payload)) + payload)
stdout.flush()
size = struct.unpack('<I', stdin.read(4))[0]
stdin.read(size)
time.sleep(60)
"#;

    const V4_TERMINAL_FLUSH_WORKER_SERVER: &str = r#"
import struct
import sys
import time

stdin = sys.stdin.buffer
stdout = sys.stdout.buffer

def write_frame(payload):
    stdout.write(struct.pack('<I', len(payload)) + payload)
    stdout.flush()

write_frame(b'fe2o3-runtime-worker-v4;extensions=flush-v1,async-copy-v1,cancellation-v1,execution-capabilities-v1')
size = struct.unpack('<I', stdin.read(4))[0]
request = stdin.read(size)
if request != bytes((18,)) + struct.pack('<Q', 1):
    sys.exit(8)
message = b'backend terminal'
write_frame(bytes((3,)) + struct.pack('<I', len(message)) + message)
time.sleep(60)
"#;

    const V4_TERMINAL_CAPABILITIES_WORKER_SERVER: &str = r#"
import struct
import sys
import time

stdin = sys.stdin.buffer
stdout = sys.stdout.buffer

def read_exact(size):
    data = b''
    while len(data) < size:
        part = stdin.read(size - len(data))
        if not part:
            raise EOFError()
        data += part
    return data

def read_frame():
    size = struct.unpack('<I', read_exact(4))[0]
    return read_exact(size)

def write_frame(payload):
    stdout.write(struct.pack('<I', len(payload)) + payload)
    stdout.flush()

write_frame(b'fe2o3-runtime-worker-v4;extensions=flush-v1,async-copy-v1,cancellation-v1,execution-capabilities-v1')
if read_frame() != bytes((1,)):
    sys.exit(8)
name = b'v4-device'
target = b'gfx942'
response = bytes((0,)) + struct.pack('<I', 1) + struct.pack('<Q', 1)
response += struct.pack('<I', len(name)) + name
response += struct.pack('<I', len(target)) + target
response += struct.pack('<QH', 1 << 30, 2)
write_frame(response)
if read_frame() != bytes((19,)) + struct.pack('<Q', 1):
    sys.exit(9)
message = b'backend terminal'
write_frame(bytes((3,)) + struct.pack('<I', len(message)) + message)
time.sleep(60)
"#;

    const UNKNOWN_VERSION_WORKER_SERVER: &str = r#"
import struct
import sys
import time

payload = b'fe2o3-runtime-worker-v5;extensions=flush-v1'
sys.stdout.buffer.write(struct.pack('<I', len(payload)) + payload)
sys.stdout.buffer.flush()
time.sleep(60)
"#;

    const NON_READING_WORKER_SERVER: &str = r#"
import struct
import sys
import time

payload = b'fe2o3-runtime-worker-v1'
sys.stdout.buffer.write(struct.pack('<I', len(payload)) + payload)
sys.stdout.buffer.flush()
time.sleep(60)
"#;

    const SINGLE_RESPONSE_WORKER_SERVER: &str = r#"
import struct
import sys
import time

stdin = sys.stdin.buffer
stdout = sys.stdout.buffer

def read_exact(size):
    data = b''
    while len(data) < size:
        part = stdin.read(size - len(data))
        if not part:
            raise EOFError()
        data += part
    return data

def write_frame(payload):
    stdout.write(struct.pack('<I', len(payload)) + payload)
    stdout.flush()

write_frame(b'fe2o3-runtime-worker-v1')
size = struct.unpack('<I', read_exact(4))[0]
read_exact(size)
write_frame(b'X')
time.sleep(60)
"#;

    const DELAYED_READ_WORKER_SERVER: &str = r#"
import struct
import sys
import time

stdin = sys.stdin.buffer
stdout = sys.stdout.buffer

def read_exact(size):
    data = b''
    while len(data) < size:
        part = stdin.read(size - len(data))
        if not part:
            raise EOFError()
        data += part
    return data

def write_frame(payload):
    stdout.write(struct.pack('<I', len(payload)) + payload)
    stdout.flush()

write_frame(b'fe2o3-runtime-worker-v1')
time.sleep(0.15)
size = struct.unpack('<I', read_exact(4))[0]
read_exact(size)
write_frame(b'P\x01')
time.sleep(60)
"#;

    const DELAYED_RESPONSE_WORKER_SERVER: &str = r#"
import struct
import sys
import time

stdin = sys.stdin.buffer
stdout = sys.stdout.buffer

def read_exact(size):
    data = b''
    while len(data) < size:
        part = stdin.read(size - len(data))
        if not part:
            raise EOFError()
        data += part
    return data

def write_frame(payload):
    stdout.write(struct.pack('<I', len(payload)) + payload)
    stdout.flush()

write_frame(b'fe2o3-runtime-worker-v1')
size = struct.unpack('<I', read_exact(4))[0]
read_exact(size)
time.sleep(0.15)
write_frame(b'P\x01')
time.sleep(60)
"#;

    const WAIT_BUDGET_OBSERVER_WORKER_SERVER: &str = r#"
import struct
import sys
import time

stdin = sys.stdin.buffer
stdout = sys.stdout.buffer

def read_exact(size):
    data = b''
    while len(data) < size:
        part = stdin.read(size - len(data))
        if not part:
            raise EOFError()
        data += part
    return data

def write_frame(payload):
    stdout.write(struct.pack('<I', len(payload)) + payload)
    stdout.flush()

write_frame(b'fe2o3-runtime-worker-v1')
time.sleep(0.05)
size = struct.unpack('<I', read_exact(4))[0]
request = read_exact(size)
if len(request) != 21 or request[0] != 13:
    sys.exit(7)
seconds = struct.unpack('<Q', request[9:17])[0]
nanoseconds = struct.unpack('<I', request[17:21])[0]
timeout_nanoseconds = seconds * 1000000000 + nanoseconds
poll = 0 if timeout_nanoseconds < 350000000 else 1
write_frame(bytes((0, poll)))
time.sleep(60)
"#;

    const TEST_WAIT_DEADLINE: Duration = Duration::from_millis(100);
    const TEST_DEADLINE_SCHEDULER_TOLERANCE: Duration = Duration::from_millis(75);

    fn assert_deadline_returned_with_scheduler_tolerance(deadline: Instant) {
        let latest = deadline
            .checked_add(TEST_DEADLINE_SCHEDULER_TOLERANCE)
            .unwrap();
        assert!(
            Instant::now() <= latest,
            "worker deadline exceeded scheduler tolerance"
        );
    }

    #[test]
    fn canonical_codec_and_dispatcher_cover_every_backend_operation() {
        let mut backend = ProtocolBackendV1::default();
        assert!(matches!(
            canonical_call_v1(
                &mut backend,
                RuntimeWorkerOperationV1::EnumerateDevices,
                RuntimeWorkerOperationKindV1::EnumerateDevices,
            )
            .unwrap(),
            RuntimeWorkerResponseV1::Devices(devices) if devices.len() == 1
        ));
        assert!(matches!(
            canonical_call_v1(
                &mut backend,
                RuntimeWorkerOperationV1::CreateStream { device: 1 },
                RuntimeWorkerOperationKindV1::Handle,
            )
            .unwrap(),
            RuntimeWorkerResponseV1::Handle(_)
        ));
        canonical_call_v1(
            &mut backend,
            RuntimeWorkerOperationV1::DestroyStream { stream: 1 },
            RuntimeWorkerOperationKindV1::Unit,
        )
        .unwrap();
        canonical_call_v1(
            &mut backend,
            RuntimeWorkerOperationV1::Allocate {
                device: 1,
                kind: RuntimeMemoryKindV1::DeviceLocal,
                byte_len: 64,
                alignment: 16,
            },
            RuntimeWorkerOperationKindV1::Handle,
        )
        .unwrap();
        canonical_call_v1(
            &mut backend,
            RuntimeWorkerOperationV1::ReleaseAllocation { allocation: 1 },
            RuntimeWorkerOperationKindV1::Unit,
        )
        .unwrap();
        canonical_call_v1(
            &mut backend,
            RuntimeWorkerOperationV1::WriteAllocation {
                allocation: 1,
                byte_offset: 0,
                bytes: &[1, 2, 3],
            },
            RuntimeWorkerOperationKindV1::Unit,
        )
        .unwrap();
        assert!(matches!(
            canonical_call_v1(
                &mut backend,
                RuntimeWorkerOperationV1::ReadAllocation {
                    allocation: 1,
                    byte_offset: 0,
                    byte_len: 3,
                },
                RuntimeWorkerOperationKindV1::Bytes,
            )
            .unwrap(),
            RuntimeWorkerResponseV1::Bytes(bytes) if bytes == [7, 7, 7]
        ));
        canonical_call_v1(
            &mut backend,
            RuntimeWorkerOperationV1::LoadModule {
                device: 1,
                image: b"object",
            },
            RuntimeWorkerOperationKindV1::Handle,
        )
        .unwrap();
        canonical_call_v1(
            &mut backend,
            RuntimeWorkerOperationV1::UnloadModule { module: 1 },
            RuntimeWorkerOperationKindV1::Unit,
        )
        .unwrap();
        canonical_call_v1(
            &mut backend,
            RuntimeWorkerOperationV1::ResolveKernel {
                module: 1,
                name: "kernel",
                signature: [1; 32],
            },
            RuntimeWorkerOperationKindV1::Handle,
        )
        .unwrap();
        let binding = BackendBindingV1 {
            region: BackendMemoryRegionV1 {
                allocation: 1,
                access: RuntimeAccessV1::ReadWrite,
                byte_offset: 0,
                byte_len: 8,
            },
            kernarg_byte_offset: 0,
        };
        let geometry = RuntimeLaunchGeometryV1 {
            grid: [1, 1, 1],
            workgroup: [1, 1, 1],
            dynamic_shared_bytes: 0,
        };
        canonical_call_v1(
            &mut backend,
            RuntimeWorkerOperationV1::Submit {
                stream: 1,
                kernel: 1,
                explicit_kernarg: &[0; 8],
                bindings: &[binding],
                dependencies: &[1],
                geometry,
            },
            RuntimeWorkerOperationKindV1::Handle,
        )
        .unwrap();
        canonical_call_v1(
            &mut backend,
            RuntimeWorkerOperationV1::Poll { submission: 1 },
            RuntimeWorkerOperationKindV1::Poll,
        )
        .unwrap();
        canonical_call_v1(
            &mut backend,
            RuntimeWorkerOperationV1::Wait {
                submission: 1,
                timeout: Duration::from_millis(1),
            },
            RuntimeWorkerOperationKindV1::Poll,
        )
        .unwrap();
        canonical_call_v1(
            &mut backend,
            RuntimeWorkerOperationV1::ReleaseSubmission { submission: 1 },
            RuntimeWorkerOperationKindV1::Unit,
        )
        .unwrap();
        canonical_call_v1(
            &mut backend,
            RuntimeWorkerOperationV1::RecordEvent {
                stream: 1,
                submission: 1,
            },
            RuntimeWorkerOperationKindV1::Handle,
        )
        .unwrap();
        canonical_call_v1(
            &mut backend,
            RuntimeWorkerOperationV1::ReleaseEvent { event: 0 },
            RuntimeWorkerOperationKindV1::Unit,
        )
        .unwrap();
        canonical_call_v1(
            &mut backend,
            RuntimeWorkerOperationV1::PeerCopy {
                stream: 1,
                source: binding.region,
                destination: binding.region,
                dependencies: &[],
            },
            RuntimeWorkerOperationKindV1::Handle,
        )
        .unwrap();
        assert_eq!(backend.calls.len(), 17);
    }

    #[test]
    fn canonical_codec_preserves_backend_failure_classes() {
        let mut backend = ProtocolBackendV1::default();
        assert!(matches!(
            canonical_call_v1(
                &mut backend,
                RuntimeWorkerOperationV1::ReleaseEvent { event: 1 },
                RuntimeWorkerOperationKindV1::Unit,
            ),
            Err(RuntimeBackendFailureV1::Rejected(_))
        ));
        assert!(matches!(
            canonical_call_v1(
                &mut backend,
                RuntimeWorkerOperationV1::ReleaseEvent { event: 2 },
                RuntimeWorkerOperationKindV1::Unit,
            ),
            Err(RuntimeBackendFailureV1::Quiescent(_))
        ));
        assert!(matches!(
            canonical_call_v1(
                &mut backend,
                RuntimeWorkerOperationV1::Poll {
                    submission: u64::MAX,
                },
                RuntimeWorkerOperationKindV1::Poll,
            ),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
    }

    #[test]
    fn v4_flush_codec_is_bounded_and_preserves_failure_classes() {
        let mut backend = ProtocolBackendV1::default();
        let mut codec = RuntimeBinaryCodecV4;
        let request = codec.encode_flush_stream_request_v4(41).unwrap();
        assert_eq!(
            request,
            [OP_FLUSH_STREAM_V4]
                .into_iter()
                .chain(41_u64.to_le_bytes())
                .collect::<Vec<_>>()
        );
        assert!(request.len() <= MAX_RUNTIME_WORKER_FRAME_BYTES_V1);
        canonical_flush_call_v4(&mut backend, 41).unwrap();
        assert_eq!(backend.calls, ["flush_stream"]);
        assert!(matches!(
            canonical_flush_call_v4(&mut backend, 2),
            Err(RuntimeBackendFailureV1::Rejected(_))
        ));
        assert!(matches!(
            canonical_flush_call_v4(&mut backend, 3),
            Err(RuntimeBackendFailureV1::Quiescent(_))
        ));
        assert!(matches!(
            canonical_flush_call_v4(&mut backend, u64::MAX),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
    }

    #[test]
    fn v4_additive_codec_and_dispatcher_preserve_bounds_and_failure_classes() {
        let mut backend = ProtocolBackendV1::default();
        let expected = backend.execution_capabilities_v1(1);
        assert_eq!(
            canonical_capabilities_call_v4(&mut backend, 1).unwrap(),
            expected
        );

        let mut codec = RuntimeBinaryCodecV4;
        assert_eq!(
            codec.encode_execution_capabilities_request_v4(1).unwrap(),
            [OP_EXECUTION_CAPABILITIES_V4]
                .into_iter()
                .chain(1_u64.to_le_bytes())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            codec.encode_cancel_request_v4(1).unwrap(),
            [OP_CANCEL_V4]
                .into_iter()
                .chain(1_u64.to_le_bytes())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            codec
                .encode_drain_request_v4(1, Duration::from_millis(1))
                .unwrap(),
            [OP_DRAIN_V4]
                .into_iter()
                .chain(1_u64.to_le_bytes())
                .chain(0_u64.to_le_bytes())
                .chain(1_000_000_u32.to_le_bytes())
                .collect::<Vec<_>>()
        );
        let source = BackendMemoryRegionV1 {
            allocation: 2,
            access: RuntimeAccessV1::Read,
            byte_offset: 3,
            byte_len: 5,
        };
        let destination = BackendMemoryRegionV1 {
            allocation: 7,
            access: RuntimeAccessV1::Write,
            byte_offset: 11,
            byte_len: 13,
        };
        let dependencies = [17, 19];
        let mut expected_async = vec![OP_ASYNC_COPY_V4];
        put_u64_v1(&mut expected_async, 23);
        put_backend_region_v1(&mut expected_async, source);
        put_backend_region_v1(&mut expected_async, destination);
        put_dependencies_v1(&mut expected_async, &dependencies).unwrap();
        assert_eq!(
            codec
                .encode_async_copy_request_v4(23, source, destination, &dependencies)
                .unwrap(),
            expected_async
        );
        assert!(expected_async.len() <= MAX_RUNTIME_WORKER_FRAME_BYTES_V1);
        assert!(matches!(
            codec.decode_execution_capabilities_response_v4(&encode_backend_failure_v1(
                RuntimeBackendFailureV1::Rejected(TestCodecError("rejected"))
            )),
            Err(RuntimeBackendFailureV1::Rejected(_))
        ));
        assert!(matches!(
            codec.decode_execution_capabilities_response_v4(&encode_backend_failure_v1(
                RuntimeBackendFailureV1::Quiescent(TestCodecError("quiescent"))
            )),
            Err(RuntimeBackendFailureV1::Quiescent(_))
        ));
        assert!(matches!(
            codec.decode_execution_capabilities_response_v4(&encode_backend_failure_v1(
                RuntimeBackendFailureV1::Terminal(TestCodecError("terminal"))
            )),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));

        assert_ne!(
            canonical_async_copy_call_v4(&mut backend, 1, &[]).unwrap(),
            0
        );
        assert!(matches!(
            canonical_async_copy_call_v4(&mut backend, 2, &[]),
            Err(RuntimeBackendFailureV1::Rejected(_))
        ));
        assert!(matches!(
            canonical_async_copy_call_v4(&mut backend, 3, &[]),
            Err(RuntimeBackendFailureV1::Quiescent(_))
        ));
        assert!(matches!(
            canonical_async_copy_call_v4(&mut backend, u64::MAX, &[]),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));

        assert_eq!(
            canonical_cancel_call_v4(&mut backend, 1).unwrap(),
            BackendCancellationV1::Cancelled
        );
        assert_eq!(
            canonical_cancel_call_v4(&mut backend, 4).unwrap(),
            BackendCancellationV1::TooLate
        );
        assert!(matches!(
            canonical_cancel_call_v4(&mut backend, 2),
            Err(RuntimeBackendFailureV1::Rejected(_))
        ));
        assert!(matches!(
            canonical_cancel_call_v4(&mut backend, 3),
            Err(RuntimeBackendFailureV1::Quiescent(_))
        ));
        assert!(matches!(
            canonical_cancel_call_v4(&mut backend, u64::MAX),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));

        assert_eq!(
            canonical_drain_call_v4(&mut backend, 1).unwrap(),
            BackendPollV1::Succeeded
        );
        assert!(matches!(
            canonical_drain_call_v4(&mut backend, 2),
            Err(RuntimeBackendFailureV1::Rejected(_))
        ));
        assert!(matches!(
            canonical_drain_call_v4(&mut backend, 3),
            Err(RuntimeBackendFailureV1::Quiescent(_))
        ));
        assert!(matches!(
            canonical_drain_call_v4(&mut backend, u64::MAX),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));

        let region = BackendMemoryRegionV1 {
            allocation: 1,
            access: RuntimeAccessV1::Read,
            byte_offset: 0,
            byte_len: 1,
        };
        let dependencies = vec![1; crate::MAX_RUNTIME_DEPENDENCIES_V1 + 1];
        assert!(matches!(
            codec.encode_async_copy_request_v4(1, region, region, &dependencies),
            Err(RuntimeBinaryCodecErrorV1::Limit("async-copy dependencies"))
        ));
        assert!(matches!(
            codec.decode_execution_capabilities_response_v4(&[0, 2, 0, 0, 0, 0, 2]),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(matches!(
            codec.decode_cancel_response_v4(&[0, 1, 0, 0, 0, 2]),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(matches!(
            codec.decode_drain_response_v4(&[0, 3]),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
    }

    #[test]
    fn v4_additive_dispatcher_rejects_trailing_and_malformed_requests() {
        let mut codec = RuntimeBinaryCodecV4;
        let region = BackendMemoryRegionV1 {
            allocation: 1,
            access: RuntimeAccessV1::ReadWrite,
            byte_offset: 0,
            byte_len: 8,
        };
        let requests = [
            codec.encode_execution_capabilities_request_v4(1).unwrap(),
            codec
                .encode_async_copy_request_v4(1, region, region, &[])
                .unwrap(),
            codec.encode_cancel_request_v4(1).unwrap(),
            codec
                .encode_drain_request_v4(1, Duration::from_millis(1))
                .unwrap(),
        ];
        for mut request in requests {
            request.push(0);
            assert!(matches!(
                dispatch_binary_request_v4(&mut ProtocolBackendV1::default(), &request),
                Err(RuntimeWorkerErrorV1::Protocol(
                    "trailing canonical request bytes"
                ))
            ));
        }
        for opcode in [
            OP_EXECUTION_CAPABILITIES_V4,
            OP_ASYNC_COPY_V4,
            OP_CANCEL_V4,
            OP_DRAIN_V4,
        ] {
            assert!(matches!(
                dispatch_binary_request_v4(&mut ProtocolBackendV1::default(), &[opcode]),
                Err(RuntimeWorkerErrorV1::Protocol(
                    "truncated canonical request"
                ))
            ));
        }
    }

    #[test]
    fn v1_dispatcher_rejects_v4_extensions_and_v4_rejects_malformed_flush() {
        let flush_request = [OP_FLUSH_STREAM_V4]
            .into_iter()
            .chain(1_u64.to_le_bytes())
            .collect::<Vec<_>>();
        let mut codec = RuntimeBinaryCodecV4;
        let region = BackendMemoryRegionV1 {
            allocation: 1,
            access: RuntimeAccessV1::ReadWrite,
            byte_offset: 0,
            byte_len: 8,
        };
        for request in [
            flush_request.clone(),
            codec.encode_execution_capabilities_request_v4(1).unwrap(),
            codec
                .encode_async_copy_request_v4(1, region, region, &[])
                .unwrap(),
            codec.encode_cancel_request_v4(1).unwrap(),
            codec
                .encode_drain_request_v4(1, Duration::from_millis(1))
                .unwrap(),
        ] {
            assert!(matches!(
                dispatch_binary_request_v1(&mut ProtocolBackendV1::default(), &request),
                Err(RuntimeWorkerErrorV1::Protocol(
                    "unknown canonical operation"
                ))
            ));
        }

        let mut trailing = flush_request;
        trailing.push(0);
        assert!(matches!(
            dispatch_binary_request_v4(&mut ProtocolBackendV1::default(), &trailing),
            Err(RuntimeWorkerErrorV1::Protocol(
                "trailing canonical request bytes"
            ))
        ));
        assert!(matches!(
            dispatch_binary_request_v4(&mut ProtocolBackendV1::default(), &[OP_FLUSH_STREAM_V4]),
            Err(RuntimeWorkerErrorV1::Protocol(
                "truncated canonical request"
            ))
        ));
    }

    #[test]
    fn canonical_byte_response_limit_fits_the_frame_exactly() {
        let mut codec = RuntimeBinaryCodecV1;
        let request = codec
            .encode_request_v1(RuntimeWorkerOperationV1::ReadAllocation {
                allocation: 1,
                byte_offset: 0,
                byte_len: MAX_RUNTIME_WORKER_BYTE_RESPONSE_BYTES_V1,
            })
            .unwrap();
        let response =
            dispatch_binary_request_v1(&mut ProtocolBackendV1::default(), &request).unwrap();
        assert_eq!(response.len(), MAX_RUNTIME_WORKER_FRAME_BYTES_V1);
        assert!(matches!(
            codec
                .decode_response_v1(RuntimeWorkerOperationKindV1::Bytes, &response)
                .unwrap(),
            RuntimeWorkerResponseV1::Bytes(bytes)
                if bytes.len() == MAX_RUNTIME_WORKER_BYTE_RESPONSE_BYTES_V1
        ));
        assert!(matches!(
            codec.encode_request_v1(RuntimeWorkerOperationV1::ReadAllocation {
                allocation: 1,
                byte_offset: 0,
                byte_len: MAX_RUNTIME_WORKER_BYTE_RESPONSE_BYTES_V1 + 1,
            }),
            Err(RuntimeBinaryCodecErrorV1::Limit("allocation read"))
        ));
    }

    #[test]
    fn server_emits_handshake_processes_requests_and_stops_on_empty_frame() {
        let mut request_bytes = Vec::new();
        write_frame_v1(&mut request_bytes, b"abc").unwrap();
        write_frame_v1(&mut request_bytes, b"xyz").unwrap();
        write_frame_v1(&mut request_bytes, &[]).unwrap();
        let mut responses = Vec::new();
        serve_runtime_worker_v1(Cursor::new(request_bytes), &mut responses, |request| {
            let mut response = request.to_vec();
            response.reverse();
            Ok(response)
        })
        .unwrap();
        let mut responses = Cursor::new(responses);
        assert_eq!(
            read_frame_v1(&mut responses).unwrap(),
            RUNTIME_WORKER_HANDSHAKE_V1
        );
        assert_eq!(read_frame_v1(&mut responses).unwrap(), b"cba");
        assert_eq!(read_frame_v1(&mut responses).unwrap(), b"zyx");
    }

    #[test]
    fn canonical_v1_server_accepts_an_admitted_immediate_progress_backend() {
        fn assert_admitted<T: RuntimeWorkerV1ImmediateProgressBackendV1>() {}
        assert_admitted::<ProtocolBackendV1>();

        let mut requests = Vec::new();
        let mut codec = RuntimeBinaryCodecV1;
        write_frame_v1(
            &mut requests,
            &codec
                .encode_request_v1(RuntimeWorkerOperationV1::EnumerateDevices)
                .unwrap(),
        )
        .unwrap();
        write_frame_v1(&mut requests, &[]).unwrap();
        let mut responses = Vec::new();
        serve_runtime_backend_worker_v1(
            ProtocolBackendV1::default(),
            Cursor::new(requests),
            &mut responses,
        )
        .unwrap();

        let mut responses = Cursor::new(responses);
        assert_eq!(
            read_frame_v1(&mut responses).unwrap(),
            RUNTIME_WORKER_HANDSHAKE_V1
        );
        assert!(matches!(
            codec
                .decode_response_v1(
                    RuntimeWorkerOperationKindV1::EnumerateDevices,
                    &read_frame_v1(&mut responses).unwrap(),
                )
                .unwrap(),
            RuntimeWorkerResponseV1::Devices(devices) if devices.len() == 1
        ));
    }

    #[test]
    fn v4_server_emits_only_the_v4_handshake() {
        let mut request_bytes = Vec::new();
        write_frame_v1(&mut request_bytes, &[]).unwrap();
        let mut responses = Vec::new();
        serve_runtime_worker_v4(Cursor::new(request_bytes), &mut responses, |_| {
            panic!("shutdown frame must not reach the handler")
        })
        .unwrap();
        let mut responses = Cursor::new(responses);
        assert_eq!(
            read_frame_v1(&mut responses).unwrap(),
            RUNTIME_WORKER_HANDSHAKE_V4
        );
    }

    #[test]
    fn v4_backend_server_dispatches_flush_after_exact_handshake() {
        let mut requests = Vec::new();
        let mut codec = RuntimeBinaryCodecV4;
        write_frame_v1(
            &mut requests,
            &codec.encode_flush_stream_request_v4(41).unwrap(),
        )
        .unwrap();
        write_frame_v1(&mut requests, &[]).unwrap();
        let mut responses = Vec::new();
        serve_runtime_backend_worker_v4(
            ProtocolBackendV1::default(),
            Cursor::new(requests),
            &mut responses,
        )
        .unwrap();

        let mut responses = Cursor::new(responses);
        assert_eq!(
            read_frame_v1(&mut responses).unwrap(),
            RUNTIME_WORKER_HANDSHAKE_V4
        );
        codec
            .decode_flush_stream_response_v4(&read_frame_v1(&mut responses).unwrap())
            .unwrap();
    }

    #[test]
    fn oversized_and_truncated_frames_fail_closed() {
        let mut oversized = ((MAX_RUNTIME_WORKER_FRAME_BYTES_V1 + 1) as u32)
            .to_le_bytes()
            .to_vec();
        oversized.extend_from_slice(b"ignored");
        assert!(matches!(
            read_frame_v1(&mut Cursor::new(oversized)),
            Err(RuntimeWorkerErrorV1::FrameTooLarge { .. })
        ));
        assert!(matches!(
            read_frame_v1(&mut Cursor::new([4, 0, 0, 0, 1, 2])),
            Err(RuntimeWorkerErrorV1::Io(_))
        ));
    }

    #[test]
    fn non_worker_is_rejected_during_handshake() {
        let command = RuntimeWorkerCommandV1::new("/bin/true");
        let error = match RuntimeWorkerTransportV1::spawn(&command, Duration::from_secs(1)) {
            Ok(_) => panic!("non-worker unexpectedly passed the handshake"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            RuntimeWorkerErrorV1::Io(_) | RuntimeWorkerErrorV1::WorkerExited
        ));
    }

    #[test]
    fn v4_backend_rejects_v1_downgrade_and_unknown_version_handshakes() {
        if std::process::Command::new("python3")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
        for script in [TEST_WORKER_SERVER, UNKNOWN_VERSION_WORKER_SERVER] {
            let command = RuntimeWorkerCommandV1::new("python3")
                .argument("-u")
                .argument("-c")
                .argument(script);
            let error = match RuntimeWorkerBackendV4::spawn(
                &command,
                RuntimeBinaryCodecV4,
                Duration::from_secs(2),
                Duration::from_secs(2),
            ) {
                Ok(_) => panic!("non-V4 handshake unexpectedly enabled flush"),
                Err(error) => error,
            };
            assert!(matches!(
                error,
                RuntimeWorkerErrorV1::Protocol("handshake mismatch")
            ));
        }
    }

    #[test]
    fn v1_backend_rejects_v4_handshake() {
        if std::process::Command::new("python3")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
        let command = RuntimeWorkerCommandV1::new("python3")
            .argument("-u")
            .argument("-c")
            .argument(V4_FLUSH_WORKER_SERVER);
        let error = match RuntimeWorkerBackendV1::spawn(
            &command,
            RuntimeBinaryCodecV1,
            Duration::from_secs(2),
            Duration::from_secs(2),
        ) {
            Ok(_) => panic!("V1 backend unexpectedly accepted a V4 worker"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            RuntimeWorkerErrorV1::Protocol("handshake mismatch")
        ));
    }

    #[test]
    fn v4_backend_flushes_and_shuts_down_over_child_process() {
        if std::process::Command::new("python3")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
        let command = RuntimeWorkerCommandV1::new("python3")
            .argument("-u")
            .argument("-c")
            .argument(V4_FLUSH_WORKER_SERVER);
        let mut backend = RuntimeWorkerBackendV4::spawn(
            &command,
            RuntimeBinaryCodecV4,
            Duration::from_secs(2),
            Duration::from_secs(2),
        )
        .unwrap();
        backend.flush_stream_v1(41).unwrap();
        assert!(!backend.is_terminal());
        backend.shutdown(Duration::from_secs(2)).unwrap();
    }

    #[test]
    fn v4_backend_exposes_portable_flush_through_runtime_context() {
        if std::process::Command::new("python3")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
        let command = RuntimeWorkerCommandV1::new("python3")
            .argument("-u")
            .argument("-c")
            .argument(V4_FLUSH_WORKER_SERVER);
        let backend = RuntimeWorkerBackendV4::spawn(
            &command,
            RuntimeBinaryCodecV4,
            Duration::from_secs(2),
            Duration::from_secs(2),
        )
        .unwrap();
        let mut context = RuntimeContextV1::open(backend).unwrap();
        let stream = context.create_stream(context.devices()[0].id()).unwrap();
        context.flush_stream(stream).unwrap();
        let backend = context.shutdown().unwrap();
        backend.shutdown(Duration::from_secs(2)).unwrap();
    }

    #[test]
    fn v4_backend_preserves_additive_surfaces_over_child_process() {
        if std::process::Command::new("python3")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
        let command = RuntimeWorkerCommandV1::new("python3")
            .argument("-u")
            .argument("-c")
            .argument(V4_ADDITIVE_WORKER_SERVER);
        let mut backend = RuntimeWorkerBackendV4::spawn(
            &command,
            RuntimeBinaryCodecV4,
            Duration::from_secs(2),
            Duration::from_secs(2),
        )
        .unwrap();
        assert_eq!(
            backend.execution_capabilities_v1(1),
            RuntimeExecutionCapabilitiesV1::default()
        );

        let first = backend.enumerate_devices_v1().unwrap();
        assert_eq!(first[0].backend_device, 1);
        assert_eq!(
            backend.execution_capabilities_v1(1),
            RuntimeExecutionCapabilitiesV1 {
                native_async_copy: true,
                compute_copy_overlap: true,
                cancellation: true,
                ..RuntimeExecutionCapabilitiesV1::default()
            }
        );
        assert_eq!(
            backend.execution_capabilities_v1(99),
            RuntimeExecutionCapabilitiesV1::default()
        );

        let second = backend.enumerate_devices_v1().unwrap();
        assert_eq!(second[0].backend_device, 2);
        assert_eq!(
            backend.execution_capabilities_v1(1),
            RuntimeExecutionCapabilitiesV1::default()
        );
        assert_eq!(
            backend.execution_capabilities_v1(2),
            RuntimeExecutionCapabilitiesV1 {
                native_peer_copy: true,
                concurrent_compute: true,
                ..RuntimeExecutionCapabilitiesV1::default()
            }
        );

        let source = BackendMemoryRegionV1 {
            allocation: 1,
            access: RuntimeAccessV1::Read,
            byte_offset: 0,
            byte_len: 8,
        };
        let destination = BackendMemoryRegionV1 {
            allocation: 2,
            access: RuntimeAccessV1::Write,
            byte_offset: 0,
            byte_len: 8,
        };
        let submission = backend.copy_async_v1(1, source, destination, &[]).unwrap();
        assert_eq!(submission, 101);
        assert_eq!(
            backend.cancel_v1(submission).unwrap(),
            BackendCancellationV1::Cancelled
        );
        assert_eq!(
            backend.cancel_v1(999).unwrap(),
            BackendCancellationV1::TooLate
        );
        let deadline = Instant::now() + Duration::from_millis(400);
        assert_eq!(
            backend.drain_v1(submission, deadline).unwrap(),
            BackendPollV1::Succeeded
        );
        assert!(Instant::now() < deadline);
        backend.flush_stream_v1(1).unwrap();
        assert!(matches!(
            backend.enumerate_devices_v1(),
            Err(RuntimeBackendFailureV1::Rejected(
                RuntimeWorkerBackendErrorV1::Codec(RuntimeBinaryCodecErrorV1::Remote(_))
            ))
        ));
        assert!(!backend.is_terminal());
        assert_eq!(
            backend.execution_capabilities_v1(2),
            RuntimeExecutionCapabilitiesV1::default()
        );
        assert_eq!(
            backend.execution_capabilities_v1(3),
            RuntimeExecutionCapabilitiesV1::default()
        );
        backend.shutdown(Duration::from_secs(2)).unwrap();
    }

    #[test]
    fn v4_terminal_capability_response_reaps_and_preserves_fail_closed_cache() {
        if std::process::Command::new("python3")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
        let command = RuntimeWorkerCommandV1::new("python3")
            .argument("-u")
            .argument("-c")
            .argument(V4_TERMINAL_CAPABILITIES_WORKER_SERVER);
        let mut backend = RuntimeWorkerBackendV4::spawn(
            &command,
            RuntimeBinaryCodecV4,
            Duration::from_secs(2),
            Duration::from_secs(2),
        )
        .unwrap();
        assert_eq!(
            backend.execution_capabilities_v1(1),
            RuntimeExecutionCapabilitiesV1::default()
        );
        assert!(matches!(
            backend.enumerate_devices_v1(),
            Err(RuntimeBackendFailureV1::Terminal(
                RuntimeWorkerBackendErrorV1::Codec(RuntimeBinaryCodecErrorV1::Remote(_))
            ))
        ));
        assert!(backend.is_terminal());
        assert!(backend.inner.transport.child.try_wait().unwrap().is_some());
        assert_eq!(
            backend.execution_capabilities_v1(1),
            RuntimeExecutionCapabilitiesV1::default()
        );
        assert!(matches!(
            backend.flush_stream_v1(1),
            Err(RuntimeBackendFailureV1::Terminal(
                RuntimeWorkerBackendErrorV1::Transport(RuntimeWorkerErrorV1::WorkerExited)
            ))
        ));
    }

    #[test]
    fn v4_zero_async_copy_handle_reaps_and_seals_the_worker() {
        if std::process::Command::new("python3")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
        let command = RuntimeWorkerCommandV1::new("python3")
            .argument("-u")
            .argument("-c")
            .argument(V4_ZERO_ASYNC_COPY_WORKER_SERVER);
        let mut backend = RuntimeWorkerBackendV4::spawn(
            &command,
            RuntimeBinaryCodecV4,
            Duration::from_secs(2),
            Duration::from_secs(2),
        )
        .unwrap();
        let region = BackendMemoryRegionV1 {
            allocation: 1,
            access: RuntimeAccessV1::ReadWrite,
            byte_offset: 0,
            byte_len: 8,
        };
        assert!(matches!(
            backend.copy_async_v1(1, region, region, &[]),
            Err(RuntimeBackendFailureV1::Terminal(
                RuntimeWorkerBackendErrorV1::Protocol("nonzero async-copy handle")
            ))
        ));
        assert!(backend.is_terminal());
        assert!(backend.inner.transport.child.try_wait().unwrap().is_some());
        assert!(matches!(
            backend.cancel_v1(1),
            Err(RuntimeBackendFailureV1::Terminal(
                RuntimeWorkerBackendErrorV1::Transport(RuntimeWorkerErrorV1::WorkerExited)
            ))
        ));
    }

    #[test]
    fn v4_flush_deadline_failure_reaps_and_seals_the_worker() {
        if std::process::Command::new("python3")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
        let command = RuntimeWorkerCommandV1::new("python3")
            .argument("-u")
            .argument("-c")
            .argument(V4_DELAYED_FLUSH_WORKER_SERVER);
        let mut backend = RuntimeWorkerBackendV4::spawn(
            &command,
            RuntimeBinaryCodecV4,
            Duration::from_secs(2),
            TEST_WAIT_DEADLINE,
        )
        .unwrap();
        let deadline = Instant::now() + TEST_WAIT_DEADLINE;
        assert!(matches!(
            backend.flush_stream_v1(1),
            Err(RuntimeBackendFailureV1::Terminal(
                RuntimeWorkerBackendErrorV1::Transport(RuntimeWorkerErrorV1::ResponseTimeout)
            ))
        ));
        assert_deadline_returned_with_scheduler_tolerance(deadline);
        assert!(backend.is_terminal());
        assert!(backend.inner.transport.child.try_wait().unwrap().is_some());
        assert!(matches!(
            backend.flush_stream_v1(1),
            Err(RuntimeBackendFailureV1::Terminal(
                RuntimeWorkerBackendErrorV1::Transport(RuntimeWorkerErrorV1::WorkerExited)
            ))
        ));
    }

    #[test]
    fn v4_invalid_request_deadline_reaps_and_seals_the_worker() {
        if std::process::Command::new("python3")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
        let command = RuntimeWorkerCommandV1::new("python3")
            .argument("-u")
            .argument("-c")
            .argument(V4_FLUSH_WORKER_SERVER);
        let mut backend = RuntimeWorkerBackendV4::spawn(
            &command,
            RuntimeBinaryCodecV4,
            Duration::from_secs(2),
            Duration::MAX,
        )
        .unwrap();
        assert!(matches!(
            backend.flush_stream_v1(41),
            Err(RuntimeBackendFailureV1::Terminal(
                RuntimeWorkerBackendErrorV1::Transport(RuntimeWorkerErrorV1::InvalidDeadline)
            ))
        ));
        assert!(backend.is_terminal());
        assert!(backend.inner.transport.child.try_wait().unwrap().is_some());
        assert!(matches!(
            backend.flush_stream_v1(41),
            Err(RuntimeBackendFailureV1::Terminal(
                RuntimeWorkerBackendErrorV1::Transport(RuntimeWorkerErrorV1::WorkerExited)
            ))
        ));
    }

    #[test]
    fn v4_decoded_terminal_flush_failure_reaps_and_seals_the_worker() {
        if std::process::Command::new("python3")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
        let command = RuntimeWorkerCommandV1::new("python3")
            .argument("-u")
            .argument("-c")
            .argument(V4_TERMINAL_FLUSH_WORKER_SERVER);
        let mut backend = RuntimeWorkerBackendV4::spawn(
            &command,
            RuntimeBinaryCodecV4,
            Duration::from_secs(2),
            Duration::from_secs(2),
        )
        .unwrap();
        assert!(matches!(
            backend.flush_stream_v1(1),
            Err(RuntimeBackendFailureV1::Terminal(
                RuntimeWorkerBackendErrorV1::Codec(RuntimeBinaryCodecErrorV1::Remote(_))
            ))
        ));
        assert!(backend.is_terminal());
        assert!(backend.inner.transport.child.try_wait().unwrap().is_some());
        assert!(matches!(
            backend.flush_stream_v1(1),
            Err(RuntimeBackendFailureV1::Terminal(
                RuntimeWorkerBackendErrorV1::Transport(RuntimeWorkerErrorV1::WorkerExited)
            ))
        ));
    }

    #[test]
    fn blocked_request_write_obeys_absolute_deadline_and_reaps_the_worker() {
        if std::process::Command::new("python3")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
        let command = RuntimeWorkerCommandV1::new("python3")
            .argument("-u")
            .argument("-c")
            .argument(NON_READING_WORKER_SERVER);
        let mut transport =
            RuntimeWorkerTransportV1::spawn(&command, Duration::from_secs(2)).unwrap();
        let request = vec![1; 8 * 1024 * 1024];
        let deadline = Instant::now() + TEST_WAIT_DEADLINE;
        assert!(matches!(
            transport.request_owned_until(request, deadline),
            Err(RuntimeWorkerErrorV1::RequestWriteTimeout)
        ));
        assert_deadline_returned_with_scheduler_tolerance(deadline);
        assert!(transport.is_terminal());
        assert!(transport.child.try_wait().unwrap().is_some());
        assert!(matches!(
            transport.request(b"second", Duration::from_secs(1)),
            Err(RuntimeWorkerErrorV1::WorkerExited)
        ));
    }

    #[test]
    fn expired_worker_wait_is_pending_without_publishing_a_request() {
        if std::process::Command::new("python3")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
        let command = RuntimeWorkerCommandV1::new("python3")
            .argument("-u")
            .argument("-c")
            .argument(TEST_WORKER_SERVER);
        let mut backend = RuntimeWorkerBackendV1::spawn(
            &command,
            TestWorkerCodecV1,
            Duration::from_secs(2),
            Duration::from_secs(2),
        )
        .unwrap();

        assert_eq!(
            backend.wait_v1(1, Instant::now()).unwrap(),
            BackendPollV1::Pending
        );
        assert!(!backend.is_terminal());
        assert_eq!(backend.enumerate_devices_v1().unwrap().len(), 1);
    }

    fn assert_delayed_wait_times_out_and_reaps(script: &str) {
        let command = RuntimeWorkerCommandV1::new("python3")
            .argument("-u")
            .argument("-c")
            .argument(script);
        let mut backend = RuntimeWorkerBackendV1::spawn(
            &command,
            TestWorkerCodecV1,
            Duration::from_secs(2),
            Duration::from_secs(2),
        )
        .unwrap();
        let deadline = Instant::now() + TEST_WAIT_DEADLINE;

        assert!(matches!(
            backend.wait_v1(1, deadline),
            Err(RuntimeBackendFailureV1::Terminal(
                RuntimeWorkerBackendErrorV1::Transport(RuntimeWorkerErrorV1::ResponseTimeout)
            ))
        ));
        assert_deadline_returned_with_scheduler_tolerance(deadline);
        assert!(backend.is_terminal());
        assert!(backend.transport.child.try_wait().unwrap().is_some());
        assert!(matches!(
            backend.wait_v1(1, Instant::now()),
            Err(RuntimeBackendFailureV1::Terminal(
                RuntimeWorkerBackendErrorV1::Transport(RuntimeWorkerErrorV1::WorkerExited)
            ))
        ));
    }

    #[test]
    fn delayed_worker_request_transit_cannot_extend_wait_deadline() {
        if std::process::Command::new("python3")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
        assert_delayed_wait_times_out_and_reaps(DELAYED_READ_WORKER_SERVER);
    }

    #[test]
    fn delayed_worker_response_cannot_extend_wait_deadline() {
        if std::process::Command::new("python3")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
        assert_delayed_wait_times_out_and_reaps(DELAYED_RESPONSE_WORKER_SERVER);
    }

    #[test]
    fn worker_wait_reserves_response_grace_inside_parent_deadline() {
        if std::process::Command::new("python3")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
        let command = RuntimeWorkerCommandV1::new("python3")
            .argument("-u")
            .argument("-c")
            .argument(WAIT_BUDGET_OBSERVER_WORKER_SERVER);
        let mut backend = RuntimeWorkerBackendV1::spawn(
            &command,
            RuntimeBinaryCodecV1,
            Duration::from_secs(2),
            Duration::from_secs(2),
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_millis(400);

        assert_eq!(
            backend.wait_v1(1, deadline).unwrap(),
            BackendPollV1::Pending
        );
        assert!(Instant::now() < deadline);
        assert!(!backend.is_terminal());
    }

    #[test]
    fn decoded_terminal_failure_seals_the_worker_backend() {
        if std::process::Command::new("python3")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
        let command = RuntimeWorkerCommandV1::new("python3")
            .argument("-u")
            .argument("-c")
            .argument(SINGLE_RESPONSE_WORKER_SERVER);
        let mut backend = RuntimeWorkerBackendV1::spawn(
            &command,
            TestWorkerCodecV1,
            Duration::from_secs(2),
            Duration::from_secs(2),
        )
        .unwrap();
        assert!(matches!(
            backend.enumerate_devices_v1(),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(backend.is_terminal());
        assert!(matches!(
            backend.enumerate_devices_v1(),
            Err(RuntimeBackendFailureV1::Terminal(
                RuntimeWorkerBackendErrorV1::Transport(RuntimeWorkerErrorV1::WorkerExited)
            ))
        ));
    }

    #[test]
    fn response_shape_mismatch_seals_the_worker_backend() {
        if std::process::Command::new("python3")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
        let command = RuntimeWorkerCommandV1::new("python3")
            .argument("-u")
            .argument("-c")
            .argument(SINGLE_RESPONSE_WORKER_SERVER);
        let mut backend = RuntimeWorkerBackendV1::spawn(
            &command,
            MismatchedCodecV1,
            Duration::from_secs(2),
            Duration::from_secs(2),
        )
        .unwrap();
        assert!(matches!(
            backend.create_stream_v1(1),
            Err(RuntimeBackendFailureV1::Terminal(
                RuntimeWorkerBackendErrorV1::Protocol("nonzero handle")
            ))
        ));
        assert!(backend.is_terminal());
        assert!(matches!(
            backend.create_stream_v1(1),
            Err(RuntimeBackendFailureV1::Terminal(
                RuntimeWorkerBackendErrorV1::Transport(RuntimeWorkerErrorV1::WorkerExited)
            ))
        ));
    }

    #[test]
    fn worker_backend_codec_drives_context_and_cleanup_over_child_process() {
        if std::process::Command::new("python3")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
        let command = RuntimeWorkerCommandV1::new("python3")
            .argument("-u")
            .argument("-c")
            .argument(TEST_WORKER_SERVER);
        let backend = RuntimeWorkerBackendV1::spawn(
            &command,
            TestWorkerCodecV1,
            Duration::from_secs(2),
            Duration::from_secs(2),
        )
        .unwrap();
        let mut context = RuntimeContextV1::open(backend).unwrap();
        let device = context.devices()[0].id();
        let stream = context.create_stream(device).unwrap();
        let allocation = context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap();
        let module = context.load_module(device, b"object").unwrap();
        let kernel = context
            .resolve_kernel::<WorkerArgumentsV1>(module, "kernel")
            .unwrap();
        let mut submission = context
            .launch(
                stream,
                &kernel,
                &WorkerArgumentsV1 { allocation },
                RuntimeLaunchGeometryV1 {
                    grid: [64, 1, 1],
                    workgroup: [64, 1, 1],
                    dynamic_shared_bytes: 0,
                },
                &[],
            )
            .unwrap();
        assert_eq!(
            context
                .wait(&mut submission, Duration::from_secs(1))
                .unwrap(),
            crate::RuntimePollV1::Succeeded
        );
        context.record_event(&submission).unwrap();
        let backend = context.shutdown().unwrap();
        backend.shutdown(Duration::from_secs(2)).unwrap();
    }

    #[test]
    fn worker_shutdown_rejects_deadline_overflow() {
        if std::process::Command::new("python3")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
        let command = RuntimeWorkerCommandV1::new("python3")
            .argument("-u")
            .argument("-c")
            .argument(TEST_WORKER_SERVER);
        let transport = RuntimeWorkerTransportV1::spawn(&command, Duration::from_secs(2)).unwrap();
        assert!(matches!(
            transport.shutdown(Duration::MAX),
            Err(RuntimeWorkerErrorV1::InvalidDeadline)
        ));
    }
}
