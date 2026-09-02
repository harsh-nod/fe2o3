//! Bounded subprocess transport for terminal native runtime backends.

use core::fmt;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::{
    BackendBindingV1, BackendDeviceDescriptionV1, BackendLaunchV1, BackendMemoryRegionV1,
    BackendPollV1, MAX_RUNTIME_MODULE_IMAGE_BYTES_V1, RuntimeBackendFailureV1, RuntimeBackendV1,
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

/// Serves the canonical bounded protocol over any concrete runtime backend.
pub fn serve_runtime_backend_worker_v1<B, R, W>(
    mut backend: B,
    input: R,
    output: W,
) -> Result<(), RuntimeWorkerErrorV1>
where
    B: RuntimeBackendV1,
    R: Read,
    W: Write,
{
    serve_runtime_worker_v1(input, output, |request| {
        dispatch_binary_request_v1(&mut backend, request)
    })
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

/// `RuntimeBackendV1` implementation that moves all backend execution into a child process.
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
        if handshake != RUNTIME_WORKER_HANDSHAKE_V1 {
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
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(RuntimeWorkerErrorV1::InvalidDeadline)?;
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

/// Runs the worker side of the bounded V1 request/response protocol.
pub fn serve_runtime_worker_v1<R, W, F>(
    mut input: R,
    mut output: W,
    mut handler: F,
) -> Result<(), RuntimeWorkerErrorV1>
where
    R: Read,
    W: Write,
    F: FnMut(&[u8]) -> Result<Vec<u8>, RuntimeWorkerErrorV1>,
{
    write_frame_v1(&mut output, RUNTIME_WORKER_HANDSHAKE_V1)?;
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
